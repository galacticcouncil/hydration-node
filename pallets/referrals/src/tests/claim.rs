use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn claim_rewards_should_work_when_amount_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
	});
}

#[test]
fn claim_rewards_should_return_zero_when_no_hdx_deposited() {
	// Shares exist but RewardPerShare is 0 (no HDX arrived yet)
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 0)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000)])
		.build()
		.execute_with(|| {
			let bob_before = Tokens::free_balance(HDX, &BOB);
			// total_rewards = 0, early return — shares untouched
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let bob_after = Tokens::free_balance(HDX, &BOB);
			assert_eq!(bob_after - bob_before, 0);
			// Shares still present since nothing was claimed
			assert_eq!(ReferrerShares::<Test>::get(BOB), 5_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_calculate_correct_portion_when_claimed() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let reserve = Tokens::free_balance(HDX, &BOB);
			assert_eq!(reserve, 5_000_000_000_000);
			let reserve = Tokens::free_balance(HDX, &Pallet::<Test>::pot_account_id());
			assert_eq!(reserve, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_decrease_total_shares_issuance_when_claimed() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let total = TotalShares::<Test>::get();
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_reset_referrer_account_shares_to_zero() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let shares = ReferrerShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_reset_trader_account_shares_to_zero() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let shares = TraderShares::<Test>::get(BOB);
			assert_eq!(shares, 0);
		});
}

#[test]
fn claim_rewards_should_reset_both_account_shares_to_zero() {
	let total_shares = 40_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let trader_shares = TraderShares::<Test>::get(BOB);
			assert_eq!(trader_shares, 0);
			let referrer_shares = ReferrerShares::<Test>::get(BOB);
			assert_eq!(referrer_shares, 0);
		});
}

#[test]
fn claim_rewards_should_emit_event_when_claimed_by_referrer() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
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
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
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
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_trader_shares(vec![(BOB, 5_000_000_000_000)])
		.with_referrer_shares(vec![(ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let (_, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_claim_both_shares_when_account_have_both() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_trader_shares(vec![(ALICE, 5_000_000_000_000)])
		.with_referrer_shares(vec![(ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

			let total = TotalShares::<Test>::get();
			assert_eq!(total, 20_000_000_000_000);
			let alice_balance = Tokens::free_balance(HDX, &ALICE);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let (_, total_ref) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total_ref, 15_000_000_000_000);

			let received = Tokens::free_balance(HDX, &ALICE) - alice_balance;
			assert_eq!(received, 20_000_000_000_000);

			// Shares burned, TotalShares = 0
			assert_eq!(TotalShares::<Test>::get(), 0);
			assert_eq!(ReferrerShares::<Test>::get(ALICE), 0);
			assert_eq!(TraderShares::<Test>::get(ALICE), 0);
		});
}

#[test]
fn claim_rewards_should_exclude_seed_amount() {
	// In the accumulator model, seed amount only matters during migration.
	// rps = 0 means no rewards have been distributed via the accumulator.
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 20_000_000_000_000)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_seed_amount(100_000_000_000_000)
		.with_reward_per_share(U256::zero())
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// rps=0 → total_rewards=0 → early return, no claim
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let (_, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(total, 0);
		});
}

#[test]
fn claim_rewards_should_increase_referrer_level_when_limit_is_reached() {
	let mut volumes = HashMap::new();
	volumes.insert(Level::Tier0, Some(0));
	volumes.insert(Level::Tier1, Some(10_000_000_000_000));
	volumes.insert(Level::Tier2, Some(20_000_000_000_000));

	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
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

	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_tier_volumes(volumes)
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_eq!(level, Level::Tier4);
			assert_eq!(total, 15_000_000_000_000);
		});
}

#[test]
fn claim_rewards_should_clear_user_debt_and_accumulated_on_claim() {
	let total_shares = 20_000_000_000_000u128;
	let pot_balance = 20_000_000_000_000u128;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, 5_000_000_000_000), (ALICE, 15_000_000_000_000)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			assert_eq!(UserRewardDebt::<Test>::get(BOB), U256::zero());
			assert_eq!(UserAccumulatedRewards::<Test>::get(BOB), 0);
		});
}

#[test]
fn claim_rewards_should_use_accumulated_rewards_from_checkpoint() {
	// User has 1000 accumulated from a previous checkpoint, and no new pending
	let rps = U256::from(50u128) * U256::from(ONE_E18) / U256::from(100u128);
	let debt = U256::from(100u128) * rps / U256::from(ONE_E18);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, 10_000)])
		.with_trader_shares(vec![(BOB, 100)])
		.with_reward_per_share(rps)
		.with_user_reward_debts(vec![(BOB, debt)])
		.with_user_accumulated_rewards(vec![(BOB, 1_000)])
		.build()
		.execute_with(|| {
			// pending = shares * rps / PRECISION - debt = 50 - 50 = 0
			// total = accumulated(1000) + pending(0) = 1000
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let bob_balance = Tokens::free_balance(HDX, &BOB);
			assert_eq!(bob_balance, 1_000);
			// Shares burned
			assert_eq!(TraderShares::<Test>::get(BOB), 0);
		});
}
