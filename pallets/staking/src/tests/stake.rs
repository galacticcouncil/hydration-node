use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn stake_should_not_work_when_staking_is_not_initialized() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			//Act & assert
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount),
				Error::<Test>::NotInitialized
			);
		});
}

#[test]
fn stake_should_work_when_staking_position_doesnt_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));
			//Assert
			assert_last_event!(Event::<Test>::PositionCreated {
				who: ALICE,
				position_id: 0,
				stake: 100_000 * ONE
			}
			.into());

			//NOTE: first person doesn't distribute rewards because staking is empty.
			assert_staking_data!(100_000 * ONE, FixedU128::from(0), NON_DUSTABLE_BALANCE);
			assert_hdx_lock!(ALICE, 100_000 * ONE, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 50_000 * ONE);

			assert_eq!(
				Staking::positions(Staking::next_position_id() - 1).unwrap(),
				Position::new(100_000 * ONE, FixedU128::from(0), 1_452_987)
			);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 120_000 * ONE));
			//Assert
			assert_last_event!(Event::<Test>::PositionCreated {
				who: BOB,
				position_id: 1,
				stake: 120_000 * ONE
			}
			.into());
			assert_staking_data!(
				220_000 * ONE,
				FixedU128::from(2),
				pending_rewards + NON_DUSTABLE_BALANCE
			);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_unlocked_balance!(BOB, HDX, 130_000 * ONE);

			assert_eq!(
				Staking::positions(Staking::next_position_id() - 1).unwrap(),
				Position::new(120_000 * ONE, FixedU128::from(2), 1_452_987)
			);

			//Arrange
			let pending_rewards = 10_000 * ONE;
			set_pending_rewards(pending_rewards);
			set_block_number(1_455_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(CHARLIE), 10_000 * ONE));
			//Assert
			assert_last_event!(Event::<Test>::PositionCreated {
				who: CHARLIE,
				position_id: 2,
				stake: 10_000 * ONE
			}
			.into());
			assert_staking_data!(
				230_000 * ONE,
				FixedU128::from_inner(2_045_454_545_454_545_454_u128),
				200_000 * ONE + pending_rewards + NON_DUSTABLE_BALANCE
			);
			assert_hdx_lock!(CHARLIE, 10_000 * ONE, STAKING_LOCK);
			assert_unlocked_balance!(CHARLIE, HDX, 0);

			assert_eq!(
				Staking::positions(Staking::next_position_id() - 1).unwrap(),
				Position::new(
					10_000 * ONE,
					FixedU128::from_inner(2_045_454_545_454_545_454_u128),
					1_455_000
				)
			);

			//Arrange
			//rewards too small to distribute
			let pending_rewards = 1;
			set_pending_rewards(pending_rewards);
			set_block_number(1_465_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(DAVE), 10 * ONE));
			assert_last_event!(Event::<Test>::PositionCreated {
				who: DAVE,
				position_id: 3,
				stake: 10 * ONE
			}
			.into());
			//Assert
			assert_staking_data!(
				230_010 * ONE,
				FixedU128::from_inner(2_045_454_545_454_545_458_u128),
				210_000 * ONE + pending_rewards + NON_DUSTABLE_BALANCE
			);
			assert_hdx_lock!(DAVE, 10 * ONE, STAKING_LOCK);
			assert_unlocked_balance!(DAVE, HDX, 99_990 * ONE);

			assert_eq!(
				Staking::positions(Staking::next_position_id() - 1).unwrap(),
				Position::new(
					10 * ONE,
					FixedU128::from_inner(2_045_454_545_454_545_458_u128),
					1_465_000
				)
			);
		});
}

#[test]
fn stake_should_not_work_when_staking_position_exits() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE)])
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			//Act & assert
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount),
				Error::<Test>::PositionAlreadyExists
			);
		});
}

#[test]
fn stake_should_work_when_there_are_no_rewards_to_distribute() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 0;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			//Assert
			assert_last_event!(Event::<Test>::PositionCreated {
				who: ALICE,
				position_id: 0,
				stake: staked_amount
			}
			.into());
			assert_staking_data!(
				staked_amount,
				FixedU128::from(0),
				pending_rewards + NON_DUSTABLE_BALANCE
			);
			assert_hdx_lock!(ALICE, staked_amount, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 50_000 * ONE);

			let next_position_id = Staking::next_position_id();
			assert_eq!(
				Staking::positions(next_position_id - 1).unwrap(),
				Position::new(staked_amount, FixedU128::from(0), 1_452_987)
			);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			//Assert
			assert_last_event!(Event::<Test>::PositionCreated {
				who: BOB,
				position_id: 1,
				stake: staked_amount / 2
			}
			.into());
			assert_staking_data!(
				staked_amount + staked_amount / 2,
				FixedU128::from(0),
				pending_rewards + NON_DUSTABLE_BALANCE
			);
			assert_hdx_lock!(BOB, staked_amount / 2, STAKING_LOCK);
			assert_unlocked_balance!(BOB, HDX, 100_000 * ONE);

			let next_position_id = Staking::next_position_id();
			assert_eq!(
				Staking::positions(next_position_id - 1).unwrap(),
				Position::new(staked_amount / 2, FixedU128::from(0), 1_452_987)
			);
		});
}

#[test]
fn stake_should_not_work_when_stake_amount_is_lt_min_stake() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 0;
			set_pending_rewards(pending_rewards);
			let staked_amount = MinStake::get() - 1_u128;

			//Act
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount),
				Error::<Test>::InsufficientStake
			);
		});
}

#[test]
fn stake_should_not_work_when_tokens_are_vestred() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(VESTED_100K, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			//Act & assert
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(VESTED_100K), staked_amount),
				Error::<Test>::InsufficientBalance
			);
		});
}
