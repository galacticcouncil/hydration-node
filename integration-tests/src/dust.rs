#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_noop;
use frame_support::storage::with_transaction;
use frame_support::{assert_ok, sp_runtime::traits::Zero};
use hex_literal::hex;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{AssetRegistry, Balances, Currencies, Duster, System, Tokens, Treasury};
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::BoundErc20;
use orml_traits::MultiReservableCurrency;
use orml_traits::{currency::MultiCurrency, GetByKey};
use sp_runtime::{DispatchResult, TransactionOutcome};
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

		assert_ok!(Balances::transfer_allow_death(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			transfer_amount,
		));

		assert_eq!(hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)), 0);
		assert_eq!(
			hydradx_runtime::Balances::free_balance(Treasury::account_id()),
			hdx_ed.checked_add(1).unwrap()
		);

		expect_hydra_last_events(vec![
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

		expect_hydra_last_events(vec![
			orml_tokens::Event::Transfer {
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

#[test]
fn dust_account_should_work_when_token_balance_below_ed() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 0);

		set_ed(DAI, 0);
		assert_ok!(Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			DAI,
			1,
			0,
		));

		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE)), 1);

		set_ed(DAI, 10);
		assert_ok!(Duster::dust_account(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ALICE.into(),
			DAI,
		));

		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE)), 0);
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 1);
	});
}

#[test]
fn account_cannot_be_dusted_when_leftover_is_reserved() {
	TestNet::reset();

	Hydra::execute_with(|| {
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 0);

		set_ed(DAI, 0);
		assert_ok!(Tokens::set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			ALICE.into(),
			DAI,
			1,
			0,
		));

		assert_ok!(hydradx_runtime::Currencies::reserve(DAI, &AccountId::from(ALICE), 1));
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE)), 0);
		assert_eq!(hydradx_runtime::Tokens::total_balance(DAI, &AccountId::from(ALICE)), 1);
		assert_eq!(
			hydradx_runtime::Tokens::reserved_balance(DAI, &AccountId::from(ALICE)),
			1
		);

		set_ed(DAI, 10);
		assert_noop!(
			Duster::dust_account(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), ALICE.into(), DAI,),
			orml_tokens::Error::<hydradx_runtime::Runtime>::BalanceTooLow
		);

		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &AccountId::from(ALICE)), 0);
		assert_eq!(
			hydradx_runtime::Tokens::reserved_balance(DAI, &AccountId::from(ALICE)),
			1
		);
		assert_eq!(hydradx_runtime::Tokens::free_balance(DAI, &Treasury::account_id()), 0);
	});
}

mod atoken_dust {
	use super::*;
	use crate::aave_router::ADOT;
	use frame_support::{assert_noop, assert_ok};
	use hydradx_runtime::EVMAccounts;
	const START_BALANCE: u128 = 1_000_000_000_000_000;

	use pallet_broadcast::types::Asset;
	use primitives::constants::currency::UNITS;
	use proptest::{
		prelude::*,
		test_runner::{Config, TestRunner},
	};

	#[test]
	fn atoken_should_not_be_dusted_when_atoken_balance_is_below_ed_after_transfer() {
		TestNet::reset();

		crate::aave_router::with_atoken(|| {
			set_ed(ADOT, 1000);

			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 1000000000000000);
			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				1000000000000000 - 1
			),);
			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 1);
		});
	}

	#[test]
	fn dust_account_should_work_when_atoken_balance_below_ed() {
		TestNet::reset();

		crate::aave_router::with_atoken(|| {
			let ed = 10000;
			set_ed(ADOT, ed);

			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), START_BALANCE);

			//Make acocunt fall below ED
			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				START_BALANCE - 1
			),);

			assert_ok!(EVMAccounts::bind_evm_address(hydradx_runtime::RuntimeOrigin::signed(
				ALICE.into()
			)));

			assert_eq!(
				hydradx_runtime::Currencies::free_balance(ADOT, &Treasury::account_id()),
				0
			);

			//We set some AToken underlying balance to be sure it wont change after dusting
			let initial_underlying_balance = 3 * UNITS;
			assert_ok!(Currencies::deposit(DOT, &ALICE.into(), initial_underlying_balance));
			assert_eq!(Currencies::free_balance(DOT, &ALICE.into()), initial_underlying_balance);

			assert_ok!(Duster::dust_account(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				ALICE.into(),
				ADOT,
			));

			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 0);
			assert_eq!(Currencies::free_balance(ADOT, &Treasury::account_id()), 1);
			assert_ok!(Currencies::deposit(DOT, &ALICE.into(), initial_underlying_balance));
		});
	}

	#[test]
	fn no_dusting_when_atoken_balance_above_ed() {
		TestNet::reset();

		crate::aave_router::with_atoken(|| {
			let ed = 10000;
			set_ed(ADOT, ed);

			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), START_BALANCE);

			assert_ok!(Currencies::transfer(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				ADOT,
				START_BALANCE - ed - 1
			),);

			assert_noop!(
				Duster::dust_account(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), ALICE.into(), ADOT,),
				pallet_duster::Error::<hydradx_runtime::Runtime>::BalanceSufficient
			);
			assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), ed + 1);
			assert_eq!(Currencies::free_balance(ADOT, &Treasury::account_id()), 0);
		});
	}

	#[test]
	fn dust_account_invariant() {
		let successfull_cases = 1;

		let ed_range = 1_u128..(START_BALANCE - 1);

		crate::aave_router::with_atoken_rollback(|| {
			// We run prop test this way to use the same state of the chain for all run without loading the snapshot again in every run
			let mut runner = TestRunner::new(Config {
				cases: successfull_cases,
				source_file: Some("integration-tests/src/dust.rs"),
				test_name: Some("dust_prop"),
				..Config::default()
			});

			let _ = runner.run(&ed_range, |ed| {
				let _ = with_transaction(|| {
					let bal = Currencies::free_balance(ADOT, &ALICE.into());
					assert_eq!(START_BALANCE, bal, "Start balance is not as expected");

					// Parameterize chain ED for this run to be `ed + 1`
					// meaning that leaving exactly `ed` in the account will be dust.
					set_ed(ADOT, ed + 1);

					// Transfer all but `ed` to BOB, leaving `ed` on ALICE → dust after ED=ed+1
					assert_ok!(Currencies::transfer(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						BOB.into(),
						ADOT,
						START_BALANCE - ed
					));

					// Dust it
					assert_ok!(Duster::dust_account(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						ALICE.into(),
						ADOT,
					));

					// Assert: ALICE should have been dusted to 0
					assert_eq!(
						Currencies::free_balance(ADOT, &ALICE.into()),
						0,
						"After dusting with ED={ed}+1, remaining `ed` should be reaped."
					);
					assert_eq!(Currencies::free_balance(ADOT, &Treasury::account_id()), ed);

					TransactionOutcome::Rollback(DispatchResult::Ok(()))
				});

				Ok(())
			}).unwrap();
		});
	}
}

fn set_ed(asset_id: AssetId, ed: u128) {
	AssetRegistry::update(
		hydradx_runtime::RuntimeOrigin::root(),
		asset_id,
		None,
		None,
		Some(ed),
		None,
		None,
		None,
		None,
		None,
	)
	.unwrap();
}
