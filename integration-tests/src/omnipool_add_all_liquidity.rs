#![cfg(test)]

use crate::omnipool_init::hydra_run_to_block;
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use pallet_omnipool::types::Tradability;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

#[test]
fn add_all_liquidity_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let lp = AccountId::from(ALICE);
		let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			dot_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		hydra_run_to_block(10);

		let ed = <hydradx_runtime::Runtime as pallet_omnipool::Config>::Currency::minimum_balance(DOT);

		// Transfer most of ALICE's DOT to BOB so that the deposit amount stays within the
		// circuit-breaker's per-block add-liquidity limit (5% of the pool's reserve).
		// The DOT pool reserve after add_token is ~87_719_298_250_000, so the limit is
		// ~4_385_964_912_500. We keep 1 * UNITS (1_000_000_000_000) for ALICE to deposit.
		let keep_amount = 1 * UNITS; // 1 DOT — well within 5% of the pool
		let lp_balance = Currencies::free_balance(DOT, &lp);
		let transfer_away = lp_balance.saturating_sub(keep_amount + ed);
		assert_ok!(Currencies::transfer(
			hydradx_runtime::RuntimeOrigin::signed(lp.clone()),
			AccountId::from(BOB),
			DOT,
			transfer_away,
		));

		let lp_balance_before = Currencies::free_balance(DOT, &lp);
		assert!(lp_balance_before > ed, "LP needs more than ED to run this test");

		let position_id = hydradx_runtime::Omnipool::next_position_id();

		assert_ok!(hydradx_runtime::Omnipool::add_all_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(lp.clone()),
			DOT,
			Balance::MIN,
		));

		// LP should hold exactly the existential deposit of DOT
		let lp_balance_after = Currencies::free_balance(DOT, &lp);
		assert_eq!(lp_balance_after, ed);

		// A position NFT was created for the LP
		assert_ok!(pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(
			position_id,
			lp.clone(),
		));
	});
}

#[test]
fn add_all_liquidity_should_fail_when_add_liquidity_not_allowed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let lp = AccountId::from(ALICE);
		let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			dot_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		hydra_run_to_block(10);

		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Tradability::SELL | Tradability::BUY | Tradability::REMOVE_LIQUIDITY,
		));

		assert_noop!(
			hydradx_runtime::Omnipool::add_all_liquidity(hydradx_runtime::RuntimeOrigin::signed(lp), DOT, Balance::MIN,),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::NotAllowed,
		);
	});
}

#[test]
fn add_all_liquidity_should_fail_when_balance_is_zero() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			dot_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		hydra_run_to_block(10);

		// CHARLIE has no DOT balance
		assert_noop!(
			hydradx_runtime::Omnipool::add_all_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(AccountId::from(CHARLIE)),
				DOT,
				Balance::MIN,
			),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::InsufficientBalance,
		);
	});
}

#[test]
fn add_all_liquidity_position_matches_explicit_add_liquidity_with_limit() {
	// Verify that add_all_liquidity(asset) gives the same pool state as
	// add_liquidity_with_limit(asset, free_balance - ed, 0).
	TestNet::reset();

	let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	// Helper: reduce ALICE's DOT to a circuit-breaker-safe amount (keep 1 DOT + ED).
	// The DOT pool reserve after add_token is ~87_719_298_250_000 and the per-block
	// add-liquidity limit is 5%, so we stay well within it.
	let prepare_lp = || {
		let lp = AccountId::from(ALICE);
		let ed = <hydradx_runtime::Runtime as pallet_omnipool::Config>::Currency::minimum_balance(DOT);
		let keep = 1 * UNITS + ed;
		let balance = Currencies::free_balance(DOT, &lp);
		let transfer_away = balance.saturating_sub(keep);
		if transfer_away > 0 {
			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(lp.clone()),
				AccountId::from(BOB),
				DOT,
				transfer_away,
			));
		}
		(lp, ed)
	};

	let pool_state_via_all = {
		TestNet::reset();
		let mut state = None;
		Hydra::execute_with(|| {
			init_omnipool();
			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				dot_price,
				Permill::from_percent(100),
				AccountId::from(BOB),
			));
			hydra_run_to_block(10);

			prepare_lp();

			assert_ok!(hydradx_runtime::Omnipool::add_all_liquidity(
				hydradx_runtime::RuntimeOrigin::signed(AccountId::from(ALICE)),
				DOT,
				Balance::MIN,
			));
			state = pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_asset_state(DOT).ok();
		});
		state
	};

	let pool_state_via_limit = {
		TestNet::reset();
		let mut state = None;
		Hydra::execute_with(|| {
			init_omnipool();
			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				DOT,
				dot_price,
				Permill::from_percent(100),
				AccountId::from(BOB),
			));
			hydra_run_to_block(10);

			let (lp, ed) = prepare_lp();
			let amount = Currencies::free_balance(DOT, &lp).saturating_sub(ed);

			assert_ok!(hydradx_runtime::Omnipool::add_liquidity_with_limit(
				hydradx_runtime::RuntimeOrigin::signed(lp),
				DOT,
				amount,
				Balance::MIN,
			));
			state = pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_asset_state(DOT).ok();
		});
		state
	};

	assert_eq!(pool_state_via_all, pool_state_via_limit);
}
