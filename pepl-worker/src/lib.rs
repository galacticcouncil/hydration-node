use codec::{Decode, Encode};
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
use pepl_worker_support::types::Timestamp;
use pepl_worker_support::Hydration;
use primitives::AccountId;
use primitives::EvmAddress;
use sc_client_api::BlockchainEvents;
use sc_client_api::HeaderBackend;
use sc_client_api::StorageKey;
use sc_transaction_pool_api::InPoolTransaction;
use sc_transaction_pool_api::TransactionPool;
use serde::Deserialize;
use sp_api::ProvideRuntimeApi;
use sp_core::H160;
use sp_core::H256;
use sp_core::U256;
use sp_runtime::traits::Block;
use sp_runtime::traits::Zero;
use sp_runtime::OpaqueExtrinsic;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Instant;

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "liquidation-worker";

// Target healt factor after liquidation
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

const ONE_HF: u128 = 1_000_000_000_000_000_000; //1.0(10^18)

// 1.0 in base currency(8 dec.)
const ONE_BASE: u128 = 100_000_000;

// URL of serve to fetch borrowers list
const OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

// Number of liquidation trasactions submited per 1 block
const LIQUIDATIONS_PER_BLOCK: u8 = 20;

// Contracts' addresses
pub mod contracts {
	use super::*;
	use sp_core::H160;

	pub const _POOL_CONFIGURATOR: EvmAddress = H160(hex!("e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4"));

	// Address of the pool address provider contract.
	pub const POOL_ADDRESS_PROVIDER: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));

	// Money market address
	pub const BORROW_CALL: EvmAddress = H160(hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38"));

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

	#[derive(Clone, Encode, Decode, Deserialize, Debug)]
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

	#[derive(Clone, Encode, Decode, Deserialize, Debug)]
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

mod storage_key {
	use super::*;

	pub const SYSTEM_EVENTS: [u8; 32] = hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7");
}

mod traits {
	//NOTE: maybe this won't be necessary
	pub trait Client {}
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
	/// Borrowers holding `< min_collaterall` are skipped
	pub min_collaterall: U256,

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
			min_collaterall: U256::from(ONE_BASE),
			log_prefix: "todo-default-log_prefix".to_string(),
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
			min_collaterall: U256::from(ONE_BASE),
			log_prefix: "defaul-pepl-log-prefix".to_string(),
		}
	}
}

pub struct LiquidationTask<C, B> {
	client: C,
	pub https: https::Client,
	pub url: Uri,
	borrowers: HashMap<EvmAddress, Borrower>,
	system_events_key: StorageKey,
	_phantom: PhantomData<B>,
	cfg: LiquidationTaskConfig,
}

impl<C: RuntimeClient<B>, B: Block> LiquidationTask<C, B> {
	pub fn new(client: C, cfg: LiquidationTaskConfig) -> Self {
		//It's ok to panic here, collator should fix URL or disable liquidation worker
		let url = cfg
			.omniwatch_url
			.parse()
			.expect("LiquidationTaks: failed to parse omniwatch_url, provide correct --omniwatch-url or disable liquidation worker");

		Self {
			client,
			https: https::new(),
			url,
			borrowers: HashMap::new(),
			system_events_key: StorageKey(storage_key::SYSTEM_EVENTS.to_vec()),
			_phantom: PhantomData,
			cfg,
		}
	}

	/// Function updates borrower's data if necessary, checks borrower's health factor and liquidate
	/// if necessary
	/// WARN: this function assume `MoneyMarket`'s state is up to date for current block.
	fn process_borrower<RA: RuntimeApiProvider<B>>(
		&self,
		hydration: &Hydration,
		api: &RA,
		money_market: &mut MoneyMarket,
		borrower: &mut Borrower,
		block: B::Hash,
		block_number: BlockNumber,
		now: Timestamp,
		//Vec of indexes of changed reserves of `Vec<MoneyMorket.reserves.idx>`
		changed_reserves: Option<Vec<usize>>,
	) {
		let timer = Instant::now();

		//TODO: check if user is blacklisted from previous liquidation. It's ok to hold outdated borrower's data
		//while borrower can't be liquidated so nothing to do

		let log_prefix = self.cfg.log_prefix.as_str();
		if borrower.updated_at != block_number {
			let Some(b) = hydration.fetch_borrower(api, block, block_number, money_market, borrower.address, now)
			else {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): failed to fetch borrower's data, skipping, borrower: {:?}, duration: {:?}", log_prefix, borrower.address, timer.elapsed().as_nanos());
				return;
			};

			*borrower = b;
		}

		if let Some(cr) = changed_reserves {
			if !borrower.has_reseserve(cr) {
				//Borrower has no changed reserve so nothing to do
				return;
			}
		}

		if borrower.total_collateral < self.cfg.min_collaterall {
			log::info!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): borrowers collateral is less than min. collateral, skipping, borrower: {:?}, borrower_collateral: {:?}, duration: {:?}", log_prefix, borrower.address, borrower.total_collateral, timer.elapsed().as_nanos());
			return;
		}

		let Ok(hf) = borrower.calc_health_factor(money_market).inspect_err(|e| {
			log::error!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): failed to calc borrower's health factor, skipping, borrower: {:?}, err: {:?}, duration: {:?}", log_prefix, borrower.address,e, timer.elapsed().as_nanos());
		}) else { return; };

		if hf.is_zero() {
			log::info!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): skipping borrower with 0 health factor, borrower: {:?}, duration: {:?}", log_prefix, borrower.address, timer.elapsed().as_nanos());
		}

		if hf >= U256::from(ONE_HF) {
			log::info!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): healthy borrower, borrower: {:?}, duration: {:?}", log_prefix, borrower.address, timer.elapsed().as_nanos());
			return;
		}

		let target_hf = self.cfg.target_hf.into();
		let liq_option = match money_market.calc_best_liquidation_option_for(&borrower, target_hf, log_prefix) {
			Ok(Some(opt)) => opt,
			Ok(None) => {
				log::info!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): no liquidation option found, borrower: {:?}, health_factor: {:?}, duration: {:?}", log_prefix, borrower.address, hf, timer.elapsed().as_nanos());
				return;
			}
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTask.process_borrower(): failed to calc. liquidation option, borrower: {:?}, health_factor: {:?}, err: {:?}, duration: {:?}", log_prefix, borrower.address, hf, e, timer.elapsed().as_nanos());
				return;
			}
		};

		let priority = borrower
			.total_collateral
			.checked_div(ONE_BASE.into())
			.unwrap_or(Zero::zero());
		//submit liquidation
	}
}

impl<C: RuntimeClient<B>, B: Block> LiquidationTask<C, B> {
	/// Function returns all events from `system.events` storage at `block`
	pub fn load_events(&self, block: B::Hash) -> Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>> {
		let timer = Instant::now();
		log::info!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): fetching events from storage", self.cfg.log_prefix);

		let events = match self.client.storage(block, &self.system_events_key) {
			Ok(Some(events)) => events,
			Ok(None) => {
				log::info!(target: LOG_TARGET, "{:?}.LiquidationTaks.load_events(): finished, stroage treturned no data, elapsed: {:?}", self.cfg.log_prefix, timer.elapsed().as_nanos());
				return Vec::new();
			}
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): failed to load events from storage. err: {:?}, elapsed: {:?}", self.cfg.log_prefix, e, timer.elapsed().as_nanos());
				return Vec::new();
			}
		};

		let events = match Vec::<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>::decode(&mut events.0.as_slice()) {
			Ok(events) => events,
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): failed to decode stroage item, err: {:?}, elapsed: {:?}", self.cfg.log_prefix, e, timer.elapsed().as_nanos());
				Vec::new()
			}
		};

		log::info!(target: LOG_TARGET, "{:?}.LiquidationTaks.load_events(): finished loading {:?} events, elapsed: {:?}", self.cfg.log_prefix, events.len(), timer.elapsed().as_nanos());
		events
	}
}

/// Function fetches and returns list of borrowes' addresses from provided `url`.
/// Returned list is not deduped nor sorted in any way.
async fn fetch_borrowers_list(https: &https::Client, url: Uri, log_prefix: &str) -> Option<Vec<EvmAddress>> {
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{:?} fetch_borrowers(): fetching borrowers list from external source", log_prefix);

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

// Function iterates over `events` and returns list of new borrowers
pub(crate) fn process_events(
	events: Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>,
	log_prefix: &str,
) -> Vec<EvmAddress> {
	let timer = Instant::now();
	log::trace!(target: LOG_TARGET, "{:?} process_events(): processing {:?} events", log_prefix, events.len());

	let mut borrowers: Vec<EvmAddress> = Vec::with_capacity(20);
	for evt in &events {
		let RuntimeEvent::EVM(pallet_evm::Event::Log { log }) = &evt.event else {
			continue;
		};

		if log.address == contracts::BORROW_CALL && log.topics.first() == Some(&events::BORROW) {
			let Some(&borrower) = log.topics.get(2) else {
				continue;
			};

			borrowers.push(borrower.into());
		}
	}

	log::info!(target: LOG_TARGET, "{:?} process_events(): finished, elapsed={:?}", log_prefix, timer.elapsed().as_nanos());
	borrowers
}

/// Function checks if transactionsis dia's oracle update transactiona and return `Transaction` or
/// `None`
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
	log::trace!(target: LOG_TARGET, "{:?} is_oracle_update_tx()", log_prefix);

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
	return Some(transaction);
}

pub async fn run<C, B, CL>(task: LiquidationTask<C, B>, client: Arc<CL>)
where
	CL: BlockchainEvents<B> + 'static,
	CL: HeaderBackend<B>,
	CL: ProvideRuntimeApi<B>,
	CL::Api: EthereumRuntimeRPCApi<B> + Erc20MappingApi<B> + CurrenciesApi<B, AssetId, AccountId, Balance>,

	B: hydradx_runtime::BlockT,
	<B as Block>::Extrinsic: From<OpaqueExtrinsic>,
{
	let mut blocks_stream = client.import_notification_stream();
	let who = H160(hex!["288e0dbd476cbfc7dfc1268c00b9e5081e9d9b1a"]);

	let borrowers = fetch_borrowers_list(&task.https, task.url, task.cfg.log_prefix.as_str())
		.await
		.expect("fetch borrowers list to work");

	loop {
		tokio::select! {
			Some(b) = blocks_stream.next() => {
				if !b.is_new_best {
					log::info!(target: LOG_TARGET, "{:?}: run() non-canon block", task.cfg.log_prefix);
					continue;
				}

				let runtime_api = client.runtime_api();
				let api = pepl_worker_support::types::ApiProvider::<&CL::Api>(runtime_api.deref());
				// let mm = pepl_worker_support::fetch_money_market(&api, b.hash, contracts::POOL_ADDRESS_PROVIDER, contracts::RUNTIME_API_CALLER).expect("fetch MM to work");

				let now = runtime_api.deref().current_block(b.hash).unwrap().unwrap().header.timestamp.saturating_div(1_000);
				// for addr in &borrowers {
				// let u =	pepl_worker_support::fetch_borrower(&api, b.hash, &mm, who, contracts::RUNTIME_API_CALLER, now).expect("fetch_borrower to work");
				// println!("borrower: {:?}", u);
				// }

				log::debug!(target: LOG_TARGET, "{:?} run() received new best block: {:?}", task.cfg.log_prefix, b.hash);
			}
		}
	}
}
