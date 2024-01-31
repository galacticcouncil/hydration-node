use crate::stableswap::types::AssetReserve;
use crate::stableswap::*;
use crate::types::Balance;
use proptest::prelude::*;
use proptest::proptest;

const D_ITERATIONS: u8 = 128;
const Y_ITERATIONS: u8 = 64;

const RESERVE_RANGE: (Balance, Balance) = (10_000, 1_000_000_000);
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

fn trade_pair(size: usize) -> impl Strategy<Value = (usize, usize)> {
	(0..size, 0..size)
		.prop_filter("cannot be equal", |(i, j)| i != j)
		.prop_map(|(i, j)| (i, j))
}

fn to_precision(value: Balance, precision: u8) -> Balance {
	value * 10u128.pow(precision as u32)
}

fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

// Note that his can generate very unbalanced pools. Should be adjusted to generate more balanced pools.
// In such case, we can see some outliers in the tests.
fn some_pool(size: usize) -> impl Strategy<Value = Vec<AssetReserve>> {
	prop::collection::vec(
		(asset_reserve(), decimals()).prop_map(|(v, dec)| AssetReserve::new(to_precision(v, dec), dec)),
		size,
	)
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out(
		pool in some_pool(3),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(3),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_out = to_precision(amount, pool[idx_out].decimals);

		let amount_in = calculate_in_given_out::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_out, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out_4_assets(
		pool in some_pool(4),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(4),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_out = to_precision(amount, pool[idx_out].decimals);

		let amount_in = calculate_in_given_out::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_out, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out_5_assets(
		pool in some_pool(5),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(5),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_out = to_precision(amount, pool[idx_out].decimals);

		let amount_in = calculate_in_given_out::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_out, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out_internal(
		pool in some_pool(4),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(4),
	) {
		let amount_out = normalize_value(
			to_precision(amount, pool[idx_out].decimals),
			pool[idx_out].decimals,
			18u8,
			Rounding::Down,
		);

		let balances = pool
			.iter()
			.map(|v| normalize_value(v.amount, v.decimals, 18u8, Rounding::Down))
			.collect::<Vec<Balance>>();

		let d0 = calculate_d_internal::<D_ITERATIONS>(&balances, amp).unwrap();
		let new_reserve_in =
			calculate_y_given_out::<D_ITERATIONS, Y_ITERATIONS>(amount_out, idx_in, idx_out, &balances, amp).unwrap();
		let updated_balances: Vec<Balance> = balances
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					new_reserve_in
				}
				else if idx == idx_out {
					v - amount_out
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d_internal::<D_ITERATIONS>(&updated_balances, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in(
		pool in some_pool(2),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(2),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_in = to_precision(amount, pool[idx_in].decimals);

		let amount_out = calculate_out_given_in::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_in, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_3_assets(
		pool in some_pool(3),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(3),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_in = to_precision(amount, pool[idx_in].decimals);

		let amount_out = calculate_out_given_in::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_in, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_4_assets(
		pool in some_pool(4),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(4),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_in = to_precision(amount, pool[idx_in].decimals);

		let amount_out = calculate_out_given_in::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_in, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_5_assets(
		pool in some_pool(5),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(5),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount_in = to_precision(amount, pool[idx_in].decimals);

		let amount_out = calculate_out_given_in::<D_ITERATIONS,Y_ITERATIONS>(&pool, idx_in, idx_out, amount_in, amp).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					AssetReserve::new(v.amount + amount_in, v.decimals)
				} else if idx == idx_out {
					AssetReserve::new(v.amount - amount_out, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 >= d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn remove_liquidity_invariant(
		pool in some_pool(3),
		amount in trade_amount(),
		amp in amplification(),
		(_, idx_out) in trade_pair(3),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&pool, amp).unwrap();
		let amount = to_precision(amount, 18u8);

		let balances = pool
			.iter()
			.map(|v| normalize_value(v.amount, v.decimals, 18u8, Rounding::Down))
			.collect::<Vec<Balance>>();
		let issuance = balances.iter().sum();

		let amount_out = calculate_withdraw_one_asset::<D_ITERATIONS,Y_ITERATIONS>(&pool, amount, idx_out, issuance, amp, Permill::zero()).unwrap();
		let updated_pool: Vec<AssetReserve> = pool
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_out {
					AssetReserve::new(v.amount - amount_out.0, v.decimals)
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d::<D_ITERATIONS>(&updated_pool, amp).unwrap();
		assert!(d1 < d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_internal(
		pool in some_pool(4),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(4),
	) {
		let amount_in = normalize_value(
			to_precision(amount, pool[idx_in].decimals),
			pool[idx_in].decimals,
			18u8,
			Rounding::Down,
		);

		let balances = pool
			.iter()
			.map(|v| normalize_value(v.amount, v.decimals, 18u8, Rounding::Down))
			.collect::<Vec<Balance>>();

		let d0 = calculate_d_internal::<D_ITERATIONS>(&balances, amp).unwrap();
		let new_reserve_out =
			calculate_y_given_in::<D_ITERATIONS, Y_ITERATIONS>(amount_in, idx_in, idx_out, &balances, amp).unwrap();

		assert!(new_reserve_out < balances[idx_out]);
		let updated_balances: Vec<Balance> = balances
			.into_iter()
			.enumerate()
			.map(|(idx, v)| {
				if idx == idx_in {
					v + amount_in
				}
				else if idx == idx_out {
					new_reserve_out
				} else {
					v
				}
			})
			.collect();
		let d1 = calculate_d_internal::<D_ITERATIONS>(&updated_balances, amp).unwrap();
		assert!(d1 >= d0);
		let diff = d1 - d0;
		assert!(diff <= 8000u128);
	}
}

use sp_arithmetic::Permill;

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn calculate_shares_for_amount_should_calculate_shares_correctly(
		pool in some_pool(2),
		amount in trade_amount(),
		amp in amplification(),
	) {
		let balances = pool
			.iter()
			.map(|v| normalize_value(v.amount, v.decimals, 18u8, Rounding::Down))
			.collect::<Vec<Balance>>();

		let issuance = balances.iter().sum();
		let amount = to_precision(amount, pool[0].decimals);
		let result = calculate_shares_for_amount::<D_ITERATIONS>(&pool, 0, amount, amp, issuance, Permill::zero()).unwrap();

		let received =
		calculate_withdraw_one_asset::<D_ITERATIONS, Y_ITERATIONS>(&pool, result, 0, issuance, amp, Permill::zero())
			.unwrap();
		// LP should not receive more than provided.
		assert!(received.0 <= amount);
		let diff = amount - received.0;
		assert!(diff <= 1000)
	}
}
