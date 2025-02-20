use crate::tests::*;
use crate::types::PoolInfo;
use frame_support::{assert_ok, BoundedVec};
use hydradx_traits::stableswap::AssetAmount;
use sp_runtime::{FixedU128, Permill};
use std::cmp::Ordering;
use std::num::NonZeroU16;

use hydra_dx_math::stableswap::calculate_d;
use hydra_dx_math::stableswap::types::AssetReserve;
use proptest::prelude::*;
use proptest::proptest;
use sp_core::U256;
use sp_runtime::traits::BlockNumberProvider;
use test_utils::assert_eq_approx;

pub const ONE: Balance = 1_000_000_000_000;

const RESERVE_RANGE: (Balance, Balance) = (500_000 * ONE, 100_000_000 * ONE);

fn trade_amount() -> impl Strategy<Value = Balance> {
	1_000_000..100_000 * ONE
}

fn share_amount() -> impl Strategy<Value = Balance> {
	1_000 * ONE * 1_000_000..100_000 * ONE * 1_000_000
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
	(0f64..0.2f64).prop_map(Permill::from_float)
}

fn get_pool_asset_pegs(pool_id: AssetId) -> Vec<PegType> {
	let pool = crate::Pools::<Test>::get(pool_id).unwrap();
	Pallet::<Test>::get_updated_pegs(pool_id, &pool).unwrap().1
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_share_price_in_add_remove_liquidity(
		initial_liquidity in asset_reserve(),
		added_liquidity in trade_amount(),
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

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					]),
				));
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					delta_s,
					0u128,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_share_price_in_add_withdraw_asset(
		initial_liquidity in asset_reserve(),
		added_liquidity in trade_amount(),
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

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::withdraw_asset_amount(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					added_liquidity / 2,
					u128::MAX,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn test_share_price_in_add_shares_withdraw_asset(
		initial_liquidity in asset_reserve(),
		added_liquidity in trade_amount(),
		shares_amount in share_amount(),
		amplification in some_amplification(),
		trade_fee in trade_fee()
	) {
		let asset_a: AssetId = 1000;
		let asset_b: AssetId = 2000;

		ExtBuilder::default()
			.with_endowed_accounts(vec![
				(BOB, asset_a, added_liquidity * 1_000_000_000),
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

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				let initial_a = Tokens::free_balance(asset_a, &BOB);
				assert_ok!(Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares_amount,
					asset_a,
					u128::MAX,
				));
				let final_a = Tokens::free_balance(asset_a, &BOB);
				let added_liquidity = initial_a - final_a;
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::withdraw_asset_amount(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					added_liquidity / 2,
					u128::MAX,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);
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
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d_prev = calculate_d::<128u8>(&reserves, amplification.get().into(), &asset_pegs).unwrap();
				let initial_spot_price = spot_price_first_asset(pool_id, asset_b);
				assert_ok!(Stableswap::sell(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					asset_b,
					amount,
					0u128, // not interested in this
				));

				let received = Tokens::free_balance(asset_b, &BOB);
				let exec_price = FixedU128::from_rational(amount * 1_000_000, received * 1_000_000);
				assert!(exec_price >= initial_spot_price);

				let final_spot_price = spot_price_first_asset(pool_id, asset_b);
				if exec_price > final_spot_price {
					let p = (exec_price - final_spot_price) / final_spot_price;
					assert!(p <= FixedU128::from_rational(1, 100_000_000_000));
				} else {
					assert!(exec_price <= final_spot_price);
				}
				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d = calculate_d::<128u8>(&reserves, amplification.get().into(), &asset_pegs).unwrap();

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
					assets:	vec![
						AssetAmount::new(asset_a, initial_liquidity),
						AssetAmount::new(asset_b, initial_liquidity),
					]},
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let pool_account = pool_account(pool_id);

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];

				let d_prev = calculate_d::<128u8>(&reserves, amplification.get().into(), &asset_pegs).unwrap();

				let bob_balance_a = Tokens::free_balance(asset_a, &BOB);
				let initial_spot_price = spot_price_first_asset(pool_id, asset_b);

				assert_ok!(Stableswap::buy(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_b,
					asset_a,
					amount,
					u128::MAX, // not interested in this
				));

				let a_balance = Tokens::free_balance(asset_a, &BOB);
				let delta_a = bob_balance_a - a_balance;
				let exec_price = FixedU128::from_rational(delta_a * 1_000_000, amount * 1_000_000);
				assert!(exec_price >= initial_spot_price);
				let final_spot_price = spot_price_first_asset(pool_id, asset_b);
				match exec_price.cmp(&final_spot_price) {
						Ordering::Less | Ordering::Equal => {
						// all good
					},
					Ordering::Greater => {
						let d = (exec_price - final_spot_price) / final_spot_price;
						assert!(d <= FixedU128::from_rational(1,100_000_000_000));
					}
				}

				let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
				let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
				let reserves = vec![
					AssetReserve::new(asset_a_reserve, 12),
					AssetReserve::new(asset_b_reserve, 12),
				];
				let d = calculate_d::<128u8>(&reserves, amplification.get().into(), &asset_pegs).unwrap();
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
				let asset_pegs = get_pool_asset_pegs(pool_id);

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
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_b, 0, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();
					let initial_spot_price = spot_price_first_asset(pool_id, asset_b);
					assert_ok!(Stableswap::sell(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_a,
						asset_b,
						amount,
						0u128, // not interested in this
					));
					let received = Tokens::free_balance(asset_b, &BOB);
					assert!(amount > received);
					let exec_price = FixedU128::from_rational(amount * 1_000_000, received * 1_000_000);
					assert!(exec_price >= initial_spot_price);

					let final_spot_price = spot_price_first_asset(pool_id, asset_b);
					match exec_price.cmp(&final_spot_price) {
						Ordering::Equal | Ordering::Less => {
							//all good
						},
						Ordering::Greater => {
							let p = (exec_price - final_spot_price) / final_spot_price;
							assert!(p <= FixedU128::from_rational(1, 100_000_000_000));
						},
					};
					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();

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
				let asset_pegs = get_pool_asset_pegs(pool_id);

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
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_b, 0, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();

					let bob_a_balance = Tokens::free_balance(asset_a, &BOB);
					let initial_spot_price = spot_price_first_asset(pool_id, asset_b);
					assert_ok!(Stableswap::buy(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_b,
						asset_a,
						amount,
						u128::MAX, // not interested in this
					));

					let a_balance = Tokens::free_balance(asset_a, &BOB);
					let delta_a = bob_a_balance - a_balance;
					let exec_price = FixedU128::from_rational(delta_a * 1_000_000, amount * 1_000_000);
					assert!(exec_price >= initial_spot_price);

					let final_spot_price = spot_price_first_asset(pool_id, asset_b);
					assert!(exec_price <= final_spot_price);

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 12),
						AssetReserve::new(asset_b_reserve, 12),
					];

					let d = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();

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
				let asset_pegs = get_pool_asset_pegs(pool_id);

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
					Tokens::set_balance(RuntimeOrigin::root(), BOB, asset_b, 0, 0).unwrap();

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 18),
						AssetReserve::new(asset_b_reserve, 18),
					];

					let d_prev = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();

					let bob_a_balance = Tokens::free_balance(asset_a, &BOB);
					let initial_spot_price = spot_price_first_asset(pool_id, asset_b);
					assert_ok!(Stableswap::buy(
						RuntimeOrigin::signed(BOB),
						pool_id,
						asset_b,
						asset_a,
						amount * adjustment,
						u128::MAX, // not interested in this
					));
					let a_balance = Tokens::free_balance(asset_a, &BOB);
					let delta_a = bob_a_balance - a_balance;
					let exec_price = FixedU128::from_rational(delta_a , amount * adjustment );
					assert!(exec_price >= initial_spot_price);

					let final_spot_price = spot_price_first_asset(pool_id, asset_b);
					assert!(exec_price <= final_spot_price);

					let asset_a_reserve = Tokens::free_balance(asset_a, &pool_account);
					let asset_b_reserve = Tokens::free_balance(asset_b, &pool_account);
					let reserves = vec![
						AssetReserve::new(asset_a_reserve, 18),
						AssetReserve::new(asset_b_reserve, 18),
					];

					let d = calculate_d::<128u8>(&reserves, amplification, &asset_pegs).unwrap();
					assert!(d >= d_prev);
				}
			});
	}
}

fn to_precision(value: hydra_dx_math::types::Balance, precision: u8) -> hydra_dx_math::types::Balance {
	value * 10u128.pow(precision as u32)
}

fn decimals() -> impl Strategy<Value = u8> {
	prop_oneof![Just(6), Just(8), Just(10), Just(12), Just(18)]
}

const RESERVE_RANGE_NO_DECIMALS: (hydra_dx_math::types::Balance, hydra_dx_math::types::Balance) =
	(10_000, 1_000_000_000);
fn reserve() -> impl Strategy<Value = hydra_dx_math::types::Balance> {
	RESERVE_RANGE_NO_DECIMALS.0..RESERVE_RANGE_NO_DECIMALS.1
}

fn balanced_pool(size: usize) -> impl Strategy<Value = Vec<AssetReserve>> {
	let reserve_amount = reserve();
	prop::collection::vec(
		(reserve_amount, decimals()).prop_map(|(v, dec)| AssetReserve::new(to_precision(v, dec), dec)),
		size,
	)
}

fn balanced_pool_with_fixed_decimals(size: usize, decimals: u8) -> impl Strategy<Value = Vec<AssetReserve>> {
	let reserve_amount = reserve();
	prop::collection::vec(
		(reserve_amount, Just(decimals)).prop_map(|(v, dec)| AssetReserve::new(to_precision(v, dec), dec)),
		size,
	)
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_remove_liquidity_invariants(
		pool in balanced_pool(3),
		liquidity_to_add in reserve(),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::from_percent(0);

		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate().map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();

		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();
		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate().map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(liquidity_to_add , dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity * 1000));

		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	initial_liquidity,}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let initial_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let initial_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&initial_reserves, amplification.get().into(), &asset_pegs).unwrap();

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let intermediate_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(intermediate_d > initial_d);

				let d = U256::from(initial_d) ;
				let d_plus = U256::from(intermediate_d) ;
				let s = U256::from(initial_shares) ;
				let s_plus = U256::from(final_shares) ;
				assert!(d_plus * s >= d * s_plus);
				assert!(d * s_plus >= (d_plus - 10u128.pow(18)) * s);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					delta_s,
					0u128,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);

				let final_shares = Tokens::total_issuance(pool_id);
				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let final_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(final_d < intermediate_d);

				let d = d_plus;
				let d_plus = U256::from(final_d);
				let s = s_plus;
				let s_plus = U256::from(final_shares);
				assert!(d * (s_plus + 10u128.pow(18) )  >= d_plus * s);
				assert!(d_plus * s >= d * s_plus);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_remove_liquidity_invariants_with_5_assets(
		pool in balanced_pool(5),
		liquidity_to_add in reserve(),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::from_percent(0);

		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate().map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();

		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();
		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate().map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(liquidity_to_add , dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity * 1000));

		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	initial_liquidity,}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let initial_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let initial_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&initial_reserves, amplification.get().into(), &asset_pegs).unwrap();

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let intermediate_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(intermediate_d > initial_d);

				let d = U256::from(initial_d) ;
				let d_plus = U256::from(intermediate_d) ;
				let s = U256::from(initial_shares) ;
				let s_plus = U256::from(final_shares) ;
				assert!(d_plus * s >= d * s_plus);
				assert!(d * s_plus >= (d_plus - 10u128.pow(18)) * s);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::remove_liquidity_one_asset(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					delta_s,
					0u128,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);

				let final_shares = Tokens::total_issuance(pool_id);
				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let final_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(final_d < intermediate_d);

				let d = d_plus;
				let d_plus = U256::from(final_d);
				let s = s_plus;
				let s_plus = U256::from(final_shares);
				assert!(d * (s_plus + 10u128.pow(18) )  >= d_plus * s);
				assert!(d_plus * s >= d * s_plus);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_remove_liquidity_with_shares_invariants(
		pool in balanced_pool(3),
		shares_to_add in share_amount(),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::from_percent(0);

		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate().map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();

		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();
		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate().map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(1_000_000_000_000, dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity * 1000));

		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	initial_liquidity,}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs= get_pool_asset_pegs(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let initial_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let initial_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&initial_reserves, amplification.get().into(), &asset_pegs).unwrap();

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				let initial_a = Tokens::free_balance(asset_a, &BOB);
				assert_ok!(Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares_to_add,
					asset_a,
					u128::MAX,
				));
				let final_a = Tokens::free_balance(asset_a, &BOB);
				let added_liquidity = initial_a - final_a;
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				assert!(delta_s == shares_to_add);
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let intermediate_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(intermediate_d > initial_d);

				let d = U256::from(initial_d) ;
				let d_plus = U256::from(intermediate_d) ;
				let s = U256::from(initial_shares) ;
				let s_plus = U256::from(final_shares) ;
				assert!(d_plus * s >= d * s_plus);
				assert!(d * s_plus >= (d_plus - 10u128.pow(18)) * s);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::withdraw_asset_amount(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					added_liquidity / 2,
					u128::MAX,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);

				let final_shares = Tokens::total_issuance(pool_id);
				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let final_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(final_d < intermediate_d);

				let d = d_plus;
				let d_plus = U256::from(final_d);
				let s = s_plus;
				let s_plus = U256::from(final_shares);
				assert!(d * (s_plus + 10u128.pow(18) )  >= d_plus * s);
				assert!(d_plus * s >= d * s_plus);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_remove_liquidity_with_shares_invariants_with_5_assets(
		pool in balanced_pool(5),
		shares_to_add in share_amount(),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::from_percent(0);

		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate().map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();

		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();
		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate().map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(1_000_000_000_000, dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity * 1000));

		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	initial_liquidity,}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let initial_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let initial_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&initial_reserves, amplification.get().into(), &asset_pegs).unwrap();

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				let initial_a = Tokens::free_balance(asset_a, &BOB);
				assert_ok!(Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares_to_add,
					asset_a,
					u128::MAX,
				));
				let final_a = Tokens::free_balance(asset_a, &BOB);
				let added_liquidity = initial_a - final_a;
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				assert!(delta_s == shares_to_add);
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let intermediate_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(intermediate_d > initial_d);

				let d = U256::from(initial_d) ;
				let d_plus = U256::from(intermediate_d) ;
				let s = U256::from(initial_shares) ;
				let s_plus = U256::from(final_shares) ;
				assert!(d_plus * s >= d * s_plus);
				assert!(d * s_plus >= (d_plus - 10u128.pow(18)) * s);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::withdraw_asset_amount(
					RuntimeOrigin::signed(BOB),
					pool_id,
					asset_a,
					added_liquidity / 2,
					u128::MAX,
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);

				let final_shares = Tokens::total_issuance(pool_id);
				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let final_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(final_d < intermediate_d);

				let d = d_plus;
				let d_plus = U256::from(final_d);
				let s = s_plus;
				let s_plus = U256::from(final_shares);
				assert!(d * (s_plus + 10u128.pow(18) )  >= d_plus * s);
				assert!(d_plus * s >= d * s_plus);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_remove_multi_asset_liquidity_invariants(
		pool in balanced_pool(3),
		liquidity_to_add in reserve(),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::from_percent(0);

		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate().map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();

		// use 0 amounts
		let min_amounts = assets_to_register.iter().map(|(_, asset_id, _)| {
					AssetAmount::new(*asset_id, 0)
				}).collect::<Vec<_>>();

		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();
		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate().map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(liquidity_to_add , dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity * 1000));

		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
				assets:	initial_liquidity,}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs = get_pool_asset_pegs(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let initial_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let initial_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&initial_reserves, amplification.get().into(), &asset_pegs).unwrap();

				let share_price_initial = get_share_price(pool_id, 0);
				let initial_shares = Tokens::total_issuance(pool_id);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::total_issuance(pool_id);
				let delta_s = final_shares - initial_shares;
				let exec_price = FixedU128::from_rational(added_liquidity , delta_s);
				assert!(share_price_initial <= exec_price);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let intermediate_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(intermediate_d > initial_d);

				let d = U256::from(initial_d) ;
				let d_plus = U256::from(intermediate_d) ;
				let s = U256::from(initial_shares) ;
				let s_plus = U256::from(final_shares) ;
				assert!(d_plus * s >= d * s_plus);
				assert!(d * s_plus >= (d_plus - 10u128.pow(18)) * s);

				let share_price_initial = get_share_price(pool_id, 0);
				let a_initial = Tokens::free_balance(asset_a, &pool_account);
				assert_ok!(Stableswap::remove_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					delta_s,
					BoundedVec::try_from(min_amounts).unwrap(),
				));
				let a_final = Tokens::free_balance(asset_a, &pool_account);
				let delta_a = a_initial - a_final;
				let exec_price = FixedU128::from_rational(delta_a, delta_s);
				assert!(share_price_initial >= exec_price);

				let final_shares = Tokens::total_issuance(pool_id);
				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				let final_d = hydra_dx_math::stableswap::calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
				assert!(final_d < intermediate_d);

				let d = d_plus;
				let d_plus = U256::from(final_d);
				let s = s_plus;
				let s_plus = U256::from(final_shares);
				assert!(d * (s_plus + 10u128.pow(18) )  >= d_plus * s);
				assert!(d_plus * s >= d * s_plus);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_shares_invariants_no_fee(
		pool in balanced_pool_with_fixed_decimals(3,12),
		amplification in some_amplification(),
	) {
		let trade_fee = Permill::zero();
		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate()
			.map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();
		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();

		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate()
			.map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(10, dec);
		let extra_unit = to_precision(1, dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity + extra_unit));

		let mut shares_received=0;
		let mut bob_holding_add_liq = 0;
		let mut pool_liquidity_add_liquid= 0;
		let mut share_issuance_add_liquid = 0;
		let mut d_add_liquid = 0;
		// Add liquidity
		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts.clone())
			.with_registered_assets(assets_to_register.clone())
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.clone().try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
					assets:	initial_liquidity.clone(),
				}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs= get_pool_asset_pegs(pool_id);
				let initial_shares = Tokens::free_balance(pool_id, &BOB);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::free_balance(pool_id, &BOB);
				shares_received = final_shares - initial_shares;

				bob_holding_add_liq = Tokens::free_balance(asset_a, &BOB);
				pool_liquidity_add_liquid = Tokens::free_balance(pool_id, &pool_account);
				share_issuance_add_liquid = Tokens::total_issuance(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				d_add_liquid = calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
			});

		let mut asset_a_used = 0;
		let mut bob_holding_buy_shares = 0;
		let mut bob_shares_buy_shares = 0;
		let mut pool_liquidity_buy_shares = 0;
		let mut share_issuance_buy_shares = 0;
		let mut d_buy_shares = 0;

		// Buy shares
		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
					assets:	initial_liquidity,
				}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs= get_pool_asset_pegs(pool_id);
				let initial_a = Tokens::free_balance(asset_a, &BOB);
				assert_ok!(Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares_received,
					asset_a,
					u128::MAX,
				));

				let final_a = Tokens::free_balance(asset_a, &BOB);
				asset_a_used = initial_a - final_a;
				bob_holding_buy_shares = Tokens::free_balance(asset_a, &BOB);
				bob_shares_buy_shares= Tokens::free_balance(pool_id, &BOB);
				pool_liquidity_buy_shares = Tokens::free_balance(pool_id, &pool_account);
				share_issuance_buy_shares = Tokens::total_issuance(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				d_buy_shares = calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
			});

		assert_eq_approx!(bob_holding_add_liq, bob_holding_buy_shares, 10, "Bob balance should be the same after adding liquidity and buying shares");
		assert_eq!(shares_received, bob_shares_buy_shares);
		assert_eq_approx!(pool_liquidity_add_liquid, pool_liquidity_buy_shares, 10, "Pool liquidity of asset A should be the same after adding liquidity and buying shares");
		assert_eq!(share_issuance_add_liquid, share_issuance_buy_shares);
		assert_eq_approx!(d_add_liquid, d_buy_shares, 1_000_000_000_000_000, "D invariants should be the same after adding liquidity and buying shares");
	}


}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_shares_invariants_with_fee(
		pool in balanced_pool_with_fixed_decimals(3,12),
		amplification in some_amplification(),
		trade_fee in trade_fee(),
	) {
		let assets_to_register: Vec<(Vec<u8>, u32, u8)> = pool.iter().enumerate()
			.map(|(asset_id, v)| ( asset_id.to_string().into_bytes(), asset_id as u32, v.decimals)).collect();
		let pool_assets: Vec<AssetId> = assets_to_register.iter().map(|(_, asset_id, _)| *asset_id).collect();

		let initial_liquidity: Vec<AssetAmount<AssetId>> = pool.iter().enumerate().map(|(asset_id, v)| AssetAmount::new(asset_id as AssetId, v.amount)).collect();
		let mut endowed_accounts: Vec<(AccountId, AssetId, hydra_dx_math::types::Balance)> = pool.iter().enumerate()
			.map(|(asset_id, v)| (ALICE, asset_id as u32, v.amount)).collect();

		let (_, asset_a, dec)= assets_to_register.first().unwrap().clone();
		let added_liquidity = to_precision(10, dec);
		let extra_unit = to_precision(1, dec);
		endowed_accounts.push((BOB, asset_a, added_liquidity + extra_unit));

		let mut shares_received=0;
		let mut bob_holding_add_liq = 0;
		let mut pool_liquidity_add_liquid= 0;
		let mut share_issuance_add_liquid = 0;
		let mut d_add_liquid = 0;
		// Add liquidity
		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts.clone())
			.with_registered_assets(assets_to_register.clone())
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.clone().try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
					assets:	initial_liquidity.clone(),
				}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs= get_pool_asset_pegs(pool_id);
				let initial_shares = Tokens::free_balance(pool_id, &BOB);
				assert_ok!(Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					BoundedVec::truncate_from(vec![
						AssetAmount::new(asset_a, added_liquidity),
					])
				));
				let final_shares = Tokens::free_balance(pool_id, &BOB);
				shares_received = final_shares - initial_shares;

				bob_holding_add_liq = Tokens::free_balance(asset_a, &BOB);
				pool_liquidity_add_liquid = Tokens::free_balance(asset_a, &pool_account);
				share_issuance_add_liquid = Tokens::total_issuance(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				d_add_liquid = calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
			});

		let mut asset_a_used = 0;
		let mut bob_asset_a_holding_buy_shares = 0;
		let mut bob_shares_buy_shares = 0;
		let mut pool_liquidity_buy_shares = 0;
		let mut share_issuance_buy_shares = 0;
		let mut d_buy_shares = 0;

		// Buy shares
		ExtBuilder::default()
			.with_endowed_accounts(endowed_accounts)
			.with_registered_assets(assets_to_register)
			.with_pool(
				ALICE,
				PoolInfo::<AssetId, u64> {
					assets: pool_assets.try_into().unwrap(),
					initial_amplification: amplification,
					final_amplification: amplification,
					initial_block: 0,
					final_block: 0,
					fee: trade_fee,
				},
				InitialLiquidity{ account: ALICE,
					assets:	initial_liquidity,
				}
			)
			.build()
			.execute_with(|| {
				let pool_id = get_pool_id_at(0);
				let pool_account = pool_account(pool_id);
				let asset_pegs= get_pool_asset_pegs(pool_id);
				let initial_a = Tokens::free_balance(asset_a, &BOB);
				assert_ok!(Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares_received,
					asset_a,
					u128::MAX,
				));

				let final_a = Tokens::free_balance(asset_a, &BOB);
				asset_a_used = initial_a - final_a;
				bob_asset_a_holding_buy_shares = Tokens::free_balance(asset_a, &BOB);
				bob_shares_buy_shares= Tokens::free_balance(pool_id, &BOB);
				pool_liquidity_buy_shares = Tokens::free_balance(asset_a, &pool_account);
				share_issuance_buy_shares = Tokens::total_issuance(pool_id);

				let pool = Pools::<Test>::get(pool_id).unwrap();
				let final_reserves = pool.reserves_with_decimals::<Test>(&pool_account).unwrap();
				d_buy_shares = calculate_d::<128u8>(&final_reserves, amplification.get().into(), &asset_pegs).unwrap();
			});

		// This will be always the same
		assert_eq!(shares_received, bob_shares_buy_shares);
		assert_eq!(share_issuance_add_liquid, share_issuance_buy_shares);

		assert_eq_approx!(added_liquidity, asset_a_used, 250_000_000_000, "Bob asset as used to get shares should be the same after adding liquidity and buying shares");
		assert_eq_approx!(bob_holding_add_liq, bob_asset_a_holding_buy_shares, 250_000_000_000, "Bob balance should be the same after adding liquidity and buying shares");

		assert_eq_approx!(d_add_liquid, d_buy_shares, 1_000_000_000_000_000_000_000, "D invariants should be the same after adding liquidity and buying shares");
		assert_eq_approx!(pool_liquidity_add_liquid, pool_liquidity_buy_shares, 250_000_000_000, "Pool liquidity of asset A should be the same after adding liquidity and buying shares");
	}

}
