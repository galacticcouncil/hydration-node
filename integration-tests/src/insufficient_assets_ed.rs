#![cfg(test)]

use crate::insufficient_assets_ed::v3::Junction::GeneralIndex;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok, traits::Contains};
use frame_system::RawOrigin;
use hydradx_runtime::RuntimeOrigin as hydra_origin;
use hydradx_runtime::{
	AssetRegistry as Registry, Currencies, DustRemovalWhitelist, InsufficientEDinHDX, MultiTransactionPayment, Tokens,
	TreasuryAccount, SUFFICIENCY_LOCK,
};
use hydradx_traits::NativePriceOracle;
use orml_traits::MultiCurrency;
use polkadot_xcm::v3::{self, Junction::Parachain, Junctions::X2, MultiLocation};
use sp_runtime::FixedPointNumber;
use xcm_emulator::TestExt;

#[test]
fn sender_should_pay_ed_in_hdx_when_it_is_not_whitelisted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
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
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_balance);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 1_000_000 * UNITS);

		assert_eq!(
			Currencies::free_balance(HDX, &BOB.into()),
			bob_balance - InsufficientEDinHDX::get()
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance + InsufficientEDinHDX::get()
		);

		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());
	});
}

#[test]
fn reciever_should_pay_ed_in_hdx_when_insuficcient_asset_was_depositted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));

		//Assert
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 1_000_000 * UNITS);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance + InsufficientEDinHDX::get()
		);

		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());
	});
}

#[test]
fn hdx_ed_should_be_released_when_account_is_killed_and_ed_was_paid_in_hdx() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_suffyciency_lock(), 1100000000000_u128);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance - InsufficientEDinHDX::get()
		);

		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn sender_should_pay_ed_only_when_dest_didnt_pay_yet() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let fee_asset = BTC;

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			fee_asset,
			1_000_000,
			0,
		));

		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(BOB.into()),
			fee_asset
		));

		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		let bob_fee_asset_balance = Currencies::free_balance(fee_asset, &BOB.into());
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		let treasury_fee_asset_balance = Currencies::free_balance(fee_asset, &TreasuryAccount::get());

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 2_000_000 * UNITS);
		assert_eq!(Currencies::free_balance(fee_asset, &BOB.into()), bob_fee_asset_balance);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance
		);

		let ed_in_hdx: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());
		assert_eq!(treasury_suffyciency_lock(), ed_in_hdx);
	});
}

#[test]
fn dest_should_pay_ed_only_once_when_insufficient_asset_was_depsitted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let fee_asset = BTC;

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			fee_asset,
			1_000_000,
			0,
		));

		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(ALICE.into()),
			fee_asset
		));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000 * UNITS));

		let alice_fee_asset_balance = Currencies::free_balance(fee_asset, &ALICE.into());
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		let treasury_fee_asset_balance = Currencies::free_balance(fee_asset, &TreasuryAccount::get());

		//Act
		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000 * UNITS));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 2_000 * UNITS);
		assert_eq!(
			Currencies::free_balance(fee_asset, &ALICE.into()),
			alice_fee_asset_balance
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance
		);
		let ed_in_hdx: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());
		assert_eq!(treasury_suffyciency_lock(), ed_in_hdx);
	});
}

#[test]
fn hdx_ed_should_be_released_when_account_is_killed_and_ed_was_paid_in_fee_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let fee_asset = BTC;

		//NOTE: this is important for this tests - it basically mean that Bob already paid ED.
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
			fee_asset,
			1_000_000,
			0,
		));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));
		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(ALICE.into()),
			fee_asset
		));

		let alice_fee_asset_balance = Currencies::free_balance(fee_asset, &ALICE.into());
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		let treasury_fee_asset_balance = Currencies::free_balance(fee_asset, &TreasuryAccount::get());

		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		//NOTE: we always returns ED in HDX
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(
			Currencies::free_balance(fee_asset, &ALICE.into()),
			alice_fee_asset_balance
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance
		);
		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn tx_should_fail_with_keepalive_err_when_dest_account_cant_pay_ed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let fee_asset = BTC;

		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(ALICE.into()),
			fee_asset
		));

		let ed_in_hdx: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());
		assert!(Tokens::free_balance(fee_asset, &ALICE.into()) < ed_in_hdx);

		assert_noop!(
			Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS),
			orml_tokens::Error::<hydradx_runtime::Runtime>::KeepAlive
		);
	});
}

#[test]
fn sender_should_pay_ed_in_fee_asset_when_sending_insufficient_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let fee_asset = BTC;

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			fee_asset,
			1_000_000,
			0,
		));

		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(BOB.into()),
			fee_asset
		));

		let bob_fee_asset_balance = Currencies::free_balance(fee_asset, &BOB.into());
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		let treasury_fee_asset_balance = Currencies::free_balance(fee_asset, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		let ed_in_hdx: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);

		assert_eq!(
			Currencies::free_balance(sht1, &BOB.into()),
			(100_000_000 - 1_000_000) * UNITS
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &BOB.into()),
			bob_fee_asset_balance - ed_in_hdx
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance + ed_in_hdx
		);
		assert_eq!(treasury_suffyciency_lock(), ed_in_hdx);
	});
}

#[test]
fn grandfathered_account_should_receive_hdx_when_account_is_killed() {
	//NOTE: thiscase simulates old account that received insufficient asset before sufficiency
	//check and didn't pay ED.

	TestNet::reset();
	Hydra::execute_with(|| {
		let dummy: AssetId = 1_000_001;

		assert_ok!(Tokens::deposit(dummy, &ALICE.into(), 1_000_000 * UNITS));

		let grandfathered_balance = Currencies::free_balance(HDX, &GRANDFATHERED_UNPAID_ED.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		let dummy_balance = Currencies::free_balance(dummy, &GRANDFATHERED_UNPAID_ED.into());
		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(GRANDFATHERED_UNPAID_ED.into()),
			ALICE.into(),
			dummy,
			dummy_balance
		));

		//Assert
		assert_eq!(
			Currencies::free_balance(HDX, &GRANDFATHERED_UNPAID_ED.into()),
			grandfathered_balance + InsufficientEDinHDX::get()
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);

		//NOTE: this is zero because Alice paid ED and it was paid to grandfathered
		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn ed_should_not_be_collected_when_transfering_or_depositing_sufficient_assets() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1 = register_shitcoin(0_u128);
		let sufficient_asset = DAI;

		//This pays ED.
		assert_ok!(Tokens::deposit(sht1, &BOB.into(), 100_000_000 * UNITS,));

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let alice_sufficient_asset_balance = Currencies::free_balance(DAI, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Act 1 - transfer
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sufficient_asset,
			1_000_000 * UNITS
		));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(
			Currencies::free_balance(sufficient_asset, &ALICE.into()),
			alice_sufficient_asset_balance + 1_000_000 * UNITS
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Arrange 2
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let alice_sufficient_asset_balance = Currencies::free_balance(DAI, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 2 - deposit
		assert_ok!(Tokens::deposit(sufficient_asset, &ALICE.into(), 1_000_000 * UNITS));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(
			Currencies::free_balance(sufficient_asset, &ALICE.into()),
			alice_sufficient_asset_balance + 1_000_000 * UNITS
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());
	});
}

#[test]
fn ed_should_not_be_released_when_sufficient_asset_killed_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sufficient_asset = DAI;

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let alice_sufficient_asset_balance = Currencies::free_balance(DAI, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sufficient_asset,
			alice_sufficient_asset_balance
		));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(Currencies::free_balance(sufficient_asset, &ALICE.into()), 0);
		//NOTE: make sure storage was killed
		assert!(orml_tokens::Accounts::<hydradx_runtime::Runtime>::try_get(
			sp_runtime::AccountId32::from(ALICE),
			sufficient_asset
		)
		.is_err());

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn ed_should_be_collected_for_each_insufficient_asset_when_transfered_or_depositted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let sht2: AssetId = register_shitcoin(1_u128);
		let sht3: AssetId = register_shitcoin(2_u128);
		let sht4: AssetId = register_shitcoin(3_u128);

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let bob_hdx_balance = Currencies::free_balance(HDX, &BOB.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(MultiTransactionPayment::account_currency(&ALICE.into()), HDX);
		assert_eq!(MultiTransactionPayment::account_currency(&BOB.into()), HDX);
		assert_eq!(treasury_suffyciency_lock(), 0);

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht2,
			100_000_000 * UNITS,
			0,
		));
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht3,
			100_000_000 * UNITS,
			0,
		));

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			10_000 * UNITS
		));

		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht3,
			10_000 * UNITS
		));

		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht2,
			10_000 * UNITS
		));

		assert_ok!(Tokens::deposit(sht4, &ALICE.into(), 1_000_000 * UNITS));

		//Assert
		//NOTE: Alice paid ED for deposit.
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance - InsufficientEDinHDX::get()
		);

		//NOTE: Bob paid ED for transfers.
		assert_eq!(
			Currencies::free_balance(HDX, &BOB.into()),
			bob_hdx_balance - InsufficientEDinHDX::get() * 3
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance + InsufficientEDinHDX::get() * 4
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get() * 4);
	});
}

#[test]
fn ed_should_be_released_for_each_insufficient_asset_when_account_is_killed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let sht2: AssetId = register_shitcoin(1_u128);
		let sht3: AssetId = register_shitcoin(2_u128);
		let sht4: AssetId = register_shitcoin(3_u128);

		//so bob doesn't pay ed
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht1, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht2, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht3, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht4, 1, 0));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht2, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht3, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht4, &ALICE.into(), 10_000 * UNITS));

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get() * 4);

		//Act  1
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht1,
			10_000 * UNITS
		));

		//Assert 1
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get() * 3);

		//Act 2
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht2,
			10_000 * UNITS
		));

		//Assert 2
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get() * 2
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get() * 2
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get() * 2);

		//Act 3
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht3,
			10_000 * UNITS
		));

		//Assert 3
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get() * 3
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get() * 3
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Act 4
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht4,
			10_000 * UNITS
		));

		//Assert 3
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get() * 4
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get() * 4
		);
		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn mix_of_sufficinet_and_insufficient_assets_should_lock_unlock_ed_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let sht2: AssetId = register_shitcoin(1_u128);
		let sht3: AssetId = register_shitcoin(2_u128);
		let sht4: AssetId = register_shitcoin(3_u128);

		//so bob doesn't pay ed
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht1, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht2, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht3, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht4, 1, 0));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht4, &ALICE.into(), 10_000 * UNITS));
		//NOTE: set_balance bypass mutation hooks so these doesn't pay ED
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			sht2,
			10_000 * UNITS,
			0
		));
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			sht3,
			10_000 * UNITS,
			0
		));

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get() * 2);

		//Act  1
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht1,
			10_000 * UNITS
		));

		//Assert 1
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Arrange 2
		let alice_dai_balance = Currencies::free_balance(DAI, &ALICE.into());
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 2
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			DAI,
			alice_dai_balance
		));

		//Assert 2 - sufficient asset so nothing should change
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Arrange 3
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 3
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht2,
			10_000 * UNITS
		));

		//Assert 3
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Arrange 4
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 4 - unlocking ED for account that doesn't paid ED
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht3,
			10_000 * UNITS
		));

		//Assert 4
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Arrange 5
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 5 - unlocking ED for account that doesn't paid ED
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht4,
			10_000 * UNITS
		));

		//Assert 5
		assert_eq!(
			Currencies::free_balance(HDX, &ALICE.into()),
			alice_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(treasury_suffyciency_lock(), 0);
	});
}

#[test]
fn sender_should_pay_ed_when_tranferred_or_deposited_to_whitelisted_dest() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let sht2: AssetId = register_shitcoin(1_u128);

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			1_000_000 * UNITS,
			0,
		));

		let treasury = TreasuryAccount::get();

		assert!(DustRemovalWhitelist::contains(&treasury));
		assert_eq!(MultiTransactionPayment::account_currency(&BOB.into()), HDX);

		let bob_fee_asset_balance = Currencies::free_balance(HDX, &BOB.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &treasury);

		//Act 1
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			treasury.clone(),
			sht1,
			10
		));

		//Assert 1
		assert_eq!(
			Currencies::free_balance(HDX, &treasury),
			treasury_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &BOB.into()),
			bob_fee_asset_balance - InsufficientEDinHDX::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &treasury), 10);
		assert_eq!(treasury_suffyciency_lock(), InsufficientEDinHDX::get());

		//Act 2
		assert_ok!(Tokens::deposit(sht2, &treasury, 20));

		//Assert 2
		assert_eq!(
			Currencies::free_balance(HDX, &treasury),
			treasury_hdx_balance + InsufficientEDinHDX::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &treasury), 10);
		assert_eq!(Currencies::free_balance(sht2, &treasury), 20);
		//NOTE: treasury paid ED in hdx so hdx balance didn't changed but locked was increased.
		assert_eq!(treasury_suffyciency_lock(), 2 * InsufficientEDinHDX::get());
	});
}

#[test]
fn ed_should_be_released_when_whitelisted_account_was_killed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let treasury = TreasuryAccount::get();

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			1_000_000 * UNITS,
			0,
		));
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			treasury.clone().into(),
			sht1,
			1_000_000 * UNITS,
			0,
		));

		assert!(DustRemovalWhitelist::contains(&treasury.clone().into()));
		assert_eq!(MultiTransactionPayment::account_currency(&treasury), HDX);
		let treasury_hdx_balance = Currencies::free_balance(HDX, &treasury);

		//NOTE: set_balance bypass mutation hooks so none was paid.
		assert_eq!(treasury_suffyciency_lock(), 0);

		//Act 1
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(treasury.clone()),
			BOB.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert 1
		assert_eq!(Currencies::free_balance(HDX, &treasury), treasury_hdx_balance);
		assert_eq!(Currencies::free_balance(sht1, &treasury), 0);
		assert_eq!(Currencies::free_balance(sht1, &BOB.into()), 2_000_000 * UNITS);

		//NOTE: bob already holds sht1 so it means additional ed is not necessary.
		assert_eq!(treasury_suffyciency_lock(), 0);

		assert!(orml_tokens::Accounts::<hydradx_runtime::Runtime>::try_get(&treasury, sht1).is_err());
	});
}

#[test]
fn tx_should_fail_with_unsupported_currency_error_when_fee_asset_price_wasn_not_provided() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_shitcoin(0_u128);
		let sht2: AssetId = register_shitcoin(1_u128);
		let fee_asset = BTC;

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			fee_asset,
			1_000_000,
			0,
		));

		assert_ok!(MultiTransactionPayment::set_currency(
			hydra_origin::signed(BOB.into()),
			fee_asset
		));

		assert_ok!(MultiTransactionPayment::remove_currency(RawOrigin::Root.into(), BTC));

		polkadot_run_to_block(2);

		//Act 1 - transfer
		assert_noop!(
			Tokens::transfer(hydra_origin::signed(BOB.into()), ALICE.into(), sht1, 1_000_000 * UNITS),
			pallet_transaction_multi_payment::Error::<hydradx_runtime::Runtime>::UnsupportedCurrency
		);

		//Act 2 - deposit
		assert_noop!(
			Tokens::deposit(sht2, &BOB.into(), 1_000_000 * UNITS),
			pallet_transaction_multi_payment::Error::<hydradx_runtime::Runtime>::UnsupportedCurrency
		);
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
