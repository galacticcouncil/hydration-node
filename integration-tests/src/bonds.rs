#![cfg(test)]

use crate::assert_balance;
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use xcm_emulator::TestExt;

use hydradx_runtime::{AssetRegistry, Bonds, Currencies, Runtime, RuntimeOrigin};
use hydradx_traits::{AssetKind, CreateRegistry};
use primitives::constants::time::unix_time::MONTH;

#[test]
fn issue_bonds_should_work_when_issued_for_native_asset() {
	Hydra::execute_with(|| {
		// Arrange
		let amount = 100 * UNITS;
		let fee = <Runtime as pallet_bonds::Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

		let maturity = NOW + MONTH;

		// Act
		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE.into()), HDX, amount, maturity));

		// Assert
		assert_eq!(Bonds::bond(bond_id).unwrap(), (HDX, maturity));

		let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

		assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
		assert!(bond_asset_details.name.is_none());
		assert_eq!(bond_asset_details.existential_deposit, NativeExistentialDeposit::get());

		assert_balance!(&ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount);
		assert_balance!(&ALICE.into(), bond_id, amount_without_fee);

		assert_balance!(&<Runtime as pallet_bonds::Config>::FeeReceiver::get(), HDX, fee);

		assert_balance!(&Bonds::pallet_account_id(), HDX, amount_without_fee);
	});
}

#[test]
fn issue_bonds_should_work_when_issued_for_shared_asset() {
	Hydra::execute_with(|| {
		// Arrange
		let amount = 100 * UNITS;
		let fee = <Runtime as pallet_bonds::Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

		let maturity = NOW + MONTH;

		let name = b"SHARED".to_vec();
		let shared_asset_id = AssetRegistry::create_asset(
			None,
			Some(&name),
			pallet_asset_registry::AssetType::PoolShare(HDX, DOT).into(),
			1_000,
			false,
		)
		.unwrap();
		assert_ok!(Currencies::deposit(shared_asset_id, &ALICE.into(), amount,));

		// Act
		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(
			RuntimeOrigin::signed(ALICE.into()),
			shared_asset_id,
			amount,
			maturity
		));

		// Assert
		assert_eq!(Bonds::bond(bond_id).unwrap(), (shared_asset_id, maturity));

		let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

		assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
		assert!(bond_asset_details.name.is_none());
		assert_eq!(bond_asset_details.existential_deposit, 1_000);

		assert_balance!(&ALICE.into(), shared_asset_id, 0);
		assert_balance!(&ALICE.into(), bond_id, amount_without_fee);

		assert_balance!(
			&<Runtime as pallet_bonds::Config>::FeeReceiver::get(),
			shared_asset_id,
			fee
		);

		assert_balance!(&Bonds::pallet_account_id(), shared_asset_id, amount_without_fee);
	});
}

#[test]
fn issue_bonds_should_not_work_when_issued_for_bond_asset() {
	Hydra::execute_with(|| {
		// Arrange
		let amount = 100 * UNITS;
		let maturity = NOW + MONTH;

		let name = b"BOND".to_vec();
		let underlying_asset_id =
			AssetRegistry::create_asset(None, Some(&name), AssetKind::Bond, 1_000, false).unwrap();
		assert_ok!(Currencies::deposit(underlying_asset_id, &ALICE.into(), amount,));

		// Act & Assert
		assert_noop!(
			Bonds::issue(
				RuntimeOrigin::signed(ALICE.into()),
				underlying_asset_id,
				amount,
				maturity
			),
			pallet_bonds::Error::<hydradx_runtime::Runtime>::DisallowedAseet
		);
	});
}
