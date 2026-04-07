use codec::{Decode, Encode};
use cumulus_primitives_core::BlockT;
use hyper::{body::Body, Client, StatusCode, Uri};
use hyper_rustls::HttpsConnector;
use hyperv14 as hyper;
use primitives::EvmAddress;
use serde::Deserialize;
use sp_api::ProvideRuntimeApi;
use std::time::Instant;
use std::{marker::PhantomData, sync::Arc};

#[cfg(test)]
mod tests;

const LOG_TARGET: &str = "liquidation-worker";
const LOG_PREFIX: &str = "PEPL";

// Target healt factor after liquidation
const TARGET_HF: u128 = 1_001_000_000_000_000_000u128; // 1.001

// URL of serve to fetch borrowers list
const OMNIWATCH_URL: &str = "https://omniwatch.play.hydration.cloud/api/borrowers/by-health";

// Number of liquidation trasactions submited per 1 block
const LIQUIDATIONS_PER_BLOCK: u8 = 20;

// Contracts' addresses
mod contracts {
	use super::*;
	use hex_literal::hex;
	use sp_core::H160;

	pub const POOL_ADDRESS_PROVIDER: EvmAddress = H160(hex!("f3ba4d1b50f78301bdd7eaea9b67822a15fca691"));
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
	runtime_api: Arc<RA>,
	https: HttpsClient,
	url: Uri,
	_phantom: PhantomData<B>,
}

impl<B, RA> LiquidationTask<B, RA>
where
	B: BlockT,
	RA: ProvideRuntimeApi<B>,
{
	fn new(client: Arc<RA>, cfg: LiquidationWorkerConfig) -> Self {
		let https = hyper_rustls::HttpsConnectorBuilder::new()
			.with_webpki_roots()
			.https_or_http()
			.enable_http1()
			.enable_http2()
			.build();
		let https_client = Client::builder().build(https);

		//This is IMO ok, collator should fix URL or disable liquidation worker
		let url = cfg
			.omniwatch_url
			.parse()
			.expect("LiquidationTaks: failed to parse provide omniwatch_url");

		Self {
			url,
			runtime_api: client,
			https: https_client,
			_phantom: PhantomData,
		}
	}
}

impl<B, RA> LiquidationTask<B, RA> {
	// Function loads initial data and starts PEPL liquidation worker
	pub async fn run() {
		todo!()
	}
}

impl<B, RA> LiquidationTask<B, RA> {
	// fn find_unhlealty(&self, borrowers: Vec<&mut UserDa)
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
	log::trace!(target: LOG_TARGET, "{:?}.fetch_borrowers(): fetching borrowers list from external source", LOG_PREFIX);
	let now = Instant::now();

	let res = match https.get(url).await {
		Ok(res) if res.status() == StatusCode::OK => res,
		Ok(res) => {
			log::error!(target: LOG_TARGET, "{:?}.fetch_borrowers(): failed to fetch borrowers data, exec_time: {:?}ns, status_code: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), res.status());
			return None;
		}
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?}.fetch_borrowers(): failed to fetch borrowers data, exec_time: {:?}ns, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let bytes = match hyper::body::to_bytes(res.into_body()).await {
		Ok(bytes) => bytes,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?}.fetch_borrowers(): failed to load response data, exec_time: {:?}ns, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match String::from_utf8(bytes.to_vec()) {
		Ok(s) => s,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?}.fetch_borrowers():, failed to parse returned data as utf8 string, exec_time: {:?}ns, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	let data = match serde_json::from_str::<omniwatch::ByHealthRes>(data.as_str()) {
		Ok(d) => d,
		Err(e) => {
			log::error!(target: LOG_TARGET, "{:?}.fetch_borrowers():, failed to deserialize response data, exec_time: {:?}ns, err: {:?}", LOG_PREFIX, now.elapsed().as_nanos(), e);
			return None;
		}
	};

	// NOTE: this is 2+ times faster than more concise way based on my testing
	let mut b = Vec::<EvmAddress>::with_capacity(data.borrowers.len());
	for (addr, _) in &data.borrowers {
		b.push(*addr);
	}

	log::info!(target: LOG_TARGET, "{:?}.fetch_borrowers(): finished fetching {:?} borrowers exec_time: {:?}ns", LOG_PREFIX, b.len(), now.elapsed().as_nanos());
	Some(b)
}
