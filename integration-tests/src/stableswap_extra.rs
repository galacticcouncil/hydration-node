// Integration coverage for stableswap extrinsics that the suite exercises only
// at the pallet level: `buy`, `remove_liquidity`, `withdraw_asset_amount`, and
// the authority-gated `update_amplification`, `update_asset_peg_source`, and
// `update_pool_max_peg_update`.

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_runtime::*;
use hydradx_traits::stableswap::AssetAmount;
use orml_traits::MultiCurrency;
use pallet_stableswap::types::{BoundedPegSources, PegSource};
use pretty_assertions::assert_eq;
use sp_runtime::{Perbill, Permill};

const ASSET_A: AssetId = 4200;
const ASSET_B: AssetId = 4201;
const POOL_ID: AssetId = 4242;

const PEG_A: AssetId = 4300;
const PEG_B: AssetId = 4301;
const PEG_POOL: AssetId = 4342;

const DEC: u8 = 18;
const ONE: Balance = 1_000_000_000_000_000_000;
const ENDOWMENT: Balance = 1_000_000 * ONE;
const INITIAL_LIQUIDITY: Balance = 1_000 * ONE;

fn balance(who: [u8; 32], asset: AssetId) -> Balance {
	Tokens::free_balance(asset, &who.into())
}

fn plain_pool() -> HydrationTestDriver {
	let driver = HydrationTestDriver::default()
		.register_asset(ASSET_A, b"exSSA", DEC, None)
		.register_asset(ASSET_B, b"exSSB", DEC, None)
		.register_asset(POOL_ID, b"exSSP", DEC, None);
	driver
		.new_block()
		.endow_account(ALICE.into(), ASSET_A, ENDOWMENT)
		.endow_account(ALICE.into(), ASSET_B, ENDOWMENT)
		.endow_account(BOB.into(), ASSET_A, ENDOWMENT)
		.endow_account(BOB.into(), ASSET_B, ENDOWMENT)
		.execute(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				POOL_ID,
				BoundedVec::truncate_from(vec![ASSET_A, ASSET_B]),
				100,
				Permill::from_percent(1),
			));
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(ASSET_A, INITIAL_LIQUIDITY),
					AssetAmount::new(ASSET_B, INITIAL_LIQUIDITY),
				]),
				0,
			));
		});
	driver
}

fn peg_pool() -> HydrationTestDriver {
	let driver = HydrationTestDriver::default()
		.register_asset(PEG_A, b"exPGA", DEC, None)
		.register_asset(PEG_B, b"exPGB", DEC, None)
		.register_asset(PEG_POOL, b"exPGP", DEC, None);
	driver
		.new_block()
		.endow_account(ALICE.into(), PEG_A, ENDOWMENT)
		.endow_account(ALICE.into(), PEG_B, ENDOWMENT)
		.execute(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				PEG_POOL,
				BoundedVec::truncate_from(vec![PEG_A, PEG_B]),
				100,
				Permill::from_percent(0),
				BoundedPegSources::truncate_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]),
				Perbill::from_percent(100),
			));
			assert_ok!(Stableswap::add_assets_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				PEG_POOL,
				BoundedVec::truncate_from(vec![
					AssetAmount::new(PEG_A, INITIAL_LIQUIDITY),
					AssetAmount::new(PEG_B, INITIAL_LIQUIDITY),
				]),
				0,
			));
		});
	driver
}

#[test]
fn buy_should_credit_exact_amount_out_and_charge_within_max_when_pool_has_liquidity() {
	plain_pool().execute(|| {
		let amount_out = 10 * ONE;
		let max_sell = 20 * ONE;

		let a_before = balance(BOB, ASSET_A);
		let b_before = balance(BOB, ASSET_B);

		assert_ok!(Stableswap::buy(
			RuntimeOrigin::signed(BOB.into()),
			POOL_ID,
			ASSET_A,
			ASSET_B,
			amount_out,
			max_sell,
		));

		let a_after = balance(BOB, ASSET_A);
		let b_after = balance(BOB, ASSET_B);

		assert_eq!(a_after - a_before, amount_out);
		let spent = b_before - b_after;
		assert!(spent > 0, "buy must charge the input asset");
		assert!(spent <= max_sell, "spent {spent} exceeded max_sell {max_sell}");
	});
}

#[test]
fn buy_should_fail_when_required_amount_in_exceeds_max_sell_limit() {
	plain_pool().execute(|| {
		assert_noop!(
			Stableswap::buy(
				RuntimeOrigin::signed(BOB.into()),
				POOL_ID,
				ASSET_A,
				ASSET_B,
				10 * ONE,
				ONE, // far below the ~10 units the buy needs
			),
			pallet_stableswap::Error::<hydradx_runtime::Runtime>::SellLimitExceeded
		);
	});
}

#[test]
fn remove_liquidity_should_return_underlying_assets_when_burning_all_shares() {
	plain_pool().execute(|| {
		let shares = balance(ALICE, POOL_ID);
		assert!(shares > 0, "setup should have issued shares to ALICE");

		let a_before = balance(ALICE, ASSET_A);
		let b_before = balance(ALICE, ASSET_B);

		assert_ok!(Stableswap::remove_liquidity(
			RuntimeOrigin::signed(ALICE.into()),
			POOL_ID,
			shares,
			BoundedVec::truncate_from(vec![AssetAmount::new(ASSET_A, 0), AssetAmount::new(ASSET_B, 0),]),
		));

		assert_eq!(balance(ALICE, POOL_ID), 0, "all shares should be burned");
		assert!(balance(ALICE, ASSET_A) > a_before);
		assert!(balance(ALICE, ASSET_B) > b_before);
	});
}

#[test]
fn withdraw_asset_amount_should_credit_exact_amount_and_burn_shares_within_max() {
	plain_pool().execute(|| {
		let withdraw = 100 * ONE;
		let max_share = 500 * ONE;

		let a_before = balance(ALICE, ASSET_A);
		let shares_before = balance(ALICE, POOL_ID);

		assert_ok!(Stableswap::withdraw_asset_amount(
			RuntimeOrigin::signed(ALICE.into()),
			POOL_ID,
			ASSET_A,
			withdraw,
			max_share,
		));

		assert_eq!(balance(ALICE, ASSET_A) - a_before, withdraw);
		let burned = shares_before - balance(ALICE, POOL_ID);
		assert!(burned > 0, "withdraw must burn shares");
		assert!(burned <= max_share, "burned {burned} exceeded max_share {max_share}");
	});
}

#[test]
fn withdraw_asset_amount_should_fail_when_share_cost_exceeds_max() {
	plain_pool().execute(|| {
		assert_noop!(
			Stableswap::withdraw_asset_amount(
				RuntimeOrigin::signed(ALICE.into()),
				POOL_ID,
				ASSET_A,
				100 * ONE,
				1, // far below the shares the withdraw would burn
			),
			pallet_stableswap::Error::<hydradx_runtime::Runtime>::SlippageLimit
		);
	});
}

#[test]
fn update_amplification_should_set_final_amplification_when_scheduled_by_authority() {
	plain_pool().execute(|| {
		let current = System::block_number();
		assert_ok!(Stableswap::update_amplification(
			RuntimeOrigin::root(),
			POOL_ID,
			200,
			current + 1,
			current + 100,
		));

		let pool = Stableswap::pools(POOL_ID).expect("pool should exist");
		assert_eq!(pool.final_amplification.get(), 200);
	});
}

#[test]
fn update_pool_max_peg_update_should_change_limit_when_called_by_authority() {
	peg_pool().execute(|| {
		assert_ok!(Stableswap::update_pool_max_peg_update(
			RuntimeOrigin::root(),
			PEG_POOL,
			Perbill::from_percent(50),
		));

		let info = Stableswap::pool_peg_info(PEG_POOL).expect("peg info should exist");
		assert_eq!(info.max_peg_update, Perbill::from_percent(50));
	});
}

#[test]
fn update_asset_peg_source_should_replace_peg_source_when_called_by_authority() {
	peg_pool().execute(|| {
		assert_ok!(Stableswap::update_asset_peg_source(
			RuntimeOrigin::root(),
			PEG_POOL,
			PEG_B,
			PegSource::Value((2, 1)),
		));

		let info = Stableswap::pool_peg_info(PEG_POOL).expect("peg info should exist");
		// assets were registered in order [PEG_A, PEG_B] → PEG_B is index 1
		assert_eq!(info.source[1], PegSource::Value((2, 1)));
	});
}
