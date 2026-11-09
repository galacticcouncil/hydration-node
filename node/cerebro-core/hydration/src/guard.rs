//! Per-extrinsic detection for a block author: judge one transaction from the state right before
//! and right after it, while the block is still being built.
//!
//! The caller reads both points through a [`Transport`] (usually a [`crate::Prefetch`] over the
//! block under construction) with [`plan`] + [`read_point`], then calls [`judge`], which is pure.
//! Detection mirrors the monitor's `scan_block` for one extrinsic, priced against the trades
//! committed earlier in the same block plus the candidate's own.

use crate::balance;
use crate::decode::{Decoded, RawDebtLeg};
use crate::pricing::Prices;
use crate::readers::Chain;
use crate::snapshot;
use crate::transport::Transport;
use cerebro_core::accounts::{evm_address, ProtocolAccounts};
use cerebro_core::confirm::{self, CONFIRM_FLOOR_USDT};
use cerebro_core::detect::{self, Counterparty};
use cerebro_core::model::{ChainEvent, ExtrinsicFlows, Phase, StateDelta, Trade};
use cerebro_core::pool_leak::{self, Leak};
use cerebro_core::prices::{dollar_raw_price, ExecPrices};
use cerebro_core::reader::{
	BalanceReader, OmnipoolAssetSnapshot, PriceSource, SnapshotProvider, StableswapSnapshot, XykSnapshot,
};
use cerebro_core::reduce;
use cerebro_core::value::Usd6;
use cerebro_core::{AccountId, AssetId, Balance, BlockHash, NATIVE_ASSET};
use sp_core::{H160, H256};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// State right before the candidate (after the earlier extrinsics of the block).
pub const PRE: BlockHash = [0; 32];
/// State right after the candidate.
pub const POST: BlockHash = [1; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AllowReason {
	Clean,
	BelowThreshold,
	/// On the node's always-include list; never judged.
	Exempt,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InconclusiveReason {
	/// A non-native leg has no executed rate and no oracle price.
	Unpriceable,
	/// Events or storage failed to decode.
	DecodeFailure,
	/// The extrinsic's balance writes don't add up to its issuance changes, or one didn't decode.
	DecodeGap,
	/// A balance read for the signer's swept set failed.
	IncompleteConfirmation,
	/// A touched pool could not be snapshotted.
	IncompleteSnapshot,
	/// The node and the chain run different runtimes.
	UnsupportedRuntime,
	/// A stage was skipped for time.
	Budget,
	/// The guard panicked.
	GuardPanic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
	Allow(AllowReason),
	Hold,
	Inconclusive(InconclusiveReason),
}

impl Verdict {
	/// Whether hold mode keeps the extrinsic out of the block.
	pub fn blocks(self) -> bool {
		self == Verdict::Hold
	}

	pub fn kind(self) -> &'static str {
		match self {
			Verdict::Allow(_) => "allow",
			Verdict::Hold => "hold",
			Verdict::Inconclusive(_) => "inconclusive",
		}
	}

	pub fn reason(self) -> &'static str {
		match self {
			Verdict::Allow(AllowReason::Clean) => "clean",
			Verdict::Allow(AllowReason::BelowThreshold) => "below_threshold",
			Verdict::Allow(AllowReason::Exempt) => "exempt",
			Verdict::Hold => "finding",
			Verdict::Inconclusive(r) => match r {
				InconclusiveReason::Unpriceable => "unpriceable",
				InconclusiveReason::DecodeFailure => "decode_failure",
				InconclusiveReason::DecodeGap => "decode_gap",
				InconclusiveReason::IncompleteConfirmation => "incomplete_confirmation",
				InconclusiveReason::IncompleteSnapshot => "incomplete_snapshot",
				InconclusiveReason::UnsupportedRuntime => "unsupported_runtime",
				InconclusiveReason::Budget => "budget",
				InconclusiveReason::GuardPanic => "guard_panic",
			},
		}
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Thresholds {
	pub min_usd: Usd6,
	pub min_pool_leak: Usd6,
	pub min_block_usd: Usd6,
	pub min_block_pool_leak: Usd6,
	/// A value-per-share drop of at most this many basis points is rounding, not a leak.
	pub pool_leak_epsilon_bps: i64,
}

/// What the judge needs besides the two state points.
pub struct GuardCtx<'a> {
	pub protocol: &'a ProtocolAccounts,
	/// ERC20 contract → asset id.
	pub contracts: &'a HashMap<H160, AssetId>,
	/// Underlying asset → its aToken's asset id.
	pub atoken_of: &'a BTreeMap<AssetId, AssetId>,
	pub thresholds: Thresholds,
	/// Trades of the extrinsics already committed to this block.
	pub committed_trades: &'a [Trade],
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PoolRef {
	Stableswap(AssetId),
	Xyk(AccountId),
	Omnipool(AssetId),
}

impl PoolRef {
	pub fn key(&self) -> String {
		match self {
			PoolRef::Stableswap(id) => format!("stableswap:{id}"),
			PoolRef::Xyk(acc) => format!("xyk:0x{}", hex::encode(acc)),
			PoolRef::Omnipool(asset) => format!("omnipool:{asset}"),
		}
	}
}

/// Block-local totals over the extrinsics committed so far, so a split extraction still crosses
/// the block threshold.
#[derive(Clone, Debug, Default)]
pub struct Accumulators {
	pub signer_gain: BTreeMap<AccountId, Usd6>,
	pub pool_loss: BTreeMap<PoolRef, Usd6>,
}

impl Accumulators {
	/// Record a judged extrinsic that made it into the block.
	pub fn commit(&mut self, j: &Judgement) {
		if let Some(signer) = j.signer {
			if j.signer_gain.is_positive() {
				let e = self.signer_gain.entry(signer).or_default();
				*e = e.saturating_add(j.signer_gain);
			}
		}
		for p in j.pools.iter().filter(|p| p.loss.is_positive()) {
			let e = self.pool_loss.entry(p.pool.clone()).or_default();
			*e = e.saturating_add(p.loss);
		}
	}
}

/// One extrinsic's flows, ready to be judged.
#[derive(Clone, Debug)]
pub struct Candidate {
	pub ext_index: u32,
	pub flows: ExtrinsicFlows,
	/// What the extrinsic changed in storage, when the caller has its writes.
	pub delta: Option<StateDelta>,
}

/// Build the extrinsic's flows from its own events. `permit_actor` re-attributes an EVM permit to
/// its real actor; `is_xcm` marks a cross-chain transfer of the signer's own funds.
pub fn candidate_of(
	events: Vec<Decoded>,
	ext_index: u32,
	protocol: &ProtocolAccounts,
	contracts: &HashMap<H160, AssetId>,
	permit_actor: Option<AccountId>,
	is_xcm: bool,
) -> Candidate {
	let events: Vec<(Phase, ChainEvent)> = events
		.into_iter()
		.map(|(phase, event, raw)| match (event, raw) {
			(ChainEvent::AavePosition(mut leg), Some(RawDebtLeg { reserve, .. })) => {
				leg.asset = balance::asset_of_reserve_in(contracts, reserve);
				(phase, ChainEvent::AavePosition(leg))
			}
			(ev, _) => (phase, ev),
		})
		.collect();
	let mut flows = reduce::reduce(&events, protocol)
		.per_ext
		.remove(&ext_index)
		.unwrap_or_default();
	if permit_actor.is_some() {
		flows.signer = permit_actor;
	}
	flows.is_xcm = is_xcm;
	Candidate {
		ext_index,
		flows,
		delta: None,
	}
}

/// Take balances and touched pools from the extrinsic's storage writes. ERC20 balances live in EVM
/// storage the delta doesn't decode yet, so their legs stay as the events report them.
pub fn with_state(
	mut c: Candidate,
	delta: StateDelta,
	protocol: &ProtocolAccounts,
	contracts: &HashMap<H160, AssetId>,
) -> Candidate {
	let erc20: BTreeSet<AssetId> = contracts.values().copied().collect();
	let mut deltas = delta.balance_deltas();
	for (who, assets) in &c.flows.deltas {
		for (asset, d) in assets.iter().filter(|(a, _)| erc20.contains(a)) {
			deltas.entry(*who).or_default().insert(*asset, *d);
		}
	}
	c.flows.deltas = deltas;
	c.delta = Some(delta);
	c.flows.touched_pool |= touched_pools(&c, protocol) != TouchedPools::default();
	c
}

fn decode_gap(delta: &StateDelta) -> bool {
	!delta.undecodable.is_empty() || !delta.unbalanced_assets().is_empty()
}

/// Whether the extrinsic needs judging: it moved pool funds, or its signer ended up with more of
/// some non-native asset. Everything else is committed straight away.
pub fn is_candidate(c: &Candidate) -> bool {
	let f = &c.flows;
	f.touched_pool
		|| c.delta.as_ref().is_some_and(decode_gap)
		|| f.signer
			.as_ref()
			.and_then(|s| f.deltas.get(s))
			.is_some_and(|d| d.iter().any(|(&a, &v)| a != NATIVE_ASSET && v > 0))
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TouchedPools {
	pub stable: BTreeSet<AssetId>,
	pub xyk: BTreeSet<AccountId>,
	pub omni: BTreeSet<AssetId>,
}

/// Everything [`read_point`] reads; fixed once at `POST` so `PRE` reads exactly the same keys.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadPlan {
	pub touched: TouchedPools,
	/// Stable pools whose members may anchor prices at $1.
	pub anchor_pools: BTreeSet<AssetId>,
	/// Stable pools to snapshot: touched ones plus those whose shares the signer holds.
	pub stable_pools: BTreeSet<AssetId>,
	pub balances: BTreeSet<(AccountId, AssetId)>,
	/// ERC20 contract of every balance asset that has one; its balance is `balanceOf`.
	pub erc20: BTreeMap<AssetId, H160>,
	/// Assets the oracle must price (anchor checks and pool reserve valuation).
	pub oracle_assets: BTreeSet<AssetId>,
}

/// Values read at one point. A missing entry means the read failed.
#[derive(Clone, Debug, Default)]
pub struct PointView {
	pub stableswap: BTreeMap<AssetId, StableswapSnapshot>,
	pub stable_assets: BTreeMap<AssetId, Vec<AssetId>>,
	pub xyk: BTreeMap<AccountId, XykSnapshot>,
	pub omnipool: BTreeMap<AssetId, OmnipoolAssetSnapshot>,
	/// `(who, asset)` → the balance confirmation diffs (ERC20 `balanceOf` for aTokens).
	pub balances: BTreeMap<(AccountId, AssetId), Balance>,
}

fn touched_pools(c: &Candidate, protocol: &ProtocolAccounts) -> TouchedPools {
	let flows = &c.flows;
	let mut t = TouchedPools::default();
	if let Some(d) = &c.delta {
		t.stable.extend(&d.stable_pools);
		t.stable.extend(
			d.issuance
				.keys()
				.filter(|a| protocol.stable_pool_account_of(**a).is_some()),
		);
		t.xyk.extend(&d.xyk_pools);
		t.omni.extend(&d.omnipool_assets);
	}
	for acc in flows.deltas.keys() {
		if let Some((id, _)) = protocol.stable_pools().iter().find(|(_, a)| a == acc) {
			t.stable.insert(*id);
		}
		if protocol.is_xyk(acc) {
			t.xyk.insert(*acc);
		}
	}
	if let Some(d) = flows.deltas.get(protocol.omnipool_account()) {
		t.omni.extend(d.keys().copied());
	}
	t
}

fn trade_assets(trades: &[Trade]) -> BTreeSet<AssetId> {
	trades.iter().flat_map(|t| [t.asset_in, t.asset_out]).collect()
}

/// Assets that moved: every trade so far in the block plus the candidate's own balance changes.
fn moved_assets(trades: &[Trade], flows: &ExtrinsicFlows) -> BTreeSet<AssetId> {
	let mut assets = trade_assets(trades);
	for m in flows.deltas.values() {
		assets.extend(m.keys().copied());
	}
	assets
}

fn all_trades(c: &Candidate, ctx: &GuardCtx) -> Vec<Trade> {
	ctx.committed_trades
		.iter()
		.chain(c.flows.trades.iter())
		.cloned()
		.collect()
}

/// The signer whose net the judge values, unless it is exempt (protocol account, XCM transfer).
fn judged_signer(flows: &ExtrinsicFlows, protocol: &ProtocolAccounts) -> Option<AccountId> {
	let signer = flows.signer?;
	(!flows.is_xcm && !protocol.contains(&signer) && flows.deltas.contains_key(&signer)).then_some(signer)
}

/// Decide what to read, from the state at `at` (normally `POST`).
pub fn plan<T: Transport>(t: &T, at: BlockHash, c: &Candidate, ctx: &GuardCtx) -> ReadPlan {
	let at_h = H256(at);
	let flows = &c.flows;
	let protocol = ctx.protocol;
	let trades = all_trades(c, ctx);
	let moved = moved_assets(&trades, flows);
	let touched = touched_pools(c, protocol);

	let anchor_pools: BTreeSet<AssetId> = protocol
		.stable_pools()
		.iter()
		.filter(|(id, acc)| flows.deltas.contains_key(acc) || moved.contains(id))
		.map(|(id, _)| *id)
		.collect();

	let mut stable_pools = touched.stable.clone();
	let mut balances = BTreeSet::new();
	if let Some(signer) = judged_signer(flows, protocol) {
		let per_ext = BTreeMap::from([(c.ext_index, flows.clone())]);
		let mut sweep = confirm::sweep_assets(&signer, Some(flows), &trade_assets(&trades), protocol, at, &Chain(t));
		let ev = evm_address(&signer);
		for f in per_ext.values() {
			for d in f.debt_legs.iter().filter(|d| d.collateral && d.who == ev) {
				if let Some(atk) = d.asset.and_then(|u| ctx.atoken_of.get(&u)) {
					sweep.insert(*atk);
				}
			}
		}
		stable_pools.extend(
			sweep
				.iter()
				.copied()
				.filter(|a| protocol.stable_pool_account_of(*a).is_some()),
		);
		balances.extend(sweep.into_iter().map(|a| (signer, a)));
	}

	let mut oracle_assets = moved;
	for pool in &anchor_pools {
		if let Some(assets) = snapshot::stable_pool_assets(t, *pool, at_h) {
			oracle_assets.extend(assets);
		}
	}
	for pool in &touched.stable {
		if let Some(assets) = snapshot::stable_pool_assets(t, *pool, at_h) {
			oracle_assets.extend(assets);
		}
	}
	for acc in &touched.xyk {
		if let Some(s) = snapshot::xyk(t, acc, at_h) {
			oracle_assets.extend([s.assets.0, s.assets.1]);
		}
	}
	oracle_assets.extend(touched.omni.iter().copied());

	let erc20 = ctx
		.contracts
		.iter()
		.filter(|(_, asset)| balances.iter().any(|(_, a)| a == *asset))
		.map(|(contract, asset)| (*asset, *contract))
		.collect();

	ReadPlan {
		touched,
		anchor_pools,
		stable_pools,
		balances,
		erc20,
		oracle_assets,
	}
}

/// Read everything in `plan` at `at`.
pub fn read_point<T: Transport>(t: &T, at: BlockHash, plan: &ReadPlan) -> PointView {
	let at_h = H256(at);
	let mut v = PointView::default();
	for pool in plan.stable_pools.iter().chain(&plan.touched.stable) {
		if let Some(s) = snapshot::stableswap(t, *pool, at_h) {
			v.stableswap.insert(*pool, s);
		}
	}
	for pool in &plan.anchor_pools {
		if let Some(a) = snapshot::stable_pool_assets(t, *pool, at_h) {
			v.stable_assets.insert(*pool, a);
		}
	}
	for acc in &plan.touched.xyk {
		if let Some(s) = snapshot::xyk(t, acc, at_h) {
			v.xyk.insert(*acc, s);
		}
	}
	for asset in &plan.touched.omni {
		if let Some(s) = snapshot::omnipool_asset(t, *asset, at_h) {
			v.omnipool.insert(*asset, s);
		}
	}
	for (who, asset) in &plan.balances {
		let b = match plan.erc20.get(asset) {
			Some(contract) => balance::erc20_balance_of(t, *contract, balance::evm_h160(who), at_h),
			None => balance::own_balance(t, who, *asset, at_h),
		};
		if let Some(b) = b {
			v.balances.insert((*who, *asset), b);
		}
	}
	v
}

/// Oracle prices for the plan's assets that `known` doesn't cover yet.
pub fn read_oracle<T: Transport>(t: &T, at: BlockHash, plan: &ReadPlan, known: &Prices) -> Prices {
	Prices::fetch(
		t,
		H256(at),
		plan.oracle_assets.iter().copied().filter(|a| !known.has_decimals(*a)),
	)
}

/// The two points as the core reader traits, keyed by [`PRE`] / [`POST`].
struct Views<'a> {
	pre: &'a PointView,
	post: &'a PointView,
}

impl Views<'_> {
	fn at(&self, at: BlockHash) -> Option<&PointView> {
		match at {
			PRE => Some(self.pre),
			POST => Some(self.post),
			_ => None,
		}
	}
}

impl SnapshotProvider for Views<'_> {
	fn stableswap(&self, pool: AssetId, at: BlockHash) -> Option<StableswapSnapshot> {
		self.at(at)?.stableswap.get(&pool).cloned()
	}
	fn omnipool_asset(&self, asset: AssetId, at: BlockHash) -> Option<OmnipoolAssetSnapshot> {
		self.at(at)?.omnipool.get(&asset).copied()
	}
	fn xyk(&self, pair_account: &AccountId, at: BlockHash) -> Option<XykSnapshot> {
		self.at(at)?.xyk.get(pair_account).cloned()
	}
}

impl BalanceReader for Views<'_> {
	fn own_balance(&self, who: &AccountId, asset: AssetId, at: BlockHash) -> Option<Balance> {
		self.at(at)?.balances.get(&(*who, asset)).copied()
	}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PriceOrigin {
	/// Anchored at $1 (oracle-confirmed stable or a member of a dollar stable pool).
	Anchor,
	/// Reached from trades committed earlier in the block.
	ExecPrior,
	/// Reached only through the candidate's own trades.
	ExecSelf,
	Oracle,
	None,
}

impl PriceOrigin {
	pub fn as_str(self) -> &'static str {
		match self {
			PriceOrigin::Anchor => "anchor",
			PriceOrigin::ExecPrior => "exec_prior",
			PriceOrigin::ExecSelf => "exec_self",
			PriceOrigin::Oracle => "oracle",
			PriceOrigin::None => "none",
		}
	}
}

#[derive(Clone, Debug, PartialEq)]
pub struct PoolOutcome {
	pub pool: PoolRef,
	pub vps_bps: i64,
	/// Value diluted from the pool's LPs by this extrinsic (zero within epsilon).
	pub loss: Usd6,
	/// `loss` plus what earlier extrinsics of the block diluted from the same pool.
	pub block_loss: Usd6,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Judgement {
	pub verdict: Verdict,
	pub signer: Option<AccountId>,
	/// Signer net at executed rates, from events.
	pub event_net: Usd6,
	/// Signer net confirmed against balances (the event net when below the confirmation floor).
	pub net: Usd6,
	/// What this extrinsic adds to the signer's block-local gain.
	pub signer_gain: Usd6,
	/// The signer's block-local gain including this extrinsic.
	pub block_signer: Usd6,
	/// The pricing-independent R1 rule fired. Recorded only, never holds.
	pub r1: bool,
	pub pools: Vec<PoolOutcome>,
	pub prices: Vec<(AssetId, PriceOrigin)>,
	/// Assets with a positive signer delta.
	pub gained: Vec<AssetId>,
}

impl Judgement {
	pub fn pool_leak(&self) -> Usd6 {
		self.pools.iter().map(|p| p.loss).sum()
	}

	/// A verdict with no detection behind it (the guard could not run).
	pub fn without_detection(verdict: Verdict, signer: Option<AccountId>) -> Self {
		Self {
			verdict,
			signer,
			event_net: Usd6::ZERO,
			net: Usd6::ZERO,
			signer_gain: Usd6::ZERO,
			block_signer: Usd6::ZERO,
			r1: false,
			pools: vec![],
			prices: vec![],
			gained: vec![],
		}
	}
}

type AnchorTiers = (HashMap<AssetId, f64>, HashMap<AssetId, f64>);

/// The monitor's anchor tiers: oracle-$1 assets are `confirmed`; members of an anchor pool that
/// holds a $1 member are `confirmed` if they are $1 themselves, `par` otherwise.
fn anchors(oracle: &Prices, moved: &BTreeSet<AssetId>, plan: &ReadPlan, post: &PointView) -> AnchorTiers {
	let mut confirmed = HashMap::new();
	let mut par = HashMap::new();
	for &a in moved {
		if oracle.is_dollar_peg(a) {
			if let Some(dec) = oracle.decimals_of(a) {
				confirmed.insert(a, dollar_raw_price(dec));
			}
		}
	}
	for pool in &plan.anchor_pools {
		let Some(pool_assets) = post.stable_assets.get(pool) else {
			continue;
		};
		if !pool_assets.iter().any(|a| oracle.is_dollar_peg(*a)) {
			continue;
		}
		for a in pool_assets {
			if let Some(dec) = oracle.decimals_of(*a) {
				if oracle.is_dollar_peg(*a) {
					confirmed.entry(*a).or_insert_with(|| dollar_raw_price(dec));
				} else {
					par.entry(*a).or_insert_with(|| dollar_raw_price(dec));
				}
			}
		}
	}
	(confirmed, par)
}

/// Executed rates first, oracle for what they can't reach; unpriceable legs count zero.
fn value_reserves(exec: &ExecPrices, oracle: &Prices, assets: &[AssetId], amounts: &[u128]) -> Usd6 {
	assets
		.iter()
		.zip(amounts)
		.map(|(a, amount)| {
			exec.value(*a, *amount as i128)
				.or_else(|| oracle.value_usd(*a, *amount as i128, PRE))
				.unwrap_or(Usd6::ZERO)
		})
		.sum()
}

/// `Ok(None)` when the venue has no pool at either point (e.g. the hub asset moving through the
/// omnipool account) or its invariant is unusable; `Err` when it exists at only one point, i.e. a
/// read failed.
fn assess_pool(
	pool: &PoolRef,
	pre: &PointView,
	post: &PointView,
	exec: &ExecPrices,
	oracle: &Prices,
) -> Result<Option<Leak>, ()> {
	fn both<T>(a: Option<T>, b: Option<T>) -> Result<Option<(T, T)>, ()> {
		match (a, b) {
			(Some(a), Some(b)) => Ok(Some((a, b))),
			(None, None) => Ok(None),
			_ => Err(()),
		}
	}
	match pool {
		PoolRef::Stableswap(id) => {
			let Some((prev, cur)) = both(pre.stableswap.get(id), post.stableswap.get(id))? else {
				return Ok(None);
			};
			let amounts: Vec<u128> = prev.reserves.iter().map(|r| r.amount).collect();
			let usd = value_reserves(exec, oracle, &prev.assets, &amounts);
			Ok(pool_leak::stableswap_leak(prev, cur, usd))
		}
		PoolRef::Xyk(acc) => {
			let Some((prev, cur)) = both(pre.xyk.get(acc), post.xyk.get(acc))? else {
				return Ok(None);
			};
			let usd = value_reserves(
				exec,
				oracle,
				&[prev.assets.0, prev.assets.1],
				&[prev.reserves.0, prev.reserves.1],
			);
			Ok(pool_leak::xyk_leak(prev, cur, usd))
		}
		PoolRef::Omnipool(asset) => {
			let Some((prev, cur)) = both(pre.omnipool.get(asset), post.omnipool.get(asset))? else {
				return Ok(None);
			};
			let usd = value_reserves(exec, oracle, &[*asset], &[prev.reserve]);
			Ok(pool_leak::omnipool_asset_leak(prev, cur, usd))
		}
	}
}

/// Pools an LP gain may have come from: the omnipool leg of every gained asset and the stable
/// pool behind any share the signer moved (as the monitor's LP-withdrawal suppression).
fn lp_pools(flows: &ExtrinsicFlows, signer: &AccountId, protocol: &ProtocolAccounts) -> Vec<PoolRef> {
	let Some(deltas) = flows.deltas.get(signer) else {
		return vec![];
	};
	let mut pools: Vec<PoolRef> = deltas
		.iter()
		.filter(|(_, &d)| d > 0)
		.map(|(a, _)| PoolRef::Omnipool(*a))
		.collect();
	pools.extend(
		deltas
			.keys()
			.filter(|a| protocol.stable_pool_account_of(**a).is_some())
			.map(|a| PoolRef::Stableswap(*a)),
	);
	pools
}

/// Judge the candidate from the two state points.
pub fn judge(
	c: &Candidate,
	plan: &ReadPlan,
	pre: &PointView,
	post: &PointView,
	oracle: &Prices,
	ctx: &GuardCtx,
	acc: &Accumulators,
) -> Judgement {
	let flows = &c.flows;
	let protocol = ctx.protocol;
	let th = ctx.thresholds;
	let per_ext = BTreeMap::from([(c.ext_index, flows.clone())]);
	let trades = all_trades(c, ctx);
	let moved = moved_assets(&trades, flows);
	let (confirmed, par) = anchors(oracle, &moved, plan, post);
	let exec = ExecPrices::build(&trades, &confirmed, &par);
	let r1 = !detect::detect_signer_gain(&per_ext, protocol).is_empty();
	let views = Views { pre, post };

	let mut incomplete_snapshot = false;
	let mut assessed = BTreeSet::new();
	let mut pools = Vec::new();
	let touched = plan
		.touched
		.stable
		.iter()
		.map(|id| PoolRef::Stableswap(*id))
		.chain(plan.touched.xyk.iter().map(|a| PoolRef::Xyk(*a)))
		.chain(plan.touched.omni.iter().map(|a| PoolRef::Omnipool(*a)));
	for pool in touched {
		match assess_pool(&pool, pre, post, &exec, oracle) {
			Err(()) => incomplete_snapshot = true,
			Ok(None) => {}
			Ok(Some(leak)) => {
				assessed.insert(pool.clone());
				let loss = if th.pool_leak_epsilon_bps > 0 && leak.vps_bps >= -th.pool_leak_epsilon_bps {
					Usd6::ZERO
				} else {
					leak.suspicious
				};
				let block_loss = acc
					.pool_loss
					.get(&pool)
					.copied()
					.unwrap_or_default()
					.saturating_add(loss);
				pools.push(PoolOutcome {
					pool,
					vps_bps: leak.vps_bps,
					loss,
					block_loss,
				});
			}
		}
	}
	let tx_leak: Usd6 = pools.iter().map(|p| p.loss).sum();
	// A pool that can't be priced loses $0, so its value-per-share drop alone holds.
	let pool_hold = tx_leak > th.min_pool_leak
		|| pools
			.iter()
			.any(|p| p.loss.is_positive() && p.block_loss > th.min_block_pool_leak)
		|| pools
			.iter()
			.any(|p| p.vps_bps < -th.pool_leak_epsilon_bps.max(0) && p.loss == Usd6::ZERO);

	let signer = judged_signer(flows, protocol);
	let mut event_net = Usd6::ZERO;
	let mut net = Usd6::ZERO;
	let mut signer_gain = Usd6::ZERO;
	let mut block_signer = Usd6::ZERO;
	let mut signer_hold = false;
	let mut unpriceable = false;
	let mut incomplete_confirmation = false;
	let mut gained = Vec::new();
	if let Some(signer) = signer {
		let action = detect::signer_actions(&per_ext, protocol, &exec)
			.into_iter()
			.find(|a| a.account == signer);
		if let Some(action) = action {
			event_net = action.net;
			net = event_net;
			gained = flows.deltas[&signer]
				.iter()
				.filter(|(_, &d)| d > 0)
				.map(|(a, _)| *a)
				.collect();
			let lp_fair = action.counterparty == Counterparty::Lp && {
				let keys = lp_pools(flows, &signer, protocol);
				let leaked = pools
					.iter()
					.any(|p| keys.contains(&p.pool) && p.loss.0 >= pool_leak::LEAK_RECORD_FLOOR);
				!leaked && keys.iter().any(|k| assessed.contains(k))
			};
			if !action.priced {
				unpriceable = true;
			} else {
				let confirmed = (event_net.abs().0 >= CONFIRM_FLOOR_USDT).then(|| {
					confirm::signer_real_net(
						&signer,
						c.ext_index,
						&per_ext,
						&trade_assets(&trades),
						protocol,
						ctx.atoken_of,
						PRE,
						POST,
						&exec,
						&views,
						&views,
					)
				});
				match confirmed {
					Some(Some(real)) => net = real,
					// A fair LP withdrawal is allowed before confirmation, as in the monitor.
					Some(None) => incomplete_confirmation = !lp_fair,
					None => {}
				}
				if !lp_fair {
					if let Some(Some(real)) = confirmed {
						signer_gain = real.max(Usd6::ZERO);
					}
					block_signer = acc
						.signer_gain
						.get(&signer)
						.copied()
						.unwrap_or_default()
						.saturating_add(signer_gain);
					signer_hold = !incomplete_confirmation
						&& ((event_net > th.min_usd && net > th.min_usd)
							|| (signer_gain.is_positive() && block_signer > th.min_block_usd));
				}
			}
		}
	}

	let verdict = if pool_hold || signer_hold {
		Verdict::Hold
	} else if c.delta.as_ref().is_some_and(decode_gap) {
		Verdict::Inconclusive(InconclusiveReason::DecodeGap)
	} else if incomplete_snapshot {
		Verdict::Inconclusive(InconclusiveReason::IncompleteSnapshot)
	} else if incomplete_confirmation {
		Verdict::Inconclusive(InconclusiveReason::IncompleteConfirmation)
	} else if unpriceable {
		Verdict::Inconclusive(InconclusiveReason::Unpriceable)
	} else if net.is_positive() || tx_leak.is_positive() {
		Verdict::Allow(AllowReason::BelowThreshold)
	} else {
		Verdict::Allow(AllowReason::Clean)
	};

	let prior = ExecPrices::build(ctx.committed_trades, &confirmed, &par);
	let mut priced_assets: BTreeSet<AssetId> = signer
		.and_then(|s| flows.deltas.get(&s))
		.map(|d| d.keys().copied().collect())
		.unwrap_or_default();
	priced_assets.extend(plan.touched.omni.iter().copied());
	let prices = priced_assets
		.into_iter()
		.map(|a| {
			let origin = if confirmed.contains_key(&a) || par.contains_key(&a) {
				PriceOrigin::Anchor
			} else if prior.price_of(a).is_some() {
				PriceOrigin::ExecPrior
			} else if exec.price_of(a).is_some() {
				PriceOrigin::ExecSelf
			} else if oracle.can_price(a) {
				PriceOrigin::Oracle
			} else {
				PriceOrigin::None
			};
			(a, origin)
		})
		.collect();

	Judgement {
		verdict,
		signer,
		event_net,
		net,
		signer_gain,
		block_signer,
		r1,
		pools,
		prices,
		gained,
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use cerebro_core::model::TransferLeg;

	const SIGNER: AccountId = [9; 32];
	const PAYMASTER: AccountId = [8; 32];
	const USDT: AssetId = 10;
	const DOT: AssetId = 5;
	const PAY: u128 = 100_000_000; // $100
	const GET: u128 = 10_000_000_000; // 1 DOT at 10 decimals, so DOT executes at $100
	const START_USDT: u128 = 1_000_000_000_000;

	fn omni() -> AccountId {
		snapshot::omnipool_account()
	}

	fn protocol() -> ProtocolAccounts {
		ProtocolAccounts {
			derived: [omni()].into_iter().collect(),
			stable_pools: vec![],
			xyk_accounts: Default::default(),
			omnipool_account: omni(),
		}
	}

	fn thresholds() -> Thresholds {
		Thresholds {
			min_usd: Usd6::from_dollars(50.0),
			min_pool_leak: Usd6::from_dollars(100.0),
			min_block_usd: Usd6::from_dollars(200.0),
			min_block_pool_leak: Usd6::from_dollars(500.0),
			pool_leak_epsilon_bps: 1,
		}
	}

	fn swap_events(received: u128, fee_payer: AccountId) -> Vec<Decoded> {
		let p = Phase::ApplyExtrinsic(3);
		vec![
			(
				p,
				ChainEvent::Transfer(TransferLeg {
					asset: USDT,
					from: Some(SIGNER),
					to: Some(omni()),
					amount: PAY,
				}),
				None,
			),
			(
				p,
				ChainEvent::Transfer(TransferLeg {
					asset: DOT,
					from: Some(omni()),
					to: Some(SIGNER),
					amount: received,
				}),
				None,
			),
			(
				p,
				ChainEvent::Trade(Trade {
					asset_in: USDT,
					amount_in: PAY,
					asset_out: DOT,
					amount_out: GET,
					router: false,
				}),
				None,
			),
			(
				p,
				ChainEvent::FeePaid {
					who: fee_payer,
					amount: 1_000,
				},
				None,
			),
		]
	}

	fn swap(received: u128) -> Candidate {
		candidate_of(
			swap_events(received, SIGNER),
			3,
			&protocol(),
			&HashMap::new(),
			None,
			false,
		)
	}

	fn omni_snap(reserve: u128) -> OmnipoolAssetSnapshot {
		OmnipoolAssetSnapshot {
			reserve,
			hub_reserve: 5_000_000_000_000,
			shares: 1_000_000_000_000,
		}
	}

	/// PRE/POST views for `swap(received)` with flat pools unless `post_reserves` says otherwise.
	fn views(received: u128, post_reserves: [Option<u128>; 2]) -> (ReadPlan, PointView, PointView) {
		let c = swap(received);
		let ctx_trades: Vec<Trade> = vec![];
		let atoken_of = BTreeMap::new();
		let contracts = HashMap::new();
		let p = protocol();
		let cx = GuardCtx {
			protocol: &p,
			contracts: &contracts,
			atoken_of: &atoken_of,
			thresholds: thresholds(),
			committed_trades: &ctx_trades,
		};
		let mut plan = plan(&NoChain, POST, &c, &cx);
		plan.balances = [(SIGNER, USDT), (SIGNER, DOT)].into_iter().collect();
		let pre = PointView {
			omnipool: [(DOT, omni_snap(1_000_000_000_000)), (USDT, omni_snap(10_000_000_000))].into(),
			balances: [((SIGNER, USDT), START_USDT), ((SIGNER, DOT), 0)].into(),
			..Default::default()
		};
		let mut post = PointView {
			balances: [((SIGNER, USDT), START_USDT - PAY), ((SIGNER, DOT), received)].into(),
			..Default::default()
		};
		for (asset, reserve, default) in [
			(DOT, post_reserves[0], 1_000_000_000_000),
			(USDT, post_reserves[1], 10_000_000_000),
		] {
			if reserve != Some(0) {
				post.omnipool.insert(asset, omni_snap(reserve.unwrap_or(default)));
			}
		}
		(plan, pre, post)
	}

	/// A transport with nothing behind it: every read is absent.
	struct NoChain;

	impl Transport for NoChain {
		fn storage_many(&self, keys: &[Vec<u8>], _at: BlockHash) -> Result<Vec<Option<Vec<u8>>>, String> {
			Ok(vec![None; keys.len()])
		}
		fn evm_view(&self, _to: [u8; 20], _data: &[u8], _at: BlockHash) -> Result<Vec<u8>, String> {
			Err("no chain".into())
		}
		fn storage_keys(&self, _prefix: &[u8], _at: BlockHash) -> Result<Vec<Vec<u8>>, String> {
			Ok(vec![])
		}
		fn block_number(&self, _at: BlockHash) -> Result<u32, String> {
			Ok(1)
		}
	}

	fn run(c: &Candidate, plan: &ReadPlan, pre: &PointView, post: &PointView, acc: &Accumulators) -> Judgement {
		run_with(thresholds(), c, plan, pre, post, acc)
	}

	fn node_thresholds() -> Thresholds {
		Thresholds {
			min_usd: Usd6(i128::MAX),
			min_pool_leak: Usd6::ZERO,
			min_block_usd: Usd6(i128::MAX),
			min_block_pool_leak: Usd6::ZERO,
			pool_leak_epsilon_bps: 0,
		}
	}

	fn run_with(
		th: Thresholds,
		c: &Candidate,
		plan: &ReadPlan,
		pre: &PointView,
		post: &PointView,
		acc: &Accumulators,
	) -> Judgement {
		let p = protocol();
		let (contracts, atoken_of) = (HashMap::new(), BTreeMap::new());
		let cx = GuardCtx {
			protocol: &p,
			contracts: &contracts,
			atoken_of: &atoken_of,
			thresholds: th,
			committed_trades: &[],
		};
		judge(c, plan, pre, post, &Prices::empty(), &cx, acc)
	}

	#[test]
	fn judge_should_allow_when_fair_swap_is_applied() {
		let (plan, pre, post) = views(GET, [None, None]);

		let j = run(&swap(GET), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Allow(AllowReason::Clean));
		assert_eq!(j.net, Usd6::ZERO);
		assert_eq!(j.signer, Some(SIGNER));
	}

	#[test]
	fn judge_should_hold_when_signer_nets_above_threshold() {
		let received = GET * 16 / 10;
		let (plan, pre, post) = views(received, [None, None]);

		let j = run(&swap(received), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Hold);
		assert_eq!(j.event_net, Usd6(60_000_000));
		assert_eq!(j.net, Usd6(60_000_000));
		assert_eq!(j.gained, vec![DOT]);
	}

	#[test]
	fn judge_should_allow_below_threshold_when_gain_is_small() {
		let received = GET * 13 / 10;
		let (plan, pre, post) = views(received, [None, None]);

		let j = run(&swap(received), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Allow(AllowReason::BelowThreshold));
		assert_eq!(j.signer_gain, Usd6(30_000_000));
	}

	#[test]
	fn judge_should_hold_when_split_extraction_crosses_block_accumulator() {
		let received = GET * 13 / 10;
		let (plan, pre, post) = views(received, [None, None]);
		let mut acc = Accumulators::default();
		acc.signer_gain.insert(SIGNER, Usd6(180_000_000));

		let j = run(&swap(received), &plan, &pre, &post, &acc);

		assert_eq!(j.verdict, Verdict::Hold);
		assert_eq!(j.block_signer, Usd6(210_000_000));
	}

	#[test]
	fn accumulators_should_add_positive_gain_when_committed() {
		let received = GET * 13 / 10;
		let (plan, pre, post) = views(received, [None, None]);
		let mut acc = Accumulators::default();

		acc.commit(&run(&swap(received), &plan, &pre, &post, &acc.clone()));
		acc.commit(&run(&swap(GET), &plan, &pre, &views(GET, [None, None]).2, &acc.clone()));

		assert_eq!(acc.signer_gain.get(&SIGNER), Some(&Usd6(30_000_000)));
	}

	#[test]
	fn judge_should_be_inconclusive_when_confirmation_read_fails() {
		let received = GET * 16 / 10;
		let (plan, mut pre, post) = views(received, [None, None]);
		pre.balances.remove(&(SIGNER, DOT));

		let j = run(&swap(received), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(
			j.verdict,
			Verdict::Inconclusive(InconclusiveReason::IncompleteConfirmation)
		);
	}

	#[test]
	fn judge_should_assess_every_pool_when_tx_touches_several_pools() {
		let (plan, pre, post) = views(GET, [Some(960_000_000_000), Some(9_600_000_000)]);

		let j = run(&swap(GET), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Hold);
		assert_eq!(
			j.pools,
			vec![
				PoolOutcome {
					pool: PoolRef::Omnipool(DOT),
					vps_bps: -202,
					loss: Usd6(202_041_029),
					block_loss: Usd6(202_041_029),
				},
				PoolOutcome {
					pool: PoolRef::Omnipool(USDT),
					vps_bps: -202,
					loss: Usd6(202_041_029),
					block_loss: Usd6(202_041_029),
				},
			]
		);
		assert_eq!(j.pool_leak(), Usd6(404_082_058));
	}

	#[test]
	fn judge_should_be_inconclusive_when_one_touched_pool_snapshot_fails() {
		let (plan, pre, post) = views(GET, [None, Some(0)]);

		let j = run(&swap(GET), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Inconclusive(InconclusiveReason::IncompleteSnapshot));
	}

	#[test]
	fn judge_should_hold_when_one_pool_leaks_and_another_snapshot_fails() {
		let (plan, pre, post) = views(GET, [Some(960_000_000_000), Some(0)]);

		let j = run(&swap(GET), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Hold);
	}

	#[test]
	fn judge_should_ignore_vps_drop_when_within_epsilon() {
		let (plan, pre, post) = views(GET, [Some(999_900_000_000), None]);

		let j = run(&swap(GET), &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Allow(AllowReason::Clean));
		assert_eq!(j.pools[0].vps_bps, -1);
		assert_eq!(j.pools[0].loss, Usd6::ZERO);
	}

	#[test]
	fn judge_should_allow_when_fair_swap_is_applied_with_node_thresholds() {
		let (plan, pre, post) = views(GET, [None, None]);

		let j = run_with(
			node_thresholds(),
			&swap(GET),
			&plan,
			&pre,
			&post,
			&Accumulators::default(),
		);

		assert_eq!(j.verdict, Verdict::Allow(AllowReason::Clean));
	}

	#[test]
	fn judge_should_hold_when_vps_drop_rounds_to_zero_bps_and_epsilon_is_zero() {
		let (plan, pre, post) = views(GET, [Some(999_990_000_000), None]);

		let j = run_with(
			node_thresholds(),
			&swap(GET),
			&plan,
			&pre,
			&post,
			&Accumulators::default(),
		);

		assert_eq!(j.verdict, Verdict::Hold);
		assert_eq!(j.pools[0].vps_bps, 0);
		assert!(j.pools[0].loss.is_positive());
	}

	#[test]
	fn judge_should_hold_when_unpriced_pool_value_per_share_drops() {
		const UNPRICED: AssetId = 99;
		let (mut plan, mut pre, mut post) = views(GET, [None, None]);
		plan.touched.omni.insert(UNPRICED);
		pre.omnipool.insert(UNPRICED, omni_snap(1_000_000_000_000));
		post.omnipool.insert(UNPRICED, omni_snap(900_000_000_000));

		let j = run_with(
			node_thresholds(),
			&swap(GET),
			&plan,
			&pre,
			&post,
			&Accumulators::default(),
		);

		assert_eq!(j.verdict, Verdict::Hold);
		let unpriced = j
			.pools
			.iter()
			.find(|p| p.pool == PoolRef::Omnipool(UNPRICED))
			.expect("assessed");
		assert_eq!(unpriced.loss, Usd6::ZERO);
		assert!(unpriced.vps_bps < 0);
	}

	#[test]
	fn judge_should_allow_when_dispatch_failed_and_no_pool_moved() {
		let events = vec![(
			Phase::ApplyExtrinsic(3),
			ChainEvent::FeePaid {
				who: SIGNER,
				amount: 1_000,
			},
			None,
		)];

		let c = candidate_of(events, 3, &protocol(), &HashMap::new(), None, false);

		assert!(!is_candidate(&c));
	}

	#[test]
	fn judge_should_hold_when_extraction_is_nested_in_utility_batch() {
		let received = GET * 16 / 10;
		let mut batched = swap_events(received, SIGNER);
		batched.insert(0, (Phase::ApplyExtrinsic(3), ChainEvent::Inert, None));
		batched.push((Phase::ApplyExtrinsic(3), ChainEvent::Inert, None));
		let c = candidate_of(batched, 3, &protocol(), &HashMap::new(), None, false);
		let (plan, pre, post) = views(received, [None, None]);

		let j = run(&c, &plan, &pre, &post, &Accumulators::default());

		assert_eq!(c.flows.deltas, swap(received).flows.deltas);
		assert_eq!(j.verdict, Verdict::Hold);
	}

	#[test]
	fn judge_should_hold_when_extraction_is_nested_in_proxy_and_permit() {
		let received = GET * 16 / 10;
		let c = candidate_of(
			swap_events(received, PAYMASTER),
			3,
			&protocol(),
			&HashMap::new(),
			Some(SIGNER),
			false,
		);
		let (plan, pre, post) = views(received, [None, None]);

		let j = run(&c, &plan, &pre, &post, &Accumulators::default());

		assert_eq!(c.flows.signer, Some(SIGNER));
		assert_eq!(j.verdict, Verdict::Hold);
	}

	#[test]
	fn judge_should_not_judge_signer_when_extrinsic_is_xcm_transfer() {
		let received = GET * 16 / 10;
		let c = candidate_of(
			swap_events(received, SIGNER),
			3,
			&protocol(),
			&HashMap::new(),
			None,
			true,
		);
		let (plan, pre, post) = views(received, [None, None]);

		let j = run(&c, &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.signer, None);
		assert_eq!(j.verdict, Verdict::Allow(AllowReason::Clean));
	}

	#[test]
	fn plan_should_sweep_signer_assets_when_candidate_has_signer() {
		let c = swap(GET);
		let p = protocol();
		let (contracts, atoken_of) = (HashMap::new(), BTreeMap::new());
		let cx = GuardCtx {
			protocol: &p,
			contracts: &contracts,
			atoken_of: &atoken_of,
			thresholds: thresholds(),
			committed_trades: &[],
		};

		let plan = plan(&NoChain, POST, &c, &cx);

		assert_eq!(plan.balances, [(SIGNER, DOT), (SIGNER, USDT)].into_iter().collect());
		assert_eq!(plan.touched.omni, [DOT, USDT].into_iter().collect());
		assert_eq!(plan.oracle_assets, [DOT, USDT].into_iter().collect());
	}

	fn swap_delta(signer_dot: u128) -> StateDelta {
		StateDelta {
			balances: [
				((SIGNER, USDT), (START_USDT, START_USDT - PAY)),
				((omni(), USDT), (0, PAY)),
				((omni(), DOT), (GET, GET - signer_dot.min(GET))),
				((SIGNER, DOT), (0, signer_dot)),
			]
			.into(),
			..Default::default()
		}
	}

	#[test]
	fn with_state_should_take_balances_from_delta_when_events_disagree() {
		let c = with_state(swap(GET), swap_delta(GET / 2), &protocol(), &HashMap::new());

		assert_eq!(
			c.flows.deltas[&SIGNER],
			BTreeMap::from([(USDT, -(PAY as i128)), (DOT, (GET / 2) as i128)])
		);
	}

	#[test]
	fn with_state_should_keep_erc20_leg_from_events_when_delta_has_none() {
		let mut delta = swap_delta(GET);
		delta.balances.retain(|(_, a), _| *a != DOT);
		let contracts = HashMap::from([(H160::repeat_byte(1), DOT)]);

		let c = with_state(swap(GET), delta, &protocol(), &contracts);

		assert_eq!(c.flows.deltas[&SIGNER][&DOT], GET as i128);
	}

	#[test]
	fn is_candidate_should_be_true_when_delta_mints_pool_shares_without_events() {
		let pool_account = [9; 32];
		let p = ProtocolAccounts {
			derived: [pool_account].into_iter().collect(),
			stable_pools: vec![(102, pool_account)],
			..protocol()
		};
		let delta = StateDelta {
			balances: [((SIGNER, 102), (0, 50))].into(),
			issuance: [(102, (1_000, 1_050))].into(),
			..Default::default()
		};
		let quiet = candidate_of(vec![], 3, &p, &HashMap::new(), None, false);
		assert!(!is_candidate(&quiet));

		let c = with_state(quiet, delta, &p, &HashMap::new());

		assert!(is_candidate(&c));
		assert_eq!(touched_pools(&c, &p).stable, BTreeSet::from([102]));
	}

	#[test]
	fn judge_should_allow_when_delta_conserves_balances() {
		let (plan, pre, post) = views(GET, [None, None]);
		let c = with_state(swap(GET), swap_delta(GET), &protocol(), &HashMap::new());

		let j = run(&c, &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Allow(AllowReason::Clean));
	}

	#[test]
	fn judge_should_report_decode_gap_when_balance_moves_without_issuance() {
		let (plan, pre, post) = views(GET, [None, None]);
		let mut delta = swap_delta(GET);
		delta.balances.insert(([4; 32], DOT), (0, 1));
		let c = with_state(swap(GET), delta, &protocol(), &HashMap::new());

		let j = run(&c, &plan, &pre, &post, &Accumulators::default());

		assert_eq!(j.verdict, Verdict::Inconclusive(InconclusiveReason::DecodeGap));
		assert!(!j.verdict.blocks());
	}
}
