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
	((0..size, 0..size))
		.prop_filter("cannot be equal", |(i, j)| i != j)
		.prop_map(|(i, j)| (i, j))
}

fn to_precision(value: Balance, precision: u8) -> Balance {
	value * 10u128.pow(precision as u32)
}

fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

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
		pool in some_pool(2),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(2),
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
		//dbg!(updated_pool[0].decimals);
		//dbg!(updated_pool[1].decimals);
		//dbg!(d1 - d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out_internal(
		pool in some_pool(2),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(2),
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
		//dbg!(d1 - d0);
		//assert!(d1 - d0 <= 10u128)
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
		//dbg!(updated_pool[0].decimals);
		//dbg!(updated_pool[1].decimals);
		//dbg!(d1 - d0);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_internal(
		pool in some_pool(2),
		amount in trade_amount(),
		amp in amplification(),
		(idx_in, idx_out) in trade_pair(2),
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
		//assert!(d1 - d0 <= 10u128)
	}
}
