// Integration coverage for the authority-gated HSM admin extrinsics that the
// suite otherwise leaves untested: `update_collateral_asset` and
// `remove_collateral_asset`. Buy/sell already have coverage in `hsm.rs`; these
// tests exercise only the collateral-config lifecycle (mutate/remove + guards)
// and therefore need no EVM/Hollar-minting setup.

use crate::driver::HydrationTestDriver;
use frame_support::dispatch::RawOrigin;
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_runtime::*;
use pallet_asset_registry::AssetType;
use pretty_assertions::assert_eq;
use primitives::{AssetId, Balance};
use sp_runtime::traits::One;
use sp_runtime::{Perbill, Permill};

const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/SNAPSHOT";

// Hollar and DAI already exist in the HSM snapshot; the stableswap share asset
// id is drawn from the reserved 4500-4559 range to avoid collisions.
const HOLLAR: AssetId = 222;
const COLLATERAL: AssetId = 2;
const POOL_ID: AssetId = 4542;

const ONE: Balance = 1_000_000_000_000_000_000;

// Creates a fresh stableswap pool holding Hollar + collateral and registers the
// collateral in HSM. Returns the driver so callers can keep operating on the
// same externalities.
fn setup_with_collateral() -> HydrationTestDriver {
	let driver = HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.execute(|| {
		assert_ok!(AssetRegistry::register(
			RawOrigin::Root.into(),
			Some(POOL_ID),
			Some(b"admpool".to_vec().try_into().unwrap()),
			AssetType::StableSwap,
			Some(1u128),
			None,
			None,
			None,
			None,
			true,
		));
		assert_ok!(Stableswap::create_pool(
			RuntimeOrigin::root(),
			POOL_ID,
			BoundedVec::truncate_from(vec![HOLLAR, COLLATERAL]),
			100u16,
			Permill::from_percent(1),
		));
		assert_ok!(HSM::add_collateral_asset(
			RuntimeOrigin::root(),
			COLLATERAL,
			POOL_ID,
			Permill::zero(),
			FixedU128::one(),
			Permill::zero(),
			Perbill::from_percent(70),
			None,
		));
	});
	driver
}

#[test]
fn update_collateral_asset_should_change_config_when_called_by_authority() {
	let driver = setup_with_collateral();
	driver.execute(|| {
		let before = HSM::collaterals(COLLATERAL).expect("collateral was registered in setup");
		assert_eq!(before.purchase_fee, Permill::zero());
		assert_eq!(before.buy_back_fee, Permill::zero());
		assert_eq!(before.buyback_rate, Perbill::from_percent(70));
		assert_eq!(before.max_in_holding, None);

		assert_ok!(HSM::update_collateral_asset(
			RuntimeOrigin::root(),
			COLLATERAL,
			Some(Permill::from_percent(2)),
			None,
			Some(Permill::from_percent(3)),
			Some(Perbill::from_percent(50)),
			Some(Some(1_000 * ONE)),
		));

		let after = HSM::collaterals(COLLATERAL).expect("collateral still registered after update");
		// Provided parameters changed.
		assert_eq!(after.purchase_fee, Permill::from_percent(2));
		assert_eq!(after.buy_back_fee, Permill::from_percent(3));
		assert_eq!(after.buyback_rate, Perbill::from_percent(50));
		assert_eq!(after.max_in_holding, Some(1_000 * ONE));
		// Omitted parameters preserved.
		assert_eq!(after.pool_id, POOL_ID);
		assert_eq!(after.max_buy_price_coefficient, before.max_buy_price_coefficient);
	});
}

#[test]
fn update_collateral_asset_should_fail_when_asset_not_collateral() {
	HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		assert_noop!(
			HSM::update_collateral_asset(
				RuntimeOrigin::root(),
				COLLATERAL,
				Some(Permill::from_percent(1)),
				None,
				None,
				None,
				None,
			),
			pallet_hsm::Error::<hydradx_runtime::Runtime>::AssetNotApproved
		);
	});
}

#[test]
fn remove_collateral_asset_should_clear_config_when_hsm_holding_is_empty() {
	let driver = setup_with_collateral();
	driver.execute(|| {
		// Ensure the HSM account holds none of the collateral so removal is allowed.
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			HSM::account_id(),
			COLLATERAL,
			0,
			0,
		));
		assert!(HSM::collaterals(COLLATERAL).is_some());

		assert_ok!(HSM::remove_collateral_asset(RuntimeOrigin::root(), COLLATERAL));

		assert_eq!(HSM::collaterals(COLLATERAL), None);
	});
}

#[test]
fn remove_collateral_asset_should_fail_when_hsm_still_holds_collateral() {
	let driver = setup_with_collateral();
	driver.execute(|| {
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			HSM::account_id(),
			COLLATERAL,
			ONE,
			0,
		));

		assert_noop!(
			HSM::remove_collateral_asset(RuntimeOrigin::root(), COLLATERAL),
			pallet_hsm::Error::<hydradx_runtime::Runtime>::CollateralNotEmpty
		);

		// Config must remain intact after the rejected removal.
		assert!(HSM::collaterals(COLLATERAL).is_some());
	});
}
