use crate::tests::mock::*;
use crate::tests::to_bounded_asset_vec;
use crate::tests::{get_share_price, spot_price};
use crate::types::PoolInfo;
use crate::types::{BoundedPegSources, PegSource};
use crate::{assert_balance, to_precision, Error};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hex_literal::hex;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use primitives::EvmAddress;
use sp_runtime::{FixedU128, Perbill, Permill};
use std::num::NonZeroU16;
use test_utils::assert_eq_approx;

#[test]
fn creating_pool_should_work_when_all_sources_are_value_type() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Perbill::from_percent(100);

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
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				2000,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value((1, 1)),
					PegSource::Value((1, 1))
				]),
				max_peg_update,
			));
		});
}

#[test]
fn creating_pool_should_work_when_all_sources_are_mmoracle_type() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Perbill::from_percent(100);

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
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				2000,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000000").as_slice()
					)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000000").as_slice()
					)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000000").as_slice()
					)),
				]),
				max_peg_update,
			));
		});
}

#[test]
fn creating_pool_should_work_when_all_sources_are_value_or_mmoracle_type() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Perbill::from_percent(100);

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
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				2000,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000000").as_slice()
					)),
					PegSource::Value((1, 1)),
					PegSource::Value((1, 1)),
				]),
				max_peg_update,
			));
		});
}

#[test]
fn creating_pool_should_fail_when_all_source_are_not_value_or_mmoracle_type() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Perbill::from_percent(100);

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
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.build()
		.execute_with(|| {
			set_peg_oracle_value(asset_a, asset_c, (1, 1), 1);
			assert_noop!(
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::root(),
					pool_id,
					to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
					2000,
					Permill::from_percent(0),
					BoundedPegSources::truncate_from(vec![
						PegSource::MMOracle(EvmAddress::from_slice(
							hex!("0000000000000000000000000000000000000000").as_slice()
						)),
						PegSource::Value((1, 1)),
						PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					]),
					max_peg_update,
				),
				Error::<Test>::IncorrectAssetDecimals
			);
		});
}

#[test]
fn add_initial_liquidity_should_work_when_pegs_are_same_value() {
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
			let max_peg_update = Perbill::from_percent(100);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b]),
				amplification,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000000").as_slice()
					)),
				]),
				max_peg_update,
			));

			let initial_liquidity_amount_a = to_precision!(100, dec_a);
			let initial_liquidity_amount_b = to_precision!(100, dec_b);

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, initial_liquidity_amount_a),
					AssetAmount::new(asset_b, initial_liquidity_amount_b),
				]),
				Balance::zero(),
			));

			assert_balance!(BOB, asset_a, to_precision!(100, dec_a));
			assert_balance!(BOB, asset_b, to_precision!(100, dec_b));
			assert_balance!(BOB, pool_id, 200 * ONE * 1_000_000);
			assert_balance!(pool_account, asset_a, to_precision!(100, dec_a));
			assert_balance!(pool_account, asset_b, to_precision!(100, dec_b));
		});
}

#[test]
fn add_liquidity_should_work_correctly_when_pegs_are_same_value() {
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
		.with_pool_with_pegs(
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
			vec![
				PegSource::Value((1, 1)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
			],
			None,
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(
				pool_id,
				&ALICE,
				5906657405945079804575283,
				frame_support::traits::ExistenceRequirement::AllowDeath,
			)
			.unwrap();
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount),]),
				Balance::zero(),
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 1947597621401945851);
		});
}

#[test]
fn remove_liquidity_should_work_when_pegs_are_same_value() {
	let asset_a: AssetId = 2; // DAI
	let asset_b: AssetId = 7; // USDC
	let asset_c: AssetId = 10; // USDT

	let dec_a: u32 = 18;
	let dec_b: u32 = 6;
	let dec_c: u32 = 6;

	let one_a = 10u128.pow(dec_a);
	let one_b = 10u128.pow(dec_b);
	let one_c = 10u128.pow(dec_c);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_c, 20 * one_c),
			(ALICE, asset_a, 3000000 * one_a),
			(ALICE, asset_b, 2000000 * one_b),
			(ALICE, asset_c, 1000000 * one_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 18)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 6)
		.with_pool_with_pegs(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_float(0.0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000 * one_a),
					AssetAmount::new(asset_b, 1_000_000 * one_b),
					AssetAmount::new(asset_c, 1_000_000 * one_c),
				],
			},
			vec![
				PegSource::Value((1, 1)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
			],
			None,
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_c, 20 * one_c)]),
				Balance::zero(),
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				shares,
				0,
			));

			let balance_received = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance_received, 19999999600399608220);
		});
}

#[test]
fn sell_should_work_when_pegs_are_same_value() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("three".as_bytes().to_vec(), 3, 18)
		.with_endowed_accounts(vec![
			(BOB, asset_c, ONE * 1_000_000),
			(ALICE, asset_a, 2000 * ONE),
			(ALICE, asset_b, 4000 * ONE),
			(ALICE, asset_c, 1000 * ONE * 1_000_000),
		])
		.with_pool_with_pegs(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000_000_000_000),
					AssetAmount::new(asset_b, 3_000_000_000_000_000),
					AssetAmount::new(asset_c, 1000 * ONE * 1_000_000),
				],
			},
			vec![
				PegSource::Value((1, 1)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
			],
			None,
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				asset_b,
				ONE * 1_000_000,
				0,
			));

			let expected = 1_001_709_976_613;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_c, 0);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_c, 1_001_000_000_000_000_000_000);
			assert_balance!(pool_account, asset_b, 3_000 * ONE - expected);
		});
}

#[test]
fn buy_should_work_when_pegs_are_same_value() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), 1, 12)
		.with_registered_asset("two".as_bytes().to_vec(), 2, 12)
		.with_registered_asset("three".as_bytes().to_vec(), 3, 18)
		.with_endowed_accounts(vec![
			(BOB, asset_c, ONE * 1_000_000),
			(ALICE, asset_a, 2000 * ONE),
			(ALICE, asset_b, 4000 * ONE),
			(ALICE, asset_c, 10000 * ONE * 1_000_000),
		])
		.with_pool_with_pegs(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				initial_amplification: NonZeroU16::new(1000).unwrap(),
				final_amplification: NonZeroU16::new(1000).unwrap(),
				initial_block: 0,
				final_block: 0,
				fee: Permill::from_percent(0),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000_000_000_000),
					AssetAmount::new(asset_b, 3_000_000_000_000_000),
					AssetAmount::new(asset_c, 1000 * ONE * 1_000_000),
				],
			},
			vec![
				PegSource::Value((1, 1)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
				PegSource::MMOracle(EvmAddress::from_slice(
					hex!("0000000000000000000000000000000000000000").as_slice(),
				)),
			],
			None,
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let buy_amount = 1_001_709_976_614;

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_c,
				buy_amount,
				2 * ONE * 1_000_000,
			));

			let paid = 999999999999187343;
			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_c, 1_000_000_000_000_000_000 - paid);
			assert_balance!(BOB, asset_b, buy_amount);
			assert_balance!(pool_account, asset_c, 1000 * ONE * 1_000_000 + paid);
			assert_balance!(pool_account, asset_b, 3_000_000_000_000_000 - buy_amount);
		});
}

#[test]
fn add_liquidity_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454; //12 dec
	let liquid_b = 1_090_909_090_909; //6 dec
	let liquid_c = 363_636_363_636_363_636; //12 dec

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_b, 10_000_000),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10_000_000),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465809610388610);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				bob_shares,
				1
			));

			assert!(Tokens::free_balance(asset_b, &BOB) < 10_000_000);
		});
}

#[test]
fn remove_liquidity_for_one_asset_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);

	let max_peg_update = Perbill::from_percent(100);
	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_b, 10_000_000),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10_000_000),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465809610388610);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				4713465809610388610,
				0,
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 0);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 9311926);
		});
}

#[test]
fn remove_liquidity_given_asset_amount_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_b, 10_000_000),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10_000_000),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465809610388610);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				9_000_000,
				u128::MAX,
			));

			let bob_shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_left, 163475652141724521);
			let bob_shares_used = 4713465809610388610u128 - Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_used, 4549990157468664089);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 9_000_000);
		});
}

#[test]
fn remove_liquidity_uniform_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_b, 10_000_000),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_b, 10_000_000),]),
				Balance::zero(),
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465809610388610);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 0);

			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				bob_shares,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, 0),
					AssetAmount::new(asset_b, 0),
					AssetAmount::new(asset_c, 0)
				])
			));

			let bob_shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares_left, 0);
			let bob_a_balance = Tokens::free_balance(asset_a, &BOB);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			let bob_c_balance = Tokens::free_balance(asset_c, &BOB);
			assert_eq!(bob_a_balance, 2_121_636_404_425);
			assert_eq!(bob_b_balance, 4_243_311);
			assert_eq!(bob_c_balance, 1_414_424_269_616);
		});
}

#[test]
fn sell_with_different_peg_should_work() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				100 * ONE,
				25_000_000,
			));

			assert_balance!(BOB, asset_a, 0);
			assert_balance!(BOB, asset_b, 190_961_825);
		});
}

#[test]
fn share_pries_should_be_correct_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			let share0 = get_share_price(pool_id, 0);
			assert_eq!(share0, FixedU128::from_float(0.000001000748975604));

			let share1 = get_share_price(pool_id, 1);
			assert_eq!(share1, FixedU128::from_float(0.000000000002001498));

			let share2 = get_share_price(pool_id, 2);
			assert_eq!(share2, FixedU128::from_float(0.000002988112571400));
		});
}

#[test]
fn spot_prices_should_be_correct_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			let spot1 = spot_price(pool_id, asset_a, asset_b);
			assert_eq_approx!(
				spot1,
				FixedU128::from_float(0.5),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
			let spot2 = spot_price(pool_id, asset_a, asset_c);
			assert_eq_approx!(
				spot2,
				FixedU128::from_float(0.334_910_065_029_717_1),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
			let spot2 = spot_price(pool_id, asset_c, asset_a);
			assert_eq_approx!(
				spot2,
				FixedU128::from_float(2.985_876_222_953_372_4),
				FixedU128::from_float(0.0000000001),
				"spot price not equal"
			);
		});
}

#[test]
fn add_liquidity_shares_should_work_correctly_with_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_b, 10_000_000),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			System::set_block_number(1);
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			System::set_block_number(2);

			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				4713465809610388610,
				asset_b,
				u128::MAX,
			));

			let bob_shares = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(bob_shares, 4713465809610388610);
			let bob_b_balance = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(bob_b_balance, 12261);
		});
}

#[test]
fn sell_with_peg_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg2 = (1, 2);
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			assert_balance!(BOB, asset_b, 0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				100 * ONE,
				25_000_000,
			));

			assert_balance!(BOB, asset_a, 0);
			assert_balance!(BOB, asset_b, 199999949);
		});
}

#[test]
fn buy_with_peg_should_work_different_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let peg3 = (1, 3);
	let max_peg_update = Perbill::from_percent(100);

	let liquid_a = 545_454_545_454_545_454;
	let liquid_b = 1_090_909_090_909;
	let liquid_c = 363_636_363_636_363_636;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, 1, 100 * ONE + 1),
			(ALICE, asset_a, liquid_a),
			(ALICE, asset_b, liquid_b),
			(ALICE, asset_c, liquid_c),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 6)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				amp,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::MMOracle(EvmAddress::from_slice(
						hex!("0000000000000000000000000000000000000001").as_slice(),
					)),
					PegSource::Value(peg3)
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, liquid_a),
					AssetAmount::new(asset_b, liquid_b),
					AssetAmount::new(asset_c, liquid_c),
				]),
				Balance::zero(),
			));

			assert_balance!(BOB, asset_b, 0);
			let bob_a_initial = Tokens::free_balance(asset_a, &BOB);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				199999949,
				250 * ONE,
			));

			let bob_a_final = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(bob_a_initial - bob_a_final, 99_999_999_277_208);
			assert_balance!(BOB, asset_b, 199_999_949);
		});
}
