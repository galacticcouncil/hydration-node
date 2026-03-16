//! RPC-backed RuntimeApiProvider for standalone mode.
//!
//! Implements `RuntimeApiProvider` by translating calls to JSON-RPC:
//! - `call()` → `eth_call`
//! - `current_timestamp()` → `eth_getBlockByNumber`
//! - `address_to_asset()` → `state_call("Erc20MappingApi_address_to_asset")`
//! - `minimum_balance()` → `state_call("CurrenciesApi_minimum_balance")`
//! - `dry_run_call()` → not supported (returns error)

use crate::standalone::types::*;
use ethabi::ethereum_types::U256;
use jsonrpsee_core::client::ClientT;
use jsonrpsee_ws_client::WsClient;
use liquidation_worker_support::RuntimeApiProvider;
use sp_core::H160;
use sp_runtime::traits::Block as BlockT;
use sp_runtime::DispatchError;
use std::sync::Arc;

/// Holds the RPC connection and tokio handle for sync→async bridging.
///
/// The `RuntimeApiProvider` trait is implemented on `&RpcState` (a shared reference),
/// which is `Copy` — matching the pattern used by the node's `ApiProvider<&C::Api>`.
pub struct RpcState {
	client: Arc<WsClient>,
	handle: tokio::runtime::Handle,
}

impl RpcState {
	pub fn new(client: Arc<WsClient>, handle: tokio::runtime::Handle) -> Self {
		Self { client, handle }
	}

	pub fn client(&self) -> &Arc<WsClient> {
		&self.client
	}

	pub fn handle(&self) -> &tokio::runtime::Handle {
		&self.handle
	}

	/// Execute `eth_call` against the RPC node.
	async fn eth_call_async(
		client: &WsClient,
		from: H160,
		to: H160,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<Vec<u8>, String> {
		let call_obj = serde_json::json!({
			"from": format!("0x{}", hex::encode(from.as_bytes())),
			"to": format!("0x{}", hex::encode(to.as_bytes())),
			"data": format!("0x{}", hex::encode(&data)),
			"gas": format!("0x{:x}", gas_limit),
		});

		let result: String = client
			.request("eth_call", vec![call_obj, serde_json::json!("latest")])
			.await
			.map_err(|e| format!("eth_call RPC error: {}", e))?;

		hex::decode(result.trim_start_matches("0x")).map_err(|e| format!("hex decode error: {}", e))
	}

	/// Execute `state_call` against the RPC node.
	async fn state_call_async(
		client: &WsClient,
		method: &str,
		data: &[u8],
	) -> Result<Vec<u8>, String> {
		let hex_data = format!("0x{}", hex::encode(data));

		let result: String = client
			.request(
				"state_call",
				vec![
					serde_json::json!(method),
					serde_json::json!(hex_data),
				],
			)
			.await
			.map_err(|e| format!("state_call({}) RPC error: {}", method, e))?;

		hex::decode(result.trim_start_matches("0x")).map_err(|e| format!("hex decode error: {}", e))
	}

	/// Get the current EVM block timestamp (in seconds).
	async fn get_timestamp_async(client: &WsClient) -> Option<u64> {
		let block: serde_json::Value = client
			.request("eth_getBlockByNumber", vec![serde_json::json!("latest"), serde_json::json!(false)])
			.await
			.ok()?;

		let ts_hex = block.get("timestamp")?.as_str()?;
		u64::from_str_radix(ts_hex.trim_start_matches("0x"), 16).ok()
	}
}

impl RuntimeApiProvider<StandaloneBlock, StandaloneOriginCaller, StandaloneRuntimeCall, StandaloneRuntimeEvent>
	for &RpcState
{
	fn current_timestamp(&self, _hash: <StandaloneBlock as BlockT>::Hash) -> Option<u64> {
		let client = self.client.clone();
		self.handle.block_on(RpcState::get_timestamp_async(&client))
	}

	fn call(
		&self,
		_hash: <StandaloneBlock as BlockT>::Hash,
		caller: H160,
		contract: H160,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<Result<fp_evm::ExecutionInfoV2<Vec<u8>>, DispatchError>, sp_api::ApiError> {
		let client = self.client.clone();
		let result =
			self.handle
				.block_on(RpcState::eth_call_async(&client, caller, contract, data, gas_limit));

		match result {
			Ok(bytes) => Ok(Ok(fp_evm::ExecutionInfoV2 {
				exit_reason: evm::ExitReason::Succeed(evm::ExitSucceed::Returned),
				value: bytes,
				used_gas: fp_evm::UsedGas {
					standard: U256::zero(),
					effective: U256::zero(),
				},
				weight_info: None,
				logs: vec![],
			})),
			Err(e) => {
				log::warn!(target: "pepl-worker", "eth_call failed: {}", e);
				// Return as an EVM execution failure rather than API error,
				// so the caller can handle it gracefully.
				Ok(Err(DispatchError::Other("eth_call failed")))
			}
		}
	}

	fn address_to_asset(
		&self,
		_hash: <StandaloneBlock as BlockT>::Hash,
		address: H160,
	) -> Result<Option<u32>, sp_api::ApiError> {
		let client = self.client.clone();
		let result = self.handle.block_on(async {
			// SCALE encode H160: just the raw 20 bytes
			let encoded = address.as_bytes().to_vec();
			RpcState::state_call_async(&client, "Erc20MappingApi_address_to_asset", &encoded).await
		});

		match result {
			Ok(bytes) => {
				// Decode SCALE-encoded Option<u32>: 0x00 = None, 0x01 + 4-byte LE = Some(id)
				if bytes.is_empty() || bytes[0] == 0 {
					Ok(None)
				} else if bytes.len() >= 5 {
					let asset_id = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]);
					Ok(Some(asset_id))
				} else {
					Ok(None)
				}
			}
			Err(e) => {
				log::warn!(target: "pepl-worker", "address_to_asset failed for {:?}: {}", address, e);
				Err(sp_api::ApiError::Application(Box::new(std::io::Error::new(
					std::io::ErrorKind::Other,
					e,
				))))
			}
		}
	}

	fn dry_run_call(
		&self,
		_hash: <StandaloneBlock as BlockT>::Hash,
		_origin: StandaloneOriginCaller,
		_call: StandaloneRuntimeCall,
	) -> Result<
		Result<
			xcm_runtime_apis::dry_run::CallDryRunEffects<StandaloneRuntimeEvent>,
			xcm_runtime_apis::dry_run::Error,
		>,
		sp_api::ApiError,
	> {
		Err(sp_api::ApiError::Application(Box::new(std::io::Error::new(
			std::io::ErrorKind::Unsupported,
			"dry_run_call not available in standalone mode",
		))))
	}

	fn minimum_balance(
		&self,
		_hash: <StandaloneBlock as BlockT>::Hash,
		asset_id: u32,
	) -> Result<u128, sp_api::ApiError> {
		let client = self.client.clone();
		let result = self.handle.block_on(async {
			// SCALE encode u32: 4 bytes little-endian
			let encoded = asset_id.to_le_bytes().to_vec();
			RpcState::state_call_async(&client, "CurrenciesApi_minimum_balance", &encoded).await
		});

		match result {
			Ok(bytes) => {
				// Decode SCALE-encoded Balance (u128): 16 bytes little-endian
				if bytes.len() >= 16 {
					let balance = u128::from_le_bytes(bytes[..16].try_into().unwrap());
					Ok(balance)
				} else {
					// Fallback: use 1 as minimum balance
					log::warn!(target: "pepl-worker", "minimum_balance: unexpected response length {}, using 1", bytes.len());
					Ok(1)
				}
			}
			Err(e) => {
				// Non-fatal: use a safe default
				log::warn!(target: "pepl-worker", "minimum_balance failed for asset {}: {}, using 1", asset_id, e);
				Ok(1)
			}
		}
	}
}
