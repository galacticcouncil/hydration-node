use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo};
use frame_support::assert_ok;
use sp_runtime::{FixedU128, Permill};
use std::num::NonZeroU16;

use hydra_dx_math::stableswap::calculate_d;
use hydra_dx_math::stableswap::types::AssetReserve;
use proptest::prelude::*;
use proptest::proptest;
use sp_runtime::traits::BlockNumberProvider;

pub const ONE: Balance = 1_000_000_000_000;

const RESERVE_RANGE: (Balance, Balance) = (500_000 * ONE, 100_000_000 * ONE);

fn trade_amount() -> impl Strategy<Value = Balance> {
	1000..100_000 * ONE
}

fn asset_reserve() -> impl Strategy<Value = Balance> {
	RESERVE_RANGE.0..RESERVE_RANGE.1
}

fn some_amplification() -> impl Strategy<Value = NonZeroU16> {
	(2..10000u16).prop_map(|v| NonZeroU16::new(v).unwrap())
}

fn initial_amplification() -> impl Strategy<Value = NonZeroU16> {
	(2..1000u16).prop_map(|v| NonZeroU16::new(v).unwrap())
}

fn final_amplification() -> impl Strategy<Value = NonZeroU16> {
	(2000..10000u16).prop_map(|v| NonZeroU16::new(v).unwrap())
}

fn trade_fee() -> impl Strategy<Value = Permill> {
	(0f64..50f64).prop_map(Permill::from_float)
}

#[macro_export]
macro_rules! assert_eq_approx {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\n left: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}
proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_liquidity_price_no_changes(
		initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		amplification in some_amplification(),
		trade_fee in trade_fee()

	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1000),
				(ALICE, asset_a, initial_liquidity),
				(ALICE, asset_b, initial_liquidity),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,12)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,12)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	vec![
					AssetAmount::new(asset_a, initial_liquidity),
					AssetAmount::new(asset_b, initial_liquidity),
					]},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);

				let pool_account = pool_account(pool_id);

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);

				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					vec![

					AssetAmount::new(asset_a, added_liquidity),
					AssetAmount::new(asset_b, added_liquidity),
				]
				));

				let new_asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let new_asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);

				assert_eq_approx!(
					FixedU128::from((asset_a_reserve, asset_b_reserve)),
					FixedU128::from((new_asset_a_reserve, new_asset_b_reserve)),
					FixedU128::from_float(0.0000000001),
					"Price has changed after add liquidity"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_invariants(
		initial_liquidity in asset_reserve(),
		amount in trade_amount(),
		amplification in some_amplification(),
	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, amount),
				(ALICE, asset_a, initial_liquidity),
				(ALICE, asset_b, initial_liquidity),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,12)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,12)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				},
				InitialLiquidity{ account: ALICE, assets:
				vec![

					AssetAmount::new(asset_a, initial_liquidity),
					AssetAmount::new(asset_b, initial_liquidity),
				]},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);

				let pool_account = pool_account(pool_id);

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d_prev = calculate_d::<128u8>(&reserves, amplification.get().into()).unwrap();

				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					amount,
					0u128, // not interested in this
				));

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d = calculate_d::<128u8>(&reserves, amplification.get().into()).unwrap();

				assert!(d >= d_prev);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_invariants(
		initial_liquidity in asset_reserve(),
		amount in trade_amount(),
		amplification in some_amplification(),
	) {
		let asset_a: AssetId = 1;
		let asset_b: AssetId = 2;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, amount * 1000),
				(ALICE, asset_a, initial_liquidity),
				(ALICE, asset_b, initial_liquidity),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,12)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,12)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				},
				InitialLiquidity{ account: ALICE,
					assets:			vec![

					AssetAmount::new(asset_a, initial_liquidity),
					AssetAmount::new(asset_b, initial_liquidity),
				]},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);

				let pool_account = pool_account(pool_id);

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d_prev = calculate_d::<128u8>(&reserves, amplification.get().into()).unwrap();

				assert_ok!(Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_b,
					asset_a,
					amount,
					u128::MAX, // not interested in this
				));
				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];
				let d = calculate_d::<128u8>(&reserves, amplification.get().into()).unwrap();
				assert!(d >= d_prev);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]
	#[test]
	fn sell_invariants_should_hold_when_amplification_is_changing(
		initial_liquidity in asset_reserve(),
		amount in trade_amount(),
		initial_amplification in initial_amplification(),
		final_amplification in final_amplification(),
	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, amount),
				(ALICE, asset_a, initial_liquidity),
				(ALICE, asset_b, initial_liquidity),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,12)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,12)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification,
					final_amplification: initial_amplification,
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				},
				InitialLiquidity{ account: ALICE, assets:
				vec![

					AssetAmount::new(asset_a, initial_liquidity),
					AssetAmount::new(asset_b, initial_liquidity),
				]},
			)
			.build()
			.execute_with(|| {
				System::set_block_number(0);
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);

				System::set_block_number(1);
				assert_ok!(
					Stableswap::update_amplification(RuntimeOrigin::root(), pool_id, final_amplification.get(), 10,100)
				);

				System::set_block_number(9);
				let pool = <crate::Pools<Test>>::get(pool_id).unwrap();

				let asset_a_balance = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_balance = Tokens::free_balance(asset_b, &pool_account);
				let bob_a_balance = Tokens::free_balance(asset_a, &BOB);

				for _ in 0..100{
					System::set_block_number(System::current_block_number() + 1);
					let amplification = crate::Pallet::<Test>::get_amplification(&pool);

					// just restore the balances
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_a, asset_a_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_b, asset_b_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_a, bob_a_balance, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification).unwrap();

					assert_ok!(Stableswap::sell(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_a,
						asset_b,
						amount,
						0u128, // not interested in this
					));

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d = calculate_d::<128u8>(&reserves, amplification).unwrap();

					assert!(d >= d_prev);
				}
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]
	#[test]
	fn buy_invariants_should_hold_when_amplification_is_changing(
		initial_liquidity in asset_reserve(),
		amount in trade_amount(),
		initial_amplification in initial_amplification(),
		final_amplification in final_amplification(),
	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, amount * 1000),
				(ALICE, asset_a, initial_liquidity),
				(ALICE, asset_b, initial_liquidity),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,12)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,12)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification,
					final_amplification: initial_amplification,
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				},
				InitialLiquidity{ account: ALICE, assets:
				vec![

					AssetAmount::new(asset_a, initial_liquidity),
					AssetAmount::new(asset_b, initial_liquidity),
				]},
			)
			.build()
			.execute_with(|| {
				System::set_block_number(0);
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);

				System::set_block_number(1);
				assert_ok!(
					Stableswap::update_amplification(RuntimeOrigin::root(), pool_id, final_amplification.get(), 10,100)
				);

				System::set_block_number(9);
				let pool = <crate::Pools<Test>>::get(pool_id).unwrap();

				let asset_a_balance = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_balance = Tokens::free_balance(asset_b, &pool_account);
				let bob_a_balance = Tokens::free_balance(asset_a, &BOB);

				for _ in 0..100{
					System::set_block_number(System::current_block_number() + 1);
					let amplification = crate::Pallet::<Test>::get_amplification(&pool);

					// just restore the balances
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_a, asset_a_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_b, asset_b_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_a, bob_a_balance, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification).unwrap();

					assert_ok!(Stableswap::buy(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_b,
						asset_a,
						amount,
						u128::MAX, // not interested in this
					));

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d = calculate_d::<128u8>(&reserves, amplification).unwrap();

					assert!(d >= d_prev);
				}
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(50))]
	#[test]
	fn buy_invariants_with_18(
		initial_liquidity in asset_reserve(),
		amount in trade_amount(),
		initial_amplification in initial_amplification(),
		final_amplification in final_amplification(),
	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		let adjustment: u128 = 1_000_000;
		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, amount * 1000 * adjustment),
				(ALICE, asset_a, initial_liquidity * adjustment),
				(ALICE, asset_b, initial_liquidity * adjustment),
			])
			.with_registered_asset("one".as_bytes().to_vec(), asset_a,18)
			.with_registered_asset("two".as_bytes().to_vec(), asset_b,18)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: vec![asset_a,asset_b].try_into().unwrap(),
					initial_amplification,
					final_amplification: initial_amplification,
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				},
				InitialLiquidity{ account: ALICE, assets:
				vec![

					AssetAmount::new(asset_a, initial_liquidity * adjustment),
					AssetAmount::new(asset_b, initial_liquidity * adjustment),
				]},
			)
			.build()
			.execute_with(|| {
				System::set_block_number(0);
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);

				System::set_block_number(1);
				assert_ok!(
					Stableswap::update_amplification(RuntimeOrigin::root(), pool_id, final_amplification.get(), 10,100)
				);

				System::set_block_number(9);
				let pool = <crate::Pools<Test>>::get(pool_id).unwrap();

				let asset_a_balance = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_balance = Tokens::free_balance(asset_b, &pool_account);
				let bob_a_balance = Tokens::free_balance(asset_a, &BOB);

				for _ in 0..100{
					System::set_block_number(System::current_block_number() + 1);
					let amplification = crate::Pallet::<Test>::get_amplification(&pool);

					// just restore the balances
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_a, asset_a_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), pool_account, asset_b, asset_b_balance, 0).unwrap();
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_a, bob_a_balance, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 18),
						AssetReserve::new(asset_b_reserve, 18),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification).unwrap();
					assert_ok!(Stableswap::buy(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_b,
						asset_a,
						amount * adjustment,
						u128::MAX, // not interested in this
					));

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 18),
						AssetReserve::new(asset_b_reserve, 18),
					];

					let d = calculate_d::<128u8>(&reserves, amplification).unwrap();
					assert!(d >= d_prev);
				}
			});
	}
}
