use crate::tests::mock::{ExtBuilder, System, Tokens, ALICE};
use frame_support::assert_ok;
use orml_traits::MultiCurrency;

#[test]
fn deposit_limit_should_work() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(10000, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Tokens::deposit(10000, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(10000, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);
		});
}

#[test]
fn deposit_limit_should_work_when_first_deposit_exceed_limit() {}

#[test]
fn deposit_limit_should_lock_deposits_when_asset_on_lockdown() {}
