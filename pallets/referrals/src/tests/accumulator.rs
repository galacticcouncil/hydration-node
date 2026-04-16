use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn on_hdx_deposited_bumps_reward_per_share_correctly() {
	let total_shares = 1_000 * ONE;
	let amount = 500 * ONE;

	ExtBuilder::default()
		.with_referrer_shares(vec![(ALICE, total_shares)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::on_hdx_deposited(amount));

			let expected_rps = U256::from(amount) * U256::from(ONE_E18) / U256::from(total_shares);
			assert_eq!(RewardPerShare::<Test>::get(), expected_rps);
		});
}

#[test]
fn on_hdx_deposited_does_nothing_when_total_shares_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(Referrals::on_hdx_deposited(500 * ONE));

		assert_eq!(RewardPerShare::<Test>::get(), U256::zero());
	});
}

#[test]
fn on_fee_received_calculates_shares_correctly_for_all_parties() {
	let referrer_pct = Permill::from_percent(50);
	let trader_pct = Permill::from_percent(30);
	let external_pct = Permill::from_percent(20);
	let hdx_amount = 1_000 * ONE;

	ExtBuilder::default()
		.with_tiers(vec![(
			HDX,
			Level::Tier0,
			FeeDistribution {
				referrer: referrer_pct,
				trader: trader_pct,
				external: external_pct,
			},
		)])
		.with_external_account(CHARLIE)
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"ALICE1".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

			assert_ok!(Referrals::on_fee_received(BOB, hdx_amount));

			assert_eq!(ReferrerShares::<Test>::get(ALICE), referrer_pct.mul_floor(hdx_amount));
			assert_eq!(TraderShares::<Test>::get(BOB), trader_pct.mul_floor(hdx_amount));
			assert_eq!(TraderShares::<Test>::get(CHARLIE), external_pct.mul_floor(hdx_amount));
		});
}

#[test]
fn on_fee_received_skips_referrer_share_when_no_code_linked() {
	let trader_pct = Permill::from_percent(30);
	let hdx_amount = 1_000 * ONE;

	ExtBuilder::default()
		.with_tiers(vec![(
			HDX,
			Level::None,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: trader_pct,
				external: Permill::zero(),
			},
		)])
		.build()
		.execute_with(|| {
			// BOB is not linked to any referrer
			assert_ok!(Referrals::on_fee_received(BOB, hdx_amount));

			assert_eq!(ReferrerShares::<Test>::get(ALICE), 0);
			assert_eq!(ReferrerShares::<Test>::get(BOB), 0);
			assert_eq!(TraderShares::<Test>::get(BOB), trader_pct.mul_floor(hdx_amount));
		});
}

#[test]
fn checkpoint_user_accumulates_pending_rewards_before_share_change() {
	let bob_trader_shares = 1_000 * ONE;
	let hdx_deposited = 500 * ONE;

	ExtBuilder::default()
		.with_trader_shares(vec![(BOB, bob_trader_shares)])
		// Level::None rewards: trader gets 30%, no referrer needed
		.with_tiers(vec![(
			HDX,
			Level::None,
			FeeDistribution {
				referrer: Permill::zero(),
				trader: Permill::from_percent(30),
				external: Permill::zero(),
			},
		)])
		.build()
		.execute_with(|| {
			// Bump RPS - BOB now has pending rewards
			assert_ok!(Referrals::on_hdx_deposited(hdx_deposited));

			let rps = RewardPerShare::<Test>::get();
			let expected_accumulated =
				Balance::try_from(U256::from(bob_trader_shares) * rps / U256::from(ONE_E18)).unwrap();

			// on_fee_received internally calls checkpoint_user for BOB
			// (because trader_shares to add > 0) before mutating his share balance.
			assert_ok!(Referrals::on_fee_received(BOB, 1_000 * ONE));

			assert_eq!(UserAccumulatedRewards::<Test>::get(BOB), expected_accumulated);
		});
}

#[test]
fn claim_rewards_cannot_double_claim_via_debt_mechanism() {
	let total_shares = 10_000 * ONE;
	let pot_balance = 10_000 * ONE;
	let bob_shares = 2_000 * ONE;
	let rps = U256::from(pot_balance) * U256::from(ONE_E18) / U256::from(total_shares);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, pot_balance)])
		.with_referrer_shares(vec![(BOB, bob_shares), (ALICE, total_shares - bob_shares)])
		.with_reward_per_share(rps)
		.build()
		.execute_with(|| {
			let bob_before = Tokens::free_balance(HDX, &BOB);

			// First claim: BOB receives their proportional share
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let bob_after_first = Tokens::free_balance(HDX, &BOB);
			assert_eq!(
				bob_after_first - bob_before,
				pot_balance * bob_shares / total_shares,
				"first claim must yield correct proportion"
			);

			// Second claim: shares burned, total_user_shares = 0 -> early return
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let bob_after_second = Tokens::free_balance(HDX, &BOB);
			assert_eq!(bob_after_second, bob_after_first, "second claim must yield nothing");
		});
}

#[test]
fn on_fee_received_external_account_none_shares_dropped() {
	let referrer_pct = Permill::from_percent(50);
	let trader_pct = Permill::from_percent(30);
	let external_pct = Permill::from_percent(20);
	let hdx_amount = 1_000 * ONE;

	ExtBuilder::default()
		.with_tiers(vec![(
			HDX,
			Level::Tier0,
			FeeDistribution {
				referrer: referrer_pct,
				trader: trader_pct,
				external: external_pct,
			},
		)])
		// No external account configured
		.build()
		.execute_with(|| {
			let code: ReferralCode<<Test as Config>::CodeLength> = b"ALICE1".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));

			assert_ok!(Referrals::on_fee_received(BOB, hdx_amount));

			let expected_referrer = referrer_pct.mul_floor(hdx_amount);
			let expected_trader = trader_pct.mul_floor(hdx_amount);

			assert_eq!(ReferrerShares::<Test>::get(ALICE), expected_referrer);
			assert_eq!(TraderShares::<Test>::get(BOB), expected_trader);
			// External percentage is dropped - TotalShares only includes referrer + trader
			assert_eq!(TotalShares::<Test>::get(), expected_referrer + expected_trader);
		});
}

#[test]
fn on_hdx_deposited_accumulates_correctly_over_multiple_calls() {
	let total_shares = 1_000 * ONE;
	let amount1 = 300 * ONE;
	let amount2 = 200 * ONE;

	ExtBuilder::default()
		.with_referrer_shares(vec![(ALICE, total_shares)])
		.build()
		.execute_with(|| {
			assert_ok!(Referrals::on_hdx_deposited(amount1));
			assert_ok!(Referrals::on_hdx_deposited(amount2));

			// Each call increments RPS by amount * ONE_E18 / total_shares;
			// the two increments sum to (amount1 + amount2) * ONE_E18 / total_shares.
			let expected_rps =
				U256::from(amount1 + amount2) * U256::from(ONE_E18) / U256::from(total_shares);
			assert_eq!(RewardPerShare::<Test>::get(), expected_rps);
		});
}

#[test]
fn claim_rewards_combines_accumulated_and_pending_rewards() {
	let bob_shares = 1_000 * ONE;
	// rps chosen so that pending = bob_shares * rps / ONE_E18 = 500 * ONE (with debt = 0)
	let rps = U256::from(ONE_E18) / U256::from(2u128);
	let pending = Balance::try_from(U256::from(bob_shares) * rps / U256::from(ONE_E18)).unwrap();
	let accumulated = 200 * ONE;
	let expected_total = accumulated + pending;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(Pallet::<Test>::pot_account_id(), HDX, expected_total)])
		.with_referrer_shares(vec![(BOB, bob_shares)])
		.with_reward_per_share(rps)
		.with_user_accumulated_rewards(vec![(BOB, accumulated)])
		// UserRewardDebt defaults to zero - full RPS applies as pending
		.build()
		.execute_with(|| {
			let bob_before = Tokens::free_balance(HDX, &BOB);

			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));

			assert_eq!(Tokens::free_balance(HDX, &BOB) - bob_before, expected_total);
			// Both debt and accumulated cleared after the claim
			assert_eq!(UserRewardDebt::<Test>::get(BOB), U256::zero());
			assert_eq!(UserAccumulatedRewards::<Test>::get(BOB), 0);
		});
}
