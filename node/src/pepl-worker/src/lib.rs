use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use fp_self_contained::SelfContainedCall;
use frame_system::EventRecord;
use hex_literal::hex;
use hydradx_runtime::RuntimeCall;
use hydradx_runtime::RuntimeEvent;
use hyper::{body::Body, Client, StatusCode, Uri};
use hyper_rustls::HttpsConnector;
use hyperv14 as hyper;
use pallet_ethereum::Transaction;
use primitives::EvmAddress;
use sc_client_api::Backend;
use sc_client_api::StorageKey;
use sc_client_api::StorageProvider;
use serde::Deserialize;
use sp_api::ProvideRuntimeApi;
use sp_core::H256;
use std::time::Instant;
use std::{marker::PhantomData, sync::Arc};

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "liquidation-worker";
const LOG_PREFIX: &str = "PEPL:";

// Target healt factor after liquidation
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

// URL of serve to fetch borrowers list
const OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

// Number of liquidation trasactions submited per 1 block
const LIQUIDATIONS_PER_BLOCK: u8 = 20;

// Contracts' addresses
mod contracts {
	use super::*;
	use sp_core::H160;

	pub const POOL_CONFIGURATOR: EvmAddress = H160(hex!("e64c38e2fa00dfe4f1d0b92f75b8e44ebdf292e4"));
	pub const POOL_PROVIDER: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));
	pub const BORROW_CALL: EvmAddress = H160(hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38"));
}

mod events {
	use super::*;

	pub const BORROW: H256 = H256(hex!("b3d084820fb1a9decffb176436bd02558d15fac9b0ddfed8c465bc7359d7dce0"));
	pub const COLLATERAL_CONFIGURATION_CHANGED: H256 =
		H256(hex!("637febbda9275aea2e85c0ff690444c8d87eb2e8339bbede9715abcc89cb0995"));
}

type HttpsClient = Client<HttpsConnector<hyper::client::HttpConnector>, Body>;

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

mod storage_key {
	use super::*;

	pub const SYSTEM_EVENTS: [u8; 32] = hex!("26aa394eea5630e07c48ae0c9558cef780d41e5e16056765bc8461851072c9d7");
}

/// The configuration for the liquidation worker.
#[derive(Clone, Debug, clap::Parser)]
pub struct LiquidationWorkerConfig {
	/// Enable/disable liquidation worker.
	#[clap(long, default_value = "true")]
	pub liquidation_worker: bool,

	/// Address of the Pool Address Provider contract.
	#[clap(long)]
	pub pool_address_provider: Option<EvmAddress>,

	/// EVM address of the account that calls Runtime API. Account needs to have WETH balance.
	#[clap(long)]
	pub api_caller: Option<EvmAddress>,

	/// EVM address of the account that signs DIA oracle update.
	#[clap(long)]
	pub oracle_signer: Option<Vec<EvmAddress>>,

	/// EVM address of the DIA oracle update call address.
	#[clap(long)]
	pub oracle_call_address: Option<Vec<EvmAddress>>,

	/// Target health factor.
	#[clap(long, default_value_t = TARGET_HF)]
	pub target_hf: u128,

	/// URL to fetch list of borrowers.
	#[clap(long, default_value = OMNIWATCH_URL)]
	pub omniwatch_url: String,

	/// Number of liquidation transaction submitted per block.
	#[clap(long, default_value_t = LIQUIDATIONS_PER_BLOCK)]
	pub liquidations_per_block: u8,
}

pub struct LiquidationTask<B, RA> {
	client: Arc<RA>,
	https: HttpsClient,
	url: Uri,
	system_events_key: StorageKey,
	_phantom: PhantomData<B>,
}

impl<Block, C> LiquidationTask<Block, C>
where
	Block: BlockT,
	C: ProvideRuntimeApi<Block>,
{
	fn new(client: Arc<C>, cfg: LiquidationWorkerConfig) -> Self {
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_webpki_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();
		let https_client = Client::builder().build(https);

		//IMO ok to panic here, collator should fix URL or disable liquidation worker
		let url = cfg
			.omniwatch_url
			.parse()
			.expect("LiquidationTaks: failed to parse provide omniwatch_url");

		Self {
			url,
			client,
			https: https_client,
			system_events_key: StorageKey(storage_key::SYSTEM_EVENTS.to_vec()),
			_phantom: PhantomData,
		}
	}
}

impl<B, RA> LiquidationTask<B, RA> {
	// Function loads initial data and starts PEPL liquidation worker
	pub async fn run(&self) {
		todo!()
	}
}

impl<B, RA> LiquidationTask<B, RA> {
	/// Function returns all events from `system.events` storage at `block`
	fn load_events<C, Block, BE>(
		&self,
		client: Arc<C>,
		block: Block::Hash,
	) -> Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>
	where
		Block: BlockT,
		C: StorageProvider<Block, BE>,
		BE: Backend<Block> + 'static,
	{
		let now = Instant::now();
		log::info!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): fetching events from storage", LOG_PREFIX);

		let events = match client.storage(block, &self.system_events_key) {
			Ok(Some(events)) => events,
			Ok(None) => {
				log::info!(target: LOG_TARGET, "{:?}.LiquidationTaks.load_events(): finished, stroage treturned no data, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
				return Vec::new();
			}
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): failed to load events from storage. err: {:?}, exec_time: {:?}", LOG_PREFIX, e, now.elapsed().as_nanos());
				return Vec::new();
			}
		};

		let events = match Vec::<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>::decode(&mut events.0.as_slice()) {
			Ok(events) => events,
			Err(e) => {
				log::error!(target: LOG_TARGET, "{:?} LiquidationTaks.load_events(): failed to decode stroage item, err: {:?}, exec_time: {:?}", LOG_PREFIX, e, now.elapsed().as_nanos());
				Vec::new()
			}
		};

		log::info!(target: LOG_TARGET, "{:?}.LiquidationTaks.load_events(): finished loading {:?} events, exec_time: {:?}", LOG_PREFIX, events.len(), now.elapsed().as_nanos());
		events
	}
}

// impl LiquidationTask {
// 	fn find_unhlealty(&self, borrowers HashMap<EvmAddress, User) -> Vec<EvmAddress> {
// 		let unhealthy = Vec<EvmAddress>
//
// 	}
// }
// //TODO: this should return Option<Vec<borrower, debt_to_liquidated>>

/// Function fetches and returns list of borrowes' addresses from provided `url`.
/// Returned list is not deduped nor sorted in any way.
async fn fetch_borrowers_list(https: &HttpsClient, url: Uri) -> Option<Vec<EvmAddress>> {
	let now = Instant::now();
	log::trace!(target: LOG_TARGET, "{:?} fetch_borrowers(): fetching borrowers list from external source", LOG_PREFIX);

	let res = match https.get(url).await {
		Ok(res) if res.status() == StatusCode::OK => res,
		Ok(res) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to fetch borrowers data, exec_time: {:?}, status_code: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), res.status());
			return None;
		}
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to fetch borrowers data, exec_time: {:?}, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let bytes = match hyper::body::to_bytes(res.into_body()).await {
		Ok(bytes) => bytes,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers(): failed to load response data, exec_time: {:?}, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match String::from_utf8(bytes.to_vec()) {
		Ok(s) => s,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers():, failed to parse returned data as utf8 string, exec_time: {:?}, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match serde_json::from_str::<omniwatch::ByHealthRes>(data.as_str()) {
		Ok(d) => d,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?} fetch_borrowers():, failed to deserialize response data, exec_time: {:?}, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	// NOTE: this is 2+ times faster than more concise way to do it based on my testing
	let mut b = Vec::<EvmAddress>::with_capacity(data.borrowers.len());
	for (addr, _) in &data.borrowers {
		b.push(*addr);
	}

	log::info!(target: LOG_TARGET, "{:?} fetch_borrowers(): finished fetching {:?} borrowers exec_time: {:?}", LOG_PREFIX, b.len(), now.elapsed().as_nanos());
	Some(b)
}

// Function iterates over `events` and returns list of new borrowers
fn process_events(events: Vec<EventRecord<RuntimeEvent, hydradx_runtime::Hash>>) -> Vec<EvmAddress> {
	let now = Instant::now();
	log::info!(target: LOG_TARGET, "{:?} process_events(): processing {:?} events", LOG_PREFIX, events.len());

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

	log::info!(target: LOG_TARGET, "{:?} process_events(): finished, exec_time={:?}", LOG_PREFIX, now.elapsed().as_nanos());
	borrowers
}

/// Function checks if transactionsis dia's oracle update transactiona and return `Transaction` or
/// `None`
fn is_oracle_update_tx(
	extrinsic: &sp_runtime::generic::UncheckedExtrinsic<
		hydradx_runtime::Address,
		RuntimeCall,
		hydradx_runtime::Signature,
		hydradx_runtime::SignedExtra,
	>,
	allowed_signers: Vec<EvmAddress>,
	allowed_callers: Vec<EvmAddress>,
) -> Option<Transaction> {
	let now = Instant::now();
	log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx()", LOG_PREFIX);

	let RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) = extrinsic.function.clone() else {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, non evm transaction, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
		return None;
	};

	let action = match transaction {
		Transaction::Legacy(ref legacy_transaction) => legacy_transaction.action,
		Transaction::EIP2930(ref eip2930_transaction) => eip2930_transaction.action,
		Transaction::EIP1559(ref eip1559_transaction) => eip1559_transaction.action,
		Transaction::EIP7702(_) => {
			log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, unsupported EIP7702 tx, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
			return None;
		}
	};

	let pallet_ethereum::TransactionAction::Call(caller) = action else {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, no caller, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
		return None;
	};

	// check if the transaction is DIA oracle update
	if !allowed_callers.contains(&caller) {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, caller is not allowed, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
		return None;
	}

	// additional check to prevent running the worker for DIA oracle updates signed by invalid address
	let Some(Ok(signer)) = extrinsic.function.check_self_contained() else {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not self contained, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
		return None;
	};

	if !allowed_signers.contains(&signer) {
		log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, signer is not allowed, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
		return None;
	}

	log::info!(target: LOG_TARGET, "{:?} is_oracle_update_tx() finished, exec_time: {:?}", LOG_PREFIX, now.elapsed().as_nanos());
	return Some(transaction);
}
