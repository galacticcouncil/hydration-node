use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn claim_should_not_work_when_origin_is_not_position_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.with_initialized_staking()
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
			let alice_position_id = Staking::get_user_position_id(&ALICE).unwrap().unwrap();

			//Act & assert & assert & assert & assert
			assert_noop!(
				Staking::claim(RuntimeOrigin::signed(BOB), alice_position_id),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn claim_should_work_when_claiming_multiple_times() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 250_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.start_at_block(1_452_987)
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			set_pending_rewards(100_000 * ONE);
			set_block_number(1_800_000);

			//Act - 2nd claim
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_last_event!(Event::<Test>::RewardsClaimed {
				who: BOB,
				position_id: bob_position_id,
				paid_rewards: 47_706_747_302_610_u128,
				unlocked_rewards: 0,
				slashed_points: 16,
				slashed_unpaid_rewards: 57_516_815_324_327_787_u128,
				payable_percentage: FixedU128::from_inner(828_752_599_443_917_u128),
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 130_382_618_992_160_451_u128);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
				Position {
					stake: 120_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_568_635_266_644_048_370_u128),
					accumulated_slash_points: 56,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				230_010 * ONE,
				FixedU128::from_inner(2_568_635_266_644_048_370_u128),
				262_100_565_683_511_763_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn claim_should_not_work_when_staking_is_not_initialized() {
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
				Staking::claim(RuntimeOrigin::signed(BOB), position_id),
				Error::<Test>::NotInitialized
			);
		});
}

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
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_last_event!(Event::<Test>::RewardsClaimed {
				who: BOB,
				position_id: bob_position_id,
				paid_rewards: 334_912_244_857_841_u128,
				unlocked_rewards: 0,
				slashed_points: 40,
				slashed_unpaid_rewards: 10_336_797_680_797_565_u128,
				payable_percentage: FixedU128::from_inner(31_383_184_812_088_337_u128),
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 130_334_912_244_857_841_u128);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
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
				209_328_290_074_344_595_u128 + NON_DUSTABLE_BALANCE
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
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 130_000 * ONE);
			assert_hdx_lock!(BOB, 120_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
				Position {
					stake: 120_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_088_930_916_047_128_389_u128),
					accumulated_slash_points: 3,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: 10_671_709_925_655_406_u128,
				}
			);

			assert_staking_data!(
				230_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				220_000_000_000_000_001_u128 + NON_DUSTABLE_BALANCE
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
		.with_initialized_staking()
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
			set_block_number(1_452_987_u64 + mock::UnclaimablePeriods::get() * mock::PeriodLength::get());
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 80_000 * ONE);
			assert_hdx_lock!(BOB, 420_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
				Position {
					stake: 420_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(2_502_134_933_892_361_376_u128),
					accumulated_slash_points: 16,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: 66_219_483_748_588_281_u128,
				}
			);

			assert_staking_data!(
				530_010 * ONE,
				FixedU128::from_inner(2_502_134_933_892_361_376_u128),
				321_000_000_000_000_001_u128 + NON_DUSTABLE_BALANCE
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
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_last_event!(Event::<Test>::RewardsClaimed {
				who: BOB,
				position_id: bob_position_id,
				paid_rewards: 587_506_297_210_440_u128,
				unlocked_rewards: 0,
				slashed_points: 29,
				slashed_unpaid_rewards: 65_631_977_451_377_841_u128,
				payable_percentage: FixedU128::from_inner(8_872_106_273_751_589_u128),
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 80_587_506_297_210_440_u128);
			assert_hdx_lock!(BOB, 420_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
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
				254_780_516_251_411_720_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn claim_should_work_when_staked_was_increased() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 150_000 * ONE),
			(BOB, HDX, 500_000 * ONE),
			(CHARLIE, HDX, 10_000 * ONE),
			(DAVE, HDX, 100_000 * ONE),
		])
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			set_block_number(1_750_000);
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			set_pending_rewards(100_000 * ONE);
			set_block_number(2_100_000);
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_last_event!(Event::<Test>::RewardsClaimed {
				who: BOB,
				position_id: bob_position_id,
				paid_rewards: 17_792_982_258_382_321_u128,
				unlocked_rewards: 46_073_370_138_355_002_u128,
				slashed_points: 77,
				slashed_unpaid_rewards: 39_992_706_885_866_227_u128,
				payable_percentage: FixedU128::from_inner(307_913_300_366_917_409_u128),
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 313_866_352_396_737_323_u128);
			assert_hdx_lock!(BOB, 250_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(bob_position_id).unwrap(),
				Position {
					stake: 250_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(3_061_853_686_489_680_776_u128),
					accumulated_slash_points: 104,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				360_010 * ONE,
				FixedU128::from_inner(3_061_853_686_489_680_776_u128),
				316_140_940_717_396_451_u128 + NON_DUSTABLE_BALANCE
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
		.with_initialized_staking()
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
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			set_block_number(1_750_000);
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			set_pending_rewards(100_000 * ONE);
			set_block_number(2_100_000);
			assert_ok!(Staking::increase_stake(
				RuntimeOrigin::signed(BOB),
				bob_position_id,
				50_000 * ONE
			));

			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Act
			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_last_event!(Event::<Test>::RewardsClaimed {
				who: BOB,
				position_id: bob_position_id,
				paid_rewards: 0,
				unlocked_rewards: 0,
				slashed_points: 0,
				slashed_unpaid_rewards: 27_771_941_672_360_647_u128,
				payable_percentage: FixedU128::from_inner(0_u128),
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 313_866_352_396_737_323_u128);
			assert_hdx_lock!(BOB, 250_000 * ONE, STAKING_LOCK);
			assert_eq!(
				Staking::positions(1).unwrap(),
				Position {
					stake: 250_000 * ONE,
					action_points: Zero::zero(),
					created_at: 1_452_987,
					reward_per_stake: FixedU128::from_inner(3_172_941_453_179_123_366_u128),
					accumulated_slash_points: 104,
					accumulated_locked_rewards: Zero::zero(),
					accumulated_unpaid_rewards: Zero::zero(),
				}
			);

			assert_staking_data!(
				360_010 * ONE,
				FixedU128::from_inner(3_172_941_453_179_123_366_u128),
				328_361_705_930_902_031_u128 + NON_DUSTABLE_BALANCE
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
		.with_initialized_staking()
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
			let non_existing_id = 131_234_123_421;

			//Act & assert & assert & assert & assert
			assert_noop!(
				Staking::claim(RuntimeOrigin::signed(BOB), non_existing_id),
				Error::<Test>::Forbidden
			);
		});
}
