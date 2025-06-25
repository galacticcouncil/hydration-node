use crate::tests::mock::{ExtBuilder, System, Test, Tokens, ALICE};
use crate::types::AssetLockdownState;
use crate::LastAssetIssuance;
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use sp_runtime::DispatchError;
use test_utils::assert_balance;

pub const ASSET_ID: u32 = 10000;
#[test]
fn deposit_limit_should_work() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 50);

			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);
		});
}

#[test]
fn deposit_limit_should_work_when_first_deposit_exceed_limit() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));
			assert_balance!(ALICE, ASSET_ID, 100);
		});
}

#[test]
fn deposit_limit_should_lock_deposits_when_asset_on_lockdown() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 100));

			let balance = Tokens::free_balance(ASSET_ID, &ALICE);
			assert_eq!(balance, 100);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 10));
			let balance = Tokens::free_balance(ASSET_ID, &ALICE);
			assert_eq!(balance, 100); //No balance change, as the asset is locked down
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));
		});
}

#[test]
fn deposit_limit_should_lock_when_lock_expires_but_amount_reaches_limit_again() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));
			assert_balance!(ALICE, ASSET_ID, 100);

			System::set_block_number(13);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			assert_balance!(ALICE, ASSET_ID, 200);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(23));
		});
}

#[test]
fn deposit_limit_should_lock_when_asset_already_in_unlocked() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: First deposit is under the limit, setting a baseline
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 100));
			assert_balance!(ALICE, ASSET_ID, 100);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Unlocked((2, 0))); // Baseline is 0

			// Act: Move time forward so the period expires, then deposit an amount that exceeds the limit
			System::set_block_number(13);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));

			// Assert: The asset should be locked, and the excess amount is not deposited
			assert_balance!(ALICE, ASSET_ID, 200); // 100 (original) + 100 (allowed from this deposit)
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(23)); // 13 (current block) + 10 (period)
		});
}

#[test]
fn deposit_limit_should_block_multiple_small_deposits_within_the_same_period() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: First deposit is under the limit
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 60));
			assert_balance!(ALICE, ASSET_ID, 60);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Unlocked((2, 0)));

			// Act: Move time forward, but stay within the period.
			// The second deposit, when combined with the first, exceeds the limit.
			System::set_block_number(5);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));

			// Assert: The asset should be locked, and only the amount up to the limit is deposited.
			assert_balance!(ALICE, ASSET_ID, 100); // 60 (original) + 40 (allowed from this deposit)
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(15)); // 5 (current block) + 10 (period)
		});
}

#[test]
fn deposit_limit_should_trigger_when_limit_is_exactly_met_then_exceeded() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: Make two deposits that exactly meet the limit.
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			System::set_block_number(3);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			assert_balance!(ALICE, ASSET_ID, 100);

			// Act: The very next deposit should trigger the breaker.
			System::set_block_number(4);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 1));

			// Assert: The asset should be locked, and the excess amount (1) is not deposited.
			assert_balance!(ALICE, ASSET_ID, 100);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(14)); // 4 (current block) + 10 (period)
		});
}

#[test]
fn deposit_limit_should_not_give_free_pass_after_lockdown_expires() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: Trigger a lockdown and then let it expire.
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			assert_balance!(ALICE, ASSET_ID, 100);
			System::set_block_number(13); // Lockdown from block 2 is now expired (2 + 10 < 13)

			// Act: Make a deposit. With the bug, this deposit gets a "free pass"
			// and incorrectly sets the baseline *after* its amount is included.
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 190);

			// Act again: This second deposit should trip the breaker, but won't due to the bug.
			System::set_block_number(14);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 200);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(24)); // Expected: 14 + 10
		});
}

#[test]
fn deposit_should_be_fully_locked_when_asset_is_already_on_lockdown() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			assert_balance!(ALICE, ASSET_ID, 100);
			let state = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));

			System::set_block_number(5);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			assert_balance!(ALICE, ASSET_ID, 100);
			let state_after = LastAssetIssuance::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(
				state_after,
				AssetLockdownState::Locked(12),
				"Lockdown should not have been modified"
			);
		});
}

#[test]
fn lockdown_should_be_ignored_when_no_limit_set_for_asset() {
	ExtBuilder::default().with_deposit_period(10).build().execute_with(|| {
		assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 210));
		let balance = Tokens::free_balance(10000, &ALICE);
		assert_eq!(balance, 210);

		System::set_block_number(2);

		assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 100));
		let balance = Tokens::free_balance(10000, &ALICE);
		assert_eq!(balance, 310);

		assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 91));
		let balance = Tokens::free_balance(10000, &ALICE);
		assert_eq!(balance, 401);
	});
}
