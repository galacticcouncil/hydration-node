//! `cerebroGuard_*` RPCs. All unsafe: release and drop change what this node authors, and a
//! public status/held endpoint would advertise which transactions it is excluding.

use super::{lock, shadow, GuardMode, Readiness, Shared};
use hydradx_runtime::opaque::Block;
use jsonrpsee::{
	core::{async_trait, RpcResult},
	proc_macros::rpc,
	types::ErrorObjectOwned,
	Extensions,
};
use sc_rpc_api::check_if_safe;
use sc_transaction_pool_api::{InPoolTransaction, TransactionPool, TxInvalidityReportMap};
use serde::Serialize;
use sp_blockchain::HeaderBackend;
use sp_core::H256;
use std::collections::BTreeMap;
use std::sync::Arc;

pub const DEFAULT_RELEASE_TTL: u32 = 10;
const HELD_NOTE: &str = "excluded from this node's proposals only";

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuardStatus {
	pub mode: String,
	pub effective_mode: String,
	pub ready: bool,
	pub not_ready_reason: Option<String>,
	pub node_spec_version: u32,
	pub chain_spec_version: u32,
	pub counters: BTreeMap<String, u64>,
	pub stage_ms: BTreeMap<String, [f64; 3]>,
	pub stops: u64,
	pub budget_exhausted: u64,
	pub events_resync: u64,
	pub cache_refresh_failures: u64,
	pub held: u32,
	pub released: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct HeldTx {
	pub hash: H256,
	pub signer: Option<String>,
	pub first_seen_block: u32,
	pub first_seen_unix_ms: u64,
	pub net_usd: f64,
	pub pool_leak_usd: f64,
	pub assets: Vec<u32>,
	pub pools: Vec<String>,
	pub note: &'static str,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseResult {
	pub hash: H256,
	pub until_block: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DropResult {
	pub removed: Vec<H256>,
	pub ban_seconds: u64,
}

/// What the RPC needs from the transaction pool and chain.
pub trait PoolControl: Send + Sync {
	fn best(&self) -> (H256, u32);
	/// Remove `hash` (and its dependents) from the pool; returns what was removed.
	fn remove(&self, at: H256, hash: H256) -> Vec<H256>;
}

pub struct NodePool<C, P> {
	pub client: Arc<C>,
	pub pool: Arc<P>,
}

impl<C, P> PoolControl for NodePool<C, P>
where
	C: HeaderBackend<Block> + Send + Sync,
	P: TransactionPool<Block = Block, Hash = H256> + Send + Sync,
{
	fn best(&self) -> (H256, u32) {
		let info = self.client.info();
		(info.best_hash, info.best_number)
	}

	fn remove(&self, at: H256, hash: H256) -> Vec<H256> {
		self.pool
			.report_invalid(Some(at), TxInvalidityReportMap::from_iter([(hash, None)]))
			.into_iter()
			.map(|tx| *tx.hash())
			.collect()
	}
}

#[rpc(server)]
pub trait CerebroGuardRpc {
	#[method(name = "cerebroGuard_status", with_extensions)]
	async fn status(&self) -> RpcResult<GuardStatus>;

	#[method(name = "cerebroGuard_held", with_extensions)]
	async fn held(&self) -> RpcResult<Vec<HeldTx>>;

	#[method(name = "cerebroGuard_release", with_extensions)]
	async fn release(&self, hash: H256, ttl_blocks: Option<u32>, reason: String) -> RpcResult<ReleaseResult>;

	#[method(name = "cerebroGuard_drop", with_extensions)]
	async fn drop_tx(&self, hash: H256, reason: String) -> RpcResult<DropResult>;
}

pub struct CerebroGuardRpcImpl {
	shared: Arc<Shared>,
	pool: Arc<dyn PoolControl>,
}

impl CerebroGuardRpcImpl {
	pub fn new(shared: Arc<Shared>, pool: Arc<dyn PoolControl>) -> Self {
		Self { shared, pool }
	}

	pub fn status(&self) -> GuardStatus {
		let state = lock(&self.shared.state).clone();
		let downgraded = self.shared.is_downgraded(&state);
		let stats = lock(&self.shared.stats);
		let (held, released) = {
			let book = lock(&self.shared.holds);
			(book.held_count() as u32, book.released_count() as u32)
		};
		let mode = self.shared.config.mode;
		let effective_mode = if mode == GuardMode::Off {
			"off"
		} else if downgraded || mode == GuardMode::Shadow {
			"shadow"
		} else {
			"hold"
		};
		GuardStatus {
			mode: mode.as_str().into(),
			effective_mode: effective_mode.into(),
			ready: state.readiness == Readiness::Ready,
			not_ready_reason: match state.readiness {
				Readiness::Ready => None,
				Readiness::NotReady(r) | Readiness::Unsupported(r) => Some(r),
			},
			node_spec_version: hydradx_runtime::VERSION.spec_version,
			chain_spec_version: state.chain_spec_version,
			counters: stats.counters.clone(),
			stage_ms: stats.stage_ms(),
			stops: stats.stops,
			budget_exhausted: stats.budget_exhausted,
			events_resync: stats.events_resync,
			cache_refresh_failures: stats.cache_refresh_failures,
			held,
			released,
		}
	}

	pub fn held(&self) -> Vec<HeldTx> {
		let book = lock(&self.shared.holds);
		let mut out: Vec<HeldTx> = book
			.held()
			.map(|(hash, h)| HeldTx {
				hash: *hash,
				signer: h.signer.map(|s| shadow::hex(&s)),
				first_seen_block: h.first_seen_block,
				first_seen_unix_ms: h.first_seen_unix_ms,
				net_usd: h.net_usd,
				pool_leak_usd: h.pool_leak_usd,
				assets: h.assets.clone(),
				pools: h.pools.clone(),
				note: HELD_NOTE,
			})
			.collect();
		out.sort_by_key(|h| (h.first_seen_block, h.hash));
		out
	}

	pub fn release(
		&self,
		hash: H256,
		ttl_blocks: Option<u32>,
		reason: String,
	) -> Result<ReleaseResult, ErrorObjectOwned> {
		let (_, best) = self.pool.best();
		let ttl = ttl_blocks.unwrap_or(DEFAULT_RELEASE_TTL);
		let until = lock(&self.shared.holds)
			.release(hash, best, ttl)
			.map_err(|_| unknown(hash))?;
		self.shared.audit(shadow::audit(
			"release",
			&hash,
			best,
			reason,
			serde_json::json!({ "ttlBlocks": ttl, "untilBlock": until }),
		));
		self.shared.update_gauges();
		Ok(ReleaseResult {
			hash,
			until_block: until,
		})
	}

	pub fn drop_tx(&self, hash: H256, reason: String) -> Result<DropResult, ErrorObjectOwned> {
		if !lock(&self.shared.holds).forget(&hash) {
			return Err(unknown(hash));
		}
		let (best_hash, best) = self.pool.best();
		let removed = self.pool.remove(best_hash, hash);
		self.shared.audit(shadow::audit(
			"drop",
			&hash,
			best,
			reason,
			serde_json::json!({ "removed": removed, "banSeconds": self.shared.config.ban_seconds }),
		));
		self.shared.update_gauges();
		Ok(DropResult {
			removed,
			ban_seconds: self.shared.config.ban_seconds,
		})
	}
}

fn unknown(hash: H256) -> ErrorObjectOwned {
	ErrorObjectOwned::owned(
		jsonrpsee::types::error::INVALID_PARAMS_CODE,
		format!("{hash:?} is not held or released by this node"),
		None::<()>,
	)
}

#[async_trait]
impl CerebroGuardRpcServer for CerebroGuardRpcImpl {
	async fn status(&self, ext: &Extensions) -> RpcResult<GuardStatus> {
		check_if_safe(ext)?;
		Ok(CerebroGuardRpcImpl::status(self))
	}

	async fn held(&self, ext: &Extensions) -> RpcResult<Vec<HeldTx>> {
		check_if_safe(ext)?;
		Ok(CerebroGuardRpcImpl::held(self))
	}

	async fn release(
		&self,
		ext: &Extensions,
		hash: H256,
		ttl_blocks: Option<u32>,
		reason: String,
	) -> RpcResult<ReleaseResult> {
		check_if_safe(ext)?;
		CerebroGuardRpcImpl::release(self, hash, ttl_blocks, reason)
	}

	async fn drop_tx(&self, ext: &Extensions, hash: H256, reason: String) -> RpcResult<DropResult> {
		check_if_safe(ext)?;
		CerebroGuardRpcImpl::drop_tx(self, hash, reason)
	}
}

#[cfg(test)]
mod tests {
	use super::super::hold::Held;
	use super::super::{GuardConfig, Shared};
	use super::*;
	use cerebro_core::value::Usd6;
	use cerebro_hydration::guard::Thresholds;
	use std::sync::Mutex;

	struct FakePool {
		removed: Mutex<Vec<H256>>,
	}

	impl PoolControl for FakePool {
		fn best(&self) -> (H256, u32) {
			(H256::repeat_byte(0xbb), 200)
		}
		fn remove(&self, _at: H256, hash: H256) -> Vec<H256> {
			self.removed.lock().unwrap().push(hash);
			vec![hash]
		}
	}

	fn rpc(mode: GuardMode) -> (CerebroGuardRpcImpl, Arc<Shared>) {
		let shared = Arc::new(Shared::new(
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
				ban_seconds: 5,
			},
			None,
			None,
		));
		let pool = Arc::new(FakePool {
			removed: Mutex::new(vec![]),
		});
		(CerebroGuardRpcImpl::new(shared.clone(), pool), shared)
	}

	fn held(block: u32) -> Held {
		Held {
			signer: Some([4; 32]),
			first_seen_block: block,
			first_seen_unix_ms: 7,
			net_usd: 61.0,
			pool_leak_usd: 0.0,
			assets: vec![5],
			pools: vec![],
			summary: "signer net $61.00".into(),
		}
	}

	#[test]
	fn release_should_use_default_ttl_when_none_is_given() {
		let (rpc, shared) = rpc(GuardMode::Hold);
		let hash = H256::repeat_byte(1);
		lock(&shared.holds).hold(hash, held(190));

		let res = rpc.release(hash, None, "reviewed".into()).unwrap();

		assert_eq!(res, ReleaseResult { hash, until_block: 210 });
		assert!(lock(&shared.holds).is_released(&hash, 210));
	}

	#[test]
	fn release_should_fail_when_hash_is_not_held() {
		let (rpc, _) = rpc(GuardMode::Hold);

		let err = rpc.release(H256::repeat_byte(9), Some(5), "x".into()).unwrap_err();

		assert_eq!(err.code(), jsonrpsee::types::error::INVALID_PARAMS_CODE);
	}

	#[test]
	fn drop_should_return_removed_txs_and_ban_duration_when_tx_is_held() {
		let (rpc, shared) = rpc(GuardMode::Hold);
		let hash = H256::repeat_byte(1);
		lock(&shared.holds).hold(hash, held(190));

		let res = rpc.drop_tx(hash, "spam".into()).unwrap();

		assert_eq!(
			res,
			DropResult {
				removed: vec![hash],
				ban_seconds: 5
			}
		);
		assert!(!lock(&shared.holds).is_held(&hash));
	}

	#[test]
	fn drop_should_fail_when_hash_is_unknown() {
		let (rpc, _) = rpc(GuardMode::Hold);

		assert!(rpc.drop_tx(H256::repeat_byte(3), "x".into()).is_err());
	}

	#[test]
	fn held_should_state_local_exclusion_when_listing() {
		let (rpc, shared) = rpc(GuardMode::Hold);
		lock(&shared.holds).hold(H256::repeat_byte(2), held(191));
		lock(&shared.holds).hold(H256::repeat_byte(1), held(190));

		let listed = rpc.held();

		assert_eq!(
			listed.iter().map(|h| h.first_seen_block).collect::<Vec<_>>(),
			vec![190, 191]
		);
		assert!(listed
			.iter()
			.all(|h| h.note == "excluded from this node's proposals only"));
	}

	#[test]
	fn status_should_report_downgrade_when_hold_runs_unsupported() {
		let (rpc, shared) = rpc(GuardMode::Hold);
		*lock(&shared.state) = super::super::RuntimeState {
			readiness: Readiness::Unsupported("runtime version: unavailable".into()),
			chain_spec_version: 452,
			effective: super::super::Effective::Unsupported,
		};

		let status = rpc.status();

		assert_eq!(status.mode, "hold");
		assert_eq!(status.effective_mode, "shadow");
		assert!(!status.ready);
		assert_eq!(status.not_ready_reason.as_deref(), Some("runtime version: unavailable"));
		assert_eq!(status.chain_spec_version, 452);
	}
}
