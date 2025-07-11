use crate::tests::mock::{CircuitBreaker, ExtBuilder, RuntimeOrigin, System, Test, Tokens, ALICE};
use crate::types::LockdownStatus;
use crate::{AssetLockdownState, Error, Event};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;

pub const ASSET_ID: u32 = 10000;

#[test]
fn lockdown_asset_should_fork_for_new_asset() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(CircuitBreaker::lockdown_asset(RuntimeOrigin::root(), ASSET_ID, 120));

			// Assert
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(120));
			System::assert_last_event(
				Event::AssetLockdown {
					asset_id: ASSET_ID,
					until: 120,
				}
				.into(),
			);
		});
}

#[test]
fn lockdown_asset_should_fork_for_unlocked_asset() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Unlocked((2, 0)));

			// Act
			assert_ok!(CircuitBreaker::lockdown_asset(RuntimeOrigin::root(), ASSET_ID, 120));

			// Assert
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(120));
			System::assert_last_event(
				Event::AssetLockdown {
					asset_id: ASSET_ID,
					until: 120,
				}
				.into(),
			);
		});
}

#[test]
fn lockdown_asset_should_fork_for_locked_asset() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(12));

			// Act
			assert_ok!(CircuitBreaker::lockdown_asset(RuntimeOrigin::root(), ASSET_ID, 35));

			// Assert
			let state = AssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, LockdownStatus::Locked(35));
			System::assert_last_event(
				Event::AssetLockdown {
					asset_id: ASSET_ID,
					until: 35,
				}
				.into(),
			);
		});
}

#[test]
fn lockdown_asset_should_not_be_called_by_normal_user() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				CircuitBreaker::lockdown_asset(RuntimeOrigin::signed(ALICE), ASSET_ID, 120),
				sp_runtime::DispatchError::BadOrigin
			);
		});
}
