use crate::stableswap::tests::ONE;
use crate::stableswap::types::AssetReserve;
use crate::stableswap::*;
use crate::types::Balance;
use proptest::prelude::*;
use proptest::proptest;

const D_ITERATIONS: u8 = 255;
const Y_ITERATIONS: u8 = 64;

const RESERVE_RANGE: (Balance, Balance) = (100_000 * ONE, 100_000_000 * ONE);
const LOW_RESERVE_RANGE: (Balance, Balance) = (10_u128, 11_u128);
const HIGH_RESERVE_RANGE: (Balance, Balance) = (500_000_000 * ONE, 500_000_001 * ONE);

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..10000 * ONE
}

fn high_trade_amount() -> impl Strategy<Value = Balance> {
	500_000_000 * ONE..500_000_001 * ONE
}

fn asset_reserve() -> impl Strategy<Value = Balance> {
	RESERVE_RANGE.0..RESERVE_RANGE.1
}
fn low_asset_reserve() -> impl Strategy<Value = Balance> {
	LOW_RESERVE_RANGE.0..LOW_RESERVE_RANGE.1
}
fn high_asset_reserve() -> impl Strategy<Value = Balance> {
	HIGH_RESERVE_RANGE.0..HIGH_RESERVE_RANGE.1
}

fn amplification() -> impl Strategy<Value = Balance> {
	2..10000u128
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_d_extreme(reserve_in in low_asset_reserve(),
		reserve_out in high_asset_reserve(),
		amp in amplification(),
	) {
		let d = calculate_d::<D_ITERATIONS>(&[AssetReserve::new(reserve_in,12), AssetReserve::new(reserve_out,12)], amp);

		assert!(d.is_some());
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_out_given_in_extreme(amount_in in high_trade_amount(),
		reserve_in in low_asset_reserve(),
		reserve_out in high_asset_reserve(),
		amp in amplification(),
	) {
		let reserves = [AssetReserve::new(reserve_in, 12), AssetReserve::new(reserve_out,12)];
		let d1 = calculate_d::<D_ITERATIONS>(&reserves, amp).unwrap();
		let result = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&reserves,0,1, amount_in, amp);

		assert!(result.is_some());

		let updated_reserves = [AssetReserve::new(reserves[0].amount + amount_in, 12),
			AssetReserve::new(reserves[1].amount - result.unwrap(),12)];
		let d2 = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();

		assert!(d2 >= d1);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_out_given_in(amount_in in trade_amount(),
		reserve_in in asset_reserve(),
		reserve_out in asset_reserve(),
		amp in amplification(),
	) {
		let reserves = [AssetReserve::new(reserve_in, 12), AssetReserve::new(reserve_out,12)];
		let d1 = calculate_d::<D_ITERATIONS>(&reserves, amp).unwrap();
		let result = calculate_out_given_in::<D_ITERATIONS, Y_ITERATIONS>(&reserves,0,1, amount_in, amp);

		assert!(result.is_some());

		let updated_reserves = [AssetReserve::new(reserves[0].amount + amount_in, 12),
			AssetReserve::new(reserves[1].amount - result.unwrap(),12)];

		let d2 = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
		assert!(d2 >= d1);
		assert!(d2 - d1 <= 10000000u128);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_in_given_out(amount_out in trade_amount(),
		reserve_in in asset_reserve(),
		reserve_out in asset_reserve(),
		amp in amplification(),
	) {
		let reserves = [AssetReserve::new(reserve_in, 12), AssetReserve::new(reserve_out,12)];
		let d1 = calculate_d::<D_ITERATIONS>(&reserves, amp).unwrap();

		let reserves = [AssetReserve::new(reserve_in, 12), AssetReserve::new(reserve_out,12)];
		let result = calculate_in_given_out::<D_ITERATIONS,Y_ITERATIONS>(&reserves,0,1, amount_out, amp);

		assert!(result.is_some());

		let updated_reserves = [AssetReserve::new(reserves[0].amount + result.unwrap(), 12),
			AssetReserve::new(reserves[1].amount - amount_out,12)];

		let d2 = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();

		assert!(d2 >= d1);
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_add_liquidity(
		amount_a in trade_amount(),
		amount_b in trade_amount(),
		reserve_a in asset_reserve(),
		reserve_b in asset_reserve(),
		amp in amplification(),
		issuance in asset_reserve(),
	) {
		let initial_reserves= [AssetReserve::new(reserve_a, 12), AssetReserve::new(reserve_b,12)];
		let updated_reserves= [AssetReserve::new(reserve_a + amount_a, 12), AssetReserve::new(reserve_b + amount_b,12)];

		let result = calculate_shares::<D_ITERATIONS>(&initial_reserves, &updated_reserves, amp, issuance);

		assert!(result.is_some());
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn round_trip_d_y_5(reserve_a in asset_reserve(),
		reserve_b in asset_reserve(),
		reserve_c in asset_reserve(),
		reserve_d in asset_reserve(),
		reserve_e in asset_reserve(),
		amp in amplification(),
	) {
		let ann = amp * 3125u128;  // 5^5

		let reserves = [AssetReserve::new(reserve_a, 12),
			AssetReserve::new(reserve_b,12),
			AssetReserve::new(reserve_c,12),
			AssetReserve::new(reserve_d,12),
			AssetReserve::new(reserve_e,12),
		];

		let d = calculate_d::<D_ITERATIONS>(&reserves, ann).unwrap();
		let y = calculate_y::<Y_ITERATIONS>(&reserves[1..], d, ann, reserves[0].decimals).unwrap();

		assert!(y - 4 <= reserve_a);
		assert!(y >= reserve_a);
	}
}

fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

fn reserve(max: Balance, precision: u8) -> impl Strategy<Value = Balance> {
	let min_reserve = 5 * 10u128.pow(precision as u32) + 10u128.pow(precision as u32);
	let max_reserve = max * 10u128.pow(precision as u32);
	min_reserve..max_reserve
}

prop_compose! {
	fn generate_reserves(dec_1: u8, dec_2: u8, dec_3: u8)
	(
		reserve_1 in reserve(1_000, dec_1),
		reserve_2 in reserve(1_000, dec_2),
		reserve_3 in reserve(1_000, dec_3),
	)
	-> Vec<AssetReserve> {
		vec![AssetReserve::new(reserve_1, dec_1), AssetReserve::new(reserve_2,dec_2), AssetReserve::new(reserve_3,dec_3)]
	}
}

prop_compose! {
	fn some_pool_reserves()
	(
		dec_1 in decimals(),
		dec_2 in decimals(),
		dec_3 in decimals(),
	)
	(
		dec_1 in Just(dec_1),
		r in generate_reserves(dec_1, dec_2, dec_3),
	)
	-> (Vec<AssetReserve>, u8) {
		(r, dec_1)
	}
}
proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn in_given_out_should_work_when_reserves_have_various_decimals(
		(reserves, dec_1) in some_pool_reserves(),
		amp in amplification(),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&reserves, amp).unwrap();
		let result = calculate_in_given_out::<D_ITERATIONS,Y_ITERATIONS>(&reserves, 0, 1, 10u128.pow(dec_1 as u32), amp);
		if let Some(amount_in) = result {
			let updated_reserves = [
				AssetReserve::new(reserves[0].amount + amount_in, reserves[0].decimals),
				AssetReserve::new(reserves[1].amount - 10u128.pow(dec_1 as u32), reserves[1].decimals),
				AssetReserve::new(reserves[2].amount, reserves[2].decimals),
			];
			let d1 = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
			assert!(d1 >= d0);
		}
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn out_given_in_should_work_when_reserves_have_various_decimals(
		(reserves, dec_1) in some_pool_reserves(),
		amp in amplification(),
	) {
		let d0 = calculate_d::<D_ITERATIONS>(&reserves, amp).unwrap();
		let result = calculate_out_given_in::<D_ITERATIONS,Y_ITERATIONS>(&reserves, 0, 1, 10u128.pow(dec_1 as u32), amp);
		if let Some(amount_out) = result {
			let updated_reserves = [
				AssetReserve::new(reserves[0].amount +10u128.pow(dec_1 as u32), reserves[0].decimals),
				AssetReserve::new(reserves[1].amount -amount_out, reserves[1].decimals),
				AssetReserve::new(reserves[2].amount, reserves[2].decimals),
			];
			let d1 = calculate_d::<D_ITERATIONS>(&updated_reserves, amp).unwrap();
			assert!(d1 >= d0);
		}
	}
}
