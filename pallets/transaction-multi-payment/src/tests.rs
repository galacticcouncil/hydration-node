pub use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};
use pallet_transaction_payment::ChargeTransactionPayment;
use sp_runtime::traits::SignedExtension;

use frame_support::weights::DispatchInfo;
use orml_traits::MultiCurrency;
use orml_utilities::OrderedSet;
use pallet_balances::Call as BalancesCall;
use primitives::Price;

const CALL: &<Test as frame_system::Config>::Call = &Call::Balances(BalancesCall::transfer(2, 69));

#[test]
fn set_unsupported_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			PaymentModule::set_currency(Origin::signed(ALICE), NOT_SUPPORTED_CURRENCY),
			Error::<Test>::UnsupportedCurrency
		);

		assert_eq!(PaymentModule::get_currency(ALICE), None);
	});
}

#[test]
fn set_supported_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(PaymentModule::set_currency(
			Origin::signed(ALICE),
			SUPPORTED_CURRENCY_WITH_BALANCE
		),);

		assert_eq!(
			PaymentModule::get_currency(ALICE),
			Some(SUPPORTED_CURRENCY_WITH_BALANCE)
		);
	});
}

#[test]
fn set_supported_currency_with_no_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			PaymentModule::set_currency(Origin::signed(ALICE), SUPPORTED_CURRENCY_NO_BALANCE),
			Error::<Test>::ZeroBalance
		);

		assert_eq!(PaymentModule::get_currency(ALICE), None);
	});
}

#[test]
fn set_native_currency() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(PaymentModule::set_currency(Origin::signed(ALICE), HDX),);

		assert_eq!(PaymentModule::get_currency(ALICE), Some(HDX));
	});
}

#[test]
fn set_native_currency_with_no_balance() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			PaymentModule::set_currency(Origin::signed(BOB), HDX),
			Error::<Test>::ZeroBalance
		);
	});
}

#[test]
fn fee_payment_in_native_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 100)
		.build()
		.execute_with(|| {
			let len = 10;
			let info = DispatchInfo {
				weight: 5,
				..Default::default()
			};
			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_ok());

			assert_eq!(Balances::free_balance(CHARLIE), 100 - 5 - 5 - 10);
		});
}

#[test]
fn fee_payment_in_native_currency_with_no_balance() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 10)
		.build()
		.execute_with(|| {
			let len = 10;
			let info = DispatchInfo {
				weight: 5,
				..Default::default()
			};
			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Balances::free_balance(CHARLIE), 10);
		});
}

#[test]
fn fee_payment_in_non_native_currency() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 0)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY_WITH_BALANCE, 1000)
		.build()
		.execute_with(|| {
			// Make sure Charlie ain't got a penny!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			assert_ok!(pallet_amm::Module::<Test>::create_pool(
				Origin::signed(ALICE),
				HDX,
				SUPPORTED_CURRENCY_WITH_BALANCE,
				100000,
				Price::from(1)
			));
			assert_ok!(PaymentModule::set_currency(
				Origin::signed(CHARLIE),
				SUPPORTED_CURRENCY_WITH_BALANCE
			));

			let len = 10;
			let info = DispatchInfo {
				weight: 5,
				..Default::default()
			};

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_ok());

			//Native balance check - Charlie should be still broke!
			assert_eq!(Balances::free_balance(CHARLIE), 0);

			// token check should be less by the fee amount and -1 as fee in amm swap
			assert_eq!(
				Tokens::free_balance(SUPPORTED_CURRENCY_WITH_BALANCE, &CHARLIE),
				1000 - 20 - 1
			);
		});
}

#[test]
fn fee_payment_non_native_insufficient_balance() {
	const CHARLIE: AccountId = 5;

	ExtBuilder::default()
		.base_weight(5)
		.account_native_balance(CHARLIE, 0)
		.account_tokens(CHARLIE, SUPPORTED_CURRENCY_WITH_BALANCE, 10)
		.build()
		.execute_with(|| {
			assert_ok!(pallet_amm::Module::<Test>::create_pool(
				Origin::signed(ALICE),
				HDX,
				SUPPORTED_CURRENCY_WITH_BALANCE,
				100000,
				Price::from(1)
			));

			assert_ok!(PaymentModule::set_currency(
				Origin::signed(CHARLIE),
				SUPPORTED_CURRENCY_WITH_BALANCE
			));

			let len = 10;
			let info = DispatchInfo {
				weight: 5,
				..Default::default()
			};

			assert!(ChargeTransactionPayment::<Test>::from(0)
				.pre_dispatch(&CHARLIE, CALL, &info, len)
				.is_err());

			assert_eq!(Tokens::free_balance(SUPPORTED_CURRENCY_WITH_BALANCE, &CHARLIE), 10);
		});
}

#[test]
fn add_new_accepted_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000]));

		assert_ok!(PaymentModule::add_currency(Origin::signed(BOB), 100));
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000, 100]));
		assert_noop!(
			PaymentModule::add_currency(Origin::signed(ALICE), 1000),
			Error::<Test>::NotAllowed
		);
		assert_noop!(
			PaymentModule::add_currency(Origin::signed(BOB), 100),
			Error::<Test>::AlreadyAccepted
		);
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000, 100]));
	});
}

#[test]
fn removed_accepted_currency() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000]));

		assert_ok!(PaymentModule::add_currency(Origin::signed(BOB), 100));
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000, 100]));

		assert_noop!(
			PaymentModule::remove_currency(Origin::signed(ALICE), 100),
			Error::<Test>::NotAllowed
		);

		assert_noop!(
			PaymentModule::remove_currency(Origin::signed(BOB), 1000),
			Error::<Test>::UnsupportedCurrency
		);

		assert_ok!(PaymentModule::remove_currency(Origin::signed(BOB), 100));

		assert_noop!(
			PaymentModule::remove_currency(Origin::signed(BOB), 100),
			Error::<Test>::UnsupportedCurrency
		);
		assert_eq!(PaymentModule::currencies(), OrderedSet::from(vec![2000, 3000]));
	});
}

#[test]
fn add_member() {
	ExtBuilder::default().base_weight(5).build().execute_with(|| {
		const CHARLIE: AccountId = 3;
		assert_eq!(PaymentModule::authorities(), vec![BOB]);

		assert_ok!(PaymentModule::add_member(Origin::root(), CHARLIE));

		assert_eq!(PaymentModule::authorities(), vec![BOB, CHARLIE]);

		// Non root should not be allowed
		assert_noop!(
			PaymentModule::add_member(Origin::signed(ALICE), CHARLIE),
			sp_runtime::traits::BadOrigin
		);

		// Adding existing member should return error
		assert_noop!(
			PaymentModule::add_member(Origin::root(), CHARLIE),
			Error::<Test>::AlreadyMember
		);

		// Non root should not be allowed
		assert_noop!(
			PaymentModule::remove_member(Origin::signed(ALICE), CHARLIE),
			sp_runtime::traits::BadOrigin
		);

		assert_ok!(PaymentModule::remove_member(Origin::root(), CHARLIE));

		assert_eq!(PaymentModule::authorities(), vec![BOB]);

		assert_noop!(
			PaymentModule::remove_member(Origin::root(), CHARLIE),
			Error::<Test>::NotAMember
		);
	});
}
