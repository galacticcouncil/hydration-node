#![cfg(test)]

//! Insufficient assets after the retirement of the ED toll: token accounts are created for free,
//! legacy `sufficients` refs are never touched, and banned assets cannot move at all.

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::RuntimeOrigin as hydra_origin;
use hydradx_runtime::{
	origins::Origin, AssetRegistry as Registry, AssetType, Balances, Currencies, Tokens, TreasuryAccount,
};
use orml_traits::MultiCurrency;
use polkadot_xcm::v5::{
	Junction::{GeneralIndex, Parachain},
	Location,
};
use xcm_emulator::TestExt;

#[test]
fn transfer_should_not_charge_ed_when_insufficient_asset_creates_new_token_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let bob_balance = Currencies::free_balance(HDX, &BOB.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 1_000_000 * UNITS);

		assert_eq!(Currencies::free_balance(HDX, &BOB.into()), bob_balance);
		assert_eq!(Balances::reserved_balance(AccountId::from(BOB)), 0);
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance);
		assert_eq!(Balances::reserved_balance(AccountId::from(ALICE)), 0);
		assert_eq!(Currencies::free_balance(HDX, &TreasuryAccount::get()), treasury_balance);

		assert_eq!(
			frame_system::Pallet::<hydradx_runtime::Runtime>::account(AccountId::from(ALICE)).sufficients,
			0
		);
	});
}

#[test]
fn deposit_should_not_charge_ed_when_insufficient_asset_creates_new_token_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act
		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));

		//Assert
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 1_000_000 * UNITS);
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance);
		assert_eq!(Balances::reserved_balance(AccountId::from(ALICE)), 0);
		assert_eq!(Currencies::free_balance(HDX, &TreasuryAccount::get()), treasury_balance);

		assert_eq!(
			frame_system::Pallet::<hydradx_runtime::Runtime>::account(AccountId::from(ALICE)).sufficients,
			0
		);
	});
}

#[test]
fn deposit_should_create_token_account_when_receiver_has_no_hdx() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let fresh: AccountId = [42u8; 32].into();

		assert_eq!(Currencies::free_balance(HDX, &fresh), 0);

		//Act - under the retired scheme this failed because the receiver could not pay the toll.
		assert_ok!(Tokens::deposit(sht1, &fresh, 1_000_000 * UNITS));

		//Assert
		assert_eq!(Currencies::free_balance(sht1, &fresh), 1_000_000 * UNITS);

		let system_account = frame_system::Pallet::<hydradx_runtime::Runtime>::account(&fresh);
		assert_eq!(system_account.providers, 1);
		assert_eq!(system_account.sufficients, 0);
	});
}

#[test]
fn token_account_kill_should_not_touch_sufficients_refs() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));

		// Simulate a ref left behind by the retired ED scheme.
		frame_system::Pallet::<hydradx_runtime::Runtime>::inc_sufficients(&ALICE.into());
		assert_eq!(
			frame_system::Pallet::<hydradx_runtime::Runtime>::account(AccountId::from(ALICE)).sufficients,
			1
		);

		//Act - empty the token account so orml reaps it.
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(
			frame_system::Pallet::<hydradx_runtime::Runtime>::account(AccountId::from(ALICE)).sufficients,
			1
		);
	});
}

#[test]
fn banned_asset_should_not_create_new_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let update_origin = hydradx_runtime::OriginCaller::Origins(Origin::GeneralAdmin);
		//Arrange
		let sht1: AssetId = register_external_asset(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Registry::ban_asset(update_origin.into(), sht1));

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);

		//Act & assert
		assert_noop!(
			Tokens::transfer(hydra_origin::signed(BOB.into()), ALICE.into(), sht1, 1_000_000 * UNITS),
			sp_runtime::DispatchError::Other("BannedAssetTransfer")
		);
	});
}

#[test]
fn banned_asset_should_not_be_transferable_to_existing_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let update_origin = hydradx_runtime::OriginCaller::Origins(Origin::GeneralAdmin);
		//Arrange
		let sht1: AssetId = register_external_asset(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Registry::ban_asset(update_origin.into(), sht1));

		//Act & assert
		assert_noop!(
			Tokens::transfer(hydra_origin::signed(BOB.into()), ALICE.into(), sht1, 1_000_000 * UNITS),
			sp_runtime::DispatchError::Other("BannedAssetTransfer")
		);
	});
}

#[test]
fn banned_asset_should_not_be_depositable() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let update_origin = hydradx_runtime::OriginCaller::Origins(Origin::GeneralAdmin);
		//Arrange
		let sht1: AssetId = register_external_asset(0_u128);

		assert_ok!(Registry::ban_asset(update_origin.into(), sht1));

		//Act & assert
		assert_noop!(
			Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS),
			sp_runtime::DispatchError::Other("BannedAssetTransfer")
		);
	});
}

fn register_external_asset(general_index: u128) -> AssetId {
	let location = hydradx_runtime::AssetLocation(Location::new(
		1,
		[Parachain(MOONBEAM_PARA_ID), GeneralIndex(general_index)],
	));

	let next_asset_id = Registry::next_asset_id().unwrap();
	Registry::register(
		RawOrigin::Root.into(),
		None,
		None,
		AssetType::External,
		Some(1_000),
		None,
		None,
		Some(location),
		None,
		false,
	)
	.unwrap();

	next_asset_id
}
