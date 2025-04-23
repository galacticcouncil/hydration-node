use crate::tests::mock::*;
use crate::tests::to_bounded_asset_vec;
use crate::types::{BoundedPegSources, PegSource, PoolInfo};
use crate::{assert_balance, Error, Event, PoolPegs, Pools};
use frame_support::traits::Contains;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_traits::stableswap::AssetAmount;
use pallet_broadcast::types::{Asset, Destination, Fee};
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn remove_liquidity_should_work_when_withdrawing_all_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

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
				RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
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
fn remove_liquidity_should_apply_fee_when_withdrawing_all_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

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
				0
			));

			let amount_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_c, 199_999_999_999_999);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 100 * ONE + amount_added);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_received);
		});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(ALICE), 0u32, 1u32, 0u128, 0),
			Error::<Test>::InvalidAssetAmount
		);
	});
}

#[test]
fn remove_liquidity_should_fail_when_shares_is_insufficient() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, pool_id, 100 * ONE)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 200 * ONE, 0),
				Error::<Test>::InsufficientShares
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_pool_does_not_exists() {
	let pool_id = 100u32;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, pool_id, 100 * ONE)])
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, 1u32, 100 * ONE, 0),
				Error::<Test>::PoolNotFound
			);
		});
}

#[test]
fn remove_liquidity_should_fail_when_requested_asset_not_in_pool() {
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
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, asset_d, shares, 0),
				Error::<Test>::AssetNotInPool
			);
		});
}

#[test]
fn remove_liquidity_should_pass_when_remaining_shares_below_min_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				shares - MinimumLiquidity::get() + 1,
				0,
			),);
		});
}

#[test]
fn verify_remove_liquidity_against_research_impl() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 100_000 * ONE),
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
			(ALICE, asset_c, 1_000_000 * ONE),
			(ALICE, asset_d, 1_000_000 * ONE),
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
				fee: Permill::from_float(0.003),
			},
			InitialLiquidity {
				account: ALICE,
				assets: vec![
					AssetAmount::new(asset_a, 1_000_000 * ONE),
					AssetAmount::new(asset_b, 1_000_000 * ONE),
					AssetAmount::new(asset_c, 1_000_000 * ONE),
					AssetAmount::new(asset_d, 1_000_000 * ONE),
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 100_000 * ONE;

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
				asset_b,
				shares,
				0
			));

			let amount_received = Tokens::free_balance(asset_b, &BOB);
			assert_balance!(BOB, asset_a, 0u128);
			assert_balance!(BOB, asset_b, 99603197897583876);
			assert_balance!(BOB, pool_id, 0u128);
			assert_balance!(pool_account, asset_a, 1_000_000 * ONE + amount_added);
			assert_balance!(pool_account, asset_b, 1_000_000 * ONE - amount_received);
			assert_balance!(pool_account, asset_b, 900396802102416124);
		});
}

#[test]
fn remove_liquidity_fail_when_desired_min_limit_is_not_reached() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);
			assert_noop!(
				Stableswap::remove_liquidity_one_asset(RuntimeOrigin::signed(BOB), pool_id, asset_c, shares, 200 * ONE,),
				Error::<Test>::SlippageLimit,
			);
		});
}

#[test]
fn scenario_add_remove_with_different_decimals() {
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
		.with_pool(
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
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_c, 20 * one_c)])
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
fn scenario_sell_with_different_decimals() {
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
		.with_pool(
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
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_c,
				asset_a,
				20 * one_c,
				0,
			));

			let balance_received = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance_received, 19999999600399608220);
		});
}

#[test]
fn specific_scenario_to_verify_remove_liquidity() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
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
				fee: Permill::from_percent(0),
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
			let to_withdraw = 599540994996813062914899;
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				to_withdraw,
				0,
			));

			let received_remove_liq = Tokens::free_balance(asset_b, &ALICE);
			assert_eq!(received_remove_liq, 615_665_495_436);
		});
}

#[test]
fn specific_scenario_to_verify_withdrawal_exact_amount() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
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
				fee: Permill::from_percent(0),
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
			let expected_shares_to_use = 599540994996813062914899;
			let exact_amount = 615_665_495_436;
			let shares = Tokens::free_balance(pool_id, &ALICE);
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				exact_amount,
				expected_shares_to_use,
			));
			let remaining_shares = Tokens::free_balance(pool_id, &ALICE);
			let shares_used = shares - remaining_shares;
			assert_eq!(shares_used, 599540994996118902897172);
		});
}

#[test]
fn specific_scenario_to_verify_difference() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, asset_a, 200 * ONE),
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
				fee: Permill::from_percent(0),
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
			let to_withdraw = 1986695389615175;
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				asset_b,
				to_withdraw,
				0,
			));
			let received_remove_liq_diff = Tokens::free_balance(asset_b, &ALICE);
			assert_eq!(received_remove_liq_diff, 2040)
		});
}

#[test]
fn scenario_3_trade() {
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
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				asset_b,
				amount,
				0,
			));
			let received = Tokens::free_balance(asset_b, &BOB);
			assert_eq!(received, 1_999_786);
		});
}

#[test]
fn removing_liquidity_with_exact_amount_should_work() {
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
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount + 3, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, desired_shares);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 0);
			// ACT
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				amount - 1,
				desired_shares,
			));

			// ASSERT

			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 0);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 1_999_999_999_999_999_999);
		});
}

#[test]
fn removing_liquidity_with_exact_amount_should_work_when_dust_left() {
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
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount + 3, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, desired_shares);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 0);
			// ACT
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				amount - 100,
				desired_shares,
			));

			// ASSERT

			let leftover = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(leftover, 96);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 1_999_999_999_999_999_900);
		});
}

#[test]
fn removing_liquidity_should_not_give_more_assets() {
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
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount + 3, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, desired_shares);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 0);
			// ACT
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				desired_shares,
				0,
			));

			// ASSERT
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, 0);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 1_999_999_999_999_999_996);
		});
}

#[test]
fn removing_liquidity_with_exact_amount_should_apply_fee() {
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
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount * 2, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, desired_shares);
			let balance = Tokens::free_balance(asset_a, &BOB);
			let amount_used = 3_000_000_000_000_000_003 - balance;
			assert_eq!(amount_used, 2011482020765837587);
			// ACT
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				1_000_000_000_000_000_000,
				desired_shares,
			));

			// ASSERT
			let shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(shares_left, 968209693349892648);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 1988517979234162416);
		});
}

#[test]
fn removing_liquidity_with_exact_amount_should_emit_swapped_event() {
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
			let amount = 2_000_000_000_000_000_000;
			Tokens::withdraw(pool_id, &ALICE, 5906657405945079804575283).unwrap();
			let desired_shares = 1947597621401945851;
			assert_ok!(Stableswap::add_liquidity_shares(
				RuntimeOrigin::signed(BOB),
				pool_id,
				desired_shares,
				asset_a,
				amount * 2, // add liquidity for shares uses slightly more
			));
			let received = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(received, desired_shares);
			let balance = Tokens::free_balance(asset_a, &BOB);
			let amount_used = 3_000_000_000_000_000_003 - balance;
			assert_eq!(amount_used, 2011482020765837587);
			// ACT
			assert_ok!(Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_a,
				1_000_000_000_000_000_000,
				desired_shares,
			));

			// ASSERT
			let shares_left = Tokens::free_balance(pool_id, &BOB);
			assert_eq!(shares_left, 968209693349892648);
			let balance = Tokens::free_balance(asset_a, &BOB);
			assert_eq!(balance, 1988517979234162416);
			let pool_account = pool_account(pool_id);

			pretty_assertions::assert_eq!(
				*get_last_swapped_events().last().unwrap(),
				RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
					swapper: BOB,
					filler: pool_account,
					filler_type: pallet_broadcast::types::Filler::Stableswap(4),
					operation: pallet_broadcast::types::TradeOperation::LiquidityRemove,
					inputs: vec![Asset::new(pool_id, 979387928052053203)],
					outputs: vec![Asset::new(asset_a, 1000000000000000000),],
					fees: vec![
						Fee::new(asset_a, 2870505165609705, Destination::Account(pool_account)),
						Fee::new(asset_b, 872, Destination::Account(pool_account)),
						Fee::new(asset_c, 1998, Destination::Account(pool_account))
					],
					operation_stack: vec![],
				})
			);
		});
}

#[test]
fn remove_multi_asset_liquidity_should_work_when_withdrawing_some_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			let min_amounts = vec![
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_b,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_c,
					amount: 0,
				},
			];

			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				shares,
				BoundedVec::try_from(min_amounts).unwrap(),
			));

			let amount_a_received = Tokens::free_balance(asset_a, &BOB);
			let amount_b_received = Tokens::free_balance(asset_b, &BOB);
			let amount_c_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 75_206_785_577_717u128);
			assert_balance!(BOB, asset_b, 50_137_857_051_811u128);
			assert_balance!(BOB, asset_c, 75_206_785_577_717u128);
			assert_balance!(BOB, pool_id, 0u128);

			assert_balance!(pool_account, asset_a, 300 * ONE - amount_a_received);
			assert_balance!(pool_account, asset_b, 200 * ONE - amount_b_received);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_c_received);
		});
}

#[test]
fn remove_multi_asset_liquidity_should_work_when_withdrawing_all_remaining_shares() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			let pool_account = pool_account(pool_id);

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let shares = Tokens::free_balance(pool_id, &BOB);

			let min_amounts = vec![
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_b,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_c,
					amount: 0,
				},
			];

			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				shares,
				BoundedVec::try_from(min_amounts.clone()).unwrap(),
			));

			let amount_a_received = Tokens::free_balance(asset_a, &BOB);
			let amount_b_received = Tokens::free_balance(asset_b, &BOB);
			let amount_c_received = Tokens::free_balance(asset_c, &BOB);
			assert_balance!(BOB, asset_a, 75_206_785_577_717u128);
			assert_balance!(BOB, asset_b, 50_137_857_051_811u128);
			assert_balance!(BOB, asset_c, 75_206_785_577_717u128);
			assert_balance!(BOB, pool_id, 0u128);

			assert_balance!(pool_account, asset_a, 300 * ONE - amount_a_received);
			assert_balance!(pool_account, asset_b, 200 * ONE - amount_b_received);
			assert_balance!(pool_account, asset_c, 300 * ONE - amount_c_received);

			let pool_a_balance = Tokens::free_balance(asset_a, &pool_account);
			let pool_b_balance = Tokens::free_balance(asset_b, &pool_account);
			let pool_c_balance = Tokens::free_balance(asset_c, &pool_account);

			// Alice has all remaining shares and should receive all remaining reserve amounts
			let shares = Tokens::free_balance(pool_id, &ALICE);
			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				shares,
				BoundedVec::try_from(min_amounts).unwrap(),
			));

			assert_balance!(ALICE, asset_a, pool_a_balance);
			assert_balance!(ALICE, asset_b, pool_b_balance);
			assert_balance!(ALICE, asset_c, pool_c_balance);

			// Ensure that pool has been removed
			// Ensure that pool account has been removed from dust list
			assert!(Pools::<Test>::get(pool_id).is_none());
			assert!(!Whitelist::contains(&pool_account));

			// Ensure events are emitted
			expect_events(vec![
				Event::PoolDestroyed { pool_id }.into(),
				Event::LiquidityRemoved {
					pool_id,
					who: ALICE,
					shares,
					amounts: vec![
						AssetAmount {
							asset_id: asset_a,
							amount: pool_a_balance,
						},
						AssetAmount {
							asset_id: asset_b,
							amount: pool_b_balance,
						},
						AssetAmount {
							asset_id: asset_c,
							amount: pool_c_balance,
						},
					],
					fee: 0u128,
				}
				.into(),
			]);
		});
}

#[test]
fn remove_multi_asset_liquidity_fails_when_min_amounts_length_is_not_correct() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let min_amounts = vec![
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_c,
					amount: 0,
				},
			];

			let shares = Tokens::free_balance(pool_id, &BOB);
			assert_noop!(
				Stableswap::remove_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares,
					BoundedVec::try_from(min_amounts).unwrap(),
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn remove_multi_asset_liquidity_fails_when_min_amounts_contains_duplicate_assets() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;

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
		.with_pool(
			ALICE,
			PoolInfo::<AssetId, u64> {
				assets: vec![asset_a, asset_b, asset_c].try_into().unwrap(),
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
				],
			},
		)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			let pool_id = get_pool_id_at(0);

			let amount_added = 200 * ONE;

			assert_ok!(Stableswap::add_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				BoundedVec::truncate_from(vec![AssetAmount::new(asset_a, amount_added)])
			));

			let min_amounts = vec![
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_c,
					amount: 0,
				},
			];

			let shares = Tokens::free_balance(pool_id, &BOB);
			assert_noop!(
				Stableswap::remove_liquidity(
					RuntimeOrigin::signed(BOB),
					pool_id,
					shares,
					BoundedVec::try_from(min_amounts).unwrap(),
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn remove_all_liquidity_should_correctly_destroy_pool_when_pool_has_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id: AssetId = 100;
	let peg2 = (1, 2);
	let peg3 = (1, 3);

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
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 18)
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				to_bounded_asset_vec(vec![asset_a, asset_b, asset_c]),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![
					PegSource::Value((1, 1)),
					PegSource::Value(peg2),
					PegSource::Value(peg3)
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

			let min_amounts = vec![
				AssetAmount {
					asset_id: asset_a,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_b,
					amount: 0,
				},
				AssetAmount {
					asset_id: asset_c,
					amount: 0,
				},
			];

			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				pool_id,
				shares,
				BoundedVec::try_from(min_amounts.clone()).unwrap(),
			));

			// Alice has all remaining shares and should receive all remaining reserve amounts
			let shares = Tokens::free_balance(pool_id, &ALICE);
			assert_ok!(Stableswap::remove_liquidity(
				RuntimeOrigin::signed(ALICE),
				pool_id,
				shares,
				BoundedVec::try_from(min_amounts).unwrap(),
			));

			// Ensure that pool has been removed
			// Ensure that pool account has been removed from dust list
			assert!(Pools::<Test>::get(pool_id).is_none());
			assert!(PoolPegs::<Test>::get(pool_id).is_none());
			assert!(!Whitelist::contains(&pool_account));
		});
}
