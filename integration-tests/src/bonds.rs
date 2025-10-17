#![cfg(test)]

use crate::assert_balance;
use crate::polkadot_test_net::*;

use frame_support::storage::with_transaction;
use frame_support::{
	assert_noop,
	assert_ok,
};
use frame_system::RawOrigin;
use hydradx_traits::registry::{
	AssetKind,
	Create,
};
use orml_traits::MultiCurrency;
use sp_runtime::{
	DispatchResult,
	TransactionOutcome,
};
use xcm_emulator::TestExt;

use hydradx_runtime::{
	AssetRegistry,
	Bonds,
	Currencies,
	MultiTransactionPayment,
	Runtime,
	RuntimeOrigin,
	Tokens,
};
use primitives::constants::time::unix_time::MONTH;

#[test]
fn issue_bonds_should_work_when_issued_for_native_asset() {
	Hydra::execute_with(|| {
		// Arrange
		set_fee_asset_and_fund(ALICE.into(), BTC, 1_000_000);

		let amount = 100 * UNITS;
		let fee = <Runtime as pallet_bonds::Config>::ProtocolFee::get().mul_ceil(amount);
		let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();
		let initial_fee_receiver_balance =
			Currencies::free_balance(HDX, &<Runtime as pallet_bonds::Config>::FeeReceiver::get());

		let maturity = NOW + MONTH;

		// Act
		let bond_id = AssetRegistry::next_asset_id().unwrap();
		assert_ok!(Bonds::issue(RuntimeOrigin::signed(ALICE.into()), HDX, amount, maturity));

		// Assert
		assert_eq!(Bonds::bond(bond_id).unwrap(), (HDX, maturity));

		let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

		assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
		assert_eq!(
			bond_asset_details.name.unwrap().into_inner(),
			Bonds::bond_name(HDX, maturity)
		);
		assert_eq!(bond_asset_details.existential_deposit, NativeExistentialDeposit::get());

		assert_balance!(&ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount);
		assert_balance!(&ALICE.into(), bond_id, amount_without_fee);

		assert_balance!(
			&<Runtime as pallet_bonds::Config>::FeeReceiver::get(),
			HDX,
			initial_fee_receiver_balance + fee
		);

		assert_balance!(&Bonds::pallet_account_id(), HDX, amount_without_fee);
	});
}

#[test]
fn issue_bonds_should_work_when_issued_for_share_asset() {
	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// Arrange
			set_fee_asset_and_fund(ALICE.into(), BTC, 1_000_000);

			let amount = 100 * UNITS;
			let fee = <Runtime as pallet_bonds::Config>::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee: Balance = amount.checked_sub(fee).unwrap();

			let maturity = NOW + MONTH;

			let name = b"SHARED".to_vec();
			let share_asset_id = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::XYK,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();
			assert_ok!(Currencies::deposit(share_asset_id, &ALICE.into(), amount,));

			// Act
			let bond_id = AssetRegistry::next_asset_id().unwrap();
			assert_ok!(Bonds::issue(
				RuntimeOrigin::signed(ALICE.into()),
				share_asset_id,
				amount,
				maturity
			));

			// Assert
			assert_eq!(Bonds::bond(bond_id).unwrap(), (share_asset_id, maturity));

			let bond_asset_details = AssetRegistry::assets(bond_id).unwrap();

			assert_eq!(bond_asset_details.asset_type, pallet_asset_registry::AssetType::Bond);
			assert_eq!(
				bond_asset_details.name.unwrap().into_inner(),
				Bonds::bond_name(share_asset_id, maturity)
			);
			assert_eq!(bond_asset_details.existential_deposit, 1_000);

			assert_balance!(&ALICE.into(), share_asset_id, 0);
			assert_balance!(&ALICE.into(), bond_id, amount_without_fee);

			assert_balance!(
				&<Runtime as pallet_bonds::Config>::FeeReceiver::get(),
				share_asset_id,
				fee
			);

			assert_balance!(&Bonds::pallet_account_id(), share_asset_id, amount_without_fee);

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

			assert_ok!(Currencies::deposit(underlying_asset_id, &ALICE.into(), amount,));

			// Act & Assert
			assert_noop!(
				Bonds::issue(
					RuntimeOrigin::signed(ALICE.into()),
					underlying_asset_id,
					amount,
					maturity
				),
				pallet_bonds::Error::<hydradx_runtime::Runtime>::DisallowedAsset
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

fn set_fee_asset_and_fund(who: AccountId, fee_asset: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		who.clone(),
		fee_asset,
		amount,
		0,
	));

	assert_ok!(MultiTransactionPayment::set_currency(
		hydradx_runtime::RuntimeOrigin::signed(who),
		fee_asset
	));
}
