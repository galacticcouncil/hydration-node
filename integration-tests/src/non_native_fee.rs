#![cfg(test)]

use crate::polkadot_test_net::*;
use primitives::Price;

use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};

use hydradx_runtime::{Balances, Currencies, MultiTransactionPayment, RuntimeOrigin, Tokens};

use frame_support::dispatch::{DispatchInfo, Weight};
use orml_traits::currency::MultiCurrency;
use polkadot_primitives::v2::BlockNumber;
use sp_runtime::traits::SignedExtension;
use xcm_emulator::TestExt;

pub fn hydra_run_to_block(to: BlockNumber) {
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::MultiTransactionPayment::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::MultiTransactionPayment::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

#[test]
fn non_native_fee_payment_works_with_omnipool_spot_price() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: BTC },
		);

		let info = DispatchInfo {
			weight: Weight::from_ref_time(106_957_000),
			..Default::default()
		};
		let len: usize = 10;

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0).pre_dispatch(
				&AccountId::from(BOB),
				&call,
				&info,
				len,
			)
		);
		let bob_balance = hydradx_runtime::Tokens::free_balance(BTC, &AccountId::from(BOB));
		assert_eq!(bob_balance, 999_959);

		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			2_000_000_000_000 * UNITS,
			0,
		));

		init_omnipool();

		hydra_run_to_block(2);

		let call = hydradx_runtime::RuntimeCall::MultiTransactionPayment(
			pallet_transaction_multi_payment::Call::set_currency { currency: DAI },
		);

		assert_ok!(
			pallet_transaction_payment::ChargeTransactionPayment::<hydradx_runtime::Runtime>::from(0).pre_dispatch(
				&AccountId::from(DAVE),
				&call,
				&info,
				len,
			)
		);

		let dave_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(DAVE));
		assert_eq!(dave_balance, 999_991_799_671_574_511_701); //Omnipool spot price
	});
}

const HITCHHIKER: [u8; 32] = [42u8; 32];

#[test]
fn fee_currency_on_account_lifecycle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);

		// ------------ set on create ------------
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_eq!(
			Tokens::free_balance(1, &AccountId::from(HITCHHIKER)),
			50_000_000_000_000
		);
		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			Some(1)
		);

		// ------------ remove on delete ------------
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(HITCHHIKER.into()),
			BOB.into(),
			1,
			false,
		));

		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);
	});
}

#[test]
fn pepe_is_not_registered() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			PEPE,
			Price::from(10)
		));
	});
}

#[test]
fn fee_currency_cannot_be_set_to_not_accepted_asset() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// assemble
		let amount = 50_000_000 * UNITS;
		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);

		// act
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(BOB.into()),
			HITCHHIKER.into(),
			PEPE,
			amount,
		));

		// assert
		assert_eq!(Tokens::free_balance(PEPE, &AccountId::from(HITCHHIKER)), amount);
		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);
	});
}

#[test]
fn fee_currency_should_not_change_when_account_holds_native_currency_already() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Balances::set_balance(
			RuntimeOrigin::root(),
			HITCHHIKER.into(),
			UNITS,
			0,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_eq!(Balances::free_balance(&AccountId::from(HITCHHIKER)), UNITS);
		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);
	});
}

#[test]
fn fee_currency_should_not_change_when_account_holds_other_token_already() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			2,
			50_000_000_000,
		));

		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			Some(1)
		);
	});
}

#[test]
fn fee_currency_should_reset_to_default_when_account_spends_tokens() {
	TestNet::reset();
	Hydra::execute_with(|| {
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			1,
			50_000_000_000_000,
		));

		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			HITCHHIKER.into(),
			2,
			50_000_000_000,
		));
		assert_ok!(Tokens::transfer_all(
			RuntimeOrigin::signed(HITCHHIKER.into()),
			ALICE.into(),
			1,
			false,
		));

		assert_eq!(
			MultiTransactionPayment::get_currency(&AccountId::from(HITCHHIKER)),
			None
		);
	});
}
