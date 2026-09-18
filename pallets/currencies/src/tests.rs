//! Unit tests for the currencies module.

#![cfg(test)]

use super::*;
use frame_support::{assert_noop, assert_ok, traits::ExistenceRequirement};
use mock::{RuntimeEvent, *};
use sp_runtime::traits::BadOrigin;

#[test]
fn multi_lockable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::set_lock(ID_1, X_TOKEN_ID, &ALICE, 50));
			assert_eq!(Tokens::locks(&ALICE, X_TOKEN_ID).len(), 1);
			assert_ok!(Currencies::set_lock(ID_1, NATIVE_CURRENCY_ID, &ALICE, 50));
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
		});
}

#[test]
fn multi_reservable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::total_issuance(NATIVE_CURRENCY_ID), 200);
			assert_eq!(Currencies::total_issuance(X_TOKEN_ID), 200);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 100);
			assert_eq!(NativeCurrency::free_balance(&ALICE), 100);

			assert_ok!(Currencies::reserve(X_TOKEN_ID, &ALICE, 30));
			assert_ok!(Currencies::reserve(NATIVE_CURRENCY_ID, &ALICE, 40));
			assert_eq!(Currencies::reserved_balance(X_TOKEN_ID, &ALICE), 30);
			assert_eq!(Currencies::reserved_balance(NATIVE_CURRENCY_ID, &ALICE), 40);
		});
}

#[test]
fn named_multi_reservable_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::total_issuance(NATIVE_CURRENCY_ID), 200);
			assert_eq!(Currencies::total_issuance(X_TOKEN_ID), 200);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 100);
			assert_eq!(NativeCurrency::free_balance(&ALICE), 100);

			assert_ok!(Currencies::reserve_named(&RID_1, X_TOKEN_ID, &ALICE, 30));
			assert_ok!(Currencies::reserve_named(&RID_2, X_TOKEN_ID, &ALICE, 50));
			assert_ok!(Currencies::reserve_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE, 20));
			assert_ok!(Currencies::reserve_named(&RID_2, NATIVE_CURRENCY_ID, &ALICE, 60));
			let r1x_before = 30;
			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, X_TOKEN_ID, &ALICE),
				r1x_before
			);
			let r2x_before = 50;
			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, X_TOKEN_ID, &ALICE),
				r2x_before
			);
			let r1n_before = 20;
			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE),
				r1n_before
			);
			let r2n_before = 60;
			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, NATIVE_CURRENCY_ID, &ALICE),
				r2n_before
			);

			let n_free_before = 20;
			assert_eq!(NativeCurrency::free_balance(&ALICE), n_free_before);
			let x_free_before = 20;
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), x_free_before);

			assert_eq!(Currencies::unreserve_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE, 100), 80);
			assert_eq!(NativeCurrency::free_balance(&ALICE), n_free_before + 20);
			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE),
				0
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, NATIVE_CURRENCY_ID, &ALICE),
				r2n_before
			);
			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, X_TOKEN_ID, &ALICE),
				r1x_before
			);
			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, X_TOKEN_ID, &ALICE),
				r2x_before
			);

			assert_eq!(Currencies::unreserve_named(&RID_1, X_TOKEN_ID, &ALICE, 100), 70);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), x_free_before + 30);
			assert_eq!(Currencies::reserved_balance_named(&RID_1, X_TOKEN_ID, &ALICE), 0);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, X_TOKEN_ID, &ALICE),
				r2x_before
			);
			assert_eq!(
				Currencies::reserved_balance_named(&RID_2, NATIVE_CURRENCY_ID, &ALICE),
				r2n_before
			);
		});
}

#[test]
fn native_currency_lockable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::set_lock(ID_1, &ALICE, 10));
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
			assert_ok!(NativeCurrency::remove_lock(ID_1, &ALICE));
			assert_eq!(PalletBalances::locks(&ALICE).len(), 0);
		});
}

#[test]
fn native_currency_reservable_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::reserve(&ALICE, 50));
			assert_eq!(NativeCurrency::reserved_balance(&ALICE), 50);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_lockable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::set_lock(ID_1, &ALICE, 10));
			assert_eq!(PalletBalances::locks(&ALICE).len(), 1);
			assert_ok!(AdaptedBasicCurrency::remove_lock(ID_1, &ALICE));
			assert_eq!(PalletBalances::locks(&ALICE).len(), 0);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_reservable() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::reserve(&ALICE, 50));
			assert_eq!(AdaptedBasicCurrency::reserved_balance(&ALICE), 50);
		});
}

#[test]
fn multi_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer(Some(ALICE).into(), BOB, X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 150);
		});
}

#[test]
fn multi_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				X_TOKEN_ID, &ALICE, 50
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 150);
		});
}

#[test]
fn native_currency_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer_native_currency(Some(ALICE).into(), BOB, 50));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 50);
			assert_eq!(NativeCurrency::free_balance(&BOB), 150);

			assert_ok!(NativeCurrency::transfer(
				&ALICE,
				&BOB,
				10,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 40);
			assert_eq!(NativeCurrency::free_balance(&BOB), 160);

			assert_eq!(Currencies::slash(NATIVE_CURRENCY_ID, &ALICE, 10), 0);
			assert_eq!(NativeCurrency::free_balance(&ALICE), 30);
			assert_eq!(NativeCurrency::total_issuance(), 190);
		});
}

#[test]
fn native_currency_extended_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(NativeCurrency::update_balance(&ALICE, 10));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 110);

			assert_ok!(<Currencies as MultiCurrencyExtended<AccountId>>::update_balance(
				NATIVE_CURRENCY_ID,
				&ALICE,
				10
			));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 120);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_transfer() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(<AdaptedBasicCurrency as BasicCurrency<AccountId>>::transfer(
				&ALICE,
				&BOB,
				50,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&ALICE), 50);
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&BOB), 150);

			// creation fee
			assert_ok!(<AdaptedBasicCurrency as BasicCurrency<AccountId>>::transfer(
				&ALICE,
				&EVA,
				10,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&ALICE), 40);
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&EVA), 10);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_deposit() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::deposit(&EVA, 50));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&EVA), 50);
			assert_eq!(PalletBalances::total_issuance(), 250);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_deposit_throw_error_when_actual_deposit_is_not_expected() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&EVA), 0);
			assert_eq!(PalletBalances::total_issuance(), 200);
			assert_noop!(AdaptedBasicCurrency::deposit(&EVA, 1), Error::<Runtime>::DepositFailed);
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&EVA), 0);
			assert_eq!(PalletBalances::total_issuance(), 200);
			assert_ok!(AdaptedBasicCurrency::deposit(&EVA, 2));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&EVA), 2);
			assert_eq!(PalletBalances::total_issuance(), 202);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_withdraw() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::withdraw(
				&ALICE,
				100,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&ALICE), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_slash() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_eq!(
				<AdaptedBasicCurrency as BasicCurrency<AccountId>>::slash(&ALICE, 101),
				1
			);
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&ALICE), 0);
			assert_eq!(PalletBalances::total_issuance(), 100);
		});
}

#[test]
fn basic_currency_adapting_pallet_balances_update_balance() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(AdaptedBasicCurrency::update_balance(&ALICE, -10));
			assert_eq!(<PalletBalances as PalletCurrency<AccountId>>::total_balance(&ALICE), 90);
			assert_eq!(PalletBalances::total_issuance(), 190);
		});
}

#[test]
fn update_balance_call_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				ALICE,
				NATIVE_CURRENCY_ID,
				-10
			));
			assert_eq!(NativeCurrency::free_balance(&ALICE), 90);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 100);
			assert_ok!(Currencies::update_balance(RuntimeOrigin::root(), ALICE, X_TOKEN_ID, 10));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 110);
		});
}

#[test]
fn update_balance_call_fails_if_not_root_origin() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Currencies::update_balance(Some(ALICE).into(), ALICE, X_TOKEN_ID, 100),
			BadOrigin
		);
	});
}

#[test]
fn call_event_should_work() {
	ExtBuilder::default()
		.one_hundred_for_alice_n_bob()
		.build()
		.execute_with(|| {
			System::set_block_number(1);

			assert_ok!(Currencies::transfer(Some(ALICE).into(), BOB, X_TOKEN_ID, 50));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 50);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 150);
			System::assert_last_event(RuntimeEvent::Currencies(crate::Event::Transferred {
				currency_id: X_TOKEN_ID,
				from: ALICE,
				to: BOB,
				amount: 50,
			}));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::transfer(
				X_TOKEN_ID,
				&ALICE,
				&BOB,
				10,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 40);
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 160);
			System::assert_last_event(RuntimeEvent::Currencies(crate::Event::Transferred {
				currency_id: X_TOKEN_ID,
				from: ALICE,
				to: BOB,
				amount: 10,
			}));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::deposit(
				X_TOKEN_ID, &ALICE, 100
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 140);
			System::assert_last_event(RuntimeEvent::Currencies(crate::Event::Deposited {
				currency_id: X_TOKEN_ID,
				who: ALICE,
				amount: 100,
			}));

			assert_ok!(<Currencies as MultiCurrency<AccountId>>::withdraw(
				X_TOKEN_ID,
				&ALICE,
				20,
				ExistenceRequirement::AllowDeath
			));
			assert_eq!(Currencies::free_balance(X_TOKEN_ID, &ALICE), 120);
			System::assert_last_event(RuntimeEvent::Currencies(crate::Event::Withdrawn {
				currency_id: X_TOKEN_ID,
				who: ALICE,
				amount: 20,
			}));
		});
}

mod repatriate_reserved_named_erc20 {
	use super::*;

	const RESERVED: Balance = 100;

	fn reserve_account() -> AccountId {
		<Runtime as Config>::ReserveAccount::get()
	}

	/// ALICE holds 1000 erc20 and has `RESERVED` of it reserved under `RID_1`.
	fn with_reserved(execute: impl FnOnce()) {
		ExtBuilder::default()
			.erc20_balances(vec![(ALICE, 1_000)])
			.build()
			.execute_with(|| {
				assert_ok!(Currencies::reserve_named(&RID_1, ERC20_TOKEN_ID, &ALICE, RESERVED));

				assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 900);
				assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
				assert_eq!(
					Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
					RESERVED
				);

				execute()
			});
	}

	#[test]
	fn repatriate_reserved_named_should_move_erc20_custody_to_beneficiary_when_status_is_free() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(&RID_1, ERC20_TOKEN_ID, &ALICE, &BOB, 40, BalanceStatus::Free),
				Ok(0)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED - 40
			);
			// the receipt is burned, not handed to the beneficiary - they get the real token
			assert_eq!(Tokens::free_balance(ERC20_TOKEN_ID, &BOB), 0);
			assert_eq!(Tokens::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &BOB), 0);

			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), 40);
			assert_eq!(
				Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()),
				RESERVED - 40
			);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 900);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_move_only_the_receipt_when_status_is_reserved() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&BOB,
					40,
					BalanceStatus::Reserved
				),
				Ok(0)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED - 40
			);
			// under the same identifier, per orml's own semantics
			assert_eq!(Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &BOB), 40);
			assert_eq!(Currencies::reserved_balance_named(&RID_2, ERC20_TOKEN_ID, &BOB), 0);

			// custody untouched: no erc20 call happened at all
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 900);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_clamp_to_reserved_amount_when_value_exceeds_it() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&BOB,
					RESERVED + 30,
					BalanceStatus::Free
				),
				Ok(30)
			);

			// only what was actually reserved moved - the shortfall is reported, not conjured
			assert_eq!(Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), RESERVED);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 900);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_return_custody_to_owner_when_slashed_is_beneficiary_and_status_is_free() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(&RID_1, ERC20_TOKEN_ID, &ALICE, &ALICE, 40, BalanceStatus::Free),
				Ok(0)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED - 40
			);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 940);
			assert_eq!(
				Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()),
				RESERVED - 40
			);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_keep_funds_reserved_when_slashed_is_beneficiary_and_status_is_reserved() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&ALICE,
					40,
					BalanceStatus::Reserved
				),
				Ok(0)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED
			);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &ALICE), 900);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_do_nothing_when_value_is_zero() {
		with_reserved(|| {
			for status in [BalanceStatus::Free, BalanceStatus::Reserved] {
				assert_eq!(
					Currencies::repatriate_reserved_named(&RID_1, ERC20_TOKEN_ID, &ALICE, &BOB, 0, status),
					Ok(0)
				);
			}

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED
			);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_do_nothing_when_reserve_id_does_not_match() {
		with_reserved(|| {
			assert_eq!(
				Currencies::repatriate_reserved_named(&RID_2, ERC20_TOKEN_ID, &ALICE, &BOB, 40, BalanceStatus::Free),
				Ok(40)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED
			);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_fail_when_beneficiary_is_the_reserve_account() {
		with_reserved(|| {
			assert_noop!(
				Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&reserve_account(),
					40,
					BalanceStatus::Free
				),
				crate::Error::<Runtime>::InvalidBeneficiary
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED
			);
			assert_eq!(Tokens::total_issuance(ERC20_TOKEN_ID), RESERVED);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_move_the_receipt_when_beneficiary_is_the_reserve_account_and_status_is_reserved(
	) {
		with_reserved(|| {
			// custody does not change here, so the invariant holds and there is nothing to guard
			assert_eq!(
				Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&reserve_account(),
					40,
					BalanceStatus::Reserved
				),
				Ok(0)
			);

			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &reserve_account()),
				40
			);
			assert_eq!(Tokens::total_issuance(ERC20_TOKEN_ID), RESERVED);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_roll_back_receipt_burn_when_erc20_transfer_reverts() {
		with_reserved(|| {
			revert_erc20_transfers(true);

			assert_noop!(
				Currencies::repatriate_reserved_named(&RID_1, ERC20_TOKEN_ID, &ALICE, &BOB, 40, BalanceStatus::Free),
				DispatchError::Other("erc20 reverted")
			);

			// no receipt burned without a token moved
			assert_eq!(
				Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE),
				RESERVED
			);
			assert_eq!(Tokens::total_issuance(ERC20_TOKEN_ID), RESERVED);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account()), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_preserve_receipt_custody_invariant_when_status_is_free() {
		with_reserved(|| {
			for amount in [10, 25, 65] {
				assert_ok!(Currencies::repatriate_reserved_named(
					&RID_1,
					ERC20_TOKEN_ID,
					&ALICE,
					&BOB,
					amount,
					BalanceStatus::Free
				));

				// every receipt in existence is backed by a token sitting in the reserve account
				assert_eq!(
					Tokens::total_issuance(ERC20_TOKEN_ID),
					Currencies::free_balance(ERC20_TOKEN_ID, &reserve_account())
				);
			}

			assert_eq!(Currencies::reserved_balance_named(&RID_1, ERC20_TOKEN_ID, &ALICE), 0);
			assert_eq!(Tokens::total_issuance(ERC20_TOKEN_ID), 0);
			assert_eq!(Currencies::free_balance(ERC20_TOKEN_ID, &BOB), RESERVED);
		});
	}

	#[test]
	fn repatriate_reserved_named_should_delegate_to_orml_when_asset_is_not_erc20() {
		ExtBuilder::default()
			.one_hundred_for_alice_n_bob()
			.build()
			.execute_with(|| {
				assert_ok!(Currencies::reserve_named(&RID_1, X_TOKEN_ID, &ALICE, 50));

				assert_eq!(
					Currencies::repatriate_reserved_named(&RID_1, X_TOKEN_ID, &ALICE, &BOB, 20, BalanceStatus::Free),
					Ok(0)
				);
				assert_eq!(
					Currencies::repatriate_reserved_named(
						&RID_1,
						X_TOKEN_ID,
						&ALICE,
						&BOB,
						30,
						BalanceStatus::Reserved
					),
					Ok(0)
				);

				assert_eq!(Currencies::reserved_balance_named(&RID_1, X_TOKEN_ID, &ALICE), 0);
				assert_eq!(Currencies::reserved_balance_named(&RID_1, X_TOKEN_ID, &BOB), 30);
				assert_eq!(Currencies::free_balance(X_TOKEN_ID, &BOB), 120);
				assert_eq!(Currencies::total_issuance(X_TOKEN_ID), 200);
			});
	}

	#[test]
	fn repatriate_reserved_named_should_delegate_to_native_currency_when_asset_is_native() {
		ExtBuilder::default()
			.one_hundred_for_alice_n_bob()
			.build()
			.execute_with(|| {
				assert_ok!(Currencies::reserve_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE, 50));

				assert_eq!(
					Currencies::repatriate_reserved_named(
						&RID_1,
						NATIVE_CURRENCY_ID,
						&ALICE,
						&BOB,
						20,
						BalanceStatus::Free
					),
					Ok(0)
				);

				assert_eq!(
					Currencies::reserved_balance_named(&RID_1, NATIVE_CURRENCY_ID, &ALICE),
					30
				);
				assert_eq!(NativeCurrency::free_balance(&BOB), 120);
			});
	}
}
