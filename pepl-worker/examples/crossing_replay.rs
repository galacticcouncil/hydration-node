//! Replays the scan decision for one borrower against real chain state, at the block's own
//! timestamp and at the timestamp the next block executes at (`next_block_timestamp`), and prints
//! the unsigned `liquidate_with_pool` extrinsic the worker would submit.
//!
//! ```text
//! cargo run -p pepl-worker --example crossing_replay -- \
//!     --rpc <url> --block <number> \
//!     --borrower <evm address>
//! ```

use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResultWithPostInfo;
use hydradx_runtime::RuntimeCall;
use pepl_worker::{
	contracts, decide_liquidation_options, encode_liquidation_opaque, liquidation_call, next_block_timestamp,
	routing::{decode_liquidity_state, route_decision},
	InstanceKind, LiquidationTaskConfig,
};
use pepl_worker_support::traits::{RuntimeApiErr, RuntimeApiProvider};
use pepl_worker_support::types::{AssetId, Balance, Timestamp};
use pepl_worker_support::Hydration;
use primitives::EvmAddress;
use sp_core::{H256, U256};
use std::process::Command;

type Block = hydradx_runtime::Block;

fn rpc(url: &str, method: &str, params: serde_json::Value) -> Result<serde_json::Value, String> {
	let body = serde_json::json!({ "id": 1, "jsonrpc": "2.0", "method": method, "params": params }).to_string();
	let out = Command::new("curl")
		.args([
			"-s",
			"-m",
			"300",
			"-H",
			"content-type: application/json",
			"-d",
			&body,
			url,
		])
		.output()
		.map_err(|e| format!("curl: {e}"))?;
	let res: serde_json::Value =
		serde_json::from_slice(&out.stdout).map_err(|e| format!("{method}: bad response: {e}"))?;
	match res.get("result") {
		Some(r) if !r.is_null() => Ok(r.clone()),
		_ => Err(format!("{method}: {}", res.get("error").cloned().unwrap_or_default())),
	}
}

fn hex_bytes(v: &serde_json::Value) -> Vec<u8> {
	hex::decode(v.as_str().expect("hex string").trim_start_matches("0x")).expect("valid hex")
}

fn block_hash(url: &str, n: u32) -> H256 {
	let r = rpc(url, "chain_getBlockHash", serde_json::json!([n])).expect("block hash");
	H256::from_slice(&hex_bytes(&r))
}

fn state_call(url: &str, method: &str, args: Vec<u8>, at: H256) -> Result<Vec<u8>, String> {
	rpc(
		url,
		"state_call",
		serde_json::json!([method, format!("0x{}", hex::encode(args)), format!("{at:?}")]),
	)
	.map(|r| hex_bytes(&r))
}

struct RemoteApi<'a>(&'a str);

impl RuntimeApiProvider<Block> for RemoteApi<'_> {
	fn call(
		&self,
		block: H256,
		from: EvmAddress,
		to: EvmAddress,
		data: Vec<u8>,
		gas_limit: U256,
	) -> Result<fp_evm::ExecutionInfoV2<Vec<u8>>, RuntimeApiErr> {
		let args = (
			from,
			to,
			data,
			U256::zero(),
			gas_limit,
			None::<U256>,
			None::<U256>,
			None::<U256>,
			false,
			None::<Vec<(EvmAddress, Vec<H256>)>>,
			None::<Vec<u8>>,
		)
			.encode();
		let raw = state_call(self.0, "EthereumRuntimeRPCApi_call", args, block)
			.unwrap_or_else(|e| panic!("EthereumRuntimeRPCApi_call: {e}"));
		Result::<fp_evm::ExecutionInfoV2<Vec<u8>>, sp_runtime::DispatchError>::decode(&mut &raw[..])
			.expect("ExecutionInfoV2 decodes")
			.map_err(RuntimeApiErr::from)
	}

	fn address_to_asset(&self, block: H256, address: EvmAddress) -> Result<Option<AssetId>, RuntimeApiErr> {
		let raw = state_call(self.0, "Erc20MappingApi_address_to_asset", address.encode(), block)
			.unwrap_or_else(|e| panic!("address_to_asset: {e}"));
		Ok(Option::<AssetId>::decode(&mut &raw[..]).expect("Option<AssetId> decodes"))
	}

	fn minimum_balance(&self, block: H256, asset_id: AssetId) -> Result<Balance, RuntimeApiErr> {
		let raw = state_call(self.0, "CurrenciesApi_minimum_balance", asset_id.encode(), block)
			.unwrap_or_else(|e| panic!("minimum_balance: {e}"));
		Ok(Balance::decode(&mut &raw[..]).expect("Balance decodes"))
	}

	fn timestamp(&self, block: H256) -> Option<Timestamp> {
		// twox128("Timestamp") ++ twox128("Now")
		let key = "0xf0c365c3cf59d671eb72da0e7a4113c49f1f0515f462cdcf84e0f1d6045dfcbb";
		let r = rpc(
			self.0,
			"state_getStorage",
			serde_json::json!([key, format!("{block:?}")]),
		)
		.ok()?;
		u64::decode(&mut &hex_bytes(&r)[..]).ok().map(|ms| ms / 1_000)
	}
}

fn dry_run(url: &str, at: H256, call: &RuntimeCall) -> String {
	let origin = hydradx_runtime::OriginCaller::system(frame_system::RawOrigin::None);
	let raw = match state_call(url, "DryRunApi_dry_run_call", (origin, call, 5u32).encode(), at) {
		Ok(r) => r,
		Err(e) => return format!("dry run failed: {e}"),
	};
	match Result::<DispatchResultWithPostInfo, xcm_runtime_apis::dry_run::Error>::decode(&mut &raw[..]) {
		Ok(Ok(Ok(_))) => "WouldSucceed".to_string(),
		Ok(Ok(Err(e))) => format!("WouldFail({:?})", e.error),
		Ok(Err(e)) => format!("dry run error: {e:?}"),
		Err(e) => format!("undecodable dry run result: {e}"),
	}
}

fn arg(args: &[String], name: &str) -> String {
	args.iter()
		.position(|a| a == name)
		.and_then(|i| args.get(i + 1))
		.unwrap_or_else(|| panic!("missing {name}"))
		.clone()
}

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let url = arg(&args, "--rpc");
	let block: u32 = arg(&args, "--block").parse().expect("--block is a number");
	let borrower: EvmAddress = arg(&args, "--borrower").parse().expect("--borrower is an H160");

	let api = RemoteApi(&url);
	let cfg = LiquidationTaskConfig::default();
	let hash = block_hash(&url, block);
	let now = api.timestamp(hash).expect("timestamp");
	let blocks_at_now = 1
		+ (1..=2)
			.take_while(|back| api.timestamp(block_hash(&url, block - back)) == Some(now))
			.count() as u32;
	let exec_now = next_block_timestamp(now, blocks_at_now);
	println!("state after block {block} ({hash:?}), timestamp {now}, {blocks_at_now} block(s) at it");
	println!("next block executes at {exec_now}\n");

	let hydration = Hydration::new(cfg.api_caller, contracts::POOL_ADDRESS_PROVIDER, "replay");
	let mm = hydration
		.fetch_money_market::<Block, _>(&api, hash)
		.expect("money market");

	let mut decisions = Vec::new();
	for (label, t) in [
		("before (block's own timestamp)", now),
		("after (next block's timestamp)", exec_now),
	] {
		let b = hydration
			.fetch_borrower::<Block, _>(&api, hash, block, &mm, borrower, t)
			.expect("borrower");
		let hf = b.calc_health_factor(&mm).expect("health factor");
		decisions = decide_liquidation_options(&cfg, &mm, &b);
		println!(
			"{label}: t={t} collateral {} debt {} HF {hf} -> {} decision(s)",
			b.total_collateral,
			b.total_debt,
			decisions.len()
		);
	}

	let raw = state_call(&url, "LiquidityStateApi_liquidity_state", Vec::new(), hash).expect("liquidity state");
	let ls = pallet_ice_runtime_api::LiquidityState::decode(&mut &raw[..]).expect("LiquidityState decodes");
	let state = decode_liquidity_state(&ls.state, "replay").expect("snapshot decodes");

	for d in decisions {
		let Ok(d) = route_decision(InstanceKind::Generic, Some(&state), d, "replay") else {
			println!("\nno route for option");
			continue;
		};
		let call = liquidation_call(&d, mm.pool);
		println!(
			"\nPEPL submits: {} -> {}, debt_to_cover {}, collateral {}, priority {}, route {:?}",
			d.collateral_asset, d.debt_asset, d.debt_to_cover, d.collateral_amount, d.priority, d.route
		);
		println!("dry run on block {block}'s state: {}", dry_run(&url, hash, &call));
		let xt = encode_liquidation_opaque(call, "replay").expect("encodes");
		println!("extrinsic 0x{}", hex::encode(xt.encode()));
		break;
	}
}
