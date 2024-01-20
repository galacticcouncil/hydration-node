#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_ok, sp_runtime::traits::Zero};
use hydradx_runtime::{Balances, Tokens, Treasury};
use orml_traits::{currency::MultiCurrency, GetByKey};
use xcm_emulator::TestExt;

#[test]
fn balance_should_be_dusted_when_native_balance_is_below_ed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let hdx_ed = <hydradx_runtime::Runtime as pallet_balances::Config>::ExistentialDeposit::get();

		assert!(!hdx_ed.is_zero());

		let transfer_amount = hdx_ed.checked_sub(1).unwrap();

		// set Treasury balance to ED so it's not dusted
		assert_ok!(Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			Treasury::account_id(),
			hdx_ed,
		));

		assert_ok!(Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			hdx_ed,
		));

		assert_ok!(Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			transfer_amount,
		));

		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)), 0);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(Treasury::account_id()),
			hdx_ed.checked_add(1).unwrap()
		);

		expect_hydra_events(vec![
			pallet_balances::Event::DustLost {
				account: ALICE.into(),
				amount: 1,
			}
			.into(),
			pallet_balances::Event::Deposit {
				who: Treasury::account_id(),
				amount: 1,
			}
			.into(),
			pallet_treasury::Event::Deposit { value: 1 }.into(),
			pallet_balances::Event::Transfer {
				from: ALICE.into(),
				to: BOB.into(),
				amount: transfer_amount,
			}
			.into(),
		]);
	});
}

#[test]
fn balance_should_be_dusted_when_token_balance_is_below_ed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 0);

		let dai_ed = <hydradx_runtime::Runtime as orml_tokens::Config>::ExistentialDeposits::get(&DAI);

		assert!(!dai_ed.is_zero());

		let transfer_amount = dai_ed.checked_sub(1).unwrap();

		assert_ok!(Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			DAI,
			dai_ed,
			0,
		));

		assert_ok!(Tokens::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			DAI,
			transfer_amount,
		));

		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE)), 0);
		// Treasury is whitelisted in Tokens
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 1);

		expect_hydra_events(vec![
			pallet_currencies::Event::Transferred {
				currency_id: DAI,
				from: ALICE.into(),
				to: Treasury::account_id(),
				amount: 1,
			}
			.into(),
			orml_tokens::Event::DustLost {
				currency_id: DAI,
				who: ALICE.into(),
				amount: 1,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: DAI,
				from: ALICE.into(),
				to: BOB.into(),
				amount: transfer_amount,
			}
			.into(),
		]);
	});
}
