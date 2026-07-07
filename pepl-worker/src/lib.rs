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
use std::collections::HashSet;
use std::marker::PhantomData;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::ops::Deref;
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
// Until the FIRST successful omniwatch fetch, re-seed more often: event discovery only sees new
// borrows, so pre-existing borrowers stay invisible until a seed succeeds (the prod-miss shape).
pub const OMNIWATCH_REFETCH_UNSEEDED_EVERY_N_BLOCKS: u32 = 10;

// Cap on blocks queued for BORROW-event scanning while the money market can't be fetched.
pub const MAX_PENDING_EVENT_BLOCKS: usize = 256;

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
}

/// The configuration for the liquidation worker cli params.
#[derive(Clone, Debug, clap::Parser)]
pub struct LiquidationWorkerCli {
	/// Enable/disable liquidation worker.
	#[clap(long)]
	pub liquidation_worker: Option<bool>,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pool_address_provider: Option<EvmAddress>,

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

/// The configuration for `LiquidationTask`.
pub struct LiquidationTaskConfig {
	pub pool_address_provider: EvmAddress,

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
			pool_address_provider: v.pool_address_provider.unwrap_or(contracts::POOL_ADDRESS_PROVIDER),
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
			pool_address_provider: contracts::POOL_ADDRESS_PROVIDER,
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
		log::info!(target: LOG_TARGET, "{:?} decide_liquidation(): collateral below min, skipping, borrower: {:?}, collateral: {:?}", log_prefix, borrower.address, borrower.total_collateral);
		return None;
	}

	let hf = borrower
		.calc_health_factor(money_market)
		.inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?} decide_liquidation(): failed to calc health factor, skipping, borrower: {:?}, err: {:?}", log_prefix, borrower.address, e);
		})
		.ok()?;

	if hf.is_zero() {
		log::info!(target: LOG_TARGET, "{:?} decide_liquidation(): borrower with 0 health factor, borrower: {:?}", log_prefix, borrower.address);
	}

	if hf >= U256::from(ONE_HF) {
		log::info!(target: LOG_TARGET, "{:?} decide_liquidation(): healthy borrower, borrower: {:?}", log_prefix, borrower.address);
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

/// Build the opaque `liquidate` extrinsic (sync — no tx pool needed). Shared by
/// `submit_liquidation` and the parallel submit-on-find scan (which spawns the async `submit_one`
/// onto a tokio handle from a worker thread). The unsigned tx carries `unsigned_priority =
/// collateral-at-risk`, so the tx pool orders competing liquidations for the block builder.
fn encode_liquidation_opaque(decision: &LiquidationDecision, log_prefix: &str) -> Option<OpaqueExtrinsic> {
	let tx = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate {
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
		// Deliberate fail-fast: a mistyped --omniwatch-url panics at startup so the operator
		// notices immediately — a collator silently running without its liquidation worker is
		// the production-incident shape this worker exists to prevent.
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

	async fn submit_liquidation(&self, block: B::Hash, decision: &LiquidationDecision) -> Result<(), ()> {
		let log_prefix = self.cfg.log_prefix.as_str();
		let opaque_tx = encode_liquidation_opaque(decision, log_prefix).ok_or(())?;

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
		log::info!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): fetching events from storage", self.cfg.log_prefix);

		let events = match self.client.storage(block, &self.system_events_key) {
			Ok(Some(events)) => events,
			Ok(None) => {
				log::info!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): finished, storage returned no data, elapsed: {:?}", self.cfg.log_prefix, timer.elapsed().as_nanos());
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

		log::info!(target: LOG_TARGET, "{:?} LiquidationTask.load_events(): finished loading {:?} events, elapsed: {:?}", self.cfg.log_prefix, events.len(), timer.elapsed().as_nanos());
		events
	}
}

/// Function fetches and returns list of borrowers' addresses from provided `url`.
/// Returned list is not deduped nor sorted in any way.
async fn fetch_borrowers_list(https: &https::Client, url: Uri, log_prefix: &str) -> Option<Vec<EvmAddress>> {
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

	let mut b = Vec::<EvmAddress>::with_capacity(data.borrowers.len());
	for (addr, _) in &data.borrowers {
		b.push(*addr);
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
) -> Option<Vec<EvmAddress>> {
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
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{:?} process_events(): processing {:?} events", log_prefix, events.len());

	let mut borrowers: Vec<EvmAddress> = Vec::with_capacity(20);
	for evt in &events {
		let RuntimeEvent::EVM(pallet_evm::Event::Log { log }) = &evt.event else {
			continue;
		};

		if log.address == pool && log.topics.first() == Some(&events::BORROW) {
			let Some(&borrower) = log.topics.get(2) else {
				continue;
			};

			borrowers.push(borrower.into());
		}
	}

	log::info!(target: LOG_TARGET, "{:?} process_events(): finished, elapsed={:?}", log_prefix, timer.elapsed().as_nanos());
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
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, non evm transaction, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	let action = match transaction {
		Transaction::Legacy(ref legacy_transaction) => legacy_transaction.action,
		Transaction::EIP2930(ref eip2930_transaction) => eip2930_transaction.action,
		Transaction::EIP1559(ref eip1559_transaction) => eip1559_transaction.action,
		Transaction::EIP7702(_) => {
			log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, unsupported EIP7702 tx, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
			return None;
		}
	};

	let pallet_ethereum::TransactionAction::Call(caller) = action else {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, no caller, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	// check if the transaction is DIA oracle update
	if !allowed_callers.contains(&caller) {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, caller is not allowed, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	}

	// additional check to prevent running the worker for DIA oracle updates signed by invalid address
	let Some(Ok(signer)) = extrinsic.function.check_self_contained() else {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not self contained, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	};

	if !allowed_signers.contains(&signer) {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not allowed, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
		return None;
	}

	log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, elapsed: {:?}", log_prefix, timer.elapsed().as_nanos());
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
/// Fixes v1's blind spot: v1 repriced only the directly-quoted reserve (symbol == base) and marked
/// derived reserves (symbol *contains* base — e.g. `gDOT`/`vDOT` for a `DOT` update) as affected
/// but NEVER repriced them, so it missed strategy-token liquidations. Here a derived reserve is
/// repriced by the ratio `new_base / old_base` (the LST/strategy exchange rate is unchanged by a
/// USD-price move, so scaling the derived reserve's current price is correct).
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
				b.total_collateral = b.total_collateral.saturating_sub(ur.collateral).saturating_add(new_coll);
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
	// Mempool monitor for pending DIA oracle-update txs — the same-block fast-path (W5).
	let mut tx_stream = task.transaction_pool.import_notification_stream();

	let hydration = Hydration::new(
		task.cfg.api_caller,
		task.cfg.pool_address_provider,
		task.cfg.log_prefix.as_str(),
	);

	// A down/unreachable omniwatch must NOT panic or kill the worker: retry with backoff and start
	// unseeded if it stays down. Coverage is a UNION of omniwatch seeds and on-chain BORROW-event
	// discovery: the set only grows on external input and only shrinks on on-chain evidence (a
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
	let mut borrowers: HashSet<EvmAddress> = seed.unwrap_or_default().into_iter().collect();
	let mut blocks_since_refetch: u32 = 0;
	// In-flight background re-seed: never awaited inline — a slow omniwatch must not stall the
	// scan loop and cost a block's liquidation round.
	let mut refetch_rx: Option<tokio::sync::oneshot::Receiver<Option<Vec<EvmAddress>>>> = None;
	// Blocks whose BORROW logs have not been scanned yet: discovery needs the resolved pool
	// address, so a block skipped on a timestamp/money-market fetch failure (or imported as
	// non-best and canonicalized later) must stay queued — a BORROW log must never be lost.
	let mut pending_event_blocks: Vec<B::Hash> = Vec::new();
	// Money market + borrowers cached from the latest per-block scan. The oracle fast-path reuses
	// them (applying only the pending price delta) instead of re-fetching — a fetch is ~200ms of
	// runtime-API EVM calls, too slow to beat the block that includes the oracle tx. Reusing the
	// cache lets the fast-path decide + submit in well under a millisecond, so the liquidation is in
	// the pool before that block seals and lands in it (ordered right after the oracle update by the
	// priority ladder, W8). A few-seconds-stale cache is fine: the per-block scan is the source of
	// truth and corrects any miss next block.
	let mut cached_mm: Option<MoneyMarket> = None;
	let mut cached_borrowers: Vec<Borrower> = Vec::new();

	loop {
	  tokio::select! {
	    Some(b) = blocks_stream.next() => {
		pending_event_blocks.push(b.hash);
		if pending_event_blocks.len() > MAX_PENDING_EVENT_BLOCKS {
			let dropped = pending_event_blocks.len() - MAX_PENDING_EVENT_BLOCKS;
			pending_event_blocks.drain(..dropped);
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
					borrowers.extend(refreshed);
					refetch_rx = None;
				}
				Ok(None) | Err(tokio::sync::oneshot::error::TryRecvError::Closed) => refetch_rx = None,
				Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {}
			}
		}

		// Periodic background re-seed. `seeded` (not set emptiness) keys the fast cadence: a
		// borrower discovered from a BORROW event must not slow seed recovery down — the seed is
		// what covers PRE-EXISTING borrowers, the prod-miss shape.
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

		// Fetch the money market once for this block (runtime API is NOT `Send`, so it is scoped
		// here and dropped before the parallel scan, which makes its own per-thread API).
		let (now, mm) = {
			let runtime_api = client.runtime_api();
			let api = pepl_worker_support::types::ApiProvider::<&CL::Api>(runtime_api.deref());

			let Some(now) = api.timestamp(b.hash) else {
				log::error!(target: LOG_TARGET, "{:?} run(): failed to read timestamp for block {:?}, skipping", task.cfg.log_prefix, b.hash);
				continue;
			};

			let Some(mm) = hydration.fetch_money_market(&api, b.hash) else {
				log::error!(target: LOG_TARGET, "{:?} run(): failed to fetch money market for block {:?}, skipping", task.cfg.log_prefix, b.hash);
				continue;
			};

			// Event-driven discovery: BORROW logs from the (dynamically resolved) pool add
			// borrowers independently of omniwatch, so a borrower omniwatch never returns is
			// still covered from their first borrow onwards. Drains the whole queue, so blocks
			// skipped while the money market was unavailable are scanned now.
			for hash in pending_event_blocks.drain(..) {
				for addr in process_events(task.load_events(hash), mm.pool, task.cfg.log_prefix.as_str()) {
					if borrowers.insert(addr) {
						log::info!(target: LOG_TARGET, "{:?} run(): discovered new borrower from BORROW event: {:?}", task.cfg.log_prefix, addr);
					}
				}
			}

			(now, mm)
		};

		let scan_list: Vec<EvmAddress> = borrowers.iter().copied().collect();

		// Lumír's model: split the borrower list across cores and scan the shards in parallel;
		// each thread makes its OWN (non-`Send`) runtime API, and the MOMENT it finds a liquidation
		// it submits it (submit-on-find) via the tokio handle — no worker-side sort/cap. The unsigned
		// tx carries `unsigned_priority = collateral-at-risk`, so the tx pool sorts and packs the
		// block. Threads report zero-debt (prune) and fetched borrowers (fast-path cache) back over a
		// channel; the money market is shared read-only.
		let handle = tokio::runtime::Handle::current();
		let (result_tx, result_rx) = std::sync::mpsc::channel::<(EvmAddress, Option<Borrower>)>();
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
					let mm = &mm;
					let cfg = &task.cfg;
					let hydration = &hydration;
					let submitted = &submitted;
					let best = b.hash;
					let log_prefix = task.cfg.log_prefix.as_str();
					scope.spawn(move || {
						let runtime_api = client.runtime_api();
						let api = pepl_worker_support::types::ApiProvider::<&CL::Api>(runtime_api.deref());
						for addr in chunk {
							let Some(borrower) = hydration.fetch_borrower(&api, best, block_number, mm, *addr, now)
							else {
								// Infra failure — keep the borrower; the set shrinks only on on-chain evidence.
								log::error!(target: LOG_TARGET, "{log_prefix:?} run(): failed to fetch borrower {addr:?}, skipping");
								continue;
							};
							if borrower.total_debt.is_zero() {
								let _ = result_tx.send((*addr, None)); // prune
								continue;
							}
							if let Some(decision) = decide_liquidation(cfg, mm, &borrower) {
								// Submit-on-find: build the tx here (sync) and spawn the async submit.
								if let Some(opaque) = encode_liquidation_opaque(&decision, log_prefix) {
									let pool = pool.clone();
									submitted.fetch_add(1, Ordering::Relaxed);
									handle.spawn(async move {
										let _ = pool.submit_one(best, TransactionSource::Local, opaque.into()).await;
									});
								}
							}
							let _ = result_tx.send((*addr, Some(borrower))); // cache
						}
					});
				}
				drop(result_tx); // let the receiver iteration end once all threads finish
			});
		});

		// Drain results: prune repaid borrowers, collect the rest for the fast-path cache.
		let mut fetched: Vec<Borrower> = Vec::with_capacity(scan_list.len());
		for (addr, maybe_borrower) in result_rx {
			match maybe_borrower {
				None => {
					borrowers.remove(&addr);
					log::info!(target: LOG_TARGET, "{:?} run(): borrower repaid all debt, pruned: {:?}", task.cfg.log_prefix, addr);
				}
				Some(borrower) => fetched.push(borrower),
			}
		}
		cached_borrowers = fetched;
		cached_mm = Some(mm);

		log::debug!(target: LOG_TARGET, "{:?} run(): parallel scan of {} borrowers over {} cores submitted {} liquidations (submit-on-find) for block {:?}", task.cfg.log_prefix, scan_list.len(), cores, submitted.load(Ordering::Relaxed), b.hash);
	    },

	    // Same-block oracle fast-path (W5): a pending DIA price-update tx in the mempool lets us
	    // react to a price move in the SAME block it lands, ahead of the next per-block scan. The
	    // priority ladder (oracle update > liquidation > user txs) orders the liquidation right
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

		// Reuse the cached money market + borrowers from the last per-block scan — NO runtime API
		// call here, so we decide + submit in well under a millisecond and beat the block that
		// includes the oracle tx. Skip until the first scan has populated the cache.
		let Some(mm_cache) = cached_mm.as_ref() else { continue };
		let mut mm = mm_cache.clone();
		let mut decisions = apply_oracle_updates_and_decide(&task.cfg, &mut mm, &updates, &cached_borrowers);

		if decisions.is_empty() {
			log::debug!(target: LOG_TARGET, "{:?} run(): oracle fast-path — no borrower flips underwater on the pending price(s)", task.cfg.log_prefix);
			continue;
		}

		decisions.sort_by(|a, b| b.priority.cmp(&a.priority));
		let cap = task.cfg.liquidations_per_block as usize;
		let best = client.info().best_hash;
		for decision in decisions.iter().take(cap) {
			let _ = task.submit_liquidation(best, decision).await;
		}
		log::info!(target: LOG_TARGET, "{:?} run(): oracle fast-path decided {} liquidations from a pending DIA update (cached mm), submitted up to {}", task.cfg.log_prefix, decisions.len(), cap);
	    },

	    else => break,
	  }
	}

	log::warn!(target: LOG_TARGET, "{:?} run(): notification stream ended, liquidation worker stopping", task.cfg.log_prefix);
}
