use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn claim_should_work_when_staking_position_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 130_334_912_244_857_841_u128);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 120_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_088_930_916_047_128_389_u128),
					accumulated_slash_points: 40,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				230_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				10_336_797_680_797_565_u128
			);
		});
}

#[test]
fn claim_should_claim_nothing_when_claiming_during_unclaimable_periods() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_470_000);

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 130_000 * ONE);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 120_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_088_930_916_047_128_389_u128),
					accumulated_slash_points: 3,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				230_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				10_671_709_925_655_406_u128
			);
		});
}

#[test]
fn claim_should_claim_nothing_when_claiming_during_unclaimable_periods_and_stake_was_increased() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 500_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(BOB, 100_000 * ONE, 1_462_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
			(BOB, 200_000 * ONE, 1_470_987, 1_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_490_000);

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 80_000 * ONE);
			assert_hdx_lock!(BOB, 420_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 420_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_502_134_933_892_361_376_u128),
					accumulated_slash_points: 6,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				530_010 * ONE,
				FixedU128::from_inner(2_502_134_933_892_361_376_u128),
				66_219_483_748_588_281_u128
			);
		});
}

#[test]
fn claim_should_work_when_claiming_after_unclaimable_periods() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 500_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 120_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(BOB, 100_000 * ONE, 1_462_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
			(BOB, 200_000 * ONE, 1_470_987, 1_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_640_000);

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 80_587_506_297_210_440_u128);
			assert_hdx_lock!(BOB, 420_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 420_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_502_134_933_892_361_376_u128),
					accumulated_slash_points: 30,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				530_010 * ONE,
				FixedU128::from_inner(2_502_134_933_892_361_376_u128),
				65_631_977_451_377_841_u128
			);
		});
}

#[test]
fn claim_should_work_when_claiming_multiple_times() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 500_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(BOB, 50_000 * ONE, 1_462_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			set_block_number(1_750_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			set_pending_rewards(100_000 * ONE);
			set_block_number(2_100_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 281_979_585_716_709_787_u128);
			assert_hdx_lock!(BOB, 281_886_766_680_027_536_u128, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 250_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(3_061_853_686_489_680_776_u128),
					accumulated_slash_points: 104,
					accumulated_locked_rewards: 31_886_766_680_027_536_u128,
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				360_010 * ONE,
				FixedU128::from_inner(3_061_853_686_489_680_776_u128),
				39_992_706_885_866_227_u128
			);
		});
}

#[test]
fn claim_should_claim_zero_rewards_when_claiming_in_same_block_without_additional_staking_rewards() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 500_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(BOB, 50_000 * ONE, 1_452_987, 0),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(BOB, 50_000 * ONE, 1_462_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			set_block_number(1_750_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			set_pending_rewards(100_000 * ONE);
			set_block_number(2_100_000);
			assert_ok!(Staking::stake(RuntimeOrigin::signed(BOB), 50_000 * ONE));

			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));
			//Assert
			assert_unlocked_balance!(&BOB, HDX, 281_979_585_716_709_787_u128);
			assert_hdx_lock!(BOB, 281_886_766_680_027_536_u128, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 250_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(3_172_941_453_179_123_366_u128),
					accumulated_slash_points: 104,
					accumulated_locked_rewards: 31_886_766_680_027_536_u128,
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				360_010 * ONE,
				FixedU128::from_inner(3_172_941_453_179_123_366_u128),
				27_771_941_672_360_647_u128
			);
		});
}

#[test]
fn claim_should_not_work_when_staking_position_doesnt_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_stakes(vec![
			(ALICE, 100_000 * ONE, 1_452_987, 200_000 * ONE),
			(CHARLIE, 10_000 * ONE, 1_455_000, 10_000 * ONE),
			(DAVE, 10 * ONE, 1_465_000, 1),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);

			//Act & assert & assert & assert & assert
			assert_noop!(
				Staking::claim(RuntimeOrigin::signed(BOB)),
				Error::<Test>::PositionNotFound
			);
		});
}
