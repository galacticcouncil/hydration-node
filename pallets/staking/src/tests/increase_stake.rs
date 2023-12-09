use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn increase_stake_should_not_work_when_staking_is_not_initialized() {
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
			set_pending_rewards(10_000 * ONE);
			let position_id = 0;

			//Act
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(ALICE), position_id, 100_000 * ONE),
				Error::<Test>::NotInitialized
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_staking_position_doesnt_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(5_000 * ONE);
			set_block_number(1_600_000);

			let non_existing_position_id = 14_321_432_u128;
			//Act
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(ALICE), non_existing_position_id, 100_000 * ONE),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_origin_is_not_position_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(5_000 * ONE);
			set_block_number(1_600_000);

			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(ALICE), bob_position_id, 100_000 * ONE),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_tokens_are_vested() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(VESTED_100K, HDX, 150_000 * ONE)])
		.with_stakes(vec![(VESTED_100K, 40_000 * ONE, 1_452_987, 100_000 * ONE)])
		.with_initialized_staking()
		.build()
		.execute_with(|| {
			//Arrange
			let pending_rewards = 200_000 * ONE;
			set_pending_rewards(pending_rewards);
			let staked_amount = 100_000 * ONE;
			let position_id = 0;

			//Act & assert
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(VESTED_100K), position_id, staked_amount),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn increase_stake_should_work_when_user_already_staked() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(5_000 * ONE);
			set_block_number(1_600_000);

			let alice_position_id = 0;
			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				100_000 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 100_000 * ONE,
				total_stake: 200_000 * ONE,
				locked_rewards: 432_086_451_705_829_u128,
				slashed_points: 12,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());
			assert_staking_data!(
				250_000 * ONE,
				FixedU128::from_inner(1_033_333_333_333_333_333_u128),
				104_567_913_548_294_171_u128 + NON_DUSTABLE_BALANCE
			);
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
fn increase_stake_should_slash_no_points_when_increase_is_small() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(5_000 * ONE);
			set_block_number(1_600_000);

			let alice_position_id = 0;
			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				10 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 10 * ONE,
				total_stake: 100_010 * ONE,
				locked_rewards: 432_086_451_705_829_u128,
				slashed_points: 0,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());
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
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.build()
		.execute_with(|| {
			//Arrange
			let alice_position_id = 0;
			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				15_000_000 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 15_000_000 * ONE,
				total_stake: 15_100_000 * ONE,
				locked_rewards: 432_086_451_705_829_u128,
				slashed_points: 24,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());
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
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 100_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
		])
		.build()
		.execute_with(|| {
			//Arrange
			let alice_position_id = 0;
			let pending_rewards = 5_000 * ONE;
			set_pending_rewards(pending_rewards);

			set_block_number(1_600_000);

			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				100_000 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 100_000 * ONE,
				total_stake: 200_000 * ONE,
				locked_rewards: 432_086_451_705_829_u128,
				slashed_points: 12,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				12
			);

			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				100_000 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 100_000 * ONE,
				total_stake: 300_000 * ONE,
				locked_rewards: 26_998_317_793_092_u128,
				slashed_points: 3,
				payable_percentage: FixedU128::from_inner(262_371_143_317_147_u128)
			}
			.into());
			assert_eq!(
				Staking::positions(alice_position_id).unwrap().accumulated_slash_points,
				15
			);

			//Arrange
			set_block_number(1_700_000);
			//Act
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				100_000 * ONE
			));

			//Assert
			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 100_000 * ONE,
				total_stake: 400_000 * ONE,
				locked_rewards: 506_092_568_094_174_u128,
				slashed_points: 4,
				payable_percentage: FixedU128::from_inner(4_919_526_267_840_874_u128)
			}
			.into());
			assert_staking_data!(
				450_000 * ONE,
				FixedU128::from_inner(1_033_333_333_333_333_333_u128),
				104_034_822_662_406_905_u128 + NON_DUSTABLE_BALANCE
			);
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
fn increase_stake_should_not_work_when_increase_is_lt_min_stake() {
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
			let alice_position_id = 0;

			assert_ok!(Staking::stake(RuntimeOrigin::signed(ALICE), staked_amount));
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), staked_amount / 2));

			//Act & assert
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(ALICE), alice_position_id, MinStake::get() - 1),
				Error::<Test>::InsufficientStake
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_tokens_are_are_alredy_staked() {
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
			let alice_position_id = 0;

			//Act & assert
			assert_noop!(
				Staking::increase_stake(RuntimeOrigin::signed(ALICE), alice_position_id, staked_amount),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn increase_stake_should_not_work_when_staking_locked_rewards() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 1_000_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 1_000_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(1_000_000 * ONE);
			set_block_number(1_600_000);

			let alice_position_id = 0;
			let alice_locked_rewards = 11_150_618_108_537_525_u128;
			//1-th increase to receive locked rewards
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(ALICE),
				alice_position_id,
				100_000 * ONE
			));

			assert_last_event!(Event::<Test>::StakeAdded {
				who: ALICE,
				position_id: alice_position_id,
				stake: 100_000 * ONE,
				total_stake: 200_000 * ONE,
				locked_rewards: alice_locked_rewards,
				slashed_points: 12,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());

			assert_eq!(Tokens::free_balance(HDX, &ALICE), 250_000 * ONE + alice_locked_rewards);

			//NOTE: balance structure: 200K locked in staking + ~11K locked in staking rewards
			//total balance is ~260K, Alice is trying to stake 60K from which 11K is locked in
			//rewards.
			//Act
			assert_noop!(
				Staking::increase_stake(
					RuntimeOrigin::signed(ALICE),
					alice_position_id,
					//NOTE: Alice has 50K unlocked + ~11k as locked rewards
					60_000 * ONE
				),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn increase_stake_should_not_return_arithmetic_error_when_vested_and_locked_rewards_are_bigger_than_free_balance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(VESTED_100K, HDX, 250_000 * ONE), (BOB, HDX, 150_000 * ONE)])
		.with_initialized_staking()
		.with_stakes(vec![
			(VESTED_100K, 100_000 * ONE, 1_452_987, 1_000_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 1_000_000 * ONE),
		])
		.start_at_block(1_452_987)
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(1_000_000 * ONE);
			set_block_number(1_600_000);

			let vested_position_id = 0;
			let vested_locked_rewards = 11_150_618_108_537_525_u128;
			//1-th increase to receive locked rewards
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(VESTED_100K),
				vested_position_id,
				25_000 * ONE
			));

			assert_last_event!(Event::<Test>::StakeAdded {
				who: VESTED_100K,
				position_id: vested_position_id,
				stake: 25_000 * ONE,
				total_stake: 125_000 * ONE,
				locked_rewards: vested_locked_rewards,
				slashed_points: 3,
				payable_percentage: FixedU128::from_inner(4_181_481_790_701_572_u128)
			}
			.into());

			Tokens::transfer(RuntimeOrigin::signed(VESTED_100K), ALICE, HDX, 100_000 * ONE).unwrap();
			assert_eq!(
				Tokens::free_balance(HDX, &VESTED_100K),
				150_000 * ONE + vested_locked_rewards
			);

			//NOTE: balance structure: 125K locked in staking + ~11K locked in staking rewards
			//+100K in vesting => sum of locked tokens is bigger than user's balance.
			//Act
			assert_noop!(
				Staking::increase_stake(
					RuntimeOrigin::signed(VESTED_100K),
					vested_position_id,
					//NOTE: Alice has 25K unlocked + ~11k as locked rewards
					25_000 * ONE
				),
				Error::<Test>::InsufficientBalance
			);
		});
}
