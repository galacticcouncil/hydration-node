use super::*;

use mock::Staking;
use pretty_assertions::assert_eq;
use sp_runtime::FixedU128;

#[test]
fn unstake_should_work_when_staking_position_exists() {
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
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 250_334_912_244_857_841_u128);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(1), None);

			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				209_663_202_319_202_436_u128
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
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 250_000 * ONE);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(1), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_088_930_916_047_128_389_u128),
				209_328_290_074_344_595_u128
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

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, 500_682_646_815_225_830_u128);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(1), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_502_134_933_892_361_376_u128),
				255_367_170_895_881_767_u128
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

			assert_ok!(Staking::claim(RuntimeOrigin::signed(BOB)));

			let bob_balance = Tokens::free_balance(HDX, &BOB);
			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB)));

			//Assert
			assert_unlocked_balance!(&BOB, HDX, bob_balance);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_eq!(Staking::positions(1), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);

			assert_staking_data!(
				110_010 * ONE,
				FixedU128::from_inner(2_624_680_135_471_373_855_u128),
				268_848_368_521_588_930_u128
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

			//Act
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(BOB)));
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(ALICE)));
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(CHARLIE)));
			assert_ok!(Staking::unstake(RuntimeOrigin::signed(DAVE)));

			//Assert
			assert_unlocked_balance!(&ALICE, HDX, 157_951_370_453_331_101_u128);
			assert_unlocked_balance!(&BOB, HDX, 500_682_646_815_225_830_u128);
			assert_unlocked_balance!(&CHARLIE, HDX, 17_804_173_029_722_706_u128);
			assert_unlocked_balance!(&DAVE, HDX, 105_371_923_939_346_989_u128);

			assert_hdx_lock!(ALICE, 0, STAKING_LOCK);
			assert_hdx_lock!(BOB, 0, STAKING_LOCK);
			assert_hdx_lock!(CHARLIE, 0, STAKING_LOCK);
			assert_hdx_lock!(DAVE, 0, STAKING_LOCK);

			assert_eq!(Staking::positions(0), None);
			assert_eq!(Staking::positions(1), None);
			assert_eq!(Staking::positions(2), None);
			assert_eq!(Staking::positions(3), None);

			assert_eq!(Staking::get_user_position_id(&ALICE).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&BOB).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&CHARLIE).unwrap(), None);
			assert_eq!(Staking::get_user_position_id(&DAVE).unwrap(), None);

			assert_staking_data!(
				0,
				FixedU128::from_inner(28_824_441_394_573_800_928_500_u128),
				21_714_122_066_870_846_u128
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

			//Act & assert
			assert_noop!(
				Staking::unstake(RuntimeOrigin::signed(DAVE)),
				Error::<Test>::PositionNotFound
			);
		});
}
