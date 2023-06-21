use super::*;

use mock::Staking;
use orml_traits::MultiCurrencyExtended;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn new_stake_should_work_when_staking_is_empty() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			//Assert
			assert_staking_data!(staked_amount, FixedU128::from(0), pending_rewards);
			assert_hdx_lock!(ALICE, staked_amount, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 50_000 * ONE);

			let next_position_id = Staking::next_position_id();
			assert_eq!(
				Staking::positions(next_position_id - 1).unwrap(),
				Position::new(staked_amount, FixedU128::from(0), 1_452_987)
			);
		});
}

#[test]
fn new_stake_should_work_when_staking_is_not_empty() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));
			//Assert
			assert_staking_data!(100_000 * ONE, FixedU128::from(0), pending_rewards);
			assert_hdx_lock!(ALICE, 100_000 * ONE, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 50_000 * ONE);

			assert_eq!(
				Staking::positions(Staking::next_position_id() - 1).unwrap(),
				Position::new(100_000 * ONE, FixedU128::from(0), 1_452_987)
			);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 120_000 * ONE));
			//Assert
			assert_staking_data!(220_000 * ONE, FixedU128::from(2), 0);
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
			assert_staking_data!(230_000 * ONE, FixedU128::from_inner(2_045_454_545_454_545_454_u128), 0);
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
			//Assert
			assert_staking_data!(230_010 * ONE, FixedU128::from_inner(2_045_454_545_454_545_458_u128), 0);
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
fn new_stake_should_work_when_there_are_no_rewards_to_distribute() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE), (BOB, HDX, 150_000 * ONE)])
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
			assert_staking_data!(staked_amount, FixedU128::from(0), pending_rewards);
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
			assert_staking_data!(staked_amount + staked_amount / 2, FixedU128::from(0), pending_rewards);
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
fn increase_stake_should_work_when_user_already_staked() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 100_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			let alice_position_id = Staking::next_position_id();
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			//Assert
			assert_staking_data!(250_000 * ONE, FixedU128::from_inner(1_033_333_333_333_333_333_u128), 0);
			assert_hdx_lock!(ALICE, 200_432_086_451_705_829_u128, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 50_000 * ONE);

			assert_eq!(
				Staking::positions(alice_position_id).unwrap(),
				Position {
					stake: 200_000 * ONE,
					reward_per_stake: FixedU128::from_inner(1_033_333_333_333_333_333_u128),
					created_at: 1_452_987,
					accumulated_unpaid_rewards: 102_901_246_881_627_504,
					action_points: 0,
					accumulated_slash_points: 12,
					accumulated_locked_rewards: 432_086_451_705_829_u128,
				}
			);
		});
}

#[test]
fn increase_stake_should_work_when_user_staked_multiple_times() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 450_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 100_000 * ONE;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			let alice_position_id = Staking::next_position_id();
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);
			set_block_number(1_600_000);

			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			let pending_rewards = 10_000 * ONE;
			set_pending_rewards(pending_rewards);
			set_block_number(1_650_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			//Assert
			assert_staking_data!(350_000 * ONE, FixedU128::from_inner(1_073_333_333_333_333_333_u128), 0);
			assert_hdx_lock!(ALICE, 300_656_207_631_252_541_u128, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 150_000 * ONE);

			assert_eq!(
				Staking::positions(alice_position_id).unwrap(),
				Position {
					stake: 300_000 * ONE,
					reward_per_stake: FixedU128::from_inner(1_073_333_333_333_333_333_u128),
					created_at: 1_452_987,
					accumulated_unpaid_rewards: 110_677_125_702_080_792_u128,
					action_points: 0,
					accumulated_slash_points: 17,
					accumulated_locked_rewards: 656_207_631_252_541_u128,
				}
			);
		});
}

#[test]
fn increase_stake_should_slash_no_points_when_increase_is_small() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 100_000 * ONE;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			let alice_position_id = Staking::next_position_id();
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 10 * ONE));

			//Assert
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				0
			);
		});
}

#[test]
fn increase_stake_should_slash_all_points_when_increase_is_big() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 20_050_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 100_000 * ONE;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			let alice_position_id = Staking::next_position_id();
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));

			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 15_000_000 * ONE));

			//Assert
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				24
			);
		});
}

#[test]
fn increase_stake_should_accumulate_slash_points_when_called_multiple_times() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 500_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 100_000 * ONE;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let alice_position_id = Staking::next_position_id();

			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			//Assert
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				12
			);

			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			//Assert
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				15
			);

			//Arrange
			set_block_number(1_700_000);
			//Act
			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), 100_000 * ONE));

			//Assert
			assert_staking_data!(450_000 * ONE, FixedU128::from_inner(1_033_333_333_333_333_333_u128), 0);
			assert_hdx_lock!(ALICE, 400_965_177_337_593_095_u128, STAKING_LOCK);
			assert_unlocked_balance!(ALICE, HDX, 100_000 * ONE);

			assert_eq!(
				Staking::positions(alice_position_id).unwrap(),
				Position {
					stake: 400_000 * ONE,
					reward_per_stake: FixedU128::from_inner(1_033_333_333_333_333_333_u128),
					created_at: 1_452_987,
					accumulated_unpaid_rewards: 102_368_155_995_740_238_u128,
					action_points: 0,
					accumulated_slash_points: 19,
					accumulated_locked_rewards: 965_177_337_593_095_u128,
				}
			);
		});
}

#[test]
fn stake_should_not_work_when_stake_amount_is_lt_min_stake() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 0;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let staked_amount = MinStake::get() - 1_u128;

			//Act
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount),
				Error::<Test>::InsufficientStake
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_increase_is_lt_min_stake() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 150_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			let pot = Staking::pot_account_id();
			let pending_rewards = 0;
			Tokens::set_balance(RuntimeOrigin::root(), pot, HDX, pending_rewards, 0).unwrap();
			Staking::add_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;

			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			//Act & assert
			assert_noop!(
				Staking::stake(RuntimeOrigin::signed(ALICE), MinStake::get() - 1),
				Error::<Test>::InsufficientStake
			);
		});
}
