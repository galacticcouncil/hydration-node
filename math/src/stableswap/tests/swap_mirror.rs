// Diagnostic + executable round-trip for the stableswap mirror question.

use crate::stableswap::tests::default_pegs;
use crate::stableswap::types::AssetReserve;
use crate::stableswap::*;
use crate::types::Balance;
use proptest::prelude::*;
use sp_arithmetic::Permill;

// Add single-sided liquidity via calculate_shares, then immediately withdraw those shares
// as the same asset. With zero fee the LP must NOT get back more than they deposited
// (pattern #2/#6 — the saturating_sub fee path in calculate_shares could over-issue shares).
proptest! {
	#![proptest_config(ProptestConfig::with_cases(3000))]
	#[test]
	fn add_then_withdraw_one_asset_no_profit(
		pool in some_pool(3),
		amount in trade_amount(),
		amp in amplification(),
		idx in 0usize..3,
	) {
		let pegs = default_pegs(pool.len());
		let deposit = to_precision(amount, pool[idx].decimals);
		let issuance: Balance = pool.iter()
			.map(|v| normalize_value(v.amount, v.decimals, 18u8, Rounding::Down))
			.sum();

		// Updated reserves: idx += deposit
		let updated: Vec<AssetReserve> = pool.iter().enumerate().map(|(i, v)| {
			if i == idx { AssetReserve::new(v.amount + deposit, v.decimals) } else { *v }
		}).collect();

		let shares = calculate_shares::<D_ITERATIONS>(
			&pool, &updated, amp, issuance, Permill::zero(), &pegs,
		);
		let (shares, _) = match shares { Some(v) if v.0 > 0 => v, _ => return Ok(()) };

		let new_issuance = issuance + shares;
		// Withdraw those shares back out as the same asset from the UPDATED pool.
		let received = calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(
			&updated, shares, idx, new_issuance, amp, Permill::zero(), &pegs,
		);
		let (received, _) = match received { Some(v) => v, None => return Ok(()) };

		prop_assert!(
			received <= deposit,
			"ADD/WITHDRAW PROFIT: deposited {deposit} of asset {idx}, got {shares} shares, withdrew {received} (> {deposit}); amp={amp} pool={pool:?}"
		);
	}
}

const D_ITERATIONS: u8 = 128;
const Y_ITERATIONS: u8 = 64;

const RESERVE_RANGE: (Balance, Balance) = (30_000, 1_000_000_000);
const TRADE_RANGE: (Balance, Balance) = (1, 5_000);

fn asset_reserve() -> impl Strategy<Value = Balance> {
	RESERVE_RANGE.0..RESERVE_RANGE.1
}
fn trade_amount() -> impl Strategy<Value = Balance> {
	TRADE_RANGE.0..TRADE_RANGE.1
}
fn amplification() -> impl Strategy<Value = Balance> {
	2..10000u128
}
fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}
fn trade_pair(size: usize) -> impl Strategy<Value = (usize, usize)> {
	(0..size).prop_flat_map(move |i| {
		(
			Just(i),
			(0..(size - 1)).prop_map(move |j| if j >= i { j + 1 } else { j }),
		)
	})
}
fn to_precision(value: Balance, precision: u8) -> Balance {
	value * 10u128.pow(precision as u32)
}
fn some_pool(size: usize) -> impl Strategy<Value = Vec<AssetReserve>> {
	prop::collection::vec(
		(asset_reserve(), decimals()).prop_map(|(v, dec)| AssetReserve::new(to_precision(v, dec), dec)),
		size,
	)
}

// Exact failing input from the same-state mirror test — measure the magnitude.
#[test]
fn diagnostic_same_state_mirror_magnitude() {
	let pool = vec![
		AssetReserve::new(3407360000000000, 10),
		AssetReserve::new(180751568000000, 6),
		AssetReserve::new(30000000000, 6),
	];
	let amp = 2u128;
	let (idx_in, idx_out) = (1usize, 0usize);
	let amount_in = to_precision(1, pool[idx_in].decimals); // 1 * 10^6
	let amount_out = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(
		&pool,
		idx_in,
		idx_out,
		amount_in,
		amp,
		&default_pegs(pool.len()),
	)
	.unwrap();
	let required_in = calculate_in_given_out::<D_ITERATIONS, Y_ITERATIONS>(
		&pool,
		idx_in,
		idx_out,
		amount_out,
		amp,
		&default_pegs(pool.len()),
	)
	.unwrap();
	println!("amount_in={amount_in} amount_out={amount_out} required_in={required_in}");
	println!(
		"shortfall (amount_in - required_in) = {}",
		amount_in as i128 - required_in as i128
	);
	// amount_out is denominated in idx_out (10 decimals). amount_in in idx_out-value terms:
}

// HONEST executable self-round-trip: sell then sell-back at the mutated pool.
proptest! {
	#![proptest_config(ProptestConfig::with_cases(4000))]
	#[test]
	fn executable_roundtrip_no_profit(
		pool in some_pool(3),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(3),
	) {
		let amount_in = to_precision(amount, pool[idx_in].decimals);
		let pegs = default_pegs(pool.len());

		let amount_out = match calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(
			&pool, idx_in, idx_out, amount_in, amp, &pegs) {
			Some(v) if v > 0 => v, _ => return Ok(()),
		};

		// Update pool: idx_in += amount_in, idx_out -= amount_out
		let pool2: Vec<AssetReserve> = pool.iter().enumerate().map(|(i, v)| {
			if i == idx_in { AssetReserve::new(v.amount + amount_in, v.decimals) }
			else if i == idx_out { AssetReserve::new(v.amount - amount_out, v.decimals) }
			else { *v }
		}).collect();

		// Sell the received amount_out back (idx_out -> idx_in) at the mutated pool.
		let x_back = match calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(
			&pool2, idx_out, idx_in, amount_out, amp, &pegs) {
			Some(v) => v, None => return Ok(()),
		};

		// A self-round-trip must never return more of asset_in than was put in.
		prop_assert!(
			x_back <= amount_in,
			"ROUND-TRIP PROFIT: put {amount_in}, got back {x_back} (amount_out={amount_out}); idx_in={idx_in} idx_out={idx_out} amp={amp} pool={pool:?}"
		);
	}
}
