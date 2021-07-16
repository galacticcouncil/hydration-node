use super::*;
use crate::mock::{
	Currencies, Duster, Event as TestEvent, ExtBuilder, Origin, System, Test, Tokens, ALICE, DUSTER, TREASURY,
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

		assert_ok!(Duster::dust_account(Origin::signed(*DUSTER), *ALICE, currency_id));
		assert_eq!(Currencies::free_balance(currency_id, &*TREASURY), 990_500);

		for (who, _, _) in orml_tokens::Accounts::<Test>::iter() {
			assert_ne!(who, *ALICE, "Alice account should have been removed!");
		}

		assert_eq!(Currencies::free_balance(0, &*DUSTER), 110_000);

		expect_events(vec![
			// dust transfer
			pallet_balances::Event::Transfer(*ALICE, *TREASURY, 500).into(),
			orml_currencies::Event::Transferred(currency_id, *ALICE, *TREASURY, 500).into(),
			// duster
			Event::Dusted(*ALICE, 500).into(),
			//reward transfer
			pallet_balances::Event::Transfer(*TREASURY, *DUSTER, 10_000).into(),
			orml_currencies::Event::Transferred(currency_id, *TREASURY, *DUSTER, 10_000).into(),
		]);
	});
}
