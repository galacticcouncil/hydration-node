#![cfg(test)]

use crate::assert_balance;
use crate::polkadot_test_net::*;

use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::registry::{AssetKind, Create};
use orml_traits::MultiCurrency;
use sp_runtime::{DispatchResult, TransactionOutcome};
use xcm_emulator::TestExt;

use hydradx_runtime::{AssetRegistry, Balances, Bonds, Currencies, Runtime, RuntimeOrigin, Treasury};
use primitives::constants::time::unix_time::MONTH;

#[test]
fn issue_bonds_should_work_when_issued_for_native_asset() {
	Hydra::execute_with(|| {
		// Arrange
		let amount = 100 * UNITS;
		let treasury = Treasury::account_id();
		let initial_treasury_hdx = Currencies::free_balance(HDX, &treasury);
		assert_ok!(Balances::force_set_balance(
			RuntimeOrigin::root(),
			treasury.clone(),
			initial_treasury_hdx + amount,
		));

		let maturity = NOW + MONTH;

		// Act
		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::root(), HDX, amount, maturity));

		// Assert
		assert_eq!(Bonds::bond(bond_id).unwrap(), (HDX, maturity));

		let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

		assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
		assert_eq!(
			bond_asset_details.name.unwrap().into_inner(),
			Bonds::bond_name(HDX, maturity)
		);
		assert_eq!(bond_asset_details.existential_deposit, NativeExistentialDeposit::get());

		assert_balance!(&treasury, HDX, initial_treasury_hdx);
		assert_balance!(&treasury, bond_id, amount);
		assert_balance!(&Bonds::pallet_account_id(), HDX, amount);
	});
}

#[test]
fn issue_bonds_should_work_when_issued_for_share_asset() {
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			let amount = 100 * UNITS;
			let ed = 1_000;
			let treasury = Treasury::account_id();
			let maturity = NOW + MONTH;

			let name = b"SHARED".to_vec();
			let share_asset_id = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::XYK,
				Some(ed),
				None,
				None,
				None,
				None,
			)
			.unwrap();
			// Fund treasury with `amount + ed` — treasury can't be reaped (has consumers from
			// holding HDX), so the transfer must leave at least ED behind.
			assert_ok!(Currencies::deposit(share_asset_id, &treasury, amount + ed));
			// Treasury is in DustRemovalWhitelist, so `SufficiencyCheck` charges the destination
			// (bonds pallet account) an HDX-denominated ED when receiving an insufficient asset.
			// Fund it so the hook succeeds.
			assert_ok!(Balances::force_set_balance(
				RuntimeOrigin::root(),
				Bonds::pallet_account_id(),
				10 * UNITS,
			));

			// Act
			let bond_id = AssetRegistry::next_asset_id().unwrap();
			assert_ok!(Bonds::issue(RuntimeOrigin::root(), share_asset_id, amount, maturity));

			// Assert
			assert_eq!(Bonds::bond(bond_id).unwrap(), (share_asset_id, maturity));

			let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

			assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
			assert_eq!(
				bond_asset_details.name.unwrap().into_inner(),
				Bonds::bond_name(share_asset_id, maturity)
			);
			assert_eq!(bond_asset_details.existential_deposit, ed);

			assert_balance!(&treasury, share_asset_id, ed);
			assert_balance!(&treasury, bond_id, amount);
			assert_balance!(&Bonds::pallet_account_id(), share_asset_id, amount);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn issue_bonds_should_not_work_when_issued_for_bond_asset() {
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			let amount = 100 * UNITS;
			let maturity = NOW + MONTH;

			let name = b"BOND".to_vec();
			let underlying_asset_id = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::Bond,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			// Act & Assert
			assert_noop!(
				Bonds::issue(RuntimeOrigin::root(), underlying_asset_id, amount, maturity),
				pallet_bonds::Error::<Runtime>::DisallowedAsset
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
