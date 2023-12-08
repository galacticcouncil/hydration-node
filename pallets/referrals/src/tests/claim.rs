use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn claim_rewards_should_work_when_amount_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
	});
}

#[test]
fn claim_rewards_should_convert_all_assets() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Pallet::<Test>::pot_account_id(), DAI, 3_000_000_000_000_000_000),
			(Pallet::<Test>::pot_account_id(), DOT, 4_000_000_000_000),
		])
		.with_assets(vec![DAI, DOT])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_conversion_price(
			(HDX, DOT),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000),
		)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let acc = Pallet::<Test>::pot_account_id();
			let reserve = Tokens::free_balance(HDX, &acc);
			assert_eq!(reserve, 7_000_000_000_000);
			let reserve = Tokens::free_balance(DOT, &acc);
			assert_eq!(reserve, 0);
			let reserve = Tokens::free_balance(DAI, &acc);
			assert_eq!(reserve, 0);
		});
}

#[test]
fn claim_rewards_should_remove_assets_from_the_list() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Pallet::<Test>::pot_account_id(), DAI, 3_000_000_000_000_000_000),
			(Pallet::<Test>::pot_account_id(), DOT, 4_000_000_000_000),
		])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_conversion_price(
			(HDX, DOT),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000),
		)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let count = Assets::<Test>::iter().count();
			assert_eq!(count, 0);
		});
}

#[test]
fn claim_rewards_should_calculate_correct_portion_when_claimed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let reserve = Tokens::free_balance(HDX, &BOB);
			assert_eq!(reserve, 5_000_000_000_000);
			let reserve = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(reserve, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_decrease_total_shares_issuance_when_claimed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let reserve = TotalShares::<Test>::get();
			assert_eq!(reserve, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_reset_account_shares_to_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let shares = Shares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_emit_event_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			expect_events(vec![Event::Claimed {
				who: BOB,
				rewards: 5_000_000_000_000,
			}
			.into()]);
		});
}

#[test]
fn claim_rewards_update_total_accumulated_for_referrer_account() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (_, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_increase_referrer_level_when_limit_is_reached() {
	let mut volumes = HashMap::new();
	volumes.insert(Level::Novice, Some(10_000_000_000_000));
	volumes.insert(Level::Advanced, Some(20_000_000_000_000));
	volumes.insert(Level::Expert, None);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(level, Level::Advanced);
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_increase_referrer_level_directly_to_top_tier_when_limit_is_reached() {
	let mut volumes = HashMap::new();
	volumes.insert(Level::Novice, Some(10_000_000_000_000));
	volumes.insert(Level::Advanced, Some(13_000_000_000_000));
	volumes.insert(Level::Expert, None);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(level, Level::Expert);
			assert_eq!(total, 15_000_000_000_000);
		});
}
