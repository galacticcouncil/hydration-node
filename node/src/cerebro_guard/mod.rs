//! Block guard: every pool transaction this node puts into a block it authors is executed at its
//! real position and judged by cerebro from the state it leaves behind. In `hold` mode a flagged
//! transaction is rolled back and kept in the pool; in `shadow` mode it is only recorded.

pub mod cache;
pub mod events;
pub mod hold;
pub mod rpc;
pub mod shadow;
pub mod transport;

use cache::{CacheSlot, Caches};
use cerebro_core::model::Trade;
use cerebro_core::value::Usd6;
use cerebro_hydration::decode::map_records;
use cerebro_hydration::guard::{
	self as judge, candidate_of, is_candidate, plan, read_oracle, read_point, with_state, Accumulators, Candidate,
	GuardCtx, InconclusiveReason, Judgement, PointView, ReadPlan, Thresholds, Verdict, POST, PRE,
};
use cerebro_hydration::pricing::Prices;
use cerebro_hydration::state::{self, Change};
use cerebro_hydration::{permit, xcm, Prefetch, Transport};
use codec::{Decode, Encode};
use events::{EventsError, EventsReader, Mark, RawStorage};
use fp_rpc::EthereumRuntimeRPCApi;
use hold::{Held, HoldBook, Removal};
use hydradx_runtime::opaque::Block;
use sc_basic_authorship::{ExtrinsicGuard, GuardContext, GuardOutcome, GuardSession};
use sc_block_builder::{BlockBuilder, BlockBuilderApi};
use shadow::{JsonlWriter, RecordCtx, StageMicros};
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_core::H256;
use std::collections::{BTreeMap, VecDeque};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use substrate_prometheus_endpoint::{
	register, Counter, CounterVec, Gauge, HistogramOpts, HistogramVec, Opts, PrometheusError, Registry, U64,
};

pub const LOG_TARGET: &str = "cerebro-guard";

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum GuardMode {
	Off,
	Shadow,
	Hold,
}

impl GuardMode {
	pub fn as_str(self) -> &'static str {
		match self {
			GuardMode::Off => "off",
			GuardMode::Shadow => "shadow",
			GuardMode::Hold => "hold",
		}
	}
}

#[derive(Debug, Clone, clap::Args)]
pub struct CerebroGuardCli {
	/// Judge every pool transaction before it goes into a block this node authors. `hold` keeps
	/// transactions that leak pool value out of this node's blocks; `shadow` only records verdicts.
	#[arg(long = "cerebro-guard", value_enum, default_value_t = GuardMode::Hold)]
	pub cerebro_guard: GuardMode,

	/// Pool value-per-share loss (USD) above which a single transaction is held.
	#[arg(long, default_value_t = 0.0)]
	pub cerebro_guard_min_pool_leak_usd: f64,

	/// Pool loss (USD) accumulated over one block above which a transaction is held.
	#[arg(long, default_value_t = 0.0)]
	pub cerebro_guard_min_block_pool_leak_usd: f64,

	/// Value-per-share drops up to this many basis points are treated as rounding. 0: none are.
	#[arg(long, default_value_t = 0)]
	pub cerebro_guard_pool_leak_epsilon_bps: i64,

	/// Time kept free before the proposer deadline for finalizing the block. Defaults to 25% of
	/// the proposal time.
	#[arg(long)]
	pub cerebro_guard_finalize_reserve_ms: Option<u64>,

	/// Most time the guard may spend per proposal. When it runs out, hold mode ends the block and
	/// shadow mode pushes the remaining transactions unchecked. Unlimited by default.
	#[arg(long)]
	pub cerebro_guard_block_budget_ms: Option<u64>,

	/// Imported blocks between refreshes of the guard's protocol-account and registry caches.
	#[arg(long, default_value_t = 300)]
	pub cerebro_guard_cache_refresh: u32,
}

#[derive(Debug, Clone)]
pub struct GuardConfig {
	pub mode: GuardMode,
	pub thresholds: Thresholds,
	pub finalize_reserve: Option<Duration>,
	pub block_budget: Option<Duration>,
	pub cache_refresh: u32,
	pub ban_seconds: u64,
}

impl GuardConfig {
	pub fn from_cli(cli: &CerebroGuardCli, ban_seconds: u64) -> Self {
		Self {
			mode: cli.cerebro_guard,
			thresholds: Thresholds {
				// Signer gain never holds: profitable arbitrage and liquidations look the same.
				min_usd: Usd6(i128::MAX),
				min_pool_leak: Usd6::from_dollars(cli.cerebro_guard_min_pool_leak_usd),
				min_block_usd: Usd6(i128::MAX),
				min_block_pool_leak: Usd6::from_dollars(cli.cerebro_guard_min_block_pool_leak_usd),
				pool_leak_epsilon_bps: cli.cerebro_guard_pool_leak_epsilon_bps,
			},
			finalize_reserve: cli.cerebro_guard_finalize_reserve_ms.map(Duration::from_millis),
			block_budget: cli.cerebro_guard_block_budget_ms.map(Duration::from_millis),
			cache_refresh: cli.cerebro_guard_cache_refresh,
			ban_seconds,
		}
	}
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Readiness {
	Ready,
	/// The guard has not loaded its caches yet.
	NotReady(String),
	/// The runtime version at the parent block could not be read.
	Unsupported(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effective {
	Run {
		enforce: bool,
	},
	/// Push everything unchecked and record it as `UnsupportedRuntime`.
	Unsupported,
}

/// Whenever the guard can't judge, blocks are authored as if it weren't there.
pub fn effective(mode: GuardMode, readiness: &Readiness) -> Effective {
	match readiness {
		Readiness::Ready => Effective::Run {
			enforce: mode == GuardMode::Hold,
		},
		_ => Effective::Unsupported,
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
	Pass1,
	Pre,
	Judge,
	Repush,
}

impl Stage {
	const ALL: [Stage; 4] = [Stage::Pass1, Stage::Pre, Stage::Judge, Stage::Repush];

	fn as_str(self) -> &'static str {
		match self {
			Stage::Pass1 => "pass1",
			Stage::Pre => "pre",
			Stage::Judge => "judge",
			Stage::Repush => "repush",
		}
	}

	/// Assumed worst case until enough samples exist.
	fn seed(self) -> Duration {
		match self {
			Stage::Pass1 => Duration::from_millis(25),
			Stage::Pre => Duration::from_millis(25),
			Stage::Judge => Duration::from_millis(5),
			Stage::Repush => Duration::from_millis(15),
		}
	}
}

const STAGE_WINDOW: usize = 2048;
const STAGE_MIN_SAMPLES: usize = 64;

#[derive(Debug, Default)]
pub struct StageStats {
	micros: VecDeque<u64>,
}

impl StageStats {
	pub fn push(&mut self, d: Duration) {
		if self.micros.len() == STAGE_WINDOW {
			self.micros.pop_front();
		}
		self.micros.push_back(d.as_micros() as u64);
	}

	pub fn quantile(&self, q: f64) -> Option<u64> {
		if self.micros.is_empty() {
			return None;
		}
		let mut sorted: Vec<u64> = self.micros.iter().copied().collect();
		sorted.sort_unstable();
		let idx = ((sorted.len() - 1) as f64 * q).ceil() as usize;
		Some(sorted[idx.min(sorted.len() - 1)])
	}

	/// Running p99.9, or `seed` while there are too few samples to trust.
	pub fn worst_case(&self, seed: Duration) -> Duration {
		if self.micros.len() < STAGE_MIN_SAMPLES {
			return seed;
		}
		self.quantile(0.999).map(Duration::from_micros).unwrap_or(seed)
	}
}

/// The guard's time budget for one proposal. Execution can't be interrupted, so it is checked
/// before each stage against that stage's worst case.
#[derive(Debug, Clone)]
pub struct Budget {
	cutoff: Instant,
	cap: Option<Duration>,
	spent: Duration,
	exhaustion_counted: bool,
}

impl Budget {
	pub fn new(now: Instant, deadline: Instant, finalize_reserve: Option<Duration>, cap: Option<Duration>) -> Self {
		let total = deadline.saturating_duration_since(now);
		let reserve = finalize_reserve.unwrap_or(total / 4);
		Self {
			cutoff: deadline.checked_sub(reserve).unwrap_or(now).max(now),
			cap,
			spent: Duration::ZERO,
			exhaustion_counted: false,
		}
	}

	pub fn fits(&self, now: Instant, worst: Duration) -> bool {
		now + worst < self.cutoff
	}

	pub fn exhausted(&self) -> bool {
		self.cap.is_some_and(|cap| self.spent >= cap)
	}

	pub fn spend(&mut self, d: Duration) {
		self.spent = self.spent.saturating_add(d);
	}

	/// True the first time it is called after the cap was reached.
	pub fn newly_exhausted(&mut self) -> bool {
		let first = self.exhausted() && !self.exhaustion_counted;
		self.exhaustion_counted |= first;
		first
	}
}

#[derive(Debug, Default)]
pub struct Stats {
	pub counters: BTreeMap<String, u64>,
	pub stages: [StageStats; 4],
	pub stops: u64,
	pub budget_exhausted: u64,
	pub events_resync: u64,
	pub cache_refresh_failures: u64,
}

impl Stats {
	fn stage(&mut self, s: Stage) -> &mut StageStats {
		&mut self.stages[s as usize]
	}

	pub fn worst_case(&self, s: Stage) -> Duration {
		self.stages[s as usize].worst_case(s.seed())
	}

	/// `stage → [p50, p99, max]` in milliseconds.
	pub fn stage_ms(&self) -> BTreeMap<String, [f64; 3]> {
		Stage::ALL
			.iter()
			.map(|s| {
				let st = &self.stages[*s as usize];
				let ms = |q| st.quantile(q).map(|us| us as f64 / 1000.0).unwrap_or_default();
				(s.as_str().to_string(), [ms(0.5), ms(0.99), ms(1.0)])
			})
			.collect()
	}
}

pub struct Metrics {
	checked: CounterVec<U64>,
	stage: HistogramVec,
	stops: Counter<U64>,
	budget_exhausted: Counter<U64>,
	events_resync: Counter<U64>,
	held: Gauge<U64>,
	released: Gauge<U64>,
	ready: Gauge<U64>,
	downgraded: Gauge<U64>,
	cache_refresh_failures: Counter<U64>,
}

impl Metrics {
	pub fn register(registry: &Registry) -> Result<Self, PrometheusError> {
		let counter = |name: &str, help: &str| register(Counter::new(format!("cerebro_guard_{name}"), help)?, registry);
		let gauge = |name: &str, help: &str| register(Gauge::new(format!("cerebro_guard_{name}"), help)?, registry);
		Ok(Self {
			checked: register(
				CounterVec::new(
					Opts::new(
						"cerebro_guard_checked_total",
						"Transactions judged, by verdict and reason",
					),
					&["verdict", "reason"],
				)?,
				registry,
			)?,
			stage: register(
				HistogramVec::new(
					HistogramOpts::new("cerebro_guard_stage_seconds", "Time per guard stage")
						.buckets(vec![0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25]),
					&["stage"],
				)?,
				registry,
			)?,
			stops: counter("stops_total", "Proposals the guard stopped early to keep the slot")?,
			budget_exhausted: counter("budget_exhausted_total", "Proposals whose guard budget ran out")?,
			events_resync: counter("events_resync_total", "Event bookkeeping fallbacks to a full decode")?,
			held: gauge("held", "Transactions currently held")?,
			released: gauge("released", "Transactions currently released")?,
			ready: gauge("ready", "1 when the guard is ready")?,
			downgraded: gauge("downgraded", "1 when hold mode runs as shadow")?,
			cache_refresh_failures: counter("cache_refresh_failures_total", "Failed cache refreshes")?,
		})
	}
}

#[derive(Debug, Clone)]
pub struct RuntimeState {
	pub readiness: Readiness,
	pub chain_spec_version: u32,
	pub effective: Effective,
}

pub struct Shared {
	pub config: GuardConfig,
	pub caches: CacheSlot,
	pub holds: Mutex<HoldBook>,
	pub stats: Mutex<Stats>,
	pub state: Mutex<RuntimeState>,
	pub best: AtomicU32,
	metrics: Option<Metrics>,
	shadow: Option<JsonlWriter>,
	audit: Option<JsonlWriter>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
	m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Shared {
	pub fn new(config: GuardConfig, base_path: Option<&Path>, registry: Option<&Registry>) -> Self {
		let open = |name: &str| {
			let path = base_path?.join("cerebro-guard").join(name);
			JsonlWriter::open(&path)
				.map_err(|e| log::error!(target: LOG_TARGET, "cannot open {}: {e}", path.display()))
				.ok()
		};
		let metrics = registry.and_then(|r| {
			Metrics::register(r)
				.map_err(|e| log::error!(target: LOG_TARGET, "metrics registration failed: {e}"))
				.ok()
		});
		Self {
			state: Mutex::new(RuntimeState {
				readiness: Readiness::NotReady("starting".into()),
				chain_spec_version: 0,
				effective: effective(config.mode, &Readiness::NotReady(String::new())),
			}),
			config,
			caches: CacheSlot::default(),
			holds: Mutex::new(HoldBook::default()),
			stats: Mutex::new(Stats::default()),
			best: AtomicU32::new(0),
			metrics,
			shadow: open("shadow.jsonl"),
			audit: open("audit.jsonl"),
		}
	}

	fn count(&self, verdict: Verdict) {
		*lock(&self.stats)
			.counters
			.entry(format!("{}:{}", verdict.kind(), verdict.reason()))
			.or_default() += 1;
		if let Some(m) = &self.metrics {
			m.checked.with_label_values(&[verdict.kind(), verdict.reason()]).inc();
		}
	}

	fn observe(&self, stage: Stage, d: Duration) {
		lock(&self.stats).stage(stage).push(d);
		if let Some(m) = &self.metrics {
			m.stage.with_label_values(&[stage.as_str()]).observe(d.as_secs_f64());
		}
	}

	fn worst_case(&self, stage: Stage) -> Duration {
		lock(&self.stats).worst_case(stage)
	}

	fn count_stop(&self) {
		lock(&self.stats).stops += 1;
		if let Some(m) = &self.metrics {
			m.stops.inc();
		}
	}

	fn count_budget_exhausted(&self) {
		lock(&self.stats).budget_exhausted += 1;
		if let Some(m) = &self.metrics {
			m.budget_exhausted.inc();
		}
	}

	fn count_resync(&self) {
		lock(&self.stats).events_resync += 1;
		if let Some(m) = &self.metrics {
			m.events_resync.inc();
		}
	}

	pub fn count_cache_failure(&self) {
		lock(&self.stats).cache_refresh_failures += 1;
		if let Some(m) = &self.metrics {
			m.cache_refresh_failures.inc();
		}
	}

	pub fn audit(&self, record: shadow::AuditRecord) {
		log::warn!(
			target: LOG_TARGET,
			"{} {} at #{}: {}",
			record.action, record.tx, record.block, record.reason
		);
		if let Some(w) = &self.audit {
			w.write(&record);
		}
	}

	fn shadow(&self, record: shadow::ShadowRecord) {
		if let Some(w) = &self.shadow {
			w.write(&record);
		}
	}

	pub fn update_gauges(&self) {
		let Some(m) = &self.metrics else { return };
		let (held, released) = {
			let book = lock(&self.holds);
			(book.held_count(), book.released_count())
		};
		m.held.set(held as u64);
		m.released.set(released as u64);
		let state = lock(&self.state);
		m.ready.set(u64::from(state.readiness == Readiness::Ready));
		m.downgraded.set(u64::from(self.is_downgraded(&state)));
	}

	pub fn is_downgraded(&self, state: &RuntimeState) -> bool {
		self.config.mode == GuardMode::Hold && state.effective == Effective::Unsupported
	}

	/// Apply a cache refresh result.
	pub fn refresh(&self, result: Result<Caches, String>) {
		if let Err(e) = self.caches.apply(result) {
			log::warn!(target: LOG_TARGET, "cache refresh failed, keeping the previous caches: {e}");
			self.count_cache_failure();
		}
	}

	/// Prune holds and releases at a new best block.
	pub fn prune(&self, best: u32, in_pool: impl Fn(&H256) -> bool) {
		self.best.store(best, Ordering::Relaxed);
		let removals = lock(&self.holds).prune(best, in_pool);
		for r in removals {
			let (action, hash) = match r {
				Removal::Pruned(h) => ("prune", h),
				Removal::Expired(h) => ("expire", h),
			};
			self.audit(shadow::audit(
				action,
				&hash,
				best,
				"left the pool".into(),
				serde_json::Value::Null,
			));
		}
		self.update_gauges();
	}
}

/// The guard installed into the proposer.
pub struct CerebroGuard<C> {
	client: Arc<C>,
	shared: Arc<Shared>,
}

impl<C> CerebroGuard<C> {
	pub fn new(client: Arc<C>, shared: Arc<Shared>) -> Self {
		Self { client, shared }
	}
}

impl<C: CallApiAt<Block>> CerebroGuard<C> {
	fn readiness(&self, parent: H256) -> (Readiness, u32) {
		let version = match self.client.runtime_version_at(parent) {
			Ok(v) => v,
			Err(e) => return (Readiness::Unsupported(format!("runtime version: {e}")), 0),
		};
		let node = hydradx_runtime::VERSION.spec_version;
		if version.spec_version != node && lock(&self.shared.state).chain_spec_version != version.spec_version {
			log::warn!(
				target: LOG_TARGET,
				"spec_version mismatch: node built for {node}, chain runs {}; guard keeps running",
				version.spec_version,
			);
		}
		if self.shared.caches.get().is_none() {
			return (Readiness::NotReady("caches not loaded".into()), version.spec_version);
		}
		(Readiness::Ready, version.spec_version)
	}
}

impl<C> ExtrinsicGuard<Block, C, H256> for CerebroGuard<C>
where
	C: ProvideRuntimeApi<Block> + CallApiAt<Block> + Send + Sync + 'static,
	C::Api: BlockBuilderApi<Block> + EthereumRuntimeRPCApi<Block>,
{
	fn start(&self, ctx: GuardContext<Block>) -> Box<dyn GuardSession<Block, C, H256> + Send + '_> {
		let (readiness, chain_spec_version) = self.readiness(ctx.parent_hash);
		let eff = effective(self.shared.config.mode, &readiness);
		match &readiness {
			Readiness::Ready => {}
			Readiness::NotReady(r) | Readiness::Unsupported(r) => log::error!(
				target: LOG_TARGET,
				"not ready at {:?} (mode {}, running as {:?}): {r}",
				ctx.parent_hash, self.shared.config.mode.as_str(), eff,
			),
		}
		*lock(&self.shared.state) = RuntimeState {
			readiness,
			chain_spec_version,
			effective: eff,
		};
		self.shared.update_gauges();
		Box::new(Session::new(&self.shared, ctx, eff))
	}
}

/// Extrinsics pushed without judging. PEPL's liquidations are unsigned, accepted only from this
/// node's own worker and valid for one block, so a delayed one is a missed liquidation. A signed
/// `liquidate` is still judged: the call takes any origin.
fn always_include(xt: &[u8]) -> bool {
	use hydradx_runtime::{HydraUncheckedExtrinsic, RuntimeCall};
	use sp_runtime::generic::Preamble;

	let Ok(ext) = HydraUncheckedExtrinsic::decode(&mut &xt[..]) else {
		return false;
	};
	matches!(ext.0.preamble, Preamble::Bare(_))
		&& matches!(
			ext.0.function,
			RuntimeCall::Liquidation(
				pallet_liquidation::Call::liquidate { .. } | pallet_liquidation::Call::liquidate_with_pool { .. }
			)
		)
}

/// Everything that belongs to one proposal. Dropped with it, so an abandoned proposal leaves
/// nothing behind but its committed holds.
pub struct Session<'a> {
	shared: &'a Shared,
	parent: H256,
	parent_number: u32,
	effective: Effective,
	caches: Option<Arc<Caches>>,
	events: EventsReader,
	acc: Accumulators,
	trades: Vec<Trade>,
	oracle: Prices,
	budget: Budget,
}

/// What pass 1 decided inside the extrinsic's own transaction.
enum Pass1 {
	/// Kept in the block straight away.
	Commit {
		mark: Option<Mark>,
		index: Option<u32>,
		judgement: Judgement,
		candidate: bool,
		resync: bool,
	},
	/// Rolled back; judged from `PRE` next.
	Candidate(Box<CandidateRead>),
}

struct CandidateRead {
	mark: Mark,
	cand: Candidate,
	plan: ReadPlan,
	post: PointView,
}

/// Trades of everything already in the block, when pass 1 decoded the whole events value.
type Rebuilt = Option<Vec<Trade>>;

fn trades_of(records: &[cerebro_hydration::decode::EventRecord]) -> Vec<Trade> {
	map_records(records)
		.into_iter()
		.filter_map(|(_, e, _)| match e {
			cerebro_core::model::ChainEvent::Trade(t) => Some(t),
			_ => None,
		})
		.collect()
}

fn pass1<T: Transport + RawStorage>(
	t: &T,
	events: &mut EventsReader,
	changes: &[Change],
	xt: &[u8],
	cx: &GuardCtx,
	caches: &Caches,
) -> (Pass1, Rebuilt) {
	let applied = match events.read(t) {
		Ok(a) => a,
		Err(e) => {
			log::warn!(target: LOG_TARGET, "events read failed: {e:?}");
			let commit = Pass1::Commit {
				mark: None,
				index: None,
				judgement: Judgement::without_detection(Verdict::Inconclusive(InconclusiveReason::DecodeFailure), None),
				candidate: false,
				resync: e == EventsError::Desync,
			};
			return (commit, None);
		}
	};
	let rebuilt = applied.earlier.as_deref().map(trades_of);
	let rebuilt_cx;
	let cx = match &rebuilt {
		Some(trades) => {
			rebuilt_cx = GuardCtx {
				committed_trades: trades,
				..*cx
			};
			&rebuilt_cx
		}
		None => cx,
	};
	(pass1_judge(t, applied, changes, xt, cx, caches), rebuilt)
}

fn pass1_judge<T: Transport + RawStorage>(
	t: &T,
	applied: events::Applied,
	changes: &[Change],
	xt: &[u8],
	cx: &GuardCtx,
	caches: &Caches,
) -> Pass1 {
	let permit_actor = permit::permit_actor(t, H256(POST), xt);
	let cand = with_state(
		candidate_of(
			map_records(&applied.records),
			applied.ext_index,
			&caches.protocol,
			&caches.contracts,
			permit_actor,
			xcm::is_xcm_transfer(xt),
		),
		state::decode(changes),
		&caches.protocol,
		&caches.contracts,
	);
	if !is_candidate(&cand) {
		return Pass1::Commit {
			mark: Some(applied.mark),
			index: Some(applied.ext_index),
			judgement: Judgement::without_detection(Verdict::Allow(judge::AllowReason::Clean), cand.flows.signer),
			candidate: false,
			resync: false,
		};
	}
	let mut pf = Prefetch::new(t, POST);
	match pf.settle(|p| {
		let plan = plan(p, POST, &cand, cx);
		let post = read_point(p, POST, &plan);
		(plan, post)
	}) {
		Ok((plan, post)) => Pass1::Candidate(Box::new(CandidateRead {
			mark: applied.mark,
			cand,
			plan,
			post,
		})),
		Err(e) => {
			log::warn!(target: LOG_TARGET, "post-state reads failed: {e}");
			Pass1::Commit {
				mark: Some(applied.mark),
				index: Some(applied.ext_index),
				judgement: Judgement::without_detection(
					Verdict::Inconclusive(InconclusiveReason::DecodeFailure),
					cand.flows.signer,
				),
				candidate: true,
				resync: false,
			}
		}
	}
}

impl<'a> Session<'a> {
	pub fn new(shared: &'a Shared, ctx: GuardContext<Block>, effective: Effective) -> Self {
		Self {
			shared,
			parent: ctx.parent_hash,
			parent_number: ctx.parent_number,
			effective,
			caches: shared.caches.get(),
			events: EventsReader::default(),
			acc: Accumulators::default(),
			trades: Vec::new(),
			oracle: Prices::empty(),
			budget: Budget::new(
				Instant::now(),
				ctx.deadline,
				shared.config.finalize_reserve,
				shared.config.block_budget,
			),
		}
	}

	fn number(&self) -> u32 {
		self.parent_number.saturating_add(1)
	}

	fn mode_str(&self) -> &'static str {
		match self.effective {
			Effective::Run { enforce: true } => "hold",
			_ => "shadow",
		}
	}

	fn record(
		&self,
		hash: &H256,
		index: Option<u32>,
		j: &Judgement,
		candidate: bool,
		stage_us: StageMicros,
		resync: bool,
	) {
		self.shared.count(j.verdict);
		// Everything else is only counted: the disk log must stay proportional to holds.
		if !j.verdict.blocks() {
			return;
		}
		log::warn!(
			target: LOG_TARGET,
			"{} {hash:?} at #{}: signer net ${:.2}, pool leak ${:.2}",
			if self.mode_str() == "hold" { "HOLD" } else { "would hold" },
			self.number(),
			j.net.to_dollars(),
			j.pool_leak().to_dollars(),
		);
		self.shared.shadow(shadow::shadow_record(
			RecordCtx {
				parent: self.parent,
				number: self.number(),
				index: index.unwrap_or(u32::MAX),
				tx: hash,
				mode: self.mode_str(),
				candidate,
				stage_us,
				events_resync: resync,
			},
			j,
		));
	}

	/// Push without judging. The events of what was pushed are unknown to the reader afterwards.
	fn push_unchecked<C>(
		&mut self,
		bb: &mut BlockBuilder<'_, Block, C>,
		xt: <Block as sp_runtime::traits::Block>::Extrinsic,
		reason: Option<InconclusiveReason>,
	) -> Result<GuardOutcome, sp_blockchain::Error>
	where
		C: ProvideRuntimeApi<Block> + CallApiAt<Block>,
		C::Api: BlockBuilderApi<Block>,
	{
		if !self.budget.fits(Instant::now(), self.shared.worst_case(Stage::Repush)) {
			self.shared.count_stop();
			return Ok(GuardOutcome::Stop);
		}
		bb.push(xt)?;
		self.events.invalidate();
		if let Some(r) = reason {
			self.shared.count(Verdict::Inconclusive(r));
		}
		Ok(GuardOutcome::Pushed)
	}

	fn held(&self, j: &Judgement) -> Held {
		Held {
			signer: j.signer,
			first_seen_block: self.number(),
			first_seen_unix_ms: shadow::unix_ms(),
			net_usd: j.net.to_dollars(),
			pool_leak_usd: j.pool_leak().to_dollars(),
			assets: j.gained.clone(),
			pools: j
				.pools
				.iter()
				.filter(|p| p.loss.is_positive())
				.map(|p| p.pool.key())
				.collect(),
			summary: format!(
				"signer net ${:.2} (block ${:.2}), pool leak ${:.2}",
				j.net.to_dollars(),
				j.block_signer.to_dollars(),
				j.pool_leak().to_dollars()
			),
		}
	}
}

impl<C> GuardSession<Block, C, H256> for Session<'_>
where
	C: ProvideRuntimeApi<Block> + CallApiAt<Block>,
	C::Api: BlockBuilderApi<Block> + EthereumRuntimeRPCApi<Block>,
{
	fn apply(
		&mut self,
		bb: &mut BlockBuilder<'_, Block, C>,
		xt: <Block as sp_runtime::traits::Block>::Extrinsic,
		hash: &H256,
	) -> Result<GuardOutcome, sp_blockchain::Error> {
		let enforce = match self.effective {
			Effective::Unsupported => return self.push_unchecked(bb, xt, Some(InconclusiveReason::UnsupportedRuntime)),
			Effective::Run { enforce } => enforce,
		};
		if always_include(&xt.encode()) {
			self.shared.count(Verdict::Allow(judge::AllowReason::Exempt));
			return self.push_unchecked(bb, xt, None);
		}
		{
			let book = lock(&self.shared.holds);
			if enforce && book.is_held(hash) {
				return Ok(GuardOutcome::Skipped);
			}
			if book.is_released(hash, self.parent_number) {
				drop(book);
				return self.push_unchecked(bb, xt, None);
			}
		}
		let Some(caches) = self.caches.clone() else {
			return self.push_unchecked(bb, xt, Some(InconclusiveReason::UnsupportedRuntime));
		};
		if self.budget.exhausted() {
			if self.budget.newly_exhausted() {
				self.shared.count_budget_exhausted();
			}
			if enforce {
				self.shared.count_stop();
				return Ok(GuardOutcome::Stop);
			}
			return self.push_unchecked(bb, xt, Some(InconclusiveReason::Budget));
		}
		if !self.budget.fits(Instant::now(), self.shared.worst_case(Stage::Pass1)) {
			self.shared.count_stop();
			return Ok(GuardOutcome::Stop);
		}

		let started = Instant::now();
		let mut stage_us = StageMicros::default();
		let xt_bytes = xt.encode();
		let (parent, number) = (self.parent, self.number());
		let cx = GuardCtx {
			protocol: &caches.protocol,
			contracts: &caches.contracts,
			atoken_of: &caches.atoken_of,
			thresholds: self.shared.config.thresholds,
			committed_trades: &self.trades,
		};
		let events = &mut self.events;
		let mut decided = None;
		let mut rebuilt = None;
		let pushed = bb.push_with(xt.clone(), |iso| {
			let changes = iso.top_changes(&state::decoded);
			let t = transport::ApiTransport::new(iso, parent, number);
			let (out, trades) = catch_unwind(AssertUnwindSafe(|| match changes {
				Ok(changes) => pass1(&t, events, &transport::changes(changes), &xt_bytes, &cx, &caches),
				Err(e) => {
					log::warn!(target: LOG_TARGET, "storage changes read failed: {e:?}");
					let commit = Pass1::Commit {
						mark: None,
						index: None,
						judgement: Judgement::without_detection(
							Verdict::Inconclusive(InconclusiveReason::DecodeFailure),
							None,
						),
						candidate: false,
						resync: false,
					};
					(commit, None)
				}
			}))
			.unwrap_or_else(|_| {
				let panicked = Pass1::Commit {
					mark: None,
					index: None,
					judgement: Judgement::without_detection(
						Verdict::Inconclusive(InconclusiveReason::GuardPanic),
						None,
					),
					candidate: false,
					resync: false,
				};
				(panicked, None)
			});
			let keep = matches!(out, Pass1::Commit { .. });
			decided = Some(out);
			rebuilt = trades;
			keep
		});
		let pass1_took = started.elapsed();
		self.shared.observe(Stage::Pass1, pass1_took);
		stage_us.pass1 = pass1_took.as_micros() as u64;
		self.budget.spend(pass1_took);
		pushed?;
		if let Some(trades) = rebuilt {
			self.trades = trades;
		}
		let Some(decided) = decided else {
			return Err(sp_blockchain::Error::Application(
				"extrinsic guard: pass 1 made no decision".into(),
			));
		};

		let (mark, cand, plan, post) = match decided {
			Pass1::Commit {
				mark,
				index,
				judgement,
				candidate,
				resync,
			} => {
				match mark {
					Some(m) => self.events.commit(m),
					None => self.events.invalidate(),
				}
				if resync {
					self.shared.count_resync();
				}
				self.record(hash, index, &judgement, candidate, stage_us, resync);
				return Ok(GuardOutcome::Pushed);
			}
			Pass1::Candidate(read) => {
				let CandidateRead { mark, cand, plan, post } = *read;
				(mark, cand, plan, post)
			}
		};

		let pre_and_judge = self.shared.worst_case(Stage::Pre) + self.shared.worst_case(Stage::Judge);
		let judgement = if self.budget.fits(Instant::now(), pre_and_judge) {
			let t0 = Instant::now();
			let read = {
				let iso = bb.isolated();
				let t = transport::ApiTransport::new(&iso, parent, number);
				catch_unwind(AssertUnwindSafe(|| {
					let mut pf = Prefetch::new(&t, PRE);
					pf.settle(|p| (read_point(p, PRE, &plan), read_oracle(p, PRE, &plan, &self.oracle)))
				}))
				.unwrap_or_else(|_| Err("panicked".into()))
			};
			let pre_took = t0.elapsed();
			self.shared.observe(Stage::Pre, pre_took);
			stage_us.pre = pre_took.as_micros() as u64;
			self.budget.spend(pre_took);
			match read {
				Ok((pre, oracle)) => {
					self.oracle.extend(oracle);
					let t1 = Instant::now();
					let cx = GuardCtx {
						protocol: &caches.protocol,
						contracts: &caches.contracts,
						atoken_of: &caches.atoken_of,
						thresholds: self.shared.config.thresholds,
						committed_trades: &self.trades,
					};
					let j = catch_unwind(AssertUnwindSafe(|| {
						judge::judge(&cand, &plan, &pre, &post, &self.oracle, &cx, &self.acc)
					}))
					.unwrap_or_else(|_| {
						Judgement::without_detection(
							Verdict::Inconclusive(InconclusiveReason::GuardPanic),
							cand.flows.signer,
						)
					});
					let judge_took = t1.elapsed();
					self.shared.observe(Stage::Judge, judge_took);
					stage_us.judge = judge_took.as_micros() as u64;
					self.budget.spend(judge_took);
					j
				}
				Err(e) => {
					log::warn!(target: LOG_TARGET, "pre-state reads failed: {e}");
					Judgement::without_detection(
						Verdict::Inconclusive(InconclusiveReason::DecodeFailure),
						cand.flows.signer,
					)
				}
			}
		} else if enforce {
			// Rolled back already: it stays in the pool and is judged in a later block.
			self.shared.count_stop();
			return Ok(GuardOutcome::Stop);
		} else {
			Judgement::without_detection(Verdict::Inconclusive(InconclusiveReason::Budget), cand.flows.signer)
		};
		if self.budget.newly_exhausted() {
			self.shared.count_budget_exhausted();
		}

		if let (Verdict::Inconclusive(InconclusiveReason::DecodeGap), Some(d)) = (judgement.verdict, &cand.delta) {
			log::warn!(
				target: LOG_TARGET,
				"decode gap in {hash:?}: unbalanced {:?}, undecodable {:?}",
				d.unbalanced_assets(),
				d.undecodable,
			);
		}
		if enforce && judgement.verdict.blocks() {
			let held = self.held(&judgement);
			let summary = held.summary.clone();
			lock(&self.shared.holds).hold(*hash, held);
			self.record(hash, Some(cand.ext_index), &judgement, true, stage_us, false);
			self.shared.audit(shadow::audit(
				"hold",
				hash,
				self.number(),
				summary,
				serde_json::json!({ "pools": judgement.pools.iter().map(|p| p.pool.key()).collect::<Vec<_>>() }),
			));
			self.shared.update_gauges();
			return Ok(GuardOutcome::Skipped);
		}

		if !self.budget.fits(Instant::now(), self.shared.worst_case(Stage::Repush)) {
			self.shared.count_stop();
			return Ok(GuardOutcome::Stop);
		}
		let t2 = Instant::now();
		let repushed = bb.push(xt);
		let repush_took = t2.elapsed();
		self.shared.observe(Stage::Repush, repush_took);
		stage_us.repush = repush_took.as_micros() as u64;
		self.budget.spend(repush_took);
		if let Err(e) = repushed {
			self.events.invalidate();
			return Err(e);
		}
		self.events.commit(mark);
		self.acc.commit(&judgement);
		self.trades.extend(cand.flows.trades.iter().cloned());
		self.record(hash, Some(cand.ext_index), &judgement, true, stage_us, false);
		Ok(GuardOutcome::Pushed)
	}
}

/// Keep the shared caches fresh and prune holds as blocks are imported. Runs on a blocking
/// thread: a cache load is a few hundred storage reads and EVM calls.
pub async fn run_maintenance<C, B, P>(client: Arc<C>, pool: Arc<P>, shared: Arc<Shared>)
where
	B: sc_client_api::Backend<Block>,
	C: sc_client_api::BlockchainEvents<Block>
		+ sc_client_api::StorageProvider<Block, B>
		+ ProvideRuntimeApi<Block>
		+ sp_blockchain::HeaderBackend<Block>,
	C::Api: EthereumRuntimeRPCApi<Block>,
	P: sc_transaction_pool_api::TransactionPool<Block = Block, Hash = H256>,
{
	use futures::StreamExt;

	let refresh = |hash: H256, number: u32| {
		let t = transport::ClientTransport::<C, B>::new(&client);
		shared.refresh(cache::load(&t, hash, number));
		if let Some(c) = shared.caches.get() {
			log::info!(
				target: LOG_TARGET,
				"caches loaded at #{number}: {} stable pools, {} xyk pools, {} erc20 contracts, {} aTokens",
				c.protocol.stable_pools.len(), c.protocol.xyk_accounts.len(), c.contracts.len(), c.atoken_of.len(),
			);
		}
	};
	let info = client.info();
	refresh(info.best_hash, info.best_number);

	let mut imports = client.import_notification_stream();
	while let Some(n) = imports.next().await {
		if !n.is_new_best {
			continue;
		}
		let number = *sp_runtime::traits::Header::number(&n.header);
		if shared.caches.due(number, shared.config.cache_refresh) {
			refresh(n.hash, number);
		}
		shared.prune(number, |h| pool.ready_transaction(h).is_some());
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	fn shared(mode: GuardMode) -> Shared {
		Shared::new(
			GuardConfig {
				mode,
				thresholds: Thresholds {
					min_usd: Usd6::from_dollars(50.0),
					min_pool_leak: Usd6::from_dollars(100.0),
					min_block_usd: Usd6::from_dollars(200.0),
					min_block_pool_leak: Usd6::from_dollars(500.0),
					pool_leak_epsilon_bps: 1,
				},
				finalize_reserve: None,
				block_budget: None,
				cache_refresh: 300,
				ban_seconds: 1800,
			},
			None,
			None,
		)
	}

	fn ctx(deadline_in: Duration) -> GuardContext<Block> {
		GuardContext {
			parent_hash: H256::repeat_byte(1),
			parent_number: 99,
			deadline: Instant::now() + deadline_in,
		}
	}

	#[test]
	fn session_should_return_stop_when_cutoff_leaves_no_time_to_repush() {
		let now = Instant::now();
		let budget = Budget::new(
			now,
			now + Duration::from_millis(100),
			Some(Duration::from_millis(30)),
			None,
		);

		assert!(budget.fits(now, Duration::from_millis(60)));
		assert!(!budget.fits(now + Duration::from_millis(60), Stage::Repush.seed()));
		assert!(!budget.fits(now, Duration::from_millis(70)));
	}

	#[derive(clap::Parser)]
	struct Cli {
		#[command(flatten)]
		guard: CerebroGuardCli,
	}

	#[test]
	fn guard_config_should_hold_on_any_pool_leak_when_flags_are_unset() {
		let cli = <Cli as clap::Parser>::parse_from(["node"]);
		let config = GuardConfig::from_cli(&cli.guard, 1800);
		let th = config.thresholds;

		assert_eq!(config.mode, GuardMode::Hold);
		assert_eq!(th.min_usd, Usd6(i128::MAX));
		assert_eq!(th.min_block_usd, Usd6(i128::MAX));
		assert_eq!(th.min_pool_leak, Usd6::ZERO);
		assert_eq!(th.min_block_pool_leak, Usd6::ZERO);
		assert_eq!(th.pool_leak_epsilon_bps, 0);
	}

	#[test]
	fn budget_should_default_to_quarter_reserve_and_no_cap_when_unset() {
		let now = Instant::now();
		let mut budget = Budget::new(now, now + Duration::from_millis(1000), None, None);

		assert_eq!(budget.cutoff, now + Duration::from_millis(750));
		budget.spend(Duration::from_secs(10));
		assert!(!budget.exhausted());
	}

	#[test]
	fn budget_should_be_exhausted_when_spent_reaches_cap() {
		let now = Instant::now();
		let mut budget = Budget::new(
			now,
			now + Duration::from_millis(1000),
			None,
			Some(Duration::from_millis(300)),
		);

		budget.spend(Duration::from_millis(299));
		assert!(!budget.exhausted());
		budget.spend(Duration::from_millis(1));
		assert!(budget.exhausted());
		assert!(budget.newly_exhausted());
		assert!(!budget.newly_exhausted());
	}

	#[test]
	fn session_should_not_share_prices_or_budget_when_proposal_is_abandoned() {
		let shared = shared(GuardMode::Shadow);
		let mut abandoned = Session::new(&shared, ctx(Duration::from_secs(2)), Effective::Run { enforce: false });
		abandoned.budget.spend(Duration::from_secs(1));
		abandoned.trades.push(Trade {
			asset_in: 0,
			amount_in: 1,
			asset_out: 5,
			amount_out: 1,
			router: false,
		});
		abandoned.acc.signer_gain.insert([1; 32], Usd6::ONE);
		drop(abandoned);

		let fresh = Session::new(&shared, ctx(Duration::from_secs(2)), Effective::Run { enforce: false });

		assert!(fresh.trades.is_empty());
		assert!(fresh.acc.signer_gain.is_empty());
		assert_eq!(fresh.budget.spent, Duration::ZERO);
		assert!(!fresh.oracle.has_decimals(10));
	}

	#[test]
	fn hold_mode_should_push_unchecked_when_caches_are_not_loaded() {
		assert_eq!(
			effective(GuardMode::Hold, &Readiness::NotReady("caches not loaded".into())),
			Effective::Unsupported
		);
		assert_eq!(
			effective(GuardMode::Shadow, &Readiness::NotReady("caches not loaded".into())),
			Effective::Unsupported
		);
	}

	#[test]
	fn hold_mode_should_downgrade_when_runtime_version_is_unreadable() {
		let shared = shared(GuardMode::Hold);
		let readiness = Readiness::Unsupported("runtime version: unavailable".into());

		let eff = effective(GuardMode::Hold, &readiness);
		*lock(&shared.state) = RuntimeState {
			readiness,
			chain_spec_version: 452,
			effective: eff,
		};

		assert_eq!(eff, Effective::Unsupported);
		assert!(shared.is_downgraded(&lock(&shared.state)));
	}

	#[test]
	fn hold_mode_should_enforce_when_ready() {
		assert_eq!(
			effective(GuardMode::Hold, &Readiness::Ready),
			Effective::Run { enforce: true }
		);
		assert_eq!(
			effective(GuardMode::Shadow, &Readiness::Ready),
			Effective::Run { enforce: false }
		);
	}

	#[test]
	fn stage_stats_should_use_seed_when_samples_are_few() {
		let mut st = StageStats::default();
		for _ in 0..STAGE_MIN_SAMPLES - 1 {
			st.push(Duration::from_micros(10));
		}
		assert_eq!(st.worst_case(Duration::from_millis(25)), Duration::from_millis(25));

		st.push(Duration::from_micros(900));
		assert_eq!(st.worst_case(Duration::from_millis(25)), Duration::from_micros(900));
	}

	fn bare(call: hydradx_runtime::RuntimeCall) -> Vec<u8> {
		hydradx_runtime::HydraUncheckedExtrinsic::new_bare(call).encode()
	}

	#[test]
	fn always_include_should_match_unsigned_liquidation() {
		let call = hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
			collateral_asset: 5,
			debt_asset: 10,
			user: Default::default(),
			debt_to_cover: 1,
			route: Default::default(),
		});

		assert!(always_include(&bare(call)));
	}

	#[test]
	fn always_include_should_not_match_other_unsigned_calls() {
		let call = hydradx_runtime::RuntimeCall::Liquidation(pallet_liquidation::Call::set_borrowing_contract {
			contract: Default::default(),
		});

		assert!(!always_include(&bare(call)));
	}

	#[test]
	fn always_include_should_not_match_when_bytes_do_not_decode() {
		assert!(!always_include(&[0xff, 0x00, 0x01]));
	}

	#[test]
	fn cerebro_guard_session_should_be_send() {
		fn assert_send<T: Send>() {}
		assert_send::<Session<'static>>();
		assert_send::<Box<dyn GuardSession<Block, crate::service::ParachainClient, H256> + Send>>();
	}
}
