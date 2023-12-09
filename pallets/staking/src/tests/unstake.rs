use crate::types::{Conviction, Vote};

use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn unstake_should_not_work_when_origin_is_not_position_owner() {
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
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act & assert
			assert_noop!(
				Staking::unstake(RuntimeOrigin::signed(DAVE), bob_position_id),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn unstake_should_not_work_when_staking_is_not_initialized() {
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
			set_block_number(1_700_000);
			let bob_position_id = 0;

			//Act & assert
			assert_noop!(
				Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id),
				Error::<Test>::NotInitialized
			);
		});
}
#[test]
fn unstake_should_work_when_staking_position_exists() {
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
			set_block_number(1_700_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: BOB,
					position_id: bob_position_id,
					paid_rewards: 334_912_244_857_841_u128,
					unlocked_rewards: 0,
					slashed_points: 40,
					slashed_unpaid_rewards: 10_336_797_680_797_565_u128,
					payable_percentage: FixedU128::from_inner(31_383_184_812_088_337_u128)
				}
				.into()
			));

			assert_last_event!(Event::<Test>::Unstaked {
				who: BOB,
				position_id: bob_position_id,
				unlocked_stake: 120_000 * ONE,
			}
			.into());

			assert_unlocked_balance!(&BOB, HDX, 250_334_912_244_857_841_u128);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);

			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				209_328_290_074_344_595_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn unstake_should_claim_zero_rewards_when_unstaking_during_unclaimable_periods() {
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
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: BOB,
					position_id: bob_position_id,
					paid_rewards: 0_u128,
					unlocked_rewards: 0,
					slashed_points: 3,
					slashed_unpaid_rewards: 10_671_709_925_655_406_u128,
					payable_percentage: FixedU128::from(0_u128)
				}
				.into()
			));

			assert_last_event!(Event::<Test>::Unstaked {
				who: BOB,
				position_id: bob_position_id,
				unlocked_stake: 120_000 * ONE,
			}
			.into());
			assert_unlocked_balance!(&BOB, HDX, 250_000 * ONE);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				209_328_290_074_344_595_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn unstake_should_work_when_called_after_unclaimable_periods_and_stake_was_increased() {
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
			(BOB, 100_000 * ONE, 1_472_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_475_000, 1),
			(BOB, 200_000 * ONE, 1_580_987, 1_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_690_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: BOB,
					position_id: bob_position_id,
					paid_rewards: 586_654_644_470_047_u128,
					unlocked_rewards: 95_992_170_755_783_u128,
					slashed_points: 29,
					slashed_unpaid_rewards: 65_536_836_933_362_451_u128,
					payable_percentage: FixedU128::from_inner(8_872_106_273_751_589_u128)
				}
				.into()
			));
			assert_last_event!(Event::<Test>::Unstaked {
				who: BOB,
				position_id: bob_position_id,
				unlocked_stake: 420_000 * ONE,
			}
			.into());
			assert_unlocked_balance!(&BOB, HDX, 500_682_646_815_225_830_u128);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_502_134_933_892_361_376_u128),
				254_780_516_251_411_720_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn unstake_should_claim_no_additional_rewards_when_called_immediately_after_claim() {
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
			(BOB, 100_000 * ONE, 1_472_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_475_000, 1),
			(BOB, 200_000 * ONE, 1_580_987, 1_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_690_000);
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();

			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB), bob_position_id));

			let bob_balance = Tokens::free_balance(HDX, &BOB);
			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: BOB,
					position_id: bob_position_id,
					paid_rewards: 0_u128,
					unlocked_rewards: 0_u128,
					slashed_points: 0,
					slashed_unpaid_rewards: 51_933_872_025_079_204_u128,
					payable_percentage: FixedU128::from_inner(0_u128)
				}
				.into()
			));

			assert_last_event!(Event::<Test>::Unstaked {
				who: BOB,
				position_id: bob_position_id,
				unlocked_stake: 420_000 * ONE,
			}
			.into());
			assert_unlocked_balance!(&BOB, HDX, bob_balance);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_625_787_010_142_549_959_u128),
				268_383_481_159_694_967_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn unstake_should_work_when_called_by_all_stakers() {
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
			(BOB, 100_000 * ONE, 1_472_987, 100_000 * ONE),
			(DAVE, 10 * ONE, 1_475_000, 1),
			(BOB, 200_000 * ONE, 1_580_987, 1_000 * ONE),
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_690_000);
			let alice_position_id = Staking::get_user_position_id(&ALICE).unwrap().unwrap();
			let bob_position_id = Staking::get_user_position_id(&BOB).unwrap().unwrap();
			let charlie_position_id = Staking::get_user_position_id(&CHARLIE).unwrap().unwrap();
			let dave_position_id = Staking::get_user_position_id(&DAVE).unwrap().unwrap();

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));
			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: BOB,
					position_id: bob_position_id,
					paid_rewards: 586_654_644_470_047_u128,
					unlocked_rewards: 95_992_170_755_783_u128,
					slashed_points: 29,
					slashed_unpaid_rewards: 65_536_836_933_362_451_u128,
					payable_percentage: FixedU128::from_inner(8_872_106_273_751_589_u128)
				}
				.into()
			));

			assert_last_event!(Event::<Test>::Unstaked {
				who: BOB,
				position_id: bob_position_id,
				unlocked_stake: 420_000 * ONE,
			}
			.into());

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(ALICE), alice_position_id));
			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: ALICE,
					position_id: alice_position_id,
					paid_rewards: 7_965_081_713_348_758_u128,
					unlocked_rewards: 0_u128,
					slashed_points: 38,
					slashed_unpaid_rewards: 301_821_938_567_408_560_u128,
					payable_percentage: FixedU128::from_inner(25_711_476_569_063_717_u128)
				}
				.into()
			));
			assert_last_event!(Event::<Test>::Unstaked {
				who: ALICE,
				position_id: alice_position_id,
				unlocked_stake: 100_000 * ONE,
			}
			.into());

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(CHARLIE), charlie_position_id));
			//Assert
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: CHARLIE,
					position_id: charlie_position_id,
					paid_rewards: 8_023_126_771_488_456_u128,
					unlocked_rewards: 0_u128,
					slashed_points: 38,
					slashed_unpaid_rewards: 304_021_447_951_301_121_u128,
					payable_percentage: FixedU128::from_inner(25_711_476_569_063_717_u128)
				}
				.into()
			));
			assert_last_event!(Event::<Test>::Unstaked {
				who: CHARLIE,
				position_id: charlie_position_id,
				unlocked_stake: 10_000 * ONE,
			}
			.into());

			assert_ok!(Staking::unstake(RuntimeOrigin::signed(DAVE), dave_position_id));
			assert!(has_event(
				Event::<Test>::RewardsClaimed {
					who: DAVE,
					position_id: dave_position_id,
					paid_rewards: 5_672_178_270_331_647_u128,
					unlocked_rewards: 0_u128,
					slashed_points: 35,
					slashed_unpaid_rewards: 298_656_966_429_605_307_u128,
					payable_percentage: FixedU128::from_inner(18_638_301_224_564_978_u128)
				}
				.into()
			));
			assert_last_event!(Event::<Test>::Unstaked {
				who: DAVE,
				position_id: dave_position_id,
				unlocked_stake: 10 * ONE,
			}
			.into());

			//Assert
			assert_unlocked_balance!(&ALICE, HDX, 157_965_081_713_348_758_u128);
			assert_unlocked_balance!(&BOB, HDX, 500_682_646_815_225_830_u128);
			assert_unlocked_balance!(&CHARLIE, HDX, 18_023_126_771_488_456_u128);
			assert_unlocked_balance!(&DAVE, HDX, 105_672_178_270_331_647_u128);

			assert_hdx_lock!(ALICE, 0, STAKING_LOCK);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_hdx_lock!(CHARLIE, 0, STAKING_LOCK);
			assert_hdx_lock!(DAVE, 0, STAKING_LOCK);

			assert_eq!(Staking::positions(alice_position_id), None);
			assert_eq!(Staking::positions(bob_position_id), None);
			assert_eq!(Staking::positions(charlie_position_id), None);
			assert_eq!(Staking::positions(dave_position_id), None);

			assert_eq!(Staking::get_user_position_id(&ALICE).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&CHARLIE).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&DAVE).unwrap(), None);

			assert_staking_data!(
				0,
				FixedU128::from_inner(30_435_394_707_147_845_603_253_u128),
				//NOTE: rounding error, nothing, except non dustable, should stay reserved when all users are gone.
				3_u128 + NON_DUSTABLE_BALANCE
			);
		});
}

#[test]
fn unstake_should_not_work_when_staking_position_doesnt_exists() {
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
		])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			let non_existing_position_id = 122_341_234_213_u128;

			//Act & assert
			assert_noop!(
				Staking::unstake(RuntimeOrigin::signed(DAVE), non_existing_position_id),
				Error::<Test>::Forbidden
			);
		});
}

#[test]
fn unstake_should_clear_votes_when_staking_position_exists() {
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
		.with_votings(vec![(
			1,
			vec![
				(
					1_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked4x,
					},
				),
				(
					2_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::Locked2x,
					},
				),
				(
					3_u32,
					Vote {
						amount: 10_000 * ONE,
						conviction: Conviction::None,
					},
				),
				(
					4_u32,
					Vote {
						amount: 230_000 * ONE,
						conviction: Conviction::Locked1x,
					},
				),
				(
					8_u32,
					Vote {
						amount: 230_000 * ONE,
						conviction: Conviction::Locked1x,
					},
				),
				(
					6_u32,
					Vote {
						amount: 2 * ONE,
						conviction: Conviction::Locked3x,
					},
				),
			],
		)])
		.build()
		.execute_with(|| {
			//Arrange
			set_pending_rewards(10_000 * ONE);
			set_block_number(1_700_000);
			let bob_position_id = 1;

			assert!(crate::PositionVotes::<Test>::contains_key(bob_position_id));
			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB), bob_position_id));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 250_903_890_918_838_024_u128);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(bob_position_id), None);

			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);
			assert!(!crate::PositionVotes::<Test>::contains_key(bob_position_id));

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				209_328_290_074_344_595_u128 + NON_DUSTABLE_BALANCE
			);
		});
}
