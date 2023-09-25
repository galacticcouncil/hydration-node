use crate::tests::mock::*;
use crate::types::{AssetAmount, PoolInfo};
use crate::{assert_balance, to_precision, Error};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn add_initial_liquidity_should_work_when_called_first_time() {
	let pool_id: AssetId = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.build()
		.execute_with(|| {
			let asset_a: AssetId = 1;
			let asset_b: AssetId = 2;
			let amplification: u16 = 100;

			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				amplification,
				Permill::from_percent(0),
			));

			let initial_liquidity_amount = 100 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![
					AssetAmount::new(asset_a, initial_liquidity_amount),
					AssetAmount::new(asset_b, initial_liquidity_amount),
				]
			));

			assert_balance!(BOB, asset_a, 100 * ONE);
			assert_balance!(BOB, asset_b, 100 * ONE);
			assert_balance!(BOB, pool_id, 200 * ONE * 1_000_000);
			assert_balance!(pool_account, asset_a, 100 * ONE);
			assert_balance!(pool_account, asset_b, 100 * ONE);
		});
}

#[test]
fn add_initial_liquidity_should_fail_when_lp_has_insufficient_balance() {
	let pool_id: AssetId = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 20 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.build()
		.execute_with(|| {
			let asset_a: AssetId = 1;
			let asset_b: AssetId = 2;
			let amplification: u16 = 100;

			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				amplification,
				Permill::from_percent(0),
			));

			let initial_liquidity_amount = 100 * ONE;

			let pool_account = pool_account(pool_id);

			assert_noop!(
				Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					vec![
						AssetAmount::new(asset_a, initial_liquidity_amount),
						AssetAmount::new(asset_b, initial_liquidity_amount),
					]
				),
				Error::<Test>::InsufficientBalance
			);

			assert_balance!(BOB, asset_a, 200 * ONE);
			assert_balance!(BOB, asset_b, 20 * ONE);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 0u128);
			assert_balance!(pool_account, asset_b, 0u128);
		});
}
#[test]
fn add_liquidity_should_work_when_initial_liquidity_has_been_provided() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 100 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![
					AssetAmount::new(asset_a, amount_added),
					AssetAmount::new(asset_b, amount_added),
				]
			));

			assert_balance!(BOB, asset_a, 100 * ONE);
			assert_balance!(BOB, asset_b, 100 * ONE);
			assert_balance!(BOB, pool_id, 199999999999999999998);
			assert_balance!(pool_account, asset_a, 200 * ONE);
			assert_balance!(pool_account, asset_b, 200 * ONE);
		});
}

#[test]
fn add_liquidity_should_work_when_order_is_not_sorted() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 100 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![
					AssetAmount::new(asset_b, amount_added),
					AssetAmount::new(asset_a, amount_added),
				]
			));

			assert_balance!(BOB, asset_a, 100 * ONE);
			assert_balance!(BOB, asset_b, 100 * ONE);
			assert_balance!(BOB, pool_id, 199999999999999999998);
			assert_balance!(pool_account, asset_a, 200 * ONE);
			assert_balance!(pool_account, asset_b, 200 * ONE);
		});
}

#[test]
fn add_liquidity_should_fail_when_providing_insufficient_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount_added = 100;

			assert_noop!(
				Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					vec![
						AssetAmount::new(asset_b, amount_added),
						AssetAmount::new(asset_a, amount_added),
					]
				),
				Error::<Test>::InsufficientTradingAmount
			);
		});
}

#[test]
fn add_liquidity_should_work_when_providing_one_asset_only() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
			(ALICE, asset_d, 400 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c, asset_d].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
					AssetAmount::new(asset_d, 400 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount_added = 200 * ONE;

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount_added),]
			));
		});
}

#[test]
fn add_liquidity_should_fail_when_providing_one_asset_not_in_pool() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;
	let asset_e: AssetId = 5;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(BOB, asset_e, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
			(ALICE, asset_d, 400 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d, 12)
		.with_registered_asset("five".as_bytes().to_vec(), asset_e, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c, asset_d].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
					AssetAmount::new(asset_d, 400 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount_added = 200 * ONE;

			assert_noop!(
				Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					vec![
						AssetAmount::new(asset_a, amount_added),
						AssetAmount::new(asset_e, amount_added),
					]
				),
				Error::<Test>::AssetNotInPool
			);
		});
}

#[test]
fn add_liquidity_should_fail_when_provided_list_contains_same_assets() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 200 * ONE),
			(BOB, 2, 200 * ONE),
			(ALICE, 1, 200 * ONE),
			(ALICE, 2, 200 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(100).unwrap(),
				final_amplification: NonZeroU16::new(100).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount_added = 100 * ONE;
			assert_noop!(
				Stableswap::add_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					vec![
						AssetAmount::new(asset_a, amount_added),
						AssetAmount::new(asset_a, amount_added),
					]
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn add_initial_liquidity_should_work_when_asset_have_different_decimals() {
	let pool_id: AssetId = 100u32;
	let asset_a: u32 = 1;
	let asset_b: u32 = 2;
	let dec_a: u8 = 18;
	let dec_b: u8 = 6;
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, to_precision!(200, dec_a)),
			(BOB, asset_b, to_precision!(200, dec_b)),
			(ALICE, asset_a, to_precision!(200, dec_a)),
			(ALICE, asset_b, to_precision!(200, dec_b)),
		])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, dec_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, dec_b)
		.build()
		.execute_with(|| {
			let amplification: u16 = 100;
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				amplification,
				Permill::from_percent(0),
			));

			let initial_liquidity_amount_a = to_precision!(100, dec_a);
			let initial_liquidity_amount_b = to_precision!(100, dec_b);

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![
					AssetAmount::new(asset_a, initial_liquidity_amount_a),
					AssetAmount::new(asset_b, initial_liquidity_amount_b),
				]
			));

			assert_balance!(BOB, asset_a, to_precision!(100, dec_a));
			assert_balance!(BOB, asset_b, to_precision!(100, dec_b));
			assert_balance!(BOB, pool_id, 200 * ONE * 1_000_000);
			assert_balance!(pool_account, asset_a, to_precision!(100, dec_a));
			assert_balance!(pool_account, asset_b, to_precision!(100, dec_b));
		});
}

#[test]
fn add_liquidity_should_work_correctly() {
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
				//fee: Permill::from_float(0.0001),
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
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount),]
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 1947597621401945851);
		});
}

#[test]
fn add_liquidity_should_work_correctly_when_fee_is_applied() {
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
				fee: Permill::from_float(0.0001),
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
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				vec![AssetAmount::new(asset_a, amount),]
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 1947487201901031408);
		});
}

#[test]
fn add_liquidity_should_work_correctly_when_providing_exact_amount_of_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 2_000_000_000_000_000_003),
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
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				1947597621401945851,
				asset_a,
				amount + 3, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 1947597621401945851);

			let used = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(used, 0);
		});
}

#[test]
fn add_liquidity_should_apply_fee_when_providing_exact_amount_of_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 3_000_000_000_000_000_000),
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
				fee: Permill::from_percent(1),
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
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				1947597621401945851,
				asset_a,
				3_000_000_000_000_000_000,
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 1947597621401945851);

			let used = 3_000_000_000_000_000_000 - Tokens::free_balance(asset_a, &BOB);
			assert_eq!(used, 2_011_482_020_765_837_587);
		});
}

#[test]
fn add_liquidity_shares_should_fail_when_pool_is_empty() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 2_000_000_000_000_000_003),
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
				assets: vec![],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			assert_noop!(
				Stableswap::add_liquidity_shares(
					RuntimeOrigin::signed(BOB),
					pool_id,
					1947597621401945851,
					asset_a,
					amount + 3, // add liquidity for shares uses slightly more
				),
				Error::<Test>::InvalidInitialLiquidity
			);
		});
}
