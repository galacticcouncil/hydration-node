use crate::tests::mock::CircuitBreaker;
use crate::tests::mock::{ExtBuilder, System, Test, Tokens, ALICE, BOB};
use crate::types::LockdownStatus;
use crate::{AssetLockdownState, Error};
use frame_support::dispatch::RawOrigin;
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use sp_runtime::DispatchError;
use test_utils::assert_balance;
pub const ASSET_ID: u32 = 10000;
#[test]
fn save_deposit_should_release_amount() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);

			//Act
			assert_ok!(CircuitBreaker::save_deposit(
				RawOrigin::Signed(ALICE).into(),
				ALICE,
				ASSET_ID,
				10
			));

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 110);
		});
}

#[test]
fn save_deposit_should_be_callable_by_other_origin() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);

			//Act
			assert_ok!(CircuitBreaker::save_deposit(
				RawOrigin::Signed(BOB).into(),
				ALICE,
				ASSET_ID,
				10
			));

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 110);
		});
}

#[test]
fn save_deposit_should_be_callable_by_root() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);

			//Act
			assert_ok!(CircuitBreaker::save_deposit(
				RawOrigin::Root.into(),
				ALICE,
				ASSET_ID,
				10
			));

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 110);
		});
}

#[test]
fn save_deposit_should_not_work_when_asset_in_lockdown() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			//Act and assert
			assert_noop!(
				CircuitBreaker::save_deposit(RawOrigin::Root.into(), ALICE, ASSET_ID, 10),
				Error::<Test>::AssetInLockdown
			);

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);
		});
}

#[test]
fn save_deposit_should_work_when_asset_in_lockdown_but_expired() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);

			//Act and assert
			assert_ok!(CircuitBreaker::save_deposit(
				RawOrigin::Root.into(),
				ALICE,
				ASSET_ID,
				10
			),);

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 110);
		});
}

#[test]
fn save_deposit_should_work_when_asset_in_unlocked_state() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 20)); //This sets the asset to unlocked state

			//Act and assert
			assert_ok!(CircuitBreaker::save_deposit(
				RawOrigin::Root.into(),
				ALICE,
				ASSET_ID,
				10
			),);

			//Assert
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 130);
		});
}

fn save_deposit_should_fail_when_amount_is_zero() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			//Arrange
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 20)); //This sets the asset to unlocked state

			//Act and assert
			assert_noop!(
				CircuitBreaker::save_deposit(RawOrigin::Root.into(), ALICE, ASSET_ID, 0),
				Error::<Test>::InvalidAmount
			);
		});
}
