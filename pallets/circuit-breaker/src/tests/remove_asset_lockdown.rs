use crate::tests::mock::{CircuitBreaker, ExtBuilder, RuntimeOrigin, System, Test, Tokens, ALICE};
use crate::types::AssetLockdownState;
use crate::{Error, Event, LastAssetLockdownState};
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;

pub const ASSET_ID: u32 = 10000;

#[test]
fn remove_asset_lockdown_should_work_when_asset_is_locked() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange
			System::set_block_number(2);
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 101));
			let state = LastAssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Locked(12));

			System::set_block_number(5);
			let total_issuance = Tokens::total_issuance(ASSET_ID);

			// Act
			assert_ok!(CircuitBreaker::remove_asset_lockdown(RuntimeOrigin::root(), ASSET_ID));

			// Assert
			let state = LastAssetLockdownState::<Test>::get(ASSET_ID).unwrap();
			assert_eq!(state, AssetLockdownState::Unlocked((5u64 + 10, total_issuance)));

			System::assert_last_event(Event::AssetLockdownRemoved { asset_id: ASSET_ID }.into());
		});
}

#[test]
fn remove_asset_lockdown_should_fail_when_asset_is_not_in_lockdown() {
	ExtBuilder::default().build().execute_with(|| {
		// Act & Assert
		assert_noop!(
			CircuitBreaker::remove_asset_lockdown(RuntimeOrigin::root(), ASSET_ID),
			Error::<Test>::AssetNotInLockdown
		);
	});
}

#[test]
fn remove_asset_lockdown_should_fail_when_asset_is_unlocked() {
	ExtBuilder::default()
		.with_deposit_period(10)
		.with_asset_limit(ASSET_ID, 100)
		.build()
		.execute_with(|| {
			// Arrange: asset is in unlocked state
			assert_ok!(Tokens::deposit(ASSET_ID, &ALICE, 50));

			// Act & Assert
			assert_noop!(
				CircuitBreaker::remove_asset_lockdown(RuntimeOrigin::root(), ASSET_ID),
				Error::<Test>::AssetNotInLockdown
			);
		});
}

#[test]
fn remove_asset_lockdown_should_fail_for_unauthorized_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Act & Assert
		assert_noop!(
			CircuitBreaker::remove_asset_lockdown(RuntimeOrigin::signed(ALICE), ASSET_ID),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}
