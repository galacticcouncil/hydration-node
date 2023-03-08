#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};

use hydradx_runtime::{Balances, Currencies, MultiTransactionPayment, RuntimeOrigin, Tokens};

use orml_traits::currency::MultiCurrency;
use polkadot_primitives::v2::BlockNumber;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
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
		// ------------ BOB ------------
		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			DAI,
		));

		let bob_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(BOB));
		assert_eq!(bob_balance, 999_999_999_051_826_230_041); // fallback price of 1.

		assert_ok!(hydradx_runtime::Balances::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			2_000_000_000_000 * UNITS,
			0,
		));

		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);
		hydradx_runtime::Omnipool::protocol_account();

		assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
			hydradx_runtime::RuntimeOrigin::root(),
			222_222_000_000_000_000_000_000,
		));

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));
		//let spot_price = hydradx_runtime::Omnipool::spot_price(HDX, DAI);
		//assert_eq!(spot_price, Some(Price::from_float(26699.999999999999999999)));

		hydra_run_to_block(2);

		assert_ok!(hydradx_runtime::MultiTransactionPayment::set_currency(
			hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
			DAI,
		));

		let dave_balance = hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(DAVE));
		assert_eq!(dave_balance, 999_974_683_760_342_094_701); //Omnipool spot price
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
