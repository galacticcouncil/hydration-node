//! Phase-2 milestone: decode ONE mainnet block's events into core `ChainEvent`s and print a
//! summary. Proves the whole dependency stack (runtime types + decode + provider) before any
//! detection logic sits on it.
//!
//! Usage: cargo run -p cerebro-hydration --example decode_block -- [block_number] [rpc_url]

use cerebro_core::model::{ChainEvent, Phase};
use cerebro_hydration::RemoteApi;

fn main() {
	let mut args = std::env::args().skip(1);
	let block: Option<u32> = args.next().and_then(|s| s.parse().ok());
	let rpc = args.next().unwrap_or_else(|| "https://rpc.hydradx.cloud".to_string());

	let api = RemoteApi::new(rpc.clone());
	let (hash, number) = match block {
		Some(n) => (api.block_hash(n).unwrap().expect("block hash"), n),
		None => api.finalized_head().unwrap(),
	};
	println!(
		"chain: {}  block #{number}  {hash:?}",
		api.system_chain().unwrap_or_default()
	);

	let events = cerebro_hydration::map_block(&api, hash).expect("map_block");
	println!("decoded {} chain-events\n", events.len());

	let (mut n_trade, mut n_transfer, mut n_fee, mut n_dca, mut n_aave, mut n_lp, mut n_reward) = (0, 0, 0, 0, 0, 0, 0);
	for (phase, ev) in &events {
		let tag = match phase {
			Phase::ApplyExtrinsic(i) => format!("ext{i}"),
			Phase::Hook => "hook".to_string(),
		};
		match ev {
			ChainEvent::Trade(t) => {
				n_trade += 1;
				let via = if t.router { "router" } else { "hop" };
				println!(
					"  [{tag}] trade({via}) {}→{} ({}→{})",
					t.asset_in, t.asset_out, t.amount_in, t.amount_out
				);
			}
			ChainEvent::Transfer(_) => n_transfer += 1,
			ChainEvent::FeePaid { who, amount } => {
				n_fee += 1;
				println!("  [{tag}] fee {amount} by 0x{}", hex::encode(&who[..6]));
			}
			ChainEvent::RewardClaimed { asset, amount, .. } => {
				n_reward += 1;
				println!("  [{tag}] reward-claim asset={asset} amount={amount}");
			}
			ChainEvent::AavePosition(d) => {
				n_aave += 1;
				println!("  [{tag}] aave leg asset={:?} amount={}", d.asset, d.amount);
			}
			ChainEvent::DcaStarted { .. } | ChainEvent::DcaClosed { .. } => n_dca += 1,
			ChainEvent::Liquidity => n_lp += 1,
			ChainEvent::Notable(_) | ChainEvent::Inert => {}
		}
	}
	println!(
		"\nsummary: {n_trade} trades, {n_transfer} transfers, {n_fee} fees, {n_dca} dca-boundaries, \
		 {n_aave} aave-legs, {n_lp} liquidity, {n_reward} rewards"
	);
}
