#![cfg(test)]

use crate::polkadot_test_net::*;
use crate::assert_balance;

use frame_support::assert_ok;
use sp_runtime::BoundedVec;
use xcm_emulator::TestExt;
use orml_traits::MultiCurrency;

use hydradx_runtime::{AssetRegistry, Bonds, Currencies, Runtime, RuntimeOrigin};
use pallet_bonds::Bond;
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
		let bond_asset_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE.into()), HDX, amount, maturity));

		// Assert
		expect_hydra_events(vec![pallet_bonds::Event::BondTokenCreated {
			issuer: ALICE.into(),
			asset_id: HDX,
			bond_asset_id,
			amount: amount_without_fee,
			fee,
		}
		.into()]);

		assert_eq!(
			Bonds::bonds(bond_asset_id).unwrap(),
			Bond {
				maturity,
				asset_id: HDX,
				amount: amount_without_fee,
			}
		);

		let bond_asset_details = AssetRegistry::assets(bond_asset_id).unwrap();

		assert!(bond_asset_details.asset_type == pallet_asset_registry::AssetType::Bond);
		assert!(bond_asset_details.name.is_empty());
		assert_eq!(bond_asset_details.existential_deposit, NativeExistentialDeposit::get());

		assert_balance!(&ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount);
		assert_balance!(&ALICE.into(), bond_asset_id, amount_without_fee);

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

		let bounded_name: BoundedVec<u8, <Runtime as pallet_asset_registry::Config>::StringLimit> =
			"SHARED".as_bytes().to_vec().try_into().unwrap();
		let shared_asset_id = AssetRegistry::register_asset(
			bounded_name,
			pallet_asset_registry::AssetType::PoolShare(HDX, DOT),
			1_000,
			None,
			None,
		)
		.unwrap();
		assert_ok!(Currencies::deposit(shared_asset_id, &ALICE.into(), amount,));

		// Act
		let bond_asset_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(
			RuntimeOrigin::signed(ALICE.into()),
			shared_asset_id,
			amount,
			maturity
		));

		// Assert
		expect_hydra_events(vec![pallet_bonds::Event::BondTokenCreated {
			issuer: ALICE.into(),
			asset_id: shared_asset_id,
			bond_asset_id,
			amount: amount_without_fee,
			fee,
		}
		.into()]);

		assert_eq!(
			Bonds::bonds(bond_asset_id).unwrap(),
			Bond {
				maturity,
				asset_id: shared_asset_id,
				amount: amount_without_fee,
			}
		);

		let bond_asset_details = AssetRegistry::assets(bond_asset_id).unwrap();

		assert!(bond_asset_details.asset_type == pallet_asset_registry::AssetType::Bond);
		assert!(bond_asset_details.name.is_empty());
		assert_eq!(bond_asset_details.existential_deposit, 1_000);

		assert_balance!(&ALICE.into(), shared_asset_id, 0);
		assert_balance!(&ALICE.into(), bond_asset_id, amount_without_fee);

		assert_balance!(
			&<Runtime as pallet_bonds::Config>::FeeReceiver::get(),
			shared_asset_id,
			fee
		);

		assert_balance!(&Bonds::pallet_account_id(), shared_asset_id, amount_without_fee);
	});
}

#[test]
fn issue_bonds_should_work_when_issued_for_bond_asset() {
	Hydra::execute_with(|| {
		// Arrange
		let amount = 100 * UNITS;
		let fee = <Runtime as pallet_bonds::Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

		let maturity = NOW + MONTH;

		let bounded_name: BoundedVec<u8, <Runtime as pallet_asset_registry::Config>::StringLimit> =
			"BOND".as_bytes().to_vec().try_into().unwrap();
		let underlying_asset_id = AssetRegistry::register_asset(
			bounded_name,
			pallet_asset_registry::AssetType::PoolShare(HDX, DOT),
			1_000,
			None,
			None,
		)
		.unwrap();
		assert_ok!(Currencies::deposit(underlying_asset_id, &ALICE.into(), amount,));

		// Act
		let bond_asset_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(
			RuntimeOrigin::signed(ALICE.into()),
			underlying_asset_id,
			amount,
			maturity
		));

		// Assert
		expect_hydra_events(vec![pallet_bonds::Event::BondTokenCreated {
			issuer: ALICE.into(),
			asset_id: underlying_asset_id,
			bond_asset_id,
			amount: amount_without_fee,
			fee,
		}
		.into()]);

		assert_eq!(
			Bonds::bonds(bond_asset_id).unwrap(),
			Bond {
				maturity,
				asset_id: underlying_asset_id,
				amount: amount_without_fee,
			}
		);

		let bond_asset_details = AssetRegistry::assets(bond_asset_id).unwrap();
		
		assert!(bond_asset_details.asset_type == pallet_asset_registry::AssetType::Bond);
		assert!(bond_asset_details.name.is_empty());
		assert_eq!(bond_asset_details.existential_deposit, 1_000);

		assert_balance!(&ALICE.into(), underlying_asset_id, 0);
		assert_balance!(&ALICE.into(), bond_asset_id, amount_without_fee);

		assert_balance!(
			&<Runtime as pallet_bonds::Config>::FeeReceiver::get(),
			underlying_asset_id,
			fee
		);

		assert_balance!(&Bonds::pallet_account_id(), underlying_asset_id, amount_without_fee);
	});
}
