#![cfg(test)]

use crate::assert_balance;
use crate::assert_event_times;
use crate::insufficient_assets_ed::v3::Junction::GeneralIndex;
use crate::polkadot_test_net::*;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok, traits::Contains};
use frame_system::RawOrigin;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Omnipool;
use hydradx_runtime::RuntimeOrigin as hydra_origin;
use hydradx_runtime::RuntimeOrigin;
use hydradx_runtime::DOT_ASSET_LOCATION;
use hydradx_runtime::XYK;
use hydradx_runtime::{
	AssetRegistry as Registry, Currencies, DustRemovalWhitelist, InsufficientEDinHDX, MultiTransactionPayment,
	NativeExistentialDeposit, RuntimeEvent, TechnicalCollective, Tokens, TreasuryAccount, SUFFICIENCY_LOCK,
};
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use hydradx_traits::NativePriceOracle;
use orml_traits::MultiCurrency;
use polkadot_xcm::v3::{self, Junction::Parachain, Junctions::X2, MultiLocation};
use sp_runtime::DispatchResult;
use sp_runtime::FixedPointNumber;
use sp_runtime::TransactionOutcome;
use xcm_emulator::TestExt;

#[test]
fn sender_should_pay_ed_in_hdx_when_it_is_not_whitelisted() {
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
		assert_eq!(treasury_sufficiency_lock(), 0);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn reciever_should_pay_ed_in_hdx_when_insuficcient_asset_was_deposited() {
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
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(treasury_sufficiency_lock(), 0);

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

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn hdx_ed_should_be_released_when_account_is_killed_and_ed_was_paid_in_hdx() {
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

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 1_000_000 * UNITS));

		let alice_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

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
			alice_balance + NativeExistentialDeposit::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_balance - NativeExistentialDeposit::get()
		);

		assert_eq!(treasury_sufficiency_lock(), 0);

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn sender_should_pay_ed_only_when_dest_didnt_pay_yet() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
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

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset,
				amount: ed_in_hdx
			}),
			1
		);
	});
}

#[test]
fn dest_should_pay_ed_only_once_when_insufficient_asset_was_deposited() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
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
		let ed_in_fee_asset: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset,
				amount: ed_in_fee_asset
			}),
			1
		);
	});
}

#[test]
fn hdx_ed_should_be_released_when_account_is_killed_and_ed_was_paid_in_fee_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
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

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

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
			alice_hdx_balance + NativeExistentialDeposit::get()
		);
		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(
			Currencies::free_balance(fee_asset, &ALICE.into()),
			alice_fee_asset_balance
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get()
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance
		);

		assert_eq!(treasury_sufficiency_lock(), 0);

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);

		//NOTE: this is colected amount, not locked amount.
		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn tx_should_fail_with_existential_deposit_err_when_dest_account_cant_pay_ed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
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
			orml_tokens::Error::<hydradx_runtime::Runtime>::ExistentialDeposit
		);
	});
}

#[test]
fn sender_should_pay_ed_in_fee_asset_when_sending_insufficient_asset() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
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
		assert_eq!(treasury_sufficiency_lock(), 0);

		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			sht1,
			1_000_000 * UNITS
		));

		//Assert
		let ed_in_fee_asset: Balance = MultiTransactionPayment::price(fee_asset)
			.unwrap()
			.saturating_mul_int(InsufficientEDinHDX::get());
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);

		assert_eq!(
			Currencies::free_balance(sht1, &BOB.into()),
			(100_000_000 - 1_000_000) * UNITS
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &BOB.into()),
			bob_fee_asset_balance - ed_in_fee_asset
		);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(
			Currencies::free_balance(fee_asset, &TreasuryAccount::get()),
			treasury_fee_asset_balance + ed_in_fee_asset
		);

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());

		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset,
				amount: ed_in_fee_asset
			}),
			1
		);
	});
}

#[test]
fn account_with_zero_sufficients_should_not_release_ed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let dummy: AssetId = 1_000_001;

		//NOTE: set balance baypass `MutationHooks` so Bob received insufficient asset without
		//locking ED.
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			dummy,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::deposit(dummy, &ALICE.into(), 1_000_000 * UNITS));
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		let bob_balance = Currencies::free_balance(HDX, &BOB.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		let dummy_balance = Currencies::free_balance(dummy, &BOB.into());
		//Act
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			ALICE.into(),
			dummy,
			dummy_balance
		));

		//Assert
		assert_eq!(Currencies::free_balance(HDX, &BOB.into()), bob_balance);

		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn ed_should_not_be_collected_when_transfering_or_depositing_sufficient_assets() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1 = register_external_asset(0_u128);
		let sufficient_asset = DAI;

		//This pays ED.
		assert_ok!(Tokens::deposit(sht1, &BOB.into(), 100_000_000 * UNITS));

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let alice_sufficient_asset_balance = Currencies::free_balance(DAI, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn ed_should_not_be_released_when_sufficient_asset_killed_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sufficient_asset = DAI;

		//This pays ED.
		assert_ok!(Tokens::deposit(sht1, &BOB.into(), 100_000_000 * UNITS));
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let alice_sufficient_asset_balance = Currencies::free_balance(DAI, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn ed_should_be_collected_for_each_insufficient_asset_when_transfered_or_depositted() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sht2: AssetId = register_external_asset(1_u128);
		let sht3: AssetId = register_external_asset(2_u128);
		let sht4: AssetId = register_external_asset(3_u128);

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let bob_hdx_balance = Currencies::free_balance(HDX, &BOB.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(MultiTransactionPayment::account_currency(&ALICE.into()), HDX);
		assert_eq!(MultiTransactionPayment::account_currency(&BOB.into()), HDX);
		assert_eq!(treasury_sufficiency_lock(), 0);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get() * 4);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			4_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			3
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn ed_should_be_released_for_each_insufficient_asset_when_account_is_killed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sht2: AssetId = register_external_asset(1_u128);
		let sht3: AssetId = register_external_asset(2_u128);
		let sht4: AssetId = register_external_asset(3_u128);

		//so bob doesn't pay ed
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht1, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht2, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht3, 1, 0));
		assert_ok!(Tokens::set_balance(RawOrigin::Root.into(), BOB.into(), sht4, 1, 0));

		assert_ok!(Tokens::deposit(sht1, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht2, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht3, &ALICE.into(), 10_000 * UNITS));
		assert_ok!(Tokens::deposit(sht4, &ALICE.into(), 10_000 * UNITS));

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			4
		);

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get() * 4);

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
			alice_hdx_balance + NativeExistentialDeposit::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get()
		);
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get() * 3);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			3_u128
		);

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
			alice_hdx_balance + NativeExistentialDeposit::get() * 2
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get() * 2
		);
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get() * 2);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			2_u128
		);

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
			alice_hdx_balance + NativeExistentialDeposit::get() * 3
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get() * 3
		);
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
			alice_hdx_balance + NativeExistentialDeposit::get() * 4
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get() * 4
		);
		assert_eq!(treasury_sufficiency_lock(), 0);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);
	});
}

#[test]
fn mix_of_sufficinet_and_insufficient_assets_should_lock_unlock_ed_correctly() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sht2: AssetId = register_external_asset(1_u128);
		let sht3: AssetId = register_external_asset(2_u128);
		let sht4: AssetId = register_external_asset(3_u128);

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

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: ALICE.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			2
		);

		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get() * 2);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			2_u128
		);

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
			alice_hdx_balance + NativeExistentialDeposit::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get()
		);
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
			alice_hdx_balance + NativeExistentialDeposit::get()
		);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance - NativeExistentialDeposit::get()
		);
		assert_eq!(treasury_sufficiency_lock(), 0);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);

		//Arrange 4
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 4
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht3,
			10_000 * UNITS
		));

		//Assert 4 - we used set_balance, nobody paid for this ED so nothing can be unlocked.
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_sufficiency_lock(), 0);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);

		//Arrange 5
		let alice_hdx_balance = Currencies::free_balance(HDX, &ALICE.into());
		let treasury_hdx_balance = Currencies::free_balance(HDX, &TreasuryAccount::get());

		//Act 5 - we used set_balance, nobody paid for this ED so nothing can be unlocked.
		assert_ok!(Tokens::transfer(
			hydra_origin::signed(ALICE.into()),
			BOB.into(),
			sht4,
			10_000 * UNITS
		));

		//Assert 5
		assert_eq!(Currencies::free_balance(HDX, &ALICE.into()), alice_hdx_balance);
		assert_eq!(
			Currencies::free_balance(HDX, &TreasuryAccount::get()),
			treasury_hdx_balance
		);
		assert_eq!(treasury_sufficiency_lock(), 0);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);
	});
}

#[test]
fn sender_should_pay_ed_when_tranferred_or_deposited_to_whitelisted_dest() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sht2: AssetId = register_external_asset(1_u128);

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
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), 2 * NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			2_u128
		);

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: treasury.clone(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);
	});
}

#[test]
fn ed_should_be_released_when_whitelisted_account_was_killed() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let treasury = TreasuryAccount::get();

		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			2_000_000 * UNITS,
			0,
		));

		assert_ok!(Tokens::transfer(
			hydra_origin::signed(BOB.into()),
			treasury.clone(),
			sht1,
			1_000_000 * UNITS
		));

		assert_event_times!(
			RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
				who: BOB.into(),
				fee_asset: HDX,
				amount: InsufficientEDinHDX::get()
			}),
			1
		);

		assert!(DustRemovalWhitelist::contains(&treasury));
		assert_eq!(MultiTransactionPayment::account_currency(&treasury), HDX);
		let treasury_hdx_balance = Currencies::free_balance(HDX, &treasury);

		//NOTE: set_balance bypass mutation hooks so only Bob paid ED for Treasury.
		assert_eq!(treasury_sufficiency_lock(), NativeExistentialDeposit::get());
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			1_u128
		);

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
		assert_eq!(treasury_sufficiency_lock(), 0);
		assert_eq!(
			pallet_asset_registry::pallet::ExistentialDepositCounter::<hydradx_runtime::Runtime>::get(),
			0_u128
		);

		assert!(orml_tokens::Accounts::<hydradx_runtime::Runtime>::try_get(&treasury, sht1).is_err());
	});
}

#[test]
fn tx_should_fail_with_unsupported_currency_error_when_fee_asset_price_was_not_provided() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let sht1: AssetId = register_external_asset(0_u128);
		let sht2: AssetId = register_external_asset(1_u128);
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

		hydradx_run_to_block(4);

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

#[test]
fn banned_asset_should_not_create_new_account() {
	TestNet::reset();
	Hydra::execute_with(|| {
		let tech_comm = pallet_collective::RawOrigin::<AccountId, TechnicalCollective>::Members(1, 1);
		//Arrange
		let sht1: AssetId = register_external_asset(0_u128);
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			BOB.into(),
			sht1,
			100_000_000 * UNITS,
			0,
		));

		assert_ok!(Registry::ban_asset(tech_comm.into(), sht1));

		assert_eq!(Currencies::free_balance(sht1, &ALICE.into()), 0);
		assert_eq!(treasury_sufficiency_lock(), 0);

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
		let tech_comm = pallet_collective::RawOrigin::<AccountId, TechnicalCollective>::Members(1, 1);
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

		assert_ok!(Registry::ban_asset(tech_comm.into(), sht1));

		//Act & assert
		assert_noop!(
			Tokens::transfer(hydra_origin::signed(BOB.into()), ALICE.into(), sht1, 1_000_000 * UNITS),
			sp_runtime::DispatchError::Other("BannedAssetTransfer")
		);
	});
}

#[test]
fn ed_should_be_paid_in_insufficient_asset_through_dot() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			hydradx_runtime::AssetRegistry::set_location(DOT, DOT_ASSET_LOCATION).unwrap();

			//Arrange
			crate::dca::init_omnipool_with_oracle_for_block_10();
			crate::dca::add_dot_as_payment_currency();
			assert_ok!(Currencies::update_balance(
				RawOrigin::Root.into(),
				BOB.into(),
				DOT,
				200 * UNITS as i128,
			));

			assert_ok!(Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				DOT,
				HDX,
				10 * UNITS,
				u128::MIN
			));

			let name = b"INSUF1".to_vec();
			let insufficient_asset = AssetRegistry::register_insufficient_asset(
				None,
				Some(name.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();
			create_xyk_pool(insufficient_asset, 1000000 * UNITS, DOT, 3000000 * UNITS);

			let name2 = b"INSUF2".to_vec();

			let insufficient_asset2 = AssetRegistry::register_insufficient_asset(
				None,
				Some(name2.try_into().unwrap()),
				AssetKind::External,
				Some(1_000),
				None,
				None,
				None,
				None,
			)
			.unwrap();

			set_relaychain_block_number(11);

			let alice_init_insuff_balance = 10 * UNITS;
			assert_ok!(hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				insufficient_asset,
				alice_init_insuff_balance as i128,
			));

			let fee_currency = insufficient_asset;

			assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				fee_currency,
			));

			assert_balance!(&Treasury::account_id(), DOT, 0);
			let alice_init_insuff_balance =
				hydradx_runtime::Currencies::free_balance(insufficient_asset, &ALICE.into());

			//Act
			assert_ok!(Tokens::deposit(insufficient_asset2, &ALICE.into(), 1 * UNITS));

			//Assert
			let alice_new_insuff_balance = hydradx_runtime::Currencies::free_balance(insufficient_asset, &ALICE.into());
			assert!(alice_new_insuff_balance < alice_init_insuff_balance);
			let spent_insuff_asset = alice_init_insuff_balance - alice_new_insuff_balance;

			let treasury_dot_balance = hydradx_runtime::Currencies::free_balance(DOT, &ALICE.into());
			assert!(treasury_dot_balance > 0, "Treasury is rugged");

			assert_event_times!(
				RuntimeEvent::AssetRegistry(pallet_asset_registry::Event::ExistentialDepositPaid {
					who: ALICE.into(),
					fee_asset: insufficient_asset,
					amount: spent_insuff_asset
				}),
				1
			);

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

fn register_external_asset(general_index: u128) -> AssetId {
	let location = hydradx_runtime::AssetLocation(MultiLocation::new(
		1,
		X2(Parachain(MOONBEAM_PARA_ID), GeneralIndex(general_index)),
	));

	let next_asset_id = Registry::next_asset_id().unwrap();
	Registry::register_external(hydra_origin::signed(BOB.into()), location).unwrap();

	next_asset_id
}

fn treasury_sufficiency_lock() -> Balance {
	pallet_balances::Locks::<hydradx_runtime::Runtime>::get(TreasuryAccount::get())
		.iter()
		.find(|x| x.id == SUFFICIENCY_LOCK)
		.map(|p| p.amount)
		.unwrap_or_default()
}

/// Assert RuntimeEvent specified number of times.
///
/// Parameters:
/// - `event`
/// - `times` - number of times event should occur.
#[macro_export]
macro_rules! assert_event_times {
	( $x:expr, $y: expr ) => {{
		let mut found: u32 = 0;

		let runtime_events: Vec<RuntimeEvent> = frame_system::Pallet::<hydradx_runtime::Runtime>::events()
			.into_iter()
			.map(|e| e.event)
			.collect();

		for evt in runtime_events {
			if evt == $x {
				found += 1;
			}

			if found > $y {
				panic!("Event found more than: {:?} times.", $y);
			}
		}
		if found != $y {
			if found == 0 {
				panic!("Event not found.");
			}

			panic!("Event found {:?} times, expected: {:?}", found, $y);
		}
	}};
}
