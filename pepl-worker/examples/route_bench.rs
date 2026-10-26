//! Times the PEPL collateral → debt route search against real chain state, and replays historical
//! `liquidate_with_pool` extrinsics with the route the worker would now attach.
//!
//! ```text
//! cargo run --release -p pepl-worker --example route_bench -- \
//!     --rpc https://rpc.kril.hydration.cloud --block 15552898 \
//!     --case 690:44:200000000000000000000 --replay 15552899:3
//! ```
//!
//! `--block N` routes against the state after block N. The snapshot comes from
//! `LiquidityStateApi_liquidity_state`, or from `IceSolverApi_solver_input` on runtimes that
//! predate it (that one is `None` on blocks without ICE intents). `--case coll:debt:amount_in`
//! times one pair; `--replay B:I` takes extrinsic I of block B, routes it against the state after
//! B-1 and dry-runs both the original call and the routed one at B-1; `--replay B` does that for
//! every `liquidate_with_pool` in block B.

use codec::{Decode, Encode};
use frame_support::dispatch::DispatchResultWithPostInfo;
use hydradx_runtime::RuntimeCall;
use hydradx_traits::amm::AMMInterface;
use pepl_worker::routing::{best_sell_route, decode_liquidity_state, LiquidityState};
use pepl_worker_support::types::{AssetId, Balance};
use std::process::Command;
use std::time::{Duration, Instant};

const ITERATIONS: usize = 50;

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
	let s = v.as_str().expect("hex string");
	hex::decode(s.trim_start_matches("0x")).expect("valid hex")
}

fn block_hash(url: &str, n: u32) -> String {
	rpc(url, "chain_getBlockHash", serde_json::json!([n]))
		.expect("block hash")
		.as_str()
		.expect("hash string")
		.to_string()
}

/// Raw snapshot bytes at `hash` and the API that served them.
fn fetch_state(url: &str, hash: &str) -> (Vec<u8>, &'static str) {
	if let Ok(r) = rpc(
		url,
		"state_call",
		serde_json::json!(["LiquidityStateApi_liquidity_state", "0x", hash]),
	) {
		let ls =
			pallet_ice_runtime_api::LiquidityState::decode(&mut &hex_bytes(&r)[..]).expect("LiquidityState decodes");
		return (ls.state, "LiquidityStateApi");
	}
	let r = rpc(
		url,
		"state_call",
		serde_json::json!(["IceSolverApi_solver_input", "0x", hash]),
	)
	.expect("solver_input");
	let input = Option::<pallet_ice_runtime_api::SolverInput>::decode(&mut &hex_bytes(&r)[..])
		.expect("SolverInput decodes")
		.expect("solver_input is None (no ICE intents at this block) and LiquidityStateApi is not deployed");
	(input.state, "IceSolverApi")
}

struct Stats {
	p50: Duration,
	p99: Duration,
	max: Duration,
}

fn time<T>(mut f: impl FnMut() -> T) -> (T, Stats) {
	let mut samples = Vec::with_capacity(ITERATIONS);
	let mut last = None;
	for _ in 0..ITERATIONS {
		let t = Instant::now();
		last = Some(f());
		samples.push(t.elapsed());
	}
	samples.sort();
	let at = |q: usize| samples[(samples.len() * q / 100).min(samples.len() - 1)];
	(
		last.expect("ITERATIONS > 0"),
		Stats {
			p50: at(50),
			p99: at(99),
			max: *samples.last().expect("ITERATIONS > 0"),
		},
	)
}

fn fmt(s: &Stats) -> String {
	format!("p50 {:>9.3?}  p99 {:>9.3?}  max {:>9.3?}", s.p50, s.p99, s.max)
}

type Simulator = amm_simulator::HydrationSimulator<pepl_worker::routing::PeplSimulatorConfig>;

fn bench_case(state: &LiquidityState, asset_in: AssetId, asset_out: AssetId, amount_in: Balance) {
	let (routes, discover) = time(|| Simulator::discover_routes(asset_in, asset_out, state).unwrap_or_default());
	let (best, full) = time(|| best_sell_route(state, asset_in, asset_out, amount_in));
	println!(
		"  case {asset_in} -> {asset_out}, amount_in {amount_in}: {} candidate route(s)",
		routes.len()
	);
	println!("    discover routes   {}", fmt(&discover));
	println!("    discover + price  {}", fmt(&full));
	for route in &routes {
		let hops: Vec<String> = route
			.iter()
			.map(|t| format!("{:?}:{}->{}", t.pool, t.asset_in, t.asset_out))
			.collect();
		match Simulator::sell(asset_in, asset_out, amount_in, route.clone(), state) {
			Ok((_, exec)) => println!("      out {:>40}  {}", exec.amount_out, hops.join(" ")),
			Err(e) => println!("      {:>44}  {}", format!("{e:?}"), hops.join(" ")),
		}
	}
	match best {
		Some((route, out)) => println!("    best: out {out} ({} hops)", route.len()),
		None => println!("    best: NO ROUTE"),
	}
}

/// Collateral needed to buy `debt` on the cheapest candidate route, plus a 5% liquidation bonus.
/// The worker gets the exact amount from the money market; the replay only has the call.
fn estimate_collateral(state: &LiquidityState, coll: AssetId, debt: AssetId, debt_amount: Balance) -> Option<Balance> {
	Simulator::discover_routes(coll, debt, state)
		.ok()?
		.into_iter()
		.filter_map(|r| Simulator::buy(coll, debt, debt_amount, r, state).ok())
		.map(|exec| exec.1.amount_in)
		.min()
		.map(|amount| amount.saturating_mul(105) / 100)
}

fn dry_run(url: &str, hash: &str, call: &RuntimeCall) -> String {
	let origin = hydradx_runtime::OriginCaller::system(frame_system::RawOrigin::None);
	let params = (origin, call, 5u32).encode();
	let r = match rpc(
		url,
		"state_call",
		serde_json::json!(["DryRunApi_dry_run_call", format!("0x{}", hex::encode(params)), hash]),
	) {
		Ok(r) => r,
		Err(e) => return format!("dry run failed: {e}"),
	};
	// Only the leading `execution_result` is decoded: the event list that follows is encoded with
	// the historical runtime's types.
	match Result::<DispatchResultWithPostInfo, xcm_runtime_apis::dry_run::Error>::decode(&mut &hex_bytes(&r)[..]) {
		Ok(Ok(Ok(_))) => "WouldSucceed".to_string(),
		Ok(Ok(Err(e))) => format!("WouldFail({:?})", e.error),
		Ok(Err(e)) => format!("dry run error: {e:?}"),
		Err(e) => format!("undecodable dry run result: {e}"),
	}
}

fn replay(url: &str, tx_block: u32, index: Option<usize>) {
	let block = rpc(url, "chain_getBlock", serde_json::json!([block_hash(url, tx_block)])).expect("block");
	let extrinsics = block["block"]["extrinsics"].as_array().expect("extrinsics").clone();
	for (i, raw) in extrinsics.iter().enumerate() {
		if index.is_some_and(|index| index != i) {
			continue;
		}
		let Ok(xt) = hydradx_runtime::HydraUncheckedExtrinsic::decode(&mut &hex_bytes(raw)[..]) else {
			println!("\n{tx_block}-{i}: does not decode with this binary's types");
			continue;
		};
		if index.is_some() || matches!(xt.0.function, RuntimeCall::Liquidation(_)) {
			println!("\nreplay {tx_block}-{i}");
			replay_call(url, tx_block, xt.0.function);
		}
	}
}

fn replay_call(url: &str, tx_block: u32, call: RuntimeCall) {
	let RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_with_pool {
		pool,
		collateral_asset,
		debt_asset,
		user,
		debt_to_cover,
		route,
		unsigned_priority,
	}) = call.clone()
	else {
		println!("  not a liquidate_with_pool call: {call:?}");
		return;
	};
	println!(
		"  {collateral_asset} -> {debt_asset}, user {user:?}, debt_to_cover {debt_to_cover}, original route {:?}",
		route.to_vec()
	);

	let parent = block_hash(url, tx_block - 1);
	let (raw_state, api) = fetch_state(url, &parent);
	let Some(state) = decode_liquidity_state(&raw_state, "route_bench") else {
		println!("  snapshot from {api} does not decode with this binary's types");
		return;
	};
	let Some(amount_in) = estimate_collateral(&state, collateral_asset, debt_asset, debt_to_cover) else {
		println!("  no route {collateral_asset} -> {debt_asset} in the snapshot");
		return;
	};
	bench_case(&state, collateral_asset, debt_asset, amount_in);
	let Some((found, out)) = best_sell_route(&state, collateral_asset, debt_asset, amount_in) else {
		return;
	};
	println!("  simulated out {out} vs debt_to_cover {debt_to_cover}");

	let routed = RuntimeCall::Liquidation(pallet_liquidation::Call::liquidate_with_pool {
		pool,
		collateral_asset,
		debt_asset,
		user,
		debt_to_cover,
		route: found,
		unsigned_priority,
	});
	println!(
		"  dry run at {}, original call: {}",
		tx_block - 1,
		dry_run(url, &parent, &call)
	);
	println!(
		"  dry run at {}, routed call:   {}",
		tx_block - 1,
		dry_run(url, &parent, &routed)
	);
}

fn main() {
	let args: Vec<String> = std::env::args().skip(1).collect();
	let arg = |name: &str| -> Vec<String> { args.windows(2).filter(|w| w[0] == name).map(|w| w[1].clone()).collect() };
	let url = arg("--rpc")
		.pop()
		.unwrap_or_else(|| "https://rpc.kril.hydration.cloud".to_string());

	for n in arg("--block") {
		let n: u32 = n.parse().expect("--block N");
		let hash = block_hash(&url, n);
		let t = Instant::now();
		let (raw, api) = fetch_state(&url, &hash);
		println!(
			"\nblock {n} ({hash}): {} bytes via {api}, rpc fetch {:?}",
			raw.len(),
			t.elapsed()
		);
		let (state, decode) = time(|| decode_liquidity_state(&raw, "route_bench"));
		let Some(state) = state else {
			println!("  snapshot does not decode with this binary's types");
			continue;
		};
		println!("  decode            {}", fmt(&decode));
		let (edges, edges_t) = time(|| Simulator::pool_edges(&state));
		println!("  pool_edges        {}  ({} edges)", fmt(&edges_t), edges.len());
		for case in arg("--case") {
			let parts: Vec<&str> = case.split(':').collect();
			let [coll, debt, amount] = parts[..] else {
				panic!("--case coll:debt:amount_in, got {case}");
			};
			bench_case(
				&state,
				coll.parse().expect("collateral asset id"),
				debt.parse().expect("debt asset id"),
				amount.parse().expect("amount_in"),
			);
		}
	}

	for r in arg("--replay") {
		let (b, i) = r.split_once(':').map_or((r.as_str(), None), |(b, i)| (b, Some(i)));
		replay(
			&url,
			b.parse().expect("block"),
			i.map(|i| i.parse().expect("extrinsic index")),
		);
	}
}
