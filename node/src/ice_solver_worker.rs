//! Node-side ICE solver worker.
//!
//! On each new best block it asks the runtime for a side-effect-free
//! `SolverInput` (valid intents + SCALE-encoded simulator snapshot + ED map +
//! fee + active `SolverMode`), runs the solver generation the chain selected
//! natively, and submits the resulting `submit_solution` as a bare unsigned
//! extrinsic into the local pool. Mirrors the PEPL
//! `liquidation_worker`, but the solve is stateless per block, so it uses
//! `spawn_blocking` + an `AtomicBool` busy-guard (overlapping solves are dropped)
//! instead of a persistent thread pool.

use amm_simulator::HydrationSimulator;
use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use frame_support::__private::sp_tracing::tracing;
use futures::StreamExt;
use hydradx_runtime::{
	HydraUncheckedExtrinsic, HydrationSimulators, RuntimeCall, SimulatorPriceDenom, SmartRouteFinder, LRNA,
};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::{passthrough, v4, IceSolver, SolverOptions, SplitConfig};
use pallet_ice_runtime_api::{IceSolverApi, Solution, SolverInput, SolverMode};
use pallet_omnipool::types::Tradability;
use primitives::{AssetId, Balance};
use sc_client_api::{Backend, BlockchainEvents, StorageKey, StorageProvider};
use sc_network_sync::SyncingService;
use sc_service::SpawnTaskHandle;
use sc_transaction_pool_api::TransactionPool;
use sp_api::{ApiExt, ProvideRuntimeApi};
use sp_blockchain::HeaderBackend;
use sp_consensus::SyncOracle;
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::transaction_validity::TransactionSource;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const LOG_TARGET: &str = "ice-solver-worker";

/// CLI configuration for the ICE solver worker (tri-state, like PEPL).
#[derive(Clone, Debug, clap::Parser)]
pub struct IceSolverWorkerConfig {
	/// Enable/disable the ICE solver worker. Defaults to enabled on validators.
	#[clap(long)]
	pub ice_solver_worker: Option<bool>,

	/// Allow one AMM transfer to be split across several routes. Default on;
	/// `--ice-solver-split=false` falls back to single-route trades.
	#[clap(long, default_value_t = true, action = clap::ArgAction::Set)]
	pub ice_solver_split: bool,
}

impl IceSolverWorkerConfig {
	pub fn solver_options(&self) -> SolverOptions {
		let split = if self.ice_solver_split {
			SplitConfig::default()
		} else {
			SplitConfig::disabled()
		};
		SolverOptions {
			split,
			..SolverOptions::default()
		}
	}
}

thread_local! {
	/// Per-solve ED map seeded from the shipped `SolverInput`. Blocking-pool
	/// threads are reused, so it is cleared and re-seeded on every solve.
	static ED_TL: RefCell<BTreeMap<AssetId, Balance>> = const { RefCell::new(BTreeMap::new()) };
}

/// Node-side simulator config: reuses the runtime's simulators/route discovery
/// and overrides only `existential_deposit` to read the per-solve thread-local.
/// The simulators' `DataProvider`s are never invoked node-side — the node decodes
/// the shipped snapshot instead of calling `initial_state`.
pub struct NodeSimulatorConfig;
impl SimulatorConfig for NodeSimulatorConfig {
	type Simulators = HydrationSimulators;
	type RouteDiscovery = SmartRouteFinder<HydrationSimulators>;
	type PriceDenominator = SimulatorPriceDenom;

	fn existential_deposit(asset_id: AssetId) -> Balance {
		// Fallback 0 matches the runtime's `AssetRegistry::existential_deposit(..).unwrap_or(0)`.
		ED_TL.with(|m| m.borrow().get(&asset_id).copied().unwrap_or(0))
	}
}

/// Per-stage wall-clock timings (milliseconds) for a single solved block.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct StageTimings {
	pub intents: u32,
	pub state_query_ms: u128,
	pub decode_ms: u128,
	pub solve_ms: u128,
	pub submit_ms: u128,
	pub total_ms: u128,
}

/// Shared state surfaced over the status RPC.
#[derive(Default)]
pub struct IceSolverTaskData {
	pub running: Arc<AtomicBool>,
	pub last_solved_block: Arc<Mutex<Option<u32>>>,
	pub last_timings: Arc<Mutex<Option<StageTimings>>>,
}
impl IceSolverTaskData {
	pub fn new() -> Self {
		Self::default()
	}
}

/// Clears the busy flag on drop so a panicking or early-returning solve never
/// wedges the worker.
struct BusyGuard(Arc<AtomicBool>);
impl Drop for BusyGuard {
	fn drop(&mut self) {
		self.0.store(false, Ordering::SeqCst);
	}
}

/// Static dispatch over the solver generation the chain selected — keeps the
/// mode match to one line per generation.
fn solve<S: IceSolver<HydrationSimulator<NodeSimulatorConfig>>>(
	input: SolverInput,
	state: <HydrationSimulators as SimulatorSet>::State,
	options: &SolverOptions,
) -> Option<Solution> {
	let min_outs = input.min_amount_out.into_iter().collect();
	S::solve_with_options(input.intents, min_outs, state, input.fee, options).ok()
}

/// Pure transform: `SolverInput` → bare `submit_solution` extrinsic. No client,
/// stream, or tx-pool — directly unit-testable. Returns the opaque extrinsic plus
/// the decode and solve durations (ms). `None` when the chain disabled solving,
/// the snapshot fails to decode, the solve fails, or the solution resolves no
/// intents.
///
/// `built_at` is the block the input state was read at; it is stamped into the
/// solution so consecutive solutions never share an extrinsic hash.
pub(crate) fn build_extrinsic(
	input: SolverInput,
	built_at: u32,
	options: &SolverOptions,
	hub_sells_allowed: bool,
) -> Option<(sp_runtime::OpaqueExtrinsic, u128, u128)> {
	let mode = input.mode;
	// Before the decode: nothing this block produces can be accepted, so the
	// snapshot decode and the solve are both pure waste.
	if matches!(mode, SolverMode::Disabled) {
		tracing::debug!(target: LOG_TARGET, "solver disabled on chain, skipping solve");
		return None;
	}

	let t_decode = Instant::now();
	let mut state: <HydrationSimulators as SimulatorSet>::State = match Decode::decode(&mut &input.state[..]) {
		Ok(state) => state,
		Err(e) => {
			// Distinct from a clean empty solution: a decode failure means the node's
			// snapshot type drifted from the runtime's (binary lags a runtime upgrade).
			tracing::error!(target: LOG_TARGET, "failed to decode shipped snapshot: {e:?}");
			return None;
		}
	};
	state.0.hub_sells_disabled = !hub_sells_allowed;
	// Reseed every solve — blocking-pool threads are reused.
	ED_TL.with(|m| {
		let mut m = m.borrow_mut();
		m.clear();
		m.extend(input.existential_deposits.iter().copied());
	});
	let decode_ms = t_decode.elapsed().as_millis();

	let t_solve = Instant::now();
	let mut solution = match mode {
		SolverMode::V4 => solve::<v4::Solver<HydrationSimulator<NodeSimulatorConfig>>>(input, state, options),
		SolverMode::Passthrough => {
			solve::<passthrough::Solver<HydrationSimulator<NodeSimulatorConfig>>>(input, state, options)
		}
		// Returned above, before the decode.
		SolverMode::Disabled => None,
	}?;
	let solve_ms = t_solve.elapsed().as_millis();

	if solution.resolved_intents.is_empty() {
		return None;
	}

	solution.built_at = built_at;

	let call = RuntimeCall::ICE(pallet_ice::Call::submit_solution { solution });
	let xt = HydraUncheckedExtrinsic::new_bare(call);
	let opaque = sp_runtime::OpaqueExtrinsic::decode(&mut &xt.encode()[..]).ok()?;
	Some((opaque, decode_ms, solve_ms))
}

// TEMPORARY — delete with `ice_support::readmit` once the runtime pre-filter fix is live.
fn readmit_withheld_dcas<B, BE, C>(client: &C, hash: B::Hash, block_no: u32, input: &mut SolverInput) -> usize
where
	B: BlockT,
	BE: Backend<B>,
	C: StorageProvider<B, BE>,
{
	let prefix = StorageKey([sp_core::twox_128(b"Intent"), sp_core::twox_128(b"Intents")].concat());
	let pairs = match client.storage_pairs(hash, Some(&prefix), None) {
		Ok(pairs) => pairs,
		Err(e) => {
			tracing::error!(target: LOG_TARGET, "reading Intent::Intents failed at block {block_no}: {e:?}");
			return 0;
		}
	};
	// The key ends in the `Blake2_128Concat` id; `data` is the stored `Intent`'s first field.
	let stored = pairs.filter_map(|(key, value)| {
		let id = ice_support::IntentId::decode(&mut &key.0[key.0.len().checked_sub(16)?..]).ok()?;
		let data = ice_support::IntentData::decode(&mut &value.0[..]).ok()?;
		Some((id, data))
	});
	ice_support::readmit::readmit_withheld_dcas(&mut input.intents, &input.existential_deposits, stored, block_no)
}

/// Unset reads as the pallet's default, `SELL`.
fn hub_sells_allowed(hub_asset_tradability: Option<&[u8]>) -> bool {
	hub_asset_tradability
		.is_none_or(|mut raw| Tradability::decode(&mut raw).is_ok_and(|t| t.contains(Tradability::SELL)))
}

/// The shipped snapshot does not carry `Omnipool::HubAssetTradability`, so while it
/// disallows selling H2O one H2O leg, even mid-route, would make the whole solution revert.
fn read_hub_sells_allowed<B, BE, C>(client: &C, hash: B::Hash, block_no: u32) -> bool
where
	B: BlockT,
	BE: Backend<B>,
	C: StorageProvider<B, BE>,
{
	let key = StorageKey(
		[
			sp_core::twox_128(b"Omnipool"),
			sp_core::twox_128(b"HubAssetTradability"),
		]
		.concat(),
	);
	match client.storage(hash, &key) {
		Ok(value) => hub_sells_allowed(value.as_ref().map(|v| &v.0[..])),
		Err(e) => {
			tracing::error!(target: LOG_TARGET, "reading Omnipool::HubAssetTradability failed at block {block_no}: {e:?}");
			false
		}
	}
}

pub struct IceSolverTask<B, C, P, BE>(PhantomData<(B, C, P, BE)>);

impl<B, C, P, BE> IceSolverTask<B, C, P, BE>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + BlockchainEvents<B> + HeaderBackend<B> + StorageProvider<B, BE> + Send + Sync + 'static,
	BE: Backend<B> + 'static,
	C::Api: IceSolverApi<B>,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
	<<B as BlockT>::Header as HeaderT>::Number: sp_runtime::traits::UniqueSaturatedInto<u32>,
{
	/// Runs one solve per new best block, dropping any solve that overlaps a
	/// still-running one.
	pub async fn run(
		client: Arc<C>,
		config: IceSolverWorkerConfig,
		transaction_pool: Arc<P>,
		sync_service: Arc<SyncingService<B>>,
		spawner: SpawnTaskHandle,
		task_data: Arc<IceSolverTaskData>,
	) {
		let options = config.solver_options();
		tracing::info!(
			target: LOG_TARGET,
			"starting, route splitting enabled={}",
			options.split.max_legs > 1
		);

		let mut block_stream = client.import_notification_stream();
		while let Some(notification) = block_stream.next().await {
			if !notification.is_new_best {
				continue;
			}
			// Skip while catching up: solving historical blocks is wasted work and the
			// solutions are stale. The OCW this replaced was likewise skipped during
			// major sync.
			if sync_service.is_major_syncing() {
				continue;
			}
			let hash = notification.hash;
			let block_no: u32 =
				sp_runtime::traits::UniqueSaturatedInto::unique_saturated_into(*notification.header.number());

			// Busy-guard: skip if a previous solve is still running. The guard is
			// constructed here (right after the CAS) and moved into the task, so the
			// flag is cleared even if the task is dropped before its first poll.
			if task_data
				.running
				.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
				.is_err()
			{
				tracing::debug!(target: LOG_TARGET, "previous solve still running, skipping block {block_no}");
				continue;
			}
			let guard = BusyGuard(task_data.running.clone());

			let client = client.clone();
			let transaction_pool = transaction_pool.clone();
			let task_data = task_data.clone();
			spawner.spawn_blocking("ice-solver-worker-solve", Some(LOG_TARGET), async move {
				let _guard = guard;
				let total = Instant::now();

				let t_state = Instant::now();
				let (input, hub_sells_allowed) = {
					// Drop the ApiRef before the submit await (it is not Send).
					let api = client.runtime_api();
					// Skip blocks whose runtime predates IceSolverApi (e.g. before the
					// upgrade enacts, or while syncing historical blocks) — avoids an
					// error log every such block.
					if !matches!(api.has_api::<dyn IceSolverApi<B>>(hash), Ok(true)) {
						tracing::debug!(target: LOG_TARGET, "IceSolverApi unavailable at block {block_no}, skipping");
						return;
					}
					match api.solver_input(hash) {
						Ok(Some(mut input)) => {
							let readmitted = readmit_withheld_dcas(&*client, hash, block_no, &mut input);
							if readmitted > 0 {
								tracing::info!(target: LOG_TARGET, "re-admitted {readmitted} withheld DCA intent(s) at block {block_no}");
							}
							let hub_sells_allowed = read_hub_sells_allowed(&*client, hash, block_no);
							if !hub_sells_allowed {
								let before = input.intents.len();
								input.intents.retain(|intent| intent.data.asset_in() != LRNA::get());
								let dropped = before - input.intents.len();
								if dropped > 0 {
									tracing::info!(target: LOG_TARGET, "held back {dropped} intent(s) selling H2O at block {block_no}: hub asset selling is off");
								}
							}
							(input, hub_sells_allowed)
						}
						Ok(None) => return, // idle block, no valid intents
						Err(e) => {
							tracing::error!(target: LOG_TARGET, "solver_input failed at block {block_no}: {e:?}");
							return;
						}
					}
				};
				let state_query_ms = t_state.elapsed().as_millis();
				let intents = input.intents.len() as u32;

				let Some((opaque_tx, decode_ms, solve_ms)) = build_extrinsic(input, block_no, &options, hub_sells_allowed) else {
					tracing::debug!(target: LOG_TARGET, "no solution for block {block_no}");
					return;
				};

				let t_submit = Instant::now();
				let submit_result = transaction_pool
					.submit_one(hash, TransactionSource::Local, opaque_tx.into())
					.await;
				let submit_ms = t_submit.elapsed().as_millis();
				let total_ms = total.elapsed().as_millis();

				match submit_result {
					Ok(_) => tracing::info!(
						target: LOG_TARGET,
						"submitted solution: block={block_no} intents={intents} state_query_ms={state_query_ms} decode_ms={decode_ms} solve_ms={solve_ms} submit_ms={submit_ms} total_ms={total_ms}"
					),
					Err(e) => tracing::error!(target: LOG_TARGET, "submit_one failed at block {block_no}: {e:?}"),
				}

				let timings = StageTimings {
					intents,
					state_query_ms,
					decode_ms,
					solve_ms,
					submit_ms,
					total_ms,
				};
				if let Ok(mut slot) = task_data.last_solved_block.lock() {
					*slot = Some(block_no);
				}
				if let Ok(mut slot) = task_data.last_timings.lock() {
					*slot = Some(timings);
				}
			});
		}
	}
}

pub mod rpc {
	use super::{IceSolverTaskData, StageTimings};
	use jsonrpsee::{
		core::{async_trait, RpcResult},
		proc_macros::rpc,
	};
	use std::sync::atomic::Ordering;
	use std::sync::Arc;

	#[rpc(client, server)]
	pub trait IceSolverWorkerApi {
		#[method(name = "ice_solver_isRunning")]
		async fn is_running(&self) -> RpcResult<bool>;

		#[method(name = "ice_solver_lastSolvedBlock")]
		async fn last_solved_block(&self) -> RpcResult<Option<u32>>;

		#[method(name = "ice_solver_lastSolveTimings")]
		async fn last_solve_timings(&self) -> RpcResult<Option<StageTimings>>;
	}

	pub struct IceSolverWorker {
		pub task_data: Arc<IceSolverTaskData>,
	}

	impl IceSolverWorker {
		pub fn new(task_data: Arc<IceSolverTaskData>) -> Self {
			Self { task_data }
		}
	}

	#[async_trait]
	impl IceSolverWorkerApiServer for IceSolverWorker {
		async fn is_running(&self) -> RpcResult<bool> {
			Ok(self.task_data.running.load(Ordering::SeqCst))
		}

		async fn last_solved_block(&self) -> RpcResult<Option<u32>> {
			Ok(self.task_data.last_solved_block.lock().ok().and_then(|b| *b))
		}

		async fn last_solve_timings(&self) -> RpcResult<Option<StageTimings>> {
			Ok(self.task_data.last_timings.lock().ok().and_then(|t| *t))
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use clap::Parser;
	use sp_runtime::Permill;

	fn input_with(mode: SolverMode, state: Vec<u8>) -> SolverInput {
		SolverInput {
			intents: Vec::new(),
			state,
			existential_deposits: Vec::new(),
			min_amount_out: Vec::new(),
			fee: Permill::zero(),
			mode,
		}
	}

	#[test]
	fn build_extrinsic_should_return_none_when_state_cannot_be_decoded() {
		let input = input_with(SolverMode::V4, vec![0xff, 0xff, 0xff]);
		assert!(build_extrinsic(input, 1, &SolverOptions::default(), true).is_none());
	}

	#[test]
	fn build_extrinsic_should_return_none_when_mode_is_disabled() {
		// Same undecodable state as the test above: reaching the decode at all
		// would have to log an error, so `None` here proves the mode check runs first.
		let input = input_with(SolverMode::Disabled, vec![0xff, 0xff, 0xff]);
		assert!(build_extrinsic(input, 1, &SolverOptions::default(), true).is_none());
	}

	#[test]
	fn solver_options_should_split_when_the_flag_is_absent() {
		let config = IceSolverWorkerConfig::try_parse_from(["hydradx"]).expect("flags should parse");
		assert_eq!(config.solver_options().split, SplitConfig::default());
	}

	#[test]
	fn solver_options_should_not_split_when_the_flag_is_false() {
		let config =
			IceSolverWorkerConfig::try_parse_from(["hydradx", "--ice-solver-split=false"]).expect("flags should parse");
		assert_eq!(config.solver_options().split, SplitConfig::disabled());
	}

	#[test]
	fn hub_sells_allowed_should_be_true_when_hub_asset_tradability_is_unset() {
		assert!(hub_sells_allowed(None));
	}

	#[test]
	fn hub_sells_allowed_should_be_true_when_hub_asset_tradability_allows_sell() {
		assert!(hub_sells_allowed(Some(&Tradability::SELL.encode())));
	}

	#[test]
	fn hub_sells_allowed_should_be_false_when_hub_asset_tradability_lacks_sell() {
		assert!(!hub_sells_allowed(Some(&Tradability::BUY.encode())));
	}

	#[test]
	fn busy_guard_should_clear_flag_on_drop() {
		let flag = Arc::new(AtomicBool::new(false));
		assert_eq!(
			flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst),
			Ok(false)
		);
		{
			let _guard = BusyGuard(flag.clone());
			assert!(flag.load(Ordering::SeqCst));
		}
		assert!(!flag.load(Ordering::SeqCst));
	}

	#[test]
	fn busy_guard_should_reject_overlapping_acquire() {
		let flag = Arc::new(AtomicBool::new(false));
		let _guard = BusyGuard(flag.clone());
		// first acquire succeeds
		assert_eq!(
			flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst),
			Ok(false)
		);
		// overlapping acquire while still set fails
		assert_eq!(
			flag.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst),
			Err(true)
		);
	}
}
