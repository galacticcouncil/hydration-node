use crate::tests::mock::{expect_events, ExtBuilder, System, Test, Tokens, ALICE};
use crate::types::LockdownStatus;
use crate::AssetLockdownState;
use crate::Event as CircuitBreakerEvent;
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
			expect_events(vec![
				CircuitBreakerEvent::AssetLockdown {
					asset_id: ASSET_ID,
					until: 12,
				}
				.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 60,
				}
				.into(),
			]);
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));
			assert_balance!(ALICE, ASSET_ID, 100);

			expect_events(vec![
				CircuitBreakerEvent::AssetLockdown {
					asset_id: ASSET_ID,
					until: 12,
				}
				.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 101,
				}
				.into(),
			]);
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));
			expect_events(vec![
				CircuitBreakerEvent::AssetLockdown {
					asset_id: ASSET_ID,
					until: 12,
				}
				.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 10,
				}
				.into(),
			]);
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));
			assert_balance!(ALICE, ASSET_ID, 100);

			System::set_block_number(13);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			assert_balance!(ALICE, ASSET_ID, 200);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(23));
			expect_events(vec![
				CircuitBreakerEvent::AssetLockdown {
					asset_id: ASSET_ID,
					until: 23,
				}
				.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 101,
				}
				.into(),
			]);
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Unlocked((2, 0))); // Baseline is 0

			// Act: Move time forward so the period expires, then deposit an amount that exceeds the limit
			System::set_block_number(13);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));

			// Assert: The asset should be locked, and the excess amount is not deposited
			assert_balance!(ALICE, ASSET_ID, 200); // 100 (original) + 100 (allowed from this deposit)
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(23)); // 13 (current block) + 10 (period)
			expect_events(vec![
				CircuitBreakerEvent::AssetLockdown {
					asset_id: ASSET_ID,
					until: 23,
				}
				.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 101,
				}
				.into(),
			]);
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Unlocked((2, 0)));

			// Act: Move time forward, but stay within the period.
			// The second deposit, when combined with the first, exceeds the limit.
			System::set_block_number(5);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));

			// Assert: The asset should be locked, and only the amount up to the limit is deposited.
			assert_balance!(ALICE, ASSET_ID, 100); // 60 (original) + 40 (allowed from this deposit)
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(15)); // 5 (current block) + 10 (period)
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(14)); // 4 (current block) + 10 (period)
		});
}

#[test]
fn deposit_limit_should_give_free_pass_when_lockdown_expires_but_new_amount_not_exceeding_limit() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: Trigger a lockdown and then let it expire.
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			assert_balance!(ALICE, ASSET_ID, 100);
			System::set_block_number(12); // Lockdown from block 2 is now expired (2 + 10 < 13)

			// Act
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 100));
			assert_balance!(ALICE, ASSET_ID, 200);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Unlocked((12, 101)));
		});
}

#[test]
fn unlock_is_updated_when_asset_is_unlocked_and_expired() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: Trigger a lockdown and then let it expire.
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 1));
			assert_balance!(ALICE, ASSET_ID, 1);

			System::set_block_number(13);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 100));
			assert_balance!(ALICE, ASSET_ID, 101);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Unlocked((13, 1)));
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

			// Act
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 190);

			// Act - This second deposit should trip the breaker
			System::set_block_number(14);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 200);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(24)); // Expected: 14 + 10
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
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));

			System::set_block_number(5);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			assert_balance!(ALICE, ASSET_ID, 100);
			let state_after = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(
				state_after,
				LockdownStatus::Locked(12),
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

#[test]
fn unlock_event_should_be_emitted_when_asset_unlocked() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			System::set_block_number(2);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			let balance = Tokens::free_balance(10000, &ALICE);
			assert_eq!(balance, 100);

			System::set_block_number(13);

			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 1));

			expect_events(vec![
				CircuitBreakerEvent::AssetLockdownRemoved { asset_id: ASSET_ID }.into(),
				orml_tokens::Event::<Test>::Deposited {
					currency_id: ASSET_ID,
					who: ALICE,
					amount: 1,
				}
				.into(),
			]);
		});
}

//TODO: fix once we have clarity
#[ignore]
#[test]
fn rate_limit_should_not_be_bypassed_by_burning_tokens() {
	ExtBuilder::default()
		.with_deposit_period(100)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: Mint 90 tokens, which is under the limit. This sets the baseline.
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 90);

			// Act 1: The attacker burns the newly created tokens to reset the *total supply*.
			// This tricks the circuit breaker which only measures net supply change.
			System::set_block_number(3);
			assert_ok!(Tokens::withdraw(ASSET_ID, &ALICE, 90));
			assert_balance!(ALICE, ASSET_ID, 0);

			// Act 2: The attacker mints another 90 tokens. The gross issuance in this period
			// is now 180, which should be blocked. But the net increase is only 90.
			System::set_block_number(4);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 90));

			// Assert: The circuit breaker should have tripped. The first 90 tokens were minted.
			// The remaining limit was 10. So only 10 of this second deposit should have succeeded.
			// The current buggy logic will allow the full 90, resulting in a balance of 90.
			assert_balance!(ALICE, ASSET_ID, 10);
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(14));
		});
}
