#![cfg(test)]

use crate::insufficient_assets_ed::v3::Junction::GeneralIndex;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_runtime::RuntimeOrigin as hydra_origin;
use hydradx_runtime::{AssetRegistry as Registry, Currencies, Tokens, TreasuryAccount, SUFFICIENCY_LOCK};
use orml_traits::MultiCurrency;
use polkadot_xcm::v3::{self, Junction::Parachain, Junctions::X2, MultiLocation};
use sp_runtime::{FixedPointNumber, FixedU128};
use xcm_emulator::TestExt;

#[test]
fn alice_should_pay_ed_in_hdx_when_receive_transfered_shitcoin() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let doge: AssetId = register_shitcoin(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			doge,
			100_000_000 * UNITS,
			0,
		));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(doge, &ALICE.into()), 0);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			doge,
			1_000_000 * UNITS
		));

		//Assert
		let hdx_ed = FixedU128::from_rational(11, 10)
			.saturating_mul_int(<hydradx_runtime::Runtime as pallet_balances::Config>::ExistentialDeposit::get());

		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance - hdx_ed);
		assert_eq!(Currencies::free_balance(doge, &ALICE.into()), 1_000_000 * UNITS);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance + hdx_ed
		);

		assert_eq!(treasury_suffyciency_lock(), hdx_ed);
	});
}

#[test]
fn alice_should_pay_ed_in_hdx_when_shitcoin_was_depositted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let doge: AssetId = register_shitcoin(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			doge,
			100_000_000 * UNITS,
			0,
		));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(doge, &ALICE.into()), 0);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::deposit(doge, &ALICE.into(), 1_000_000 * UNITS));

		//Assert
		let hdx_ed = FixedU128::from_rational(11, 10)
			.saturating_mul_int(<hydradx_runtime::Runtime as pallet_balances::Config>::ExistentialDeposit::get());

		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance - hdx_ed);
		assert_eq!(Currencies::free_balance(doge, &ALICE.into()), 1_000_000 * UNITS);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance + hdx_ed
		);

		assert_eq!(treasury_suffyciency_lock(), hdx_ed);
	});
}

#[test]
fn hdx_ed_should_be_released_when_alice_account_is_killed_and_ed_was_paid_in_hdx() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let doge: AssetId = register_shitcoin(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			doge,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::deposit(doge, &ALICE.into(), 1_000_000 * UNITS));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_suffyciency_lock(), 1100000000000_u128);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			doge,
			1_000_000 * UNITS
		));

		//Assert
		let hdx_ed = FixedU128::from_rational(11, 10)
			.saturating_mul_int(<hydradx_runtime::Runtime as pallet_balances::Config>::ExistentialDeposit::get());

		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance + hdx_ed);
		assert_eq!(Currencies::free_balance(doge, &ALICE.into()), 0);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance - hdx_ed
		);

		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

fn register_shitcoin(general_index: u128) -> AssetId {
	let location = hydradx_runtime::AssetLocation(MultiLocation::new(
		1,
		X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(general_index)),
	));

	let next_asset_id = Registry::next_asset_id().unwrap();
	Registry::register_external(hydra_origin::signed(BOB.into()), location).unwrap();

	next_asset_id
}

fn treasury_suffyciency_lock() -> Balance {
	pallet_balances::Locks::<hydradx_runtime::Runtime>::get(TreasuryAccount::get())
		.iter()
		.find(|x| x.id == SUFFICIENCY_LOCK)
		.map(|p| p.amount)
		.unwrap_or_default()
}
