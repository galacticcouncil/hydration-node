// Integration coverage for value-moving extrinsics that the suite otherwise
// exercises only at the pallet level: `bonds::redeem`, `lbp::add_liquidity`,
// `lbp::remove_liquidity`, and `currencies::transfer_native_currency`.

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::*;
use orml_traits::MultiCurrency;
use pallet_lbp::WeightCurveType;
use pretty_assertions::assert_eq;
use primitives::constants::time::unix_time::MONTH;

const LBP_ASSET_A: AssetId = 4700;
const LBP_ASSET_B: AssetId = 4701;

const LBP_INIT_A: Balance = 100 * UNITS;
const LBP_INIT_B: Balance = 200 * UNITS;
const LBP_ENDOWMENT: Balance = 1_000 * UNITS;

fn lbp_pool() -> HydrationTestDriver {
	let driver = HydrationTestDriver::default()
		.register_asset(LBP_ASSET_A, b"exLBA", 12, None)
		.register_asset(LBP_ASSET_B, b"exLBB", 12, None);
	driver
		.endow_account(BOB.into(), LBP_ASSET_A, LBP_ENDOWMENT)
		.endow_account(BOB.into(), LBP_ASSET_B, LBP_ENDOWMENT)
		.execute(|| {
			// BOB owns the pool; CHARLIE is the fee collector.
			assert_ok!(LBP::create_pool(
				RuntimeOrigin::root(),
				BOB.into(),
				LBP_ASSET_A,
				LBP_INIT_A,
				LBP_ASSET_B,
				LBP_INIT_B,
				20_000_000,
				80_000_000,
				WeightCurveType::Linear,
				(2, 1_000),
				CHARLIE.into(),
				0,
			));
		});
	driver
}

fn lbp_pair_account() -> AccountId {
	LBP::pair_account_from_assets(LBP_ASSET_A, LBP_ASSET_B)
}

// ---------------------------------------------------------------------------
// bonds::redeem
// ---------------------------------------------------------------------------

#[test]
fn redeem_should_return_underlying_asset_when_bond_is_mature() {
	HydrationTestDriver::default().execute(|| {
		let treasury = Treasury::account_id();
		let amount = 100 * UNITS;

		// Keep the issuer account well above ED so it retains its provider ref
		// after the underlying is debited during `issue`.
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			treasury.clone(),
			3_000_000 * UNITS,
		));

		Timestamp::set_timestamp(NOW);
		let maturity = NOW + MONTH;

		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::root(), HDX, amount, maturity));

		// Bonds are credited 1:1 to the issuer (treasury).
		assert_eq!(Currencies::free_balance(bond_id, &treasury), amount);

		// Advance time past maturity.
		Timestamp::set_timestamp(maturity);

		let hdx_before = Currencies::free_balance(HDX, &treasury);

		assert_ok!(Bonds::redeem(RuntimeOrigin::signed(treasury.clone()), bond_id, amount,));

		// Bonds are burned and the underlying is returned 1:1.
		assert_eq!(Currencies::free_balance(bond_id, &treasury), 0);
		assert_eq!(Currencies::free_balance(HDX, &treasury), hdx_before + amount);
	});
}

#[test]
fn redeem_should_fail_when_bond_is_not_mature() {
	HydrationTestDriver::default().execute(|| {
		let treasury = Treasury::account_id();
		let amount = 100 * UNITS;

		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			treasury.clone(),
			3_000_000 * UNITS,
		));

		Timestamp::set_timestamp(NOW);
		let maturity = NOW + MONTH;

		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::root(), HDX, amount, maturity));

		// Time has not advanced to maturity.
		assert_noop!(
			Bonds::redeem(RuntimeOrigin::signed(treasury), bond_id, amount),
			pallet_bonds::Error::<Runtime>::NotMature
		);
	});
}

// ---------------------------------------------------------------------------
// lbp::add_liquidity
// ---------------------------------------------------------------------------

#[test]
fn add_liquidity_should_credit_pool_and_debit_owner_when_called_by_owner() {
	lbp_pool().execute(|| {
		let bob: AccountId = BOB.into();
		let pool = lbp_pair_account();

		let add_a = 10 * UNITS;
		let add_b = 20 * UNITS;

		let pool_a_before = Tokens::free_balance(LBP_ASSET_A, &pool);
		let pool_b_before = Tokens::free_balance(LBP_ASSET_B, &pool);
		let bob_a_before = Tokens::free_balance(LBP_ASSET_A, &bob);
		let bob_b_before = Tokens::free_balance(LBP_ASSET_B, &bob);

		assert_ok!(LBP::add_liquidity(
			RuntimeOrigin::signed(bob.clone()),
			(LBP_ASSET_A, add_a),
			(LBP_ASSET_B, add_b),
		));

		assert_eq!(Tokens::free_balance(LBP_ASSET_A, &pool) - pool_a_before, add_a);
		assert_eq!(Tokens::free_balance(LBP_ASSET_B, &pool) - pool_b_before, add_b);
		assert_eq!(bob_a_before - Tokens::free_balance(LBP_ASSET_A, &bob), add_a);
		assert_eq!(bob_b_before - Tokens::free_balance(LBP_ASSET_B, &bob), add_b);
	});
}

#[test]
fn add_liquidity_should_fail_when_caller_is_not_pool_owner() {
	lbp_pool().execute(|| {
		assert_noop!(
			LBP::add_liquidity(
				RuntimeOrigin::signed(ALICE.into()),
				(LBP_ASSET_A, 10 * UNITS),
				(LBP_ASSET_B, 20 * UNITS),
			),
			pallet_lbp::Error::<Runtime>::NotOwner
		);
	});
}

// ---------------------------------------------------------------------------
// lbp::remove_liquidity
// ---------------------------------------------------------------------------

#[test]
fn remove_liquidity_should_return_all_assets_and_destroy_pool_when_called_by_owner() {
	lbp_pool().execute(|| {
		let bob: AccountId = BOB.into();
		let pool = lbp_pair_account();

		// Pool holds the liquidity provided at creation.
		assert_eq!(Tokens::free_balance(LBP_ASSET_A, &pool), LBP_INIT_A);
		assert_eq!(Tokens::free_balance(LBP_ASSET_B, &pool), LBP_INIT_B);

		let bob_a_before = Tokens::free_balance(LBP_ASSET_A, &bob);
		let bob_b_before = Tokens::free_balance(LBP_ASSET_B, &bob);

		// Sale window was never set (start/end are None) so the pool is not
		// running and can be torn down immediately.
		assert_ok!(LBP::remove_liquidity(RuntimeOrigin::signed(bob.clone()), pool.clone()));

		assert!(LBP::pool_data(pool.clone()).is_none(), "pool should be destroyed");
		assert_eq!(Tokens::free_balance(LBP_ASSET_A, &pool), 0);
		assert_eq!(Tokens::free_balance(LBP_ASSET_B, &pool), 0);
		assert_eq!(Tokens::free_balance(LBP_ASSET_A, &bob) - bob_a_before, LBP_INIT_A);
		assert_eq!(Tokens::free_balance(LBP_ASSET_B, &bob) - bob_b_before, LBP_INIT_B);
	});
}

#[test]
fn remove_liquidity_should_fail_when_caller_is_not_pool_owner() {
	lbp_pool().execute(|| {
		let pool = lbp_pair_account();
		assert_noop!(
			LBP::remove_liquidity(RuntimeOrigin::signed(ALICE.into()), pool),
			pallet_lbp::Error::<Runtime>::NotOwner
		);
	});
}

// ---------------------------------------------------------------------------
// currencies::transfer_native_currency
// ---------------------------------------------------------------------------

#[test]
fn transfer_native_currency_should_move_hdx_between_accounts() {
	HydrationTestDriver::default().execute(|| {
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let amount = 10 * UNITS;

		let alice_before = Currencies::free_balance(HDX, &alice);
		let bob_before = Currencies::free_balance(HDX, &bob);

		assert_ok!(Currencies::transfer_native_currency(
			RuntimeOrigin::signed(alice.clone()),
			bob.clone(),
			amount,
		));

		assert_eq!(Currencies::free_balance(HDX, &alice), alice_before - amount);
		assert_eq!(Currencies::free_balance(HDX, &bob), bob_before + amount);
	});
}

#[test]
fn transfer_native_currency_should_fail_when_balance_is_insufficient() {
	HydrationTestDriver::default().execute(|| {
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();

		let alice_balance = Currencies::free_balance(HDX, &alice);

		assert_noop!(
			Currencies::transfer_native_currency(RuntimeOrigin::signed(alice), bob, alice_balance + UNITS,),
			sp_runtime::TokenError::FundsUnavailable
		);
	});
}
