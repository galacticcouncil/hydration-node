//! Node-side ICE solver worker.
//!
//! On each new best block it asks the runtime for a side-effect-free
//! `SolverInput` (valid intents + SCALE-encoded simulator snapshot + ED map +
//! fee), runs the v4 solver natively, and submits the resulting `submit_solution`
//! as a bare unsigned extrinsic into the local pool. Mirrors the PEPL
//! `liquidation_worker`, but the solve is stateless per block, so it uses
//! `spawn_blocking` + an `AtomicBool` busy-guard (overlapping solves are dropped)
//! instead of a persistent thread pool.

use amm_simulator::HydrationSimulator;
use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use frame_support::__private::sp_tracing::tracing;
use futures::StreamExt;
use hydradx_runtime::{
	HydraUncheckedExtrinsic, HydrationSimulators, RuntimeCall, SimulatorPriceDenom, SmartRouteFinder,
};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v4::Solver;
use pallet_ice_runtime_api::{IceSolverApi, SolverInput};
use primitives::{AssetId, Balance};
use sc_client_api::BlockchainEvents;
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

/// Pure transform: `SolverInput` → bare `submit_solution` extrinsic. No client,
/// stream, or tx-pool — directly unit-testable. Returns the opaque extrinsic plus
/// the decode and solve durations (ms). `None` when the snapshot fails to decode,
/// the solve fails, or the solution resolves no intents.
pub(crate) fn build_extrinsic(input: SolverInput) -> Option<(sp_runtime::OpaqueExtrinsic, u128, u128)> {
	let t_decode = Instant::now();
	let state: <HydrationSimulators as SimulatorSet>::State = match Decode::decode(&mut &input.state[..]) {
		Ok(state) => state,
		Err(e) => {
			// Distinct from a clean empty solution: a decode failure means the node's
			// snapshot type drifted from the runtime's (binary lags a runtime upgrade).
			tracing::error!(target: LOG_TARGET, "failed to decode shipped snapshot: {e:?}");
			return None;
		}
	};
	// Reseed every solve — blocking-pool threads are reused.
	ED_TL.with(|m| {
		let mut m = m.borrow_mut();
		m.clear();
		m.extend(input.existential_deposits.iter().copied());
	});
	let decode_ms = t_decode.elapsed().as_millis();

	let t_solve = Instant::now();
	let solution = Solver::<HydrationSimulator<NodeSimulatorConfig>>::solve(input.intents, state, input.fee).ok()?;
	let solve_ms = t_solve.elapsed().as_millis();

	if solution.resolved_intents.is_empty() {
		return None;
	}

	let call = RuntimeCall::ICE(pallet_ice::Call::submit_solution { solution });
	let xt = HydraUncheckedExtrinsic::new_bare(call);
	let opaque = sp_runtime::OpaqueExtrinsic::decode(&mut &xt.encode()[..]).ok()?;
	Some((opaque, decode_ms, solve_ms))
}

pub struct IceSolverTask<B, C, P>(PhantomData<(B, C, P)>);

impl<B, C, P> IceSolverTask<B, C, P>
where
	B: BlockT,
	C: ProvideRuntimeApi<B> + BlockchainEvents<B> + HeaderBackend<B> + Send + Sync + 'static,
	C::Api: IceSolverApi<B>,
	P: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
	<<B as BlockT>::Header as HeaderT>::Number: sp_runtime::traits::UniqueSaturatedInto<u32>,
{
	/// Runs one solve per new best block, dropping any solve that overlaps a
	/// still-running one.
	pub async fn run(
		client: Arc<C>,
		_config: IceSolverWorkerConfig,
		transaction_pool: Arc<P>,
		sync_service: Arc<SyncingService<B>>,
		spawner: SpawnTaskHandle,
		task_data: Arc<IceSolverTaskData>,
	) {
		tracing::info!(target: LOG_TARGET, "starting");

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
				let input = {
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
						Ok(Some(input)) => input,
						Ok(None) => return, // idle block, no valid intents
						Err(e) => {
							tracing::error!(target: LOG_TARGET, "solver_input failed at block {block_no}: {e:?}");
							return;
						}
					}
				};
				let state_query_ms = t_state.elapsed().as_millis();
				let intents = input.intents.len() as u32;

				let Some((opaque_tx, decode_ms, solve_ms)) = build_extrinsic(input) else {
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
	use sp_runtime::Permill;

	#[test]
	fn build_extrinsic_should_return_none_when_state_cannot_be_decoded() {
		let input = SolverInput {
			intents: Vec::new(),
			state: vec![0xff, 0xff, 0xff],
			existential_deposits: Vec::new(),
			fee: Permill::zero(),
		};
		assert!(build_extrinsic(input).is_none());
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
