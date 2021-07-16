use super::*;
use crate::mock::{
	Currencies, Duster, Event as TestEvent, ExtBuilder, Origin, System, Test, Tokens, ALICE, DUSTER, KILLED, TREASURY,
};
use frame_support::{assert_noop, assert_ok};
use primitives::AssetId;

#[test]
fn dust_account_works() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1));
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 100);

			for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
				assert_ne!(who, *ALICE, "Alice account should have been removed!");
			}

			assert_eq!(Currencies::free_balance(0, &*DUSTER), 10_000);
		});
}
#[test]
fn dust_account_with_sufficient_balance_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 1_000_000)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
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
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::BalanceSufficient
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_account_with_zero_fails() {
	ExtBuilder::default()
		.with_balance(*ALICE, 1, 0)
		.build()
		.execute_with(|| {
			assert_noop!(
				Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 1),
				Error::<Test>::ZeroBalance
			);
			assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
		});
}

#[test]
fn dust_nonexisting_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(Origin::signed(*DUSTER), 123456, 1),
			Error::<Test>::ZeroBalance
		); // Fails with zero balance because total_balance for non-existing account returns default value = Zero.
		assert_eq!(Tokens::free_balance(1, &*TREASURY), 0);
	});
}

#[test]
fn dust_treasury_account_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Duster::dust_account(Origin::signed(*DUSTER), *TREASURY, 1),
			Error::<Test>::AccountBlacklisted
		);
	});
}

fn last_events(n: usize) -> Vec<TestEvent> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

fn expect_events(e: Vec<TestEvent>) {
	assert_eq!(last_events(e.len()), e);
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

		assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, currency_id));
		assert_eq!(Currencies::free_balance(currency_id, &*TREASURY), 990_500);

		assert_eq!(Currencies::free_balance(0, &*DUSTER), 110_000);

		assert_eq!(KILLED.with(|r| r.borrow().clone()), vec![*ALICE]);
		for (a, _) in frame_system::Account::<Test>::iter() {
			assert_ne!(a, *ALICE, "Alice account should have been removed!");
		}

		expect_events(vec![
			// dust transfer
			pallet_balances::Event::Transfer(*ALICE, *TREASURY, 500).into(),
			orml_currencies::Event::Transferred(currency_id, *ALICE, *TREASURY, 500).into(),
			// system
			frame_system::Event::KilledAccount(*ALICE).into(),
			// duster
			Event::Dusted(*ALICE, 500).into(),
			//reward transfer
			pallet_balances::Event::Transfer(*TREASURY, *DUSTER, 10_000).into(),
			orml_currencies::Event::Transferred(currency_id, *TREASURY, *DUSTER, 10_000).into(),
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

		assert_ok!(Currencies::transfer(Origin::signed(*DUSTER), *ALICE, 2, 20_000));
		assert_ok!(Currencies::transfer(Origin::signed(*DUSTER), *ALICE, 0, 600));
		assert_ok!(Currencies::transfer(Origin::signed(*ALICE), *DUSTER, 0, 300));

		assert_eq!(Currencies::free_balance(0, &*ALICE), 300);

		assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, 0));

		assert_eq!(Currencies::free_balance(0, &*ALICE), 0);

		// should be empty, because there is one provider (tokens)( for alice account, so not killed
		assert!(KILLED.with(|r| r.borrow().is_empty()));

		expect_events(vec![
			// first transfer
			frame_system::Event::NewAccount(*ALICE).into(),
			orml_tokens::Event::Endowed(currency_id, *ALICE, 20_000).into(),
			orml_currencies::Event::Transferred(currency_id, *DUSTER, *ALICE, 20_000).into(),
			//second tranfer
			pallet_balances::Event::Endowed(*ALICE, 600).into(),
			pallet_balances::Event::Transfer(*DUSTER, *ALICE, 600).into(),
			orml_currencies::Event::Transferred(0, *DUSTER, *ALICE, 600).into(),
			// 3rd transfer
			pallet_balances::Event::Transfer(*ALICE, *DUSTER, 300).into(),
			orml_currencies::Event::Transferred(0, *ALICE, *DUSTER, 300).into(),
			// dust transfer
			pallet_balances::Event::Transfer(*ALICE, *TREASURY, 300).into(),
			orml_currencies::Event::Transferred(0, *ALICE, *TREASURY, 300).into(),
			// duster
			Event::Dusted(*ALICE, 300).into(),
			//reward transfer
			pallet_balances::Event::Transfer(*TREASURY, *DUSTER, 10_000).into(),
			orml_currencies::Event::Transferred(0, *TREASURY, *DUSTER, 10_000).into(),
		]);

		System::reset_events();

		// TTransfer all remaining tokens from Alice accounts - should kill the account

		assert_ok!(Currencies::transfer(Origin::signed(*ALICE), *DUSTER, 2, 20_000));

		assert_eq!(KILLED.with(|r| r.borrow().clone()), vec![*ALICE]);

		expect_events(vec![
			// first transfer
			frame_system::Event::KilledAccount(*ALICE).into(),
			orml_currencies::Event::Transferred(currency_id, *ALICE, *DUSTER, 20_000).into(),
		]);
	});
}
