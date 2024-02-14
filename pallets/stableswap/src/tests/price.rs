use crate::tests::*;
use crate::types::{AssetAmount, PoolInfo};
use frame_support::assert_ok;
use sp_runtime::{FixedU128, Permill};
use std::num::NonZeroU16;

#[test]
fn test_spot_price_in_sell() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::zero(),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let amount = 1_000_000_000_000_000_000;

			let initial_spot_price = asset_spot_price(pool_id, asset_b);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				amount,
				0,
			));

			let received = Tokens::free_balance(asset_b, &BOB);
			let exec_price = FixedU128::from_rational(amount, received);
			let exec_price = exec_price / FixedU128::from(1_000_000_000_000);
			assert!(exec_price >= initial_spot_price);

			let final_spot_price = asset_spot_price(pool_id, asset_b);
			if exec_price > final_spot_price {
				let p = (exec_price - final_spot_price) / final_spot_price;
				assert!(p <= FixedU128::from_rational(1, 100_000));
			} else {
				assert!(exec_price <= final_spot_price);
			}
		});
}

#[test]
fn test_spot_price_in_buy() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 2_000_000_000_000_000_000),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::zero(),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let amount = 1_000_000;

			let initial_spot_price = asset_spot_price(pool_id, asset_b);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				amount,
				u128::MAX,
			));

			let a_balance = Tokens::free_balance(asset_a, &BOB);
			let delta_a = 2_000_000_000_000_000_000 - a_balance;
			let exec_price = FixedU128::from_rational(delta_a, amount);
			let exec_price = exec_price / FixedU128::from(1_000_000_000_000);
			assert!(exec_price >= initial_spot_price);

			let final_spot_price = asset_spot_price(pool_id, asset_b);
			assert!(exec_price <= final_spot_price);
		});
}

#[test]
fn test_share_price_in_add_remove_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(2000).unwrap(),
				final_amplification: NonZeroU16::new(2000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::zero(),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let pool_account = pool_account(pool_id);
			let amount = 1_000_000_000_000_000_000;
			let share_price_initial = get_share_price(pool_id, 0);
			let initial_shares = Tokens::total_issuance(pool_id);
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount)],
			));

			let final_shares = Tokens::total_issuance(pool_id);
			let delta_s = final_shares - initial_shares;
			let exec_price = FixedU128::from_rational(amount, delta_s);

			assert!(share_price_initial <= exec_price);

			// Remove liquidity
			let share_price_initial = get_share_price(pool_id, 0);
			let a_initial = Tokens::free_balance(asset_a, &pool_account);
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				delta_s,
				900_000_000_000_000_000,
			));
			let a_final = Tokens::free_balance(asset_a, &pool_account);
			let delta_a = a_initial - a_final;
			let exec_price = FixedU128::from_rational(delta_a, delta_s);
			assert!(share_price_initial >= exec_price);
		});
}

#[test]
fn test_share_price_in_add_shares_remove_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_003),
			(ALICE, asset_a, 52425995641788588073263117),
			(ALICE, asset_b, 52033213790329),
			(ALICE, asset_c, 119135337044269),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(767).unwrap(),
				final_amplification: NonZeroU16::new(767).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::zero(),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 52425995641788588073263117),
					AssetAmount::new(asset_b, 52033213790329),
					AssetAmount::new(asset_c, 119135337044269),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();

			let pool_account = pool_account(pool_id);
			let share_price_initial = get_share_price(pool_id, 0);
			let initial_shares = Tokens::total_issuance(pool_id);
			let desired_shares = 973798810707557758;
			let intial_a = Tokens::free_balance(asset_a, &BOB);
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				1_100_000_000_000_000_000,
			));

			let final_a = Tokens::free_balance(asset_a, &BOB);
			let delta_a = intial_a - final_a;
			let final_shares = Tokens::total_issuance(pool_id);
			let delta_s = final_shares - initial_shares;
			assert_eq!(delta_s, desired_shares);
			let exec_price = FixedU128::from_rational(delta_a, delta_s);
			assert!(share_price_initial <= exec_price);

			// Remove liquidity
			let share_price_initial = get_share_price(pool_id, 0);
			let a_initial = Tokens::free_balance(asset_a, &pool_account);
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				delta_s,
				900_000_000_000_000_000,
			));
			let a_final = Tokens::free_balance(asset_a, &pool_account);
			let delta_a = a_initial - a_final;
			let exec_price = FixedU128::from_rational(delta_a, delta_s);
			assert!(share_price_initial >= exec_price);
		});
}

#[test]
fn test_share_price_case() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 1_000_000_000_000_000_000),
			(ALICE, asset_a, 88_555_000_000_000_000_000_000),
			(ALICE, asset_b, 66_537_000_000_000_000_000_000),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 18)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(767).unwrap(),
				final_amplification: NonZeroU16::new(767).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::zero(),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 88_555_000_000_000_000_000_000),
					AssetAmount::new(asset_b, 66_537_000_000_000_000_000_000),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let pool_account = pool_account(pool_id);
			let amount = 1_000_000_000_000_000_000;
			let share_price_initial = get_share_price(pool_id, 0);
			let initial_shares = Tokens::total_issuance(pool_id);
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount)],
			));

			let final_shares = Tokens::total_issuance(pool_id);
			let delta_s = final_shares - initial_shares;
			let exec_price = FixedU128::from_rational(amount, delta_s);
			assert!(share_price_initial <= exec_price);

			// Remove liquidity
			let share_price_initial = get_share_price(pool_id, 0);
			let a_initial = Tokens::free_balance(asset_a, &pool_account);
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				delta_s,
				900_000_000_000_000_000,
			));
			let a_final = Tokens::free_balance(asset_a, &pool_account);
			let delta_a = a_initial - a_final;
			let exec_price = FixedU128::from_rational(delta_a, delta_s);
			assert!(share_price_initial >= exec_price);
		});
}
