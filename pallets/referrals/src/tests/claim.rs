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
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_conversion_price((HDX, DOT), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000))
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
fn claim_rewards_should_remove_assets_from_the_list_when_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Pallet::<Test>::pot_account_id(), DAI, 3_000_000_000_000_000_000),
			(Pallet::<Test>::pot_account_id(), DOT, 4_000_000_000_000),
		])
		.with_assets(vec![DAI, DOT])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_conversion_price((HDX, DOT), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000))
		.build()
		.execute_with(|| {
			let count = PendingConversions::<Test>::count();
			assert_eq!(count, 2);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let count = PendingConversions::<Test>::count();
			assert_eq!(count, 0);
		});
}

#[test]
fn claim_rewards_should_remove_assets_from_the_list_when_not_successful() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Pallet::<Test>::pot_account_id(), DAI, 3_000_000_000_000_000_000),
			(Pallet::<Test>::pot_account_id(), DOT, 4_000_000_000_000),
		])
		.with_assets(vec![DAI, DOT])
		.with_conversion_price((HDX, DOT), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000))
		.build()
		.execute_with(|| {
			let count = PendingConversions::<Test>::count();
			assert_eq!(count, 2);
			// conversion for DAI fails, but the asset should be removed from PendingConversions
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let count = PendingConversions::<Test>::count();
			assert_eq!(count, 0);
		});
}

#[test]
fn claim_rewards_should_calculate_correct_portion_when_claimed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
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
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let reserve = TotalShares::<Test>::get();
			assert_eq!(reserve, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_reset_referrer_account_shares_to_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let shares = ReferrerShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_reset_trader_account_shares_to_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let shares = TraderShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_reset_both_account_shares_to_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			let shares = TraderShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
			let shares = ReferrerShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_emit_event_when_claimed_by_referrer() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			expect_events(vec![Event::Claimed {
				who: BOB,
				referrer_rewards: 5_000_000_000_000,
				trade_rewards: 0,
			}
			.into()]);
		});
}

#[test]
fn claim_rewards_should_emit_event_when_claimed_by_trader() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			// Assert
			expect_events(vec![Event::Claimed {
				who: BOB,
				referrer_rewards: 0,
				trade_rewards: 5_000_000_000_000,
			}
			.into()]);
		});
}

#[test]
fn claim_rewards_update_total_accumulated_for_referrer_account() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000)])
		.with_referrer_shares(vec![(ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (_, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_claim_both_shares_when_account_have_both() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_trader_shares(vec![(ALICE, 5_000_000_000_000)])
		.with_referrer_shares(vec![(ALICE, 15_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

			let reserve = TotalShares::<Test>::get();
			assert_eq!(reserve, 20_000_000_000_000);
			let alice_balance = Tokens::free_balance(HDX, &ALICE);
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (_, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total, 15_000_000_000_000);

			let reserve = Tokens::free_balance(HDX, &ALICE) - alice_balance;
			assert_eq!(reserve, 20_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_exclude_seed_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_seed_amount(100_000_000_000_000)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
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
	volumes.insert(Level::Tier0, Some(0));
	volumes.insert(Level::Tier1, Some(10_000_000_000_000));
	volumes.insert(Level::Tier2, Some(20_000_000_000_000));

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(level, Level::Tier1);
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_increase_referrer_level_directly_to_top_tier_when_limit_is_reached() {
	let mut volumes = HashMap::new();
	volumes.insert(Level::Tier0, Some(0));
	volumes.insert(Level::Tier1, Some(10_000_000_000_000));
	volumes.insert(Level::Tier2, Some(11_000_000_000_000));
	volumes.insert(Level::Tier3, Some(12_000_000_000_000));
	volumes.insert(Level::Tier4, Some(13_000_000_000_000));

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			// Assert
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(level, Level::Tier4);
			assert_eq!(total, 15_000_000_000_000);
		});
}
