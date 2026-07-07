use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use fp_rpc::EthereumRuntimeRPCApi;
use fp_self_contained::SelfContainedCall;
use frame_system::EventRecord;
use futures::StreamExt;
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::erc20_mapping::Erc20MappingApi;
use hydradx_runtime::RuntimeCall;
use hydradx_runtime::RuntimeEvent;
use hyper::{StatusCode, Uri};
use hyperv14 as hyper;
use pallet_currencies_rpc_runtime_api::CurrenciesApi;
use pallet_ethereum::Transaction;
use pepl_worker_support::traits::RuntimeApiProvider;
use pepl_worker_support::traits::RuntimeClient;
use pepl_worker_support::types::AssetId;
use pepl_worker_support::types::Balance;
use pepl_worker_support::types::BlockNumber;
use pepl_worker_support::types::Borrower;
use pepl_worker_support::types::MoneyMarket;
use pepl_worker_support::Hydration;
use primitives::AccountId;
use primitives::EvmAddress;
use sc_client_api::BlockchainEvents;
use sc_client_api::HeaderBackend;
use sc_client_api::StorageKey;
use sc_transaction_pool_api::InPoolTransaction;
use sc_transaction_pool_api::TransactionPool;
use sc_transaction_pool_api::TransactionSource;
use serde::Deserialize;
use sp_api::ProvideRuntimeApi;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::traits::Block;
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::traits::Zero;
use sp_runtime::BoundedVec;
use sp_runtime::OpaqueExtrinsic;
use sp_runtime::SaturatedConversion;
use std::collections::{HashMap, HashSet};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "liquidation-worker";

// Target health factor after liquidation
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

const ONE_HF: u128 = 1_000_000_000_000_000_000; //1.0(10^18)

// 1.0 in base currency(8 dec.)
const ONE_BASE: u128 = 100_000_000;

// URL of serve to fetch borrowers list
const OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

// Number of liquidation transactions submitted per 1 block
const LIQUIDATIONS_PER_BLOCK: u8 = 20;

// Default worker log prefix (overridable once a CLI flag exists).
const DEFAULT_LOG_PREFIX: &str = "pepl-worker";

// Borrowers holding less than this collateral (in base currency, 8 dec.) are skipped as dust.
const MIN_COLLATERAL_BASE: u128 = ONE_BASE;

// Omniwatch fetch: bounded retries + backoff + per-attempt timeout (an omniwatch that accepts
// connections but never responds must not hang the worker), and how often `run()` re-fetches the
// borrower list (to pick up new borrowers and to recover a seed that failed while omniwatch was
// down).
pub const OMNIWATCH_FETCH_ATTEMPTS: u32 = 5;
pub const OMNIWATCH_FETCH_BACKOFF: Duration = Duration::from_secs(3);
pub const OMNIWATCH_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
pub const OMNIWATCH_REFETCH_EVERY_N_BLOCKS: u32 = 100;
// Until the first successful omniwatch fetch, re-seed more often: event discovery only sees new
// borrows, so pre-existing borrowers stay invisible until a seed succeeds.
pub const OMNIWATCH_REFETCH_UNSEEDED_EVERY_N_BLOCKS: u32 = 10;

// Cap on blocks queued for BORROW-event scanning while the money market can't be fetched.
pub const MAX_PENDING_EVENT_BLOCKS: usize = 256;

// Cap on watched money-market instances — a backstop against a hostile/buggy omniwatch
// serving many bogus pools, set far above any realistic market count.
pub const MAX_MM_INSTANCES: usize = 16;

// Contracts' addresses
pub mod contracts {
	use super::*;
	use sp_core::H160;

	pub const _POOL_CONFIGURATOR: EvmAddress = H160(hex!("e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4"));

	// Address of the pool address provider contract.
	pub const POOL_ADDRESS_PROVIDER: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));

	// Account that calls the runtime API. Needs to have enough of WETH to pay for the runtime API call.
	pub const RUNTIME_API_CALLER: EvmAddress = H160(hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e"));

	// Account that signs the DIA oracle update transactions.
	pub const ORACLE_SIGNER: &[EvmAddress] = &[
		H160(hex!("33a5e905fB83FcFB62B0Dd1595DfBc06792E054e")),
		H160(hex!("ff0c624016c873d359dde711b42a2f475a5a07d3")),
	];

	// Address of the DIA oracle contract.
	pub const ORACLE_UPDATE_CALL: &[EvmAddress] = &[
		H160(hex!("dee629af973ebf5bf261ace12ffd1900ac715f5e")),
		H160(hex!("48ae7803cd09c48434e3fc5629f15fb76f0b5ce5")),
	];
}

mod events {
	use super::*;

	pub const BORROW: H256 = H256(hex!("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0"));
}

mod omniwatch {
	use super::*;
	use primitives::AccountId;

	// Fields mirror the omniwatch JSON schema; only the borrower addresses are consumed.
	#[allow(dead_code)]
	#[derive(Clone, Deserialize, Debug)]
	#[serde(rename_all = "camelCase")]
	pub struct BorrowerData {
		pub total_collateral_base: f32,
		pub total_debt_base: f32,
		pub available_borrows_base: f32,
		pub current_liquidation_threshold: f32,
		pub ltv: f32,
		pub health_factor: f32,
		pub updated: u64,
		pub account: AccountId,
		pub pool: EvmAddress,
	}

	#[allow(dead_code)]
	#[derive(Clone, Deserialize, Debug)]
	#[serde(rename_all = "camelCase")]
	pub struct ByHealthRes {
		pub last_global_update: u32,
		pub last_update: u32,
		pub borrowers: Vec<(EvmAddress, BorrowerData)>,
	}
}

mod https {
	use hyper::{body::Body, client::HttpConnector, Client as HyperClient};
	use hyper_rustls::HttpsConnector;
	use hyperv14 as hyper;

	pub type Client = HyperClient<HttpsConnector<HttpConnector>, Body>;

	pub fn new() -> Client {
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_webpki_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();

		HyperClient::builder().build(https)
	}
}

pub mod storage_key {
	use super::*;

	pub const SYSTEM_EVENTS: [u8; 32] = hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7");

	/// `pallet_liquidation::BorrowingContract` — the main money-market pool. ValueQuery:
	/// an absent value means the chain uses the pallet's default (the main pool).
	pub fn borrowing_contract() -> [u8; 32] {
		frame_support::storage::storage_prefix(b"Liquidation", b"BorrowingContract")
	}

	/// `pallet_gigahdx::GigaHdxPoolContract` — the gigahdx money-market pool (OptionQuery).
	pub fn gigahdx_pool_contract() -> [u8; 32] {
		frame_support::storage::storage_prefix(b"GigaHdx", b"GigaHdxPoolContract")
	}
}

/// `(borrower, pool)` pairs as served by omniwatch's by-health endpoint.
pub type PoolTaggedBorrowers = Vec<(EvmAddress, EvmAddress)>;

/// Persists the working borrower set (grouped by pool) to disk so a restart while omniwatch is
/// unreachable keeps last-known coverage instead of starting empty. The file is only ever read
/// as a fallback (never sets `seeded`, so a live fetch stays authoritative and the fast re-seed
/// cadence keeps running); staleness is harmless — a repaid borrower is pruned on its first scan.
/// All I/O errors are logged and swallowed.
pub mod borrower_cache {
	use super::*;
	use std::collections::BTreeMap;
	use std::path::Path;

	#[derive(serde::Serialize, serde::Deserialize, Default)]
	struct CacheFile {
		/// pool address (0x-hex) → borrower addresses (0x-hex).
		pools: BTreeMap<String, Vec<String>>,
	}

	fn addr_hex(a: &EvmAddress) -> String {
		format!("0x{}", hex::encode(a.as_bytes()))
	}

	fn parse_addr(s: &str) -> Option<EvmAddress> {
		let b = hex::decode(s.trim().trim_start_matches("0x")).ok()?;
		(b.len() == 20).then(|| EvmAddress::from_slice(&b))
	}

	/// Load the cache as `(borrower, pool)` pairs (empty on any error / missing file).
	pub fn load(path: &Path, log_prefix: &str) -> PoolTaggedBorrowers {
		let raw = match std::fs::read_to_string(path) {
			Ok(s) => s,
			Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
			Err(e) => {
				log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: read {path:?} failed: {e}");
				return Vec::new();
			}
		};
		let parsed: CacheFile = match serde_json::from_str(&raw) {
			Ok(c) => c,
			Err(e) => {
				log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: {path:?} is corrupt, ignoring: {e}");
				return Vec::new();
			}
		};
		let mut out = Vec::new();
		for (pool_s, borrowers) in parsed.pools {
			let Some(pool) = parse_addr(&pool_s) else { continue };
			for b in borrowers {
				if let Some(addr) = parse_addr(&b) {
					out.push((addr, pool));
				}
			}
		}
		log::info!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: loaded {} borrowers from {path:?}", out.len());
		out
	}

	/// Atomically persist the per-pool working set (tmp file + rename), REPLACING the file. Only
	/// instances whose pool is resolved are written; borrowers of an unresolved instance are
	/// re-seeded anyway. No-ops on an empty total set, so a reachable-but-empty omniwatch
	/// response can never wipe a good file.
	pub fn save(path: &Path, instances: &[MmInstance], log_prefix: &str) {
		let mut file = CacheFile::default();
		for inst in instances {
			let Some(pool) = inst.pool else { continue };
			if inst.borrowers.is_empty() {
				continue;
			}
			let mut list: Vec<String> = inst.borrowers.iter().map(addr_hex).collect();
			list.sort(); // stable on-disk order for clean diffs
			file.pools.insert(addr_hex(&pool), list);
		}
		if file.pools.is_empty() {
			return; // never overwrite a good file with an empty set
		}
		let json = match serde_json::to_string_pretty(&file) {
			Ok(j) => j,
			Err(e) => {
				log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: serialize failed: {e}");
				return;
			}
		};
		if let Some(dir) = path.parent() {
			if let Err(e) = std::fs::create_dir_all(dir) {
				log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: mkdir {dir:?} failed: {e}");
				return;
			}
		}
		let tmp = path.with_extension("json.tmp");
		if let Err(e) = std::fs::write(&tmp, json.as_bytes()) {
			log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: write {tmp:?} failed: {e}");
			return;
		}
		if let Err(e) = std::fs::rename(&tmp, path) {
			log::warn!(target: LOG_TARGET, "{log_prefix:?} borrower_cache: rename into {path:?} failed: {e}");
		}
	}
}

/// Which liquidation path a market's decisions must take on-chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceKind {
	/// Regular Aave market — `liquidate_with_pool` runs the generic path against the
	/// borrowing contract.
	Generic,
	/// The gigahdx market — the pallet routes on the GIGAHDX aToken asset id, so decisions
	/// must carry the aToken (not the underlying) as collateral.
	GigaHdx,
}

/// How a money-market instance entered the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstanceSource {
	/// Bootstrapped from on-chain storage (borrowing contract / gigahdx pool contract).
	Chain,
	/// Pinned via `--pool-address-provider`.
	Config,
	/// Discovered from omniwatch's per-borrower `pool` field.
	Discovered,
}

/// One watched money market. The registry only grows (keep-until-restart): an instance, once
/// seen — from chain state, config, or omniwatch — is never dropped on external omission;
/// only the operator denylist prevents creation.
pub struct MmInstance {
	pub pap: EvmAddress,
	/// Resolved pool contract — known at creation for pool-discovered instances, after the
	/// first money-market fetch for PAP-pinned ones.
	pub pool: Option<EvmAddress>,
	/// Re-detected every block against `GigaHdxPoolContract` storage.
	pub kind: InstanceKind,
	pub source: InstanceSource,
	pub borrowers: HashSet<EvmAddress>,
	/// Money market + borrowers from this instance's latest scan — the oracle fast-path cache.
	pub cached_mm: Option<MoneyMarket>,
	pub cached_borrowers: Vec<Borrower>,
	/// underlying asset id → aToken asset id, refreshed per block; populated for GigaHdx only.
	pub atoken_map: HashMap<AssetId, AssetId>,
	pub log_prefix: String,
}

impl MmInstance {
	pub fn new(base_log_prefix: &str, pap: EvmAddress, pool: Option<EvmAddress>, source: InstanceSource) -> Self {
		let tag = pool.unwrap_or(pap);
		let b = tag.as_bytes();
		let log_prefix = format!("{base_log_prefix}/mm-{:02x}{:02x}{:02x}{:02x}", b[0], b[1], b[2], b[3]);
		Self {
			pap,
			pool,
			kind: InstanceKind::Generic,
			source,
			borrowers: HashSet::new(),
			cached_mm: None,
			cached_borrowers: Vec::new(),
			atoken_map: HashMap::new(),
			log_prefix,
		}
	}
}

pub fn find_instance_by_pool(instances: &[MmInstance], pool: EvmAddress) -> Option<usize> {
	instances.iter().position(|i| i.pool == Some(pool))
}

pub fn find_instance_by_pap(instances: &[MmInstance], pap: EvmAddress) -> Option<usize> {
	instances.iter().position(|i| i.pap == pap)
}

/// Ensures a registry instance exists for `pool`, creating one if needed. Returns the
/// instance index, or `None` when the pool is denylisted, the registry is full, or the
/// PAP resolution / sanity round-trip fails (self-healing: the pool re-arrives with the
/// next omniwatch re-seed or bootstrap read).
pub fn ensure_instance_for_pool<B: Block, RA: RuntimeApiProvider<B>>(
	instances: &mut Vec<MmInstance>,
	api: &RA,
	block: B::Hash,
	pool: EvmAddress,
	source: InstanceSource,
	cfg: &LiquidationTaskConfig,
) -> Option<usize> {
	if cfg.pool_denylist.contains(&pool) {
		log::debug!(target: LOG_TARGET, "{:?} ensure_instance_for_pool(): pool {pool:?} is denylisted, skipping", cfg.log_prefix);
		return None;
	}
	if let Some(idx) = find_instance_by_pool(instances, pool) {
		return Some(idx);
	}

	// Unknown pool: resolve its PAP on-chain.
	let pap = match pepl_worker_support::fetch_addresses_provider(api, block, cfg.api_caller, pool) {
		Ok(pap) => pap,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} ensure_instance_for_pool(): failed to resolve ADDRESSES_PROVIDER for pool {pool:?}: {e:?} — not instancing", cfg.log_prefix);
			return None;
		}
	};

	// Sanity round-trip BEFORE adopt or create: the resolved PAP must point back at the
	// pool. Guards against a bogus contract answering ADDRESSES_PROVIDER() with garbage —
	// including one naming a pinned PAP to hijack that instance's pool slot.
	let probe = Hydration::new(cfg.api_caller, pap, cfg.log_prefix.as_str());
	match probe.fetch_pool(api, block) {
		Ok(resolved) if resolved == pool => {}
		other => {
			log::error!(target: LOG_TARGET, "{:?} ensure_instance_for_pool(): sanity round-trip failed for pool {pool:?} (PAP {pap:?} resolves to {other:?}) — not instancing", cfg.log_prefix);
			return None;
		}
	}

	// A PAP-pinned instance that has not resolved its pool yet is adopted instead of duplicated.
	if let Some(idx) = find_instance_by_pap(instances, pap) {
		match instances[idx].pool {
			None => {
				instances[idx].pool = Some(pool);
				return Some(idx);
			}
			Some(existing) if existing == pool => return Some(idx),
			Some(existing) => {
				log::error!(target: LOG_TARGET, "{:?} ensure_instance_for_pool(): PAP {pap:?} already bound to pool {existing:?}, refusing pool {pool:?}", cfg.log_prefix);
				return None;
			}
		}
	}

	if instances.len() >= MAX_MM_INSTANCES {
		log::warn!(target: LOG_TARGET, "{:?} ensure_instance_for_pool(): registry full ({MAX_MM_INSTANCES}), not instancing pool {pool:?}", cfg.log_prefix);
		return None;
	}

	let instance = MmInstance::new(cfg.log_prefix.as_str(), pap, Some(pool), source);
	log::info!(target: LOG_TARGET, "{:?} run(): new money-market instance {} (pool {pool:?}, PAP {pap:?}, source {source:?})", cfg.log_prefix, instance.log_prefix, );
	instances.push(instance);
	Some(instances.len() - 1)
}

/// Rewrites a decision for on-chain submission according to the market's kind.
///
/// `Generic` markets pass through untouched. For the `GigaHdx` market the pallet routes on
/// `collateral_asset == gigahdx aToken`, while `decide_liquidation` produces the underlying
/// (stHDX) — the aToken asset id is substituted from `atoken_map` (underlying → aToken,
/// built from the market's reserve data). A missing mapping returns `None`: submitting the
/// underlying would route to the generic path and fail the on-chain pool check, so we fail
/// closed here with a log instead.
pub fn map_decision_collateral(
	decision: &LiquidationDecision,
	kind: InstanceKind,
	atoken_map: &HashMap<AssetId, AssetId>,
	log_prefix: &str,
) -> Option<LiquidationDecision> {
	match kind {
		InstanceKind::Generic => Some(decision.clone()),
		InstanceKind::GigaHdx => match atoken_map.get(&decision.collateral_asset) {
			Some(atoken) => {
				let mut mapped = decision.clone();
				mapped.collateral_asset = *atoken;
				Some(mapped)
			}
			None => {
				log::error!(target: LOG_TARGET, "{log_prefix:?} map_decision_collateral(): no aToken mapping for collateral {:?} on the gigahdx market — skipping submission (fail-closed)", decision.collateral_asset);
				None
			}
		},
	}
}

/// The configuration for the liquidation worker cli params.
#[derive(Clone, Debug, clap::Parser)]
pub struct LiquidationWorkerCli {
	/// Enable/disable liquidation worker.
	#[clap(long)]
	pub liquidation_worker: Option<bool>,

	/// Address of a Pool Address Provider contract to watch. Repeatable — one money-market
	/// instance per address. Optional: instances are auto-discovered from chain state and
	/// omniwatch; this flag only pins additional markets.
	#[clap(long)]
	pub pool_address_provider: Vec<EvmAddress>,

	/// Money-market pool addresses that must never be instantiated, even if discovered.
	#[clap(long)]
	pub mm_pool_denylist: Vec<EvmAddress>,

	/// Enable/disable money-market instance discovery from omniwatch (default: enabled).
	/// On-chain markets (borrowing contract, gigahdx pool) are always watched.
	#[clap(long)]
	pub mm_discovery: Option<bool>,

	/// Path to persist the borrower set across restarts. Defaults to
	/// `<node-base-path>/pepl/borrowers.json`. Set to an empty string to disable persistence.
	#[clap(long)]
	pub borrower_cache_path: Option<String>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub runtime_api_caller: Option<EvmAddress>,

	/// EVM address of the account that signs DIA oracle update.
	#[clap(long)]
	pub oracle_signer: Option<Vec<EvmAddress>>,

	/// EVM address of the DIA oracle update call.
	#[clap(long)]
	pub oracle_update_call_address: Option<Vec<EvmAddress>>,

	/// Target health factor.
	#[clap(long)]
	pub target_hf: Option<u128>,

	/// URL to fetch list of borrowers.
	#[clap(long)]
	pub omniwatch_url: Option<String>,

	/// Number of liquidation transaction submitted per block.
	#[clap(long)]
	pub liquidations_per_block: Option<u8>,

	/// Run the legacy (v1) liquidation worker instead of the default v2 worker.
	#[clap(long)]
	pub liquidation_worker_v1: bool,
}

impl LiquidationWorkerCli {
	/// Resolve the borrower-cache path, given the node base path:
	/// - explicit empty `--borrower-cache-path ""` → `None` (persistence off),
	/// - explicit non-empty value → that path,
	/// - unset → `<base>/pepl/borrowers.json`.
	pub fn resolve_borrower_cache_path(&self, base: &std::path::Path) -> Option<std::path::PathBuf> {
		match &self.borrower_cache_path {
			Some(s) if s.trim().is_empty() => None,
			Some(s) => Some(std::path::PathBuf::from(s)),
			None => Some(base.join("pepl").join("borrowers.json")),
		}
	}
}

/// The configuration for `LiquidationTask`.
pub struct LiquidationTaskConfig {
	/// Pinned Pool Address Providers — one money-market instance each, on top of the
	/// instances bootstrapped from chain state and discovered from omniwatch.
	pub pool_address_providers: Vec<EvmAddress>,

	/// Pool addresses that must never become instances.
	pub pool_denylist: Vec<EvmAddress>,

	/// Whether omniwatch-driven instance discovery is active.
	pub discovery_enabled: bool,

	/// Where to persist the borrower set. `None` disables persistence. The node fills this with
	/// `<base-path>/pepl/borrowers.json` when the operator does not override it.
	pub borrower_cache_path: Option<std::path::PathBuf>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	pub api_caller: EvmAddress,

	/// EVM address of the account that signs DIA oracle update.
	pub oracle_signer: Vec<EvmAddress>,

	/// EVM address of the DIA oracle update call.
	pub oracle_update_call: Vec<EvmAddress>,

	/// Target health factor.
	pub target_hf: u128,

	/// URL to fetch list of borrowers.
	pub omniwatch_url: String,

	/// Number of liquidation transaction submitted per block.
	pub liquidations_per_block: u8,

	/// Min. borrower's collateral in [BASE] to calculate liquidation.
	/// Borrowers holding `< min_collateral` are skipped
	pub min_collateral: U256,

	pub log_prefix: String,
}

impl From<LiquidationWorkerCli> for LiquidationTaskConfig {
	fn from(v: LiquidationWorkerCli) -> Self {
		Self {
			pool_address_providers: v.pool_address_provider,
			pool_denylist: v.mm_pool_denylist,
			discovery_enabled: v.mm_discovery.unwrap_or(true),
			// Resolved by the node from `--borrower-cache-path` + the node base path (the
			// From impl has no base path); `None` here = persistence off (the safe default
			// for tests and any non-node caller).
			borrower_cache_path: None,
			api_caller: v.runtime_api_caller.unwrap_or(contracts::RUNTIME_API_CALLER),
			oracle_signer: v.oracle_signer.unwrap_or(contracts::ORACLE_SIGNER.to_vec()),
			oracle_update_call: v
				.oracle_update_call_address
				.unwrap_or(contracts::ORACLE_UPDATE_CALL.to_vec()),
			target_hf: v.target_hf.unwrap_or(TARGET_HF),
			omniwatch_url: v.omniwatch_url.unwrap_or(OMNIWATCH_URL.to_string()),
			liquidations_per_block: v.liquidations_per_block.unwrap_or(LIQUIDATIONS_PER_BLOCK),

			//TODO: make these configurable
			min_collateral: U256::from(MIN_COLLATERAL_BASE),
			log_prefix: DEFAULT_LOG_PREFIX.to_string(),
		}
	}
}

impl Default for LiquidationTaskConfig {
	fn default() -> Self {
		Self {
			pool_address_providers: Vec::new(),
			pool_denylist: Vec::new(),
			discovery_enabled: true,
			borrower_cache_path: None,
			api_caller: contracts::RUNTIME_API_CALLER,
			oracle_signer: contracts::ORACLE_SIGNER.to_vec(),
			oracle_update_call: contracts::ORACLE_UPDATE_CALL.to_vec(),
			target_hf: TARGET_HF,
			omniwatch_url: OMNIWATCH_URL.to_string(),
			liquidations_per_block: LIQUIDATIONS_PER_BLOCK,
			min_collateral: U256::from(MIN_COLLATERAL_BASE),
			log_prefix: DEFAULT_LOG_PREFIX.to_string(),
		}
	}
}

/// Parameters of the `liquidate` extrinsic the worker has decided to submit for a borrower.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LiquidationDecision {
	pub collateral_asset: AssetId,
	pub debt_asset: AssetId,
	pub user: EvmAddress,
	pub debt_to_cover: Balance,
	pub priority: u64,
}

/// Pure decision step: given an up-to-date money market and borrower, decide whether — and how —
/// to liquidate. Returns `None` when the borrower should be skipped (dust, healthy, no liquidation
/// option, or a reserve lookup miss). Does no I/O and touches no transaction pool, so it is
/// unit-testable without a node.
pub fn decide_liquidation(
	cfg: &LiquidationTaskConfig,
	money_market: &MoneyMarket,
	borrower: &Borrower,
) -> Option<LiquidationDecision> {
	let log_prefix = cfg.log_prefix.as_str();

	if borrower.total_collateral < cfg.min_collateral {
		log::trace!(target: LOG_TARGET, "{:?} decide_liquidation(): collateral below min, skipping, borrower: {:?}, collateral: {:?}", log_prefix, borrower.address, borrower.total_collateral);
		return None;
	}

	let hf = borrower
		.calc_health_factor(money_market)
		.inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?} decide_liquidation(): failed to calc health factor, skipping, borrower: {:?}, err: {:?}", log_prefix, borrower.address, e);
		})
		.ok()?;

	if hf.is_zero() {
		log::debug!(target: LOG_TARGET, "{:?} decide_liquidation(): borrower with 0 health factor, borrower: {:?}", log_prefix, borrower.address);
	}

	if hf >= U256::from(ONE_HF) {
		log::trace!(target: LOG_TARGET, "{:?} decide_liquidation(): healthy borrower, borrower: {:?}", log_prefix, borrower.address);
		return None;
	}

	let target_hf = cfg.target_hf.into();
	let liq_option = match money_market.calc_best_liquidation_option_for(borrower, target_hf, log_prefix) {
		Ok(Some(opt)) => opt,
		Ok(None) => {
			log::info!(target: LOG_TARGET, "{:?} decide_liquidation(): no liquidation option, borrower: {:?}, health_factor: {:?}", log_prefix, borrower.address, hf);
			return None;
		}
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} decide_liquidation(): failed to calc liquidation option, borrower: {:?}, health_factor: {:?}, err: {:?}", log_prefix, borrower.address, hf, e);
			return None;
		}
	};

	let priority: u64 = borrower
		.total_collateral
		.checked_div(ONE_BASE.into())
		.unwrap_or(Zero::zero())
		.saturated_into();

	let Some(coll) = money_market.reserves.get(&liq_option.collateral_asset) else {
		log::error!(target: LOG_TARGET, "{:?} decide_liquidation(): collateral reserve not found. THIS SHOULD NEVER HAPPEN, please report to project maintainers, reserve: {:?}", log_prefix, liq_option.collateral_asset);
		return None;
	};
	let Some(debt) = money_market.reserves.get(&liq_option.debt_asset) else {
		log::error!(target: LOG_TARGET, "{:?} decide_liquidation(): debt reserve not found. THIS SHOULD NEVER HAPPEN, please report to project maintainers, reserve: {:?}", log_prefix, liq_option.debt_asset);
		return None;
	};

	Some(LiquidationDecision {
		collateral_asset: coll.asset_id,
		debt_asset: debt.asset_id,
		user: borrower.address,
		debt_to_cover: liq_option.debt_to_liquidate.saturated_into(),
		priority,
	})
}

/// Build the opaque `liquidate_with_pool` extrinsic (sync — no tx pool needed). Shared by
/// `submit_liquidation` and the parallel submit-on-find scan (which spawns the async `submit_one`
/// onto a tokio handle from a worker thread). The unsigned tx carries `unsigned_priority =
/// collateral-at-risk`, so the tx pool orders competing liquidations for the block builder.
/// `pool` is the market the decision was made against; the pallet rejects the call if it is not
/// the pool the liquidation would execute on.
pub fn encode_liquidation_opaque(
	decision: &LiquidationDecision,
	pool: EvmAddress,
	log_prefix: &str,
) -> Option<OpaqueExtrinsic> {
	let tx = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_with_pool {
		pool,
		collateral_asset: decision.collateral_asset,
		debt_asset: decision.debt_asset,
		user: decision.user,
		debt_to_cover: decision.debt_to_cover,
		route: BoundedVec::new(),
		unsigned_priority: Some(decision.priority),
	});

	let encoded_tx: fp_self_contained::UncheckedExtrinsic<
		hydradx_runtime::Address,
		RuntimeCall,
		hydradx_runtime::Signature,
		hydradx_runtime::SignedExtra,
	> = fp_self_contained::UncheckedExtrinsic::new_bare(tx.clone());
	let encoded = encoded_tx.encode();

	OpaqueExtrinsic::decode(&mut &encoded[..])
		.map_err(|e| {
			log::error!(target: LOG_TARGET, "{log_prefix:?} encode_liquidation_opaque(): failed to decode tx. THIS SHOULD NEVER HAPPEN, please report to project maintainers: err: {e:?}, tx: {tx:?}");
		})
		.ok()
}

pub struct LiquidationTask<C, B, TP> {
	client: C,
	pub https: https::Client,
	pub url: Uri,
	pub transaction_pool: Arc<TP>,
	system_events_key: StorageKey,
	_phantom: PhantomData<B>,
	cfg: LiquidationTaskConfig,
}

impl<C, B, TP> LiquidationTask<C, B, TP>
where
	C: RuntimeClient<B>,
	B: Block,
	TP: TransactionPool<Block = B> + 'static,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
{
	pub fn new(client: C, transaction_pool: Arc<TP>, cfg: LiquidationTaskConfig) -> Self {
		// Fail fast: a mistyped --omniwatch-url panics at startup so the operator notices
		// immediately, rather than the worker running silently against a dead endpoint.
		let url = cfg
			.omniwatch_url
			.parse()
			.expect("LiquidationTask: failed to parse omniwatch_url, provide correct --omniwatch-url or disable liquidation worker");

		Self {
			client,
			https: https::new(),
			transaction_pool,
			url,
			system_events_key: StorageKey(storage_key::SYSTEM_EVENTS.to_vec()),
			_phantom: PhantomData,
			cfg,
		}
	}

	async fn submit_liquidation(
		&self,
		block: B::Hash,
		pool: EvmAddress,
		decision: &LiquidationDecision,
	) -> Result<(), ()> {
		let log_prefix = self.cfg.log_prefix.as_str();
		let opaque_tx = encode_liquidation_opaque(decision, pool, log_prefix).ok_or(())?;

		match self
			.transaction_pool
			.submit_one(block, TransactionSource::Local, opaque_tx.into())
			.await
		{
			Ok(_) => Ok(()),
			Err(e) => {
				log::error!(target: LOG_TARGET, "{log_prefix:?} LiquidationTask.submit_liquidation(): failed to submit liquidation transaction, err: {e:?}");
				Err(())
			}
		}
	}
}

impl<C, B, TP> LiquidationTask<C, B, TP>
where
	C: RuntimeClient<B>,
	B: Block,
	TP: TransactionPool<Block = B> + 'static,
{
	/// Function returns all events from `system.events` storage at `block`
	pub fn load_events(&self, block: B::Hash) -> Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>> {
		let timer = Instant::now();
		log::trace!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): fetching events from storage", self.cfg.log_prefix);

		let events = match self.client.storage(block, &self.system_events_key) {
			Ok(Some(events)) => events,
			Ok(None) => {
				log::trace!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): finished, storage returned no data, elapsed: {:?}", self.cfg.log_prefix, timer.elapsed().as_nanos());
				return Vec::new();
			}
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): failed to load events from storage. err: {:?}, elapsed: {:?}", self.cfg.log_prefix, e, timer.elapsed().as_nanos());
				return Vec::new();
			}
		};

		let events = match Vec::<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>::decode(&mut events.0.as_slice()) {
			Ok(events) => events,
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): failed to decode storage item, err: {:?}, elapsed: {:?}", self.cfg.log_prefix, e, timer.elapsed().as_nanos());
				Vec::new()
			}
		};

		log::debug!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): finished loading {:?} events, elapsed: {:?}", self.cfg.log_prefix, events.len(), timer.elapsed().as_nanos());
		events
	}
}

/// Function fetches and returns the list of `(borrower, pool)` pairs from provided `url` —
/// each borrower tagged with the money-market pool omniwatch tracks them in.
/// Returned list is not deduped nor sorted in any way.
async fn fetch_borrowers_list(
	https: &https::Client,
	url: Uri,
	log_prefix: &str,
) -> Option<Vec<(EvmAddress, EvmAddress)>> {
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{log_prefix:?} fetch_borrowers(): fetching borrowers list from external source");

	let res = match https.get(url).await {
		Ok(res) if res.status() == StatusCode::OK => res,
		Ok(res) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to fetch borrowers data, elapsed: {:?}, status_code: {:?}", log_prefix, timer.elapsed().as_nanos(), res.status());
			return None;
		}
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to fetch borrowers data, elapsed: {:?}, err: {:?}", log_prefix, timer.elapsed().as_nanos(), e);
			return None;
		}
	};

	let bytes = match hyper::body::to_bytes(res.into_body()).await {
		Ok(bytes) => bytes,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to load response data, elapsed: {:?}, err: {:?}", log_prefix, timer.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match String::from_utf8(bytes.to_vec()) {
		Ok(s) => s,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers():, failed to parse returned data as utf8 string, elapsed: {:?}, err: {:?}", log_prefix, timer.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match serde_json::from_str::<omniwatch::ByHealthRes>(data.as_str()) {
		Ok(d) => d,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers():, failed to deserialize response data, elapsed: {:?}, err: {:?}", log_prefix, timer.elapsed().as_nanos(), e);
			return None;
		}
	};

	let mut b = Vec::<(EvmAddress, EvmAddress)>::with_capacity(data.borrowers.len());
	for (addr, details) in &data.borrowers {
		b.push((*addr, details.pool));
	}

	log::info!(target: LOG_TARGET, "{:?} fetch_borrowers(): finished fetching {:?} borrowers elapsed: {:?}", log_prefix, b.len(), timer.elapsed().as_nanos());
	Some(b)
}

/// Fetches the omniwatch borrower list with bounded retries + backoff. Each attempt is bounded by
/// `timeout` so an omniwatch that accepts connections but never responds cannot hang the worker.
/// Returns `None` (never panics) when every attempt fails, so a transient omniwatch outage never
/// kills the worker — `run()` starts unseeded and recovers via background re-seeds and
/// event-driven discovery.
async fn fetch_borrowers_list_with_retry(
	https: &https::Client,
	url: Uri,
	log_prefix: &str,
	max_attempts: u32,
	backoff: Duration,
	timeout: Duration,
) -> Option<Vec<(EvmAddress, EvmAddress)>> {
	for attempt in 1..=max_attempts {
		match tokio::time::timeout(timeout, fetch_borrowers_list(https, url.clone(), log_prefix)).await {
			Ok(Some(borrowers)) => return Some(borrowers),
			Ok(None) => {}
			Err(_) => {
				log::error!(target: LOG_TARGET, "{log_prefix:?} fetch_borrowers_list_with_retry(): attempt timed out after {timeout:?}");
			}
		}
		log::warn!(target: LOG_TARGET, "{log_prefix:?} fetch_borrowers_list_with_retry(): attempt {attempt}/{max_attempts} failed");
		if attempt < max_attempts {
			tokio::time::sleep(backoff).await;
		}
	}
	log::error!(target: LOG_TARGET, "{log_prefix:?} fetch_borrowers_list_with_retry(): all {max_attempts} attempts failed; starting unseeded — event-driven discovery still adds new borrowers and re-seeding continues in the background");
	None
}

// Function iterates over `events` and returns the borrowers from `pool`'s BORROW logs. The pool
// address is the dynamically resolved one (`MoneyMarket.pool`), not a hardcoded constant, so
// discovery keeps working if the PoolAddressesProvider ever points at a new pool.
pub fn process_events(
	events: Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>,
	pool: EvmAddress,
	log_prefix: &str,
) -> Vec<EvmAddress> {
	process_events_multi(events, &[pool], log_prefix)
		.into_iter()
		.map(|(_, borrower)| borrower)
		.collect()
}

/// Multi-market BORROW-event discovery: one pass over `events`, matching each EVM log against
/// the full set of watched pools. Returns `(pool, borrower)` pairs so the caller can route each
/// discovery to its owning market instance.
pub fn process_events_multi(
	events: Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>,
	pools: &[EvmAddress],
	log_prefix: &str,
) -> Vec<(EvmAddress, EvmAddress)> {
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{:?} process_events(): processing {:?} events against {} pool(s)", log_prefix, events.len(), pools.len());

	let mut borrowers: Vec<(EvmAddress, EvmAddress)> = Vec::with_capacity(20);
	for evt in &events {
		let RuntimeEvent::EVM(pallet_evm::Event::Log { log }) = &evt.event else {
			continue;
		};

		if pools.contains(&log.address) && log.topics.first() == Some(&events::BORROW) {
			let Some(&borrower) = log.topics.get(2) else {
				continue;
			};

			borrowers.push((log.address, borrower.into()));
		}
	}

	log::debug!(target: LOG_TARGET, "{:?} process_events(): finished, elapsed={:?}", log_prefix, timer.elapsed().as_nanos());
	borrowers
}

/// Function checks if the transaction is DIA's oracle update transaction and returns `Transaction`
/// or `None`.
#[allow(dead_code)]
pub(crate) fn is_oracle_update_tx(
	extrinsic: &sp_runtime::generic::UncheckedExtrinsic<
		hydradx_runtime::Address,
		RuntimeCall,
		hydradx_runtime::Signature,
		hydradx_runtime::SignedExtra,
	>,
	allowed_signers: Vec<EvmAddress>,
	allowed_callers: Vec<EvmAddress>,
	log_prefix: &str,
) -> Option<Transaction> {
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{log_prefix:?} is_oracle_update_tx()");

	let RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = extrinsic.function.clone() else {
		log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, non evm transaction, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	let action = match transaction {
		Transaction::Legacy(ref legacy_transaction) => legacy_transaction.action,
		Transaction::EIP2930(ref eip2930_transaction) => eip2930_transaction.action,
		Transaction::EIP1559(ref eip1559_transaction) => eip1559_transaction.action,
		Transaction::EIP7702(_) => {
			log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, unsupported EIP7702 tx, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
			return None;
		}
	};

	let pallet_ethereum::TransactionAction::Call(caller) = action else {
		log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, no caller, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	// check if the transaction is DIA oracle update
	if !allowed_callers.contains(&caller) {
		log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, caller is not allowed, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	}

	// additional check to prevent running the worker for DIA oracle updates signed by invalid address
	let Some(Ok(signer)) = extrinsic.function.check_self_contained() else {
		log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not self contained, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	if !allowed_signers.contains(&signer) {
		log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not allowed, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	}

	log::debug!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
	Some(transaction)
}

/// Parse a DIA oracle-update EVM transaction into `(base_asset_name_lowercase, new_price)` pairs.
/// Handles both `setValue(string,uint128,uint128)` and `setMultipleValues(string[],uint256[])`.
/// The DIA key is `"ASSET/USD"`; we keep the lowercased `ASSET` part. `new_price` is the DIA value
/// (oracle 8-decimal USD, same scale as `Reserve.price`). Ports v1's `parse_oracle_transaction`.
pub(crate) fn parse_oracle_price_updates(transaction: &Transaction) -> Vec<(String, U256)> {
	let input = match transaction {
		Transaction::Legacy(t) => &t.input,
		Transaction::EIP2930(t) => &t.input,
		Transaction::EIP1559(t) => &t.input,
		Transaction::EIP7702(_) => return Vec::new(),
	};
	if input.len() < 4 {
		return Vec::new();
	}

	let selector = &input[0..4];
	// (dia_key, price) before splitting "ASSET/USD".
	let mut raw: Vec<(String, U256)> = Vec::new();

	if selector == Into::<u32>::into(pepl_worker_support::Function::SetValue).to_be_bytes() {
		if let Ok(decoded) = ethabi::decode(
			&[
				ethabi::ParamType::String,
				ethabi::ParamType::Uint(16),
				ethabi::ParamType::Uint(16),
			],
			&input[4..],
		) {
			if let (Some(key), Some(price)) = (
				decoded.first().and_then(|t| t.clone().into_string()),
				decoded.get(1).and_then(|t| t.clone().into_uint()),
			) {
				raw.push((key, price));
			}
		}
	} else if selector == Into::<u32>::into(pepl_worker_support::Function::SetMultipleValues).to_be_bytes() {
		if let Ok(decoded) = ethabi::decode(
			&[
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
			],
			&input[4..],
		) {
			if let (Some(keys), Some(vals)) = (
				decoded.first().and_then(|t| t.clone().into_array()),
				decoded.get(1).and_then(|t| t.clone().into_array()),
			) {
				for (k, v) in keys.iter().zip(vals.iter()) {
					// DIA packs `value = (price << 128) | timestamp`; the price is the high 128 bits.
					if let (Some(key), Some(packed)) = (k.clone().into_string(), v.clone().into_uint()) {
						let le = packed.to_little_endian();
						raw.push((key, U256::from_little_endian(&le[16..32])));
					}
				}
			}
		}
	}

	raw.into_iter()
		.filter_map(|(key, price)| {
			let base = key.split('/').next()?.trim_end_matches('\0').to_ascii_lowercase();
			if base.is_empty() {
				None
			} else {
				Some((base, price))
			}
		})
		.collect()
}

/// Apply pending DIA price updates to a *fresh* money market, then re-decide liquidations for the
/// borrowers holding any repriced reserve — the same-block oracle fast-path.
///
/// Both directly-quoted reserves (symbol == base) and derived reserves (symbol *contains* base —
/// e.g. `gDOT`/`vDOT` for a `DOT` update) are repriced. A derived reserve is scaled by the ratio
/// `new_base / old_base`: the LST/strategy exchange rate is unchanged by a USD-price move, so
/// scaling its current price tracks the underlying correctly.
pub fn apply_oracle_updates_and_decide(
	cfg: &LiquidationTaskConfig,
	money_market: &mut MoneyMarket,
	updates: &[(String, U256)],
	borrowers: &[Borrower],
) -> Vec<LiquidationDecision> {
	use ethabi::ethereum_types::U512;

	let scale = |v: U256, num: U256, den: U256| -> U256 {
		if den.is_zero() {
			return v;
		}
		TryInto::<U256>::try_into(v.full_mul(num) / U512::from(den)).unwrap_or(v)
	};

	// Per repriced reserve: (idx, old_price, new_price). Both the reserve price AND each borrower's
	// base-currency amounts must be scaled — the borrower stores collateral/debt already converted
	// to base at fetch time, so changing the reserve price alone would NOT move the health factor.
	let mut affected: Vec<(usize, U256, U256)> = Vec::new();

	for (base, new_price) in updates {
		// Old price of the directly-quoted reserve (symbol == base) — the ratio denominator.
		let old_base_price = money_market
			.reserves
			.values()
			.find(|r| r.symbol.to_ascii_lowercase() == *base)
			.map(|r| r.price);

		// Collect (address, old_price, new_price) first to avoid a mutable borrow while iterating.
		let mut repriced: Vec<(EvmAddress, U256, U256)> = Vec::new();
		for r in money_market.reserves.values() {
			let sym = r.symbol.to_ascii_lowercase();
			if !sym.contains(base.as_str()) {
				continue;
			}
			if sym == *base {
				repriced.push((r.address, r.price, *new_price)); // direct
			} else if let Some(old_base) = old_base_price {
				// derived: new = current * new_base / old_base (LST/strategy exchange rate is
				// unchanged by a USD-price move, so scaling the current derived price is correct).
				repriced.push((r.address, r.price, scale(r.price, *new_price, old_base)));
			}
		}

		for (addr, old, np) in repriced {
			if money_market.update_price(addr, np).is_ok() {
				if let Some(r) = money_market.reserves.get(&addr) {
					if !affected.iter().any(|(i, _, _)| *i == r.idx) {
						affected.push((r.idx, old, np));
					}
				}
			}
		}
	}

	if affected.is_empty() {
		return Vec::new();
	}

	let affected_idx: Vec<usize> = affected.iter().map(|(i, _, _)| *i).collect();

	let mut decisions = Vec::new();
	for borrower in borrowers {
		// Only borrowers touching a repriced reserve (as collateral or debt) can flip.
		if !borrower.configuration.uses_any(&affected_idx) {
			continue;
		}

		// Re-scale the borrower's base-currency collateral/debt for the repriced reserves so the
		// simulated health factor reflects the pending price.
		let mut b = borrower.clone();
		for (idx, old, np) in &affected {
			if let Some(Some(ur)) = b.reserves.get_mut(*idx) {
				let new_coll = scale(ur.collateral, *np, *old);
				let new_debt = scale(ur.debt, *np, *old);
				b.total_collateral = b
					.total_collateral
					.saturating_sub(ur.collateral)
					.saturating_add(new_coll);
				b.total_debt = b.total_debt.saturating_sub(ur.debt).saturating_add(new_debt);
				ur.collateral = new_coll;
				ur.debt = new_debt;
			}
		}

		if let Some(decision) = decide_liquidation(cfg, money_market, &b) {
			decisions.push(decision);
		}
	}
	decisions
}

pub async fn run<C, B, CL, TP>(task: LiquidationTask<C, B, TP>, client: Arc<CL>)
where
	CL: BlockchainEvents<B> + 'static,
	CL: HeaderBackend<B>,
	CL: ProvideRuntimeApi<B>,
	CL::Api: EthereumRuntimeRPCApi<B> + Erc20MappingApi<B> + CurrenciesApi<B, AssetId, AccountId, Balance>,
	B: hydradx_runtime::BlockT,
	<B as Block>::Extrinsic: From<OpaqueExtrinsic>,
	<B as BlockT>::Extrinsic: frame_support::traits::IsType<hydradx_runtime::opaque::UncheckedExtrinsic>,
	TP: TransactionPool<Block = B> + 'static,
	C: RuntimeClient<B>,
{
	let mut blocks_stream = client.import_notification_stream();
	// Mempool monitor for pending DIA oracle-update txs — drives the same-block fast-path.
	let mut tx_stream = task.transaction_pool.import_notification_stream();

	// Multi-market instance registry. Instances arrive from three sources — on-chain
	// bootstrap (BorrowingContract + GigaHdxPoolContract storage, every block), omniwatch
	// discovery (per-borrower `pool` tags), and config pins — and are never dropped
	// (keep-until-restart). Config pins are registered up-front; their pools resolve at the
	// first money-market fetch.
	let mut instances: Vec<MmInstance> = Vec::new();
	for pap in &task.cfg.pool_address_providers {
		if find_instance_by_pap(&instances, *pap).is_some() {
			log::warn!(target: LOG_TARGET, "{:?} run(): duplicate --pool-address-provider {pap:?}, ignoring", task.cfg.log_prefix);
			continue;
		}
		instances.push(MmInstance::new(
			task.cfg.log_prefix.as_str(),
			*pap,
			None,
			InstanceSource::Config,
		));
	}

	// A down/unreachable omniwatch must NOT panic or kill the worker: retry with backoff and start
	// unseeded if it stays down. Coverage is a UNION of omniwatch seeds and on-chain BORROW-event
	// discovery: the sets only grow on external input and only shrink on on-chain evidence (a
	// scanned borrower with zero debt) — an omniwatch response that omits a known borrower must
	// never evict it.
	let seed = fetch_borrowers_list_with_retry(
		&task.https,
		task.url.clone(),
		task.cfg.log_prefix.as_str(),
		OMNIWATCH_FETCH_ATTEMPTS,
		OMNIWATCH_FETCH_BACKOFF,
		OMNIWATCH_FETCH_TIMEOUT,
	)
	.await;
	// An empty-but-successful response (e.g. omniwatch restarted with a cold DB) does NOT count
	// as seeded — pre-existing borrowers are still unknown, keep the fast re-seed cadence.
	let mut seeded = matches!(&seed, Some(list) if !list.is_empty());
	// Pool-tagged borrowers awaiting routing: bucketing needs resolved pools (and may create
	// instances), which needs the runtime API — drained inside each block's API scope.
	//
	// The on-disk cache is a fallback, read only when omniwatch is unreachable at startup (fetch
	// returned None) and a file exists. It recovers last-known coverage without setting `seeded`,
	// so a live fetch stays authoritative and the fast re-seed cadence keeps running. A live fetch
	// (even empty) does not read the file. `persist_after_scan` requests a cache write once a
	// successful fetch has been routed into the instances (see the save site after the scan).
	let mut persist_after_scan;
	let mut pending_seed: PoolTaggedBorrowers = match seed {
		Some(list) => {
			persist_after_scan = true;
			list
		}
		None => {
			persist_after_scan = false; // a stale load must not be written back
			match task.cfg.borrower_cache_path.as_deref() {
				Some(path) => borrower_cache::load(path, task.cfg.log_prefix.as_str()),
				None => Vec::new(),
			}
		}
	};
	let mut blocks_since_refetch: u32 = 0;
	// In-flight background re-seed: never awaited inline — a slow omniwatch must not stall the
	// scan loop and cost a block's liquidation round.
	let mut refetch_rx: Option<tokio::sync::oneshot::Receiver<Option<PoolTaggedBorrowers>>> = None;
	// Blocks whose BORROW logs have not been scanned yet: discovery needs the resolved pool
	// addresses, so a block skipped on a timestamp/money-market fetch failure (or imported as
	// non-best and canonicalized later) must stay queued — a BORROW log must never be lost.
	let mut pending_event_blocks: Vec<B::Hash> = Vec::new();
	// While some instance's pool is still unresolved, queued blocks are scanned for the
	// RESOLVED pools without being drained (so the late market still gets its logs from the
	// eventual full drain — borrower insertion is idempotent). This watermark tracks how far
	// that partial scanning has progressed, so each block's logs are read at most once.
	let mut event_scan_watermark: usize = 0;
	// Per-instance money market + borrowers are cached on each instance from its latest scan.
	// The oracle fast-path reuses them (applying only the pending price delta) instead of
	// re-fetching — a fetch is ~200ms of runtime-API EVM calls, too slow to beat the block that
	// includes the oracle tx. Reusing the cache lets the fast-path decide + submit in well under
	// a millisecond, so the liquidation is in the pool before that block seals and lands in it,
	// ordered right after the oracle update by the tx-priority ladder. A few-seconds-stale cache
	// is fine: the per-block scan is the source of truth and corrects any miss next block.

	// Per-instance scan context, rebuilt each block inside the API scope and moved out as plain
	// (`Send`) data for the parallel scan.
	struct ScanCtx {
		mm: MoneyMarket,
		kind: InstanceKind,
		atoken_map: HashMap<AssetId, AssetId>,
		hydration: Hydration,
		log_prefix: String,
	}

	loop {
		tokio::select! {
		  Some(b) = blocks_stream.next() => {
		  pending_event_blocks.push(b.hash);
		  if pending_event_blocks.len() > MAX_PENDING_EVENT_BLOCKS {
			  let dropped = pending_event_blocks.len() - MAX_PENDING_EVENT_BLOCKS;
			  pending_event_blocks.drain(..dropped);
			  event_scan_watermark = event_scan_watermark.saturating_sub(dropped);
			  log::warn!(target: LOG_TARGET, "{:?} run(): dropped {} unscanned block(s) from the event-discovery queue", task.cfg.log_prefix, dropped);
		  }

		  if !b.is_new_best {
			  continue;
		  }

		  // Harvest a completed background re-seed, if any (non-blocking).
		  if let Some(rx) = refetch_rx.as_mut() {
			  match rx.try_recv() {
				  Ok(Some(refreshed)) => {
					  seeded |= !refreshed.is_empty();
					  pending_seed.extend(refreshed);
					  persist_after_scan = true; // refresh the cache from this live re-fetch
					  refetch_rx = None;
				  }
				  Ok(None) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => refetch_rx = None,
				  Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
			  }
		  }

		  // Periodic background re-seed. The fast cadence keys on `seeded`, not on set emptiness:
		  // a borrower discovered from a BORROW event must not slow seed recovery down, since only
		  // a successful omniwatch fetch covers pre-existing borrowers.
		  blocks_since_refetch += 1;
		  let refetch_after = if seeded {
			  OMNIWATCH_REFETCH_EVERY_N_BLOCKS
		  } else {
			  OMNIWATCH_REFETCH_UNSEEDED_EVERY_N_BLOCKS
		  };
		  if blocks_since_refetch >= refetch_after && refetch_rx.is_none() {
			  blocks_since_refetch = 0;
			  let (tx, rx) = tokio::sync::oneshot::channel();
			  refetch_rx = Some(rx);
			  let https = task.https.clone();
			  let url = task.url.clone();
			  let log_prefix = task.cfg.log_prefix.clone();
			  tokio::spawn(async move {
				  let refreshed = match tokio::time::timeout(
					  OMNIWATCH_FETCH_TIMEOUT,
					  fetch_borrowers_list(&https, url, log_prefix.as_str()),
				  )
				  .await
				  {
					  Ok(r) => r,
					  Err(_) => {
						  log::error!(target: LOG_TARGET, "{log_prefix:?} run(): omniwatch re-fetch timed out after {OMNIWATCH_FETCH_TIMEOUT:?}");
						  None
					  }
				  };
				  let _ = tx.send(refreshed);
			  });
		  }

		  let block_number: BlockNumber = (*b.header.number()).saturated_into();

		  // Everything that needs the (non-`Send`) runtime API happens in this scope: instance
		  // bootstrap/discovery, per-instance money-market fetches, and event discovery. Plain
		  // data (`now` + per-instance scan contexts) moves out for the parallel scan.
		  let (now, mut scan_ctxs) = {
			  let runtime_api = client.runtime_api();
			  let api = pepl_worker_support::types::ApiProvider::<&CL::Api>(runtime_api.deref());

			  let Some(now) = api.timestamp(b.hash) else {
				  log::error!(target: LOG_TARGET, "{:?} run(): failed to read timestamp for block {:?}, skipping", task.cfg.log_prefix, b.hash);
				  continue;
			  };

			  // --- On-chain instance bootstrap (primary source, no external dependency) ---
			  // The chain itself names the markets: `Liquidation.BorrowingContract` (main) and
			  // `GigaHdx.GigaHdxPoolContract` (gigahdx). A collator needs ZERO flags to watch
			  // every market the runtime knows about — even with omniwatch fully down.
			  let read_pool_storage = |key: [u8; 32]| -> Option<EvmAddress> {
				  match task.client.storage(b.hash, &StorageKey(key.to_vec())) {
					  Ok(Some(data)) if data.0.len() >= 20 => Some(EvmAddress::from_slice(&data.0[..20])),
					  Ok(_) => None,
					  Err(e) => {
						  log::error!(target: LOG_TARGET, "{:?} run(): pool storage read failed: {e:?}", task.cfg.log_prefix);
						  None
					  }
				  }
			  };
			  match read_pool_storage(storage_key::borrowing_contract()) {
				  Some(main_pool) => {
					  let _ = ensure_instance_for_pool(&mut instances, &api, b.hash, main_pool, InstanceSource::Chain, &task.cfg);
				  }
				  // ValueQuery storage: absent means the chain runs on the pallet default (the
				  // main pool). We can't read the default's POOL address generically, but its
				  // PAP is the known main-market default — pin it as the fallback instance.
				  None => {
					  if find_instance_by_pap(&instances, contracts::POOL_ADDRESS_PROVIDER).is_none()
						  && instances.len() < MAX_MM_INSTANCES
					  {
						  instances.push(MmInstance::new(
							  task.cfg.log_prefix.as_str(),
							  contracts::POOL_ADDRESS_PROVIDER,
							  None,
							  InstanceSource::Chain,
						  ));
					  }
				  }
			  }
			  let giga_pool = read_pool_storage(storage_key::gigahdx_pool_contract());
			  if let Some(giga) = giga_pool {
				  let _ = ensure_instance_for_pool(&mut instances, &api, b.hash, giga, InstanceSource::Chain, &task.cfg);
			  }

			  // --- Route pool-tagged omniwatch borrowers; unknown pools create instances ---
			  // A pair that fails only TRANSIENTLY (PAP resolution / sanity round-trip on a
			  // hiccuping runtime API) is RE-QUEUED for the next block: the borrower set must
			  // only shrink on on-chain evidence, never on an infra failure. Permanent
			  // conditions (denylist, discovery off, registry full) drop the pair — the next
			  // re-seed re-lists it if it still matters.
			  let mut seed_retry: PoolTaggedBorrowers = Vec::new();
			  let mut seen_pairs: HashSet<(EvmAddress, EvmAddress)> = HashSet::new();
			  let mut failed_pools: HashSet<EvmAddress> = HashSet::new();
			  let mut seed_new: HashMap<usize, Vec<EvmAddress>> = HashMap::new();
			  for (addr, pool) in pending_seed.drain(..) {
				  if !seen_pairs.insert((addr, pool)) {
					  continue;
				  }
				  if failed_pools.contains(&pool) {
					  seed_retry.push((addr, pool));
					  continue;
				  }
				  if let Some(idx) = find_instance_by_pool(&instances, pool) {
					  if instances[idx].borrowers.insert(addr) {
						  seed_new.entry(idx).or_default().push(addr);
					  }
					  continue;
				  }
				  if !task.cfg.discovery_enabled {
					  log::debug!(target: LOG_TARGET, "{:?} run(): discovery disabled — dropping borrower {addr:?} of unknown pool {pool:?}", task.cfg.log_prefix);
					  continue;
				  }
				  if task.cfg.pool_denylist.contains(&pool) || instances.len() >= MAX_MM_INSTANCES {
					  continue; // permanent for this process — ensure_instance would refuse anyway
				  }
				  match ensure_instance_for_pool(&mut instances, &api, b.hash, pool, InstanceSource::Discovered, &task.cfg) {
					  Some(idx) => {
						  if instances[idx].borrowers.insert(addr) {
							  seed_new.entry(idx).or_default().push(addr);
						  }
					  }
					  // Transient resolve/sanity failure — retry next block (one attempt per
					  // pool per block; further pairs for the same pool go straight to retry).
					  None => {
						  failed_pools.insert(pool);
						  seed_retry.push((addr, pool));
					  }
				  }
			  }
			  pending_seed = seed_retry;
			  // Coverage changes are reportable events: per-borrower lines for a trickle, one
			  // summary per market for bulk loads (initial seed / seed recovery).
			  for (idx, addrs) in seed_new {
				  if addrs.len() <= 5 {
					  for addr in &addrs {
						  log::info!(target: LOG_TARGET, "{:?} run(): new borrower from omniwatch: {:?}", instances[idx].log_prefix, addr);
					  }
				  } else {
					  log::info!(target: LOG_TARGET, "{:?} run(): omniwatch added {} new borrowers", instances[idx].log_prefix, addrs.len());
				  }
			  }

			  // --- Per-instance money market fetch + kind detection + aToken map refresh ---
			  let mut ctxs: Vec<Option<ScanCtx>> = Vec::with_capacity(instances.len());
			  for inst in instances.iter_mut() {
				  let hydration = Hydration::new(task.cfg.api_caller, inst.pap, inst.log_prefix.as_str());
				  let Some(mm) = hydration.fetch_money_market(&api, b.hash) else {
					  // One broken market must not disable the rest: skip only this instance's
					  // scan; its borrowers, cache and the event queue are retained.
					  log::error!(target: LOG_TARGET, "{:?} run(): failed to fetch money market for block {:?}, skipping this market", inst.log_prefix, b.hash);
					  ctxs.push(None);
					  continue;
				  };
				  inst.pool = Some(mm.pool);
				  // The denylist is authoritative even for pinned/fallback instances whose pool
				  // only becomes known here: never scan or submit against a denylisted pool.
				  if task.cfg.pool_denylist.contains(&mm.pool) {
					  log::error!(target: LOG_TARGET, "{:?} run(): pinned PAP {:?} resolves to DENYLISTED pool {:?} — this market will not be scanned", inst.log_prefix, inst.pap, mm.pool);
					  ctxs.push(None);
					  continue;
				  }
				  inst.kind = if giga_pool == Some(mm.pool) {
					  InstanceKind::GigaHdx
				  } else {
					  InstanceKind::Generic
				  };
				  if inst.kind == InstanceKind::GigaHdx {
					  let mut map = HashMap::new();
					  for r in mm.reserves.values() {
						  match api.address_to_asset(b.hash, r.data.a_token_address) {
							  Ok(Some(atoken_id)) => {
								  map.insert(r.asset_id, atoken_id);
							  }
							  // Fine unless this reserve ends up as the decision's COLLATERAL —
							  // map_decision_collateral fails closed (with an error) in that case.
							  other => {
								  log::debug!(target: LOG_TARGET, "{:?} run(): no registered asset id for aToken {:?} (underlying {:?}): {other:?}", inst.log_prefix, r.data.a_token_address, r.asset_id);
							  }
						  }
					  }
					  inst.atoken_map = map;
				  }
				  ctxs.push(Some(ScanCtx {
					  mm,
					  kind: inst.kind,
					  atoken_map: inst.atoken_map.clone(),
					  hydration,
					  log_prefix: inst.log_prefix.clone(),
				  }));
			  }

			  // Two instances can resolve to the same pool (e.g. a config pin and a chain
			  // bootstrap that raced the pin's first fetch): merge the later into the earlier.
			  let mut i = 0;
			  while i < instances.len() {
				  let Some(pool) = instances[i].pool else {
					  i += 1;
					  continue;
				  };
				  if let Some(first) = find_instance_by_pool(&instances, pool) {
					  if first < i {
						  log::warn!(target: LOG_TARGET, "{:?} run(): merging duplicate instance for pool {pool:?}", task.cfg.log_prefix);
						  let dup = instances.remove(i);
						  let _ = ctxs.remove(i);
						  instances[first].borrowers.extend(dup.borrowers);
						  continue;
					  }
				  }
				  i += 1;
			  }

			  // --- Event-driven discovery across ALL watched pools ---
			  // BORROW logs add borrowers independently of omniwatch, so a borrower omniwatch
			  // never returns is still covered from their first borrow onwards. When every
			  // instance has a resolved pool the queue is DRAINED; while any pool is still
			  // unresolved, the resolved pools are scanned WITHOUT draining (watermarked, each
			  // block read once) so one unresolvable instance cannot stall discovery for the
			  // rest — the late market gets its logs from the eventual full drain (borrower
			  // insertion is idempotent). An empty registry never consumes the queue.
			  let pools: Vec<EvmAddress> = instances.iter().filter_map(|inst| inst.pool).collect();
			  if !pools.is_empty() {
				  let route_discovered = |discovered: Vec<(EvmAddress, EvmAddress)>, instances: &mut Vec<MmInstance>| {
					  for (pool, addr) in discovered {
						  if let Some(idx) = find_instance_by_pool(instances, pool) {
							  if instances[idx].borrowers.insert(addr) {
								  log::info!(target: LOG_TARGET, "{:?} run(): discovered new borrower from BORROW event: {:?}", instances[idx].log_prefix, addr);
							  }
						  }
					  }
				  };
				  if instances.iter().all(|inst| inst.pool.is_some()) {
					  for hash in pending_event_blocks.drain(..) {
						  let found = process_events_multi(task.load_events(hash), &pools, task.cfg.log_prefix.as_str());
						  route_discovered(found, &mut instances);
					  }
					  event_scan_watermark = 0;
				  } else {
					  for hash in pending_event_blocks.iter().skip(event_scan_watermark).copied().collect::<Vec<_>>() {
						  let found = process_events_multi(task.load_events(hash), &pools, task.cfg.log_prefix.as_str());
						  route_discovered(found, &mut instances);
					  }
					  event_scan_watermark = pending_event_blocks.len();
				  }
			  }

			  (now, ctxs)
		  };

		  // Flattened (instance, borrower) pairs — one parallel fan-out serves all markets.
		  let scan_list: Vec<(usize, EvmAddress)> = instances
			  .iter()
			  .enumerate()
			  .filter(|(idx, _)| scan_ctxs.get(*idx).map(|c| c.is_some()).unwrap_or(false))
			  .flat_map(|(idx, inst)| inst.borrowers.iter().map(move |addr| (idx, *addr)))
			  .collect();

		  // Flatten the (instance, borrower) pairs across markets and scan the shards in parallel.
		  // Each thread makes its own (non-`Send`) runtime API and submits the moment it finds a
		  // liquidation, via the tokio handle — no worker-side sort/cap. The unsigned tx carries
		  // `unsigned_priority = collateral-at-risk`, so the tx pool sorts and packs the block.
		  // Threads report zero-debt borrowers (to prune) and fetched borrowers (for the fast-path
		  // cache) back over a channel; the per-instance scan contexts are shared read-only.
		  let handle = tokio::runtime::Handle::current();
		  let (result_tx, result_rx) = std::sync::mpsc::channel::<(usize, EvmAddress, Option<Borrower>)>();
		  let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).max(1);
		  let chunk_size = scan_list.len().div_ceil(cores).max(1);
		  let submitted = AtomicUsize::new(0);

		  tokio::task::block_in_place(|| {
			  std::thread::scope(|scope| {
				  for chunk in scan_list.chunks(chunk_size) {
					  let client = client.clone();
					  let handle = handle.clone();
					  let pool = task.transaction_pool.clone();
					  let result_tx = result_tx.clone();
					  let scan_ctxs = &scan_ctxs;
					  let cfg = &task.cfg;
					  let submitted = &submitted;
					  let best = b.hash;
					  scope.spawn(move || {
						  let runtime_api = client.runtime_api();
						  let api = pepl_worker_support::types::ApiProvider::<&CL::Api>(runtime_api.deref());
						  for (idx, addr) in chunk {
							  let Some(Some(ctx)) = scan_ctxs.get(*idx) else { continue };
							  let log_prefix = ctx.log_prefix.as_str();
							  let Some(borrower) =
								  ctx.hydration.fetch_borrower(&api, best, block_number, &ctx.mm, *addr, now)
							  else {
								  // Infra failure — keep the borrower; the set shrinks only on on-chain evidence.
								  log::error!(target: LOG_TARGET, "{log_prefix:?} run(): failed to fetch borrower {addr:?}, skipping");
								  continue;
							  };
							  if borrower.total_debt.is_zero() {
								  let _ = result_tx.send((*idx, *addr, None)); // prune
								  continue;
							  }
							  if let Some(decision) = decide_liquidation(cfg, &ctx.mm, &borrower) {
								  // Submit-on-find: build the tx here (sync) and spawn the async submit.
								  // GigaHdx decisions are re-mapped to the aToken collateral first
								  // (fail-closed on a missing mapping).
								  if let Some(mapped) = map_decision_collateral(&decision, ctx.kind, &ctx.atoken_map, log_prefix) {
									  if let Some(opaque) = encode_liquidation_opaque(&mapped, ctx.mm.pool, log_prefix) {
										  let pool = pool.clone();
										  submitted.fetch_add(1, Ordering::Relaxed);
										  handle.spawn(async move {
											  let _ = pool.submit_one(best, TransactionSource::Local, opaque.into()).await;
										  });
									  }
								  }
							  }
							  let _ = result_tx.send((*idx, *addr, Some(borrower))); // cache
						  }
					  });
				  }
				  drop(result_tx); // let the receiver iteration end once all threads finish
			  });
		  });

		  // Drain results: prune repaid borrowers, collect the rest for each instance's
		  // fast-path cache.
		  let mut fetched: Vec<Vec<Borrower>> = instances.iter().map(|_| Vec::new()).collect();
		  for (idx, addr, maybe_borrower) in result_rx {
			  match maybe_borrower {
				  None => {
					  instances[idx].borrowers.remove(&addr);
					  log::info!(target: LOG_TARGET, "{:?} run(): borrower repaid all debt, pruned: {:?}", instances[idx].log_prefix, addr);
				  }
				  Some(borrower) => fetched[idx].push(borrower),
			  }
		  }
		  for (idx, ctx) in scan_ctxs.iter_mut().enumerate() {
			  if let Some(ctx) = ctx.take() {
				  instances[idx].cached_borrowers = std::mem::take(&mut fetched[idx]);
				  instances[idx].cached_mm = Some(ctx.mm);
			  }
		  }

		  log::debug!(target: LOG_TARGET, "{:?} run(): parallel scan of {} borrowers across {} market(s) over {} cores submitted {} liquidations (submit-on-find) for block {:?}", task.cfg.log_prefix, scan_list.len(), instances.len(), cores, submitted.load(Ordering::Relaxed), b.hash);

		  // After a successful omniwatch fetch has been routed into the instances, replace the
		  // on-disk cache with the current working set (union incl. event-discovered, minus
		  // pruned). `save` no-ops on an empty set, so a reachable-but-empty response can never
		  // wipe a good file. Only ever set by a live fetch (startup seed or re-fetch).
		  if persist_after_scan {
			  if let Some(cache_path) = task.cfg.borrower_cache_path.as_deref() {
				  borrower_cache::save(cache_path, &instances, task.cfg.log_prefix.as_str());
			  }
			  persist_after_scan = false;
		  }
		  },

		  // Same-block oracle fast-path: a pending DIA price-update tx in the mempool lets us react
		  // to a price move in the same block it lands, ahead of the next per-block scan. The
		  // tx-priority ladder (oracle update > liquidation > user txs) orders the liquidation right
		  // after the oracle update on-chain.
		  Some(tx_hash) = tx_stream.next() => {
		  let Some(pool_tx) = task.transaction_pool.ready_transaction(&tx_hash) else { continue };
		  let encoded = pool_tx.data().encode();
		  let Ok(xt) = hydradx_runtime::HydraUncheckedExtrinsic::decode(&mut &encoded[..]) else { continue };
		  let Some(oracle_tx) = is_oracle_update_tx(
			  &xt.0,
			  task.cfg.oracle_signer.clone(),
			  task.cfg.oracle_update_call.clone(),
			  task.cfg.log_prefix.as_str(),
		  ) else { continue };

		  let updates = parse_oracle_price_updates(&oracle_tx);
		  log::debug!(target: LOG_TARGET, "{:?} run(): oracle fast-path saw a DIA update tx, parsed {} price update(s): {:?}", task.cfg.log_prefix, updates.len(), updates);
		  if updates.is_empty() {
			  continue;
		  }

		  // Reuse each instance's cached money market + borrowers from its last per-block scan —
		  // NO runtime API call here, so we decide + submit in well under a millisecond and beat
		  // the block that includes the oracle tx. One DIA update can move several markets, so
		  // every cached instance is repriced; instances not yet scanned are skipped. GigaHdx
		  // decisions are re-mapped to the aToken collateral (fail-closed). Note: a market with
		  // no reserve matching the update's base symbol (e.g. no plain-HDX reserve on the
		  // gigahdx market) is a fast-path no-op there — the next per-block scan covers it.
		  let mut decisions: Vec<(EvmAddress, LiquidationDecision)> = Vec::new();
		  for inst in &instances {
			  let Some(mm_cache) = inst.cached_mm.as_ref() else { continue };
			  let mut mm = mm_cache.clone();
			  for decision in apply_oracle_updates_and_decide(&task.cfg, &mut mm, &updates, &inst.cached_borrowers) {
				  if let Some(mapped) =
					  map_decision_collateral(&decision, inst.kind, &inst.atoken_map, inst.log_prefix.as_str())
				  {
					  decisions.push((mm.pool, mapped));
				  }
			  }
		  }

		  if decisions.is_empty() {
			  log::debug!(target: LOG_TARGET, "{:?} run(): oracle fast-path — no borrower flips underwater on the pending price(s)", task.cfg.log_prefix);
			  continue;
		  }

		  decisions.sort_by(|a, b| b.1.priority.cmp(&a.1.priority));
		  let cap = task.cfg.liquidations_per_block as usize;
		  let best = client.info().best_hash;
		  for (market_pool, decision) in decisions.iter().take(cap) {
			  let _ = task.submit_liquidation(best, *market_pool, decision).await;
		  }
		  log::info!(target: LOG_TARGET, "{:?} run(): oracle fast-path decided {} liquidations across {} market(s) from a pending DIA update (cached mm), submitted up to {}", task.cfg.log_prefix, decisions.len(), instances.len(), cap);
		  },

		  else => break,
		}
	}

	log::warn!(target: LOG_TARGET, "{:?} run(): notification stream ended, liquidation worker stopping", task.cfg.log_prefix);
}
