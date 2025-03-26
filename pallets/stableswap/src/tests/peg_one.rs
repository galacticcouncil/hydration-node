use crate::tests::mock::*;
use crate::types::{BoundedPegSources, PegSource};
use crate::{assert_balance, Error, Event};
use hydradx_traits::stableswap::AssetAmount;

use crate::tests::to_bounded_asset_vec;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_traits::OraclePeriod;
use pallet_broadcast::types::{Asset, Destination, Fee};
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::Permill;

#[test]
fn sell_with_peg_should_work_as_before_when_all_pegs_are_one() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b]),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				])
			));

			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				30 * ONE,
				25 * ONE,
			));

			let expected = 29_902_625_420_922u128;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_a, 170 * ONE);
			assert_balance!(BOB, asset_b, expected);
			assert_balance!(pool_account, asset_a, 130 * ONE);
			assert_balance!(pool_account, asset_b, 100 * ONE - expected);
		});
}

#[test]
fn buy_should_work_as_before_when_all_pegs_are_one() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;
	let max_peg_update = Permill::from_percent(100);
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b]),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 100 * ONE),
				])
			));

			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_b,
				asset_a,
				30 * ONE,
				35 * ONE,
			));

			let expected_to_sell = 30098072706882u128;

			let pool_account = pool_account(pool_id);

			assert_balance!(BOB, asset_a, 200 * ONE - expected_to_sell);
			assert_balance!(BOB, asset_b, 30 * ONE);
			assert_balance!(pool_account, asset_a, 100 * ONE + expected_to_sell);
			assert_balance!(pool_account, asset_b, 70 * ONE);

			expect_events(vec![
				Event::BuyExecuted {
					who: BOB,
					pool_id,
					asset_in: asset_a,
					asset_out: asset_b,
					amount_in: 30098072706882,
					amount_out: 30000000000000,
					fee: 0,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB,
					filler: pool_account,
					filler_type: pallet_broadcast::types::Filler::Stableswap(pool_id),
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(asset_a, 30098072706882)],
					outputs: vec![Asset::new(asset_b, 30000000000000)],
					fees: vec![Fee::new(asset_a, 0, Destination::Account(pool_account))],
					operation_stack: vec![],
				}
				.into(),
			]);
		});
}

#[test]
fn remove_liquidity_with_peg_should_work_as_before_when_pegs_are_one() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
			(ALICE, asset_a, 100 * ONE),
			(ALICE, asset_b, 200 * ONE),
			(ALICE, asset_c, 300 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value((1, 1)),
					PegSource::Value((1, 1))
				]),
				max_peg_update,
			));

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(asset_a, 100 * ONE),
					AssetAmount::new(asset_b, 200 * ONE),
					AssetAmount::new(asset_c, 300 * ONE),
				])
			));

			let amount_added = 200 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares,
				0,
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 199999999999999u128);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);

			pretty_assertions::assert_eq!(
				*get_last_swapped_events().last().unwrap(),
				RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped {
					swapper: BOB,
					filler: pool_account,
					filler_type: pallet_broadcast::types::Filler::Stableswap(pool_id),
					operation: pallet_broadcast::types::TradeOperation::LiquidityRemove,
					inputs: vec![Asset::new(pool_id, 200516043533380244763),],
					outputs: vec![Asset::new(asset_c, 199999999999999)],
					fees: vec![Fee::new(pool_id, 0, Destination::Account(pool_account))],
					operation_stack: vec![],
				})
			);
		});
}

#[test]
fn creating_pool_with_pegs_shoud_fails_when_assets_have_different_decimals() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let max_peg_update = Permill::from_percent(100);

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
			assert_noop!(
				Stableswap::create_pool_with_pegs(
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
				),
				Error::<Test>::IncorrectAssetDecimals
			);
		});
}

#[test]
fn should_fail_when_called_by_invalid_origin() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::signed(BOB),
					pool_id,
					to_bounded_asset_vec(vec![asset_a, asset_b]),
					100,
					Permill::from_percent(0),
					BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
					max_peg_update
				),
				BadOrigin
			);
		});
}

#[test]
fn should_fail_when_invalid_amplification_specified() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::root(),
					pool_id,
					to_bounded_asset_vec(vec![asset_a, asset_b]),
					0,
					Permill::from_percent(0),
					BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
					max_peg_update,
				),
				Error::<Test>::InvalidAmplification
			);
		});
}

#[test]
fn should_fail_when_asset_decimals_are_not_same() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 18)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::root(),
					pool_id,
					to_bounded_asset_vec(vec![asset_a, asset_b]),
					100,
					Permill::from_percent(0),
					BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
					max_peg_update,
				),
				Error::<Test>::IncorrectAssetDecimals
			);
		});
}

#[test]
fn should_fail_when_no_target_peg_oracle() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;
	let max_peg_update = Permill::from_percent(100);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, 1, 200 * ONE), (ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 18)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::root(),
					pool_id,
					to_bounded_asset_vec(vec![asset_a, asset_b]),
					100,
					Permill::from_percent(0),
					BoundedPegSources::truncate_from(vec![
						PegSource::Value((1, 1)),
						PegSource::Oracle((*b"testtest", OraclePeriod::Short, asset_a)),
					]),
					max_peg_update,
				),
				Error::<Test>::MissingTargetPegOracle
			);
		});
}
