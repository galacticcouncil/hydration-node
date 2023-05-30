use super::*;
use crate::mock::{
	AssetId, Currencies, Duster, ExtBuilder, RuntimeEvent as TestEvent, RuntimeOrigin, System, Test, Tokens, ALICE,
	BOB, DUSTER, KILLED, TREASURY,
};

use frame_support::{assert_noop, assert_ok};

use sp_runtime::traits::BadOrigin;

#[test]
fn dust_account_works() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);

			for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
				assert_ne!(who, *ALICE, "Alice account should have been removed!");
			}

			assert_eq!(Currencies::free_balance(0, &*DUSTER), 10_000);
		});
}

#[test]
fn reward_duster_can_fail() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Currencies::transfer(
				RuntimeOrigin::signed(*TREASURY),
				*BOB,
				0,
				1_000_000
			));

			assert_ok!(Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);

			for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
				assert_ne!(who, *ALICE, "Alice account should have been removed!");
			}

			assert_eq!(Currencies::free_balance(0, &*DUSTER), 0);
		});
}

#[test]
fn dust_account_with_sufficient_balance_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 1_000_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_account_with_exact_dust_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_nonexisting_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(RuntimeOrigin::signed(*DUSTER), 123456, 1),
			Error::<Test>::ZeroBalance
		); // Fails with zero balance because total_balance for non-existing account returns default value = Zero.
		assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
	});
}

#[test]
fn dust_treasury_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *TREASURY, 1),
			Error::<Test>::AccountBlacklisted
		);
	});
}

fn expect_events(e: Vec<TestEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

#[test]
fn dust_account_native_works() {
	let mut ext = ExtBuilder::default()
		.with_native_balance(*ALICE, 500)
		.with_native_balance(*DUSTER, 100_000)
		.build();
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext.execute_with(|| {
		let currency_id: AssetId = 0;

		assert!(KILLED.with(|r| r.borrow().is_empty()));

		assert_ok!(Duster::dust_account(
			RuntimeOrigin::signed(*DUSTER),
			*ALICE,
			currency_id
		));
		assert_eq!(Currencies::free_balance(currency_id, &*TREASURY), 990_500);

		assert_eq!(Currencies::free_balance(0, &*DUSTER), 110_000);

		assert_eq!(KILLED.with(|r| r.borrow().clone()), vec![*ALICE]);
		for (a, _) in frame_system::Account::<Test>::iter() {
			assert_ne!(a, *ALICE, "Alice account should have been removed!");
		}

		expect_events(vec![
			// system
			frame_system::Event::KilledAccount { account: *ALICE }.into(),
			// dust transfer
			pallet_balances::Event::Transfer {
				from: *ALICE,
				to: *TREASURY,
				amount: 500,
			}
			.into(),
			// duster
			Event::Dusted {
				who: *ALICE,
				amount: 500,
			}
			.into(),
			//reward transfer
			pallet_balances::Event::Transfer {
				from: *TREASURY,
				to: *DUSTER,
				amount: 10_000,
			}
			.into(),
		]);
	});
}

#[test]
fn native_existential_deposit() {
	let mut ext = ExtBuilder::default()
		.with_native_balance(*DUSTER, 100_000)
		.with_balance(*DUSTER, 1, 100_000)
		.with_balance(*DUSTER, 2, 100_000)
		.build();
	ext.execute_with(|| {
		System::set_block_number(1);
	});
	ext.execute_with(|| {
		let currency_id: AssetId = 2;

		assert_ok!(Currencies::transfer(RuntimeOrigin::signed(*DUSTER), *ALICE, 2, 20_000));
		assert_ok!(Currencies::transfer(RuntimeOrigin::signed(*DUSTER), *ALICE, 0, 600));
		assert_ok!(Currencies::transfer(RuntimeOrigin::signed(*ALICE), *DUSTER, 0, 300));

		assert_eq!(Currencies::free_balance(0, &*ALICE), 300);

		assert_ok!(Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 0));

		assert_eq!(Currencies::free_balance(0, &*ALICE), 0);

		// should be empty, because there is one provider (tokens)( for alice account, so not killed
		assert!(KILLED.with(|r| r.borrow().is_empty()));

		expect_events(vec![
			// first transfer
			frame_system::Event::NewAccount { account: *ALICE }.into(),
			orml_tokens::Event::Endowed {
				currency_id,
				who: *ALICE,
				amount: 20_000,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id,
				from: *DUSTER,
				to: *ALICE,
				amount: 20_000,
			}
			.into(),
			//second tranfer
			pallet_balances::Event::Endowed {
				account: *ALICE,
				free_balance: 600,
			}
			.into(),
			pallet_balances::Event::Transfer {
				from: *DUSTER,
				to: *ALICE,
				amount: 600,
			}
			.into(),
			// 3rd transfer
			pallet_balances::Event::Transfer {
				from: *ALICE,
				to: *DUSTER,
				amount: 300,
			}
			.into(),
			// dust transfer
			pallet_balances::Event::Transfer {
				from: *ALICE,
				to: *TREASURY,
				amount: 300,
			}
			.into(),
			// duster
			Event::Dusted {
				who: *ALICE,
				amount: 300,
			}
			.into(),
			//reward transfer
			pallet_balances::Event::Transfer {
				from: *TREASURY,
				to: *DUSTER,
				amount: 10_000,
			}
			.into(),
		]);

		System::reset_events();

		// Transfer all remaining tokens from Alice accounts - should kill the account

		assert_ok!(Currencies::transfer(RuntimeOrigin::signed(*ALICE), *DUSTER, 2, 20_000));

		assert_eq!(KILLED.with(|r| r.borrow().clone()), vec![*ALICE]);

		expect_events(vec![
			// first transfer
			frame_system::Event::KilledAccount { account: *ALICE }.into(),
			orml_tokens::Event::Transfer {
				currency_id,
				from: *ALICE,
				to: *DUSTER,
				amount: 20_000,
			}
			.into(),
		]);
	});
}

#[test]
fn add_nondustable_account_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::add_nondustable_account(RuntimeOrigin::signed(*DUSTER), *ALICE),
			BadOrigin
		);

		assert!(Duster::blacklisted(*ALICE).is_none());

		assert_ok!(Duster::add_nondustable_account(RuntimeOrigin::root(), *ALICE));

		assert!(Duster::blacklisted(*ALICE).is_some());

		assert_ok!(Duster::add_nondustable_account(RuntimeOrigin::root(), *ALICE));

		assert!(Duster::blacklisted(*ALICE).is_some());
	});
}

#[test]
fn remove_nondustable_account_works() {
	ExtBuilder::default()
		.with_native_balance(*ALICE, 500)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::add_nondustable_account(RuntimeOrigin::root(), *ALICE));
			assert!(Duster::blacklisted(*ALICE).is_some());

			assert_ok!(Duster::add_nondustable_account(RuntimeOrigin::root(), *ALICE));

			// Dust dont work now
			assert_noop!(
				Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::AccountBlacklisted
			);

			assert_noop!(
				Duster::remove_nondustable_account(RuntimeOrigin::signed(*DUSTER), *ALICE),
				BadOrigin
			);

			//remove non-existing account
			assert_noop!(
				Duster::remove_nondustable_account(RuntimeOrigin::root(), 1234556),
				Error::<Test>::AccountNotBlacklisted
			);

			assert_ok!(Duster::remove_nondustable_account(RuntimeOrigin::root(), *ALICE));
			assert!(Duster::blacklisted(*ALICE).is_none());

			// We can dust again
			assert_ok!(Duster::dust_account(RuntimeOrigin::signed(*DUSTER), *ALICE, 0),);
		});
}
