use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn complete_referral_flow_should_work_as_expected() {
	let mut volumes = HashMap::new();
	volumes.insert(Level::Tier0, Some(0));
	volumes.insert(Level::Tier1, Some(10_000_000_000));
	volumes.insert(Level::Tier2, Some(20_000_000_000));
	volumes.insert(Level::Tier3, Some(30_000_000_000));
	volumes.insert(Level::Tier4, Some(40_000_000_000));

	let bob_initial_hdx = 10_000_000_000_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, DAI, 2_000_000_000_000_000_000),
			(BOB, HDX, bob_initial_hdx),
			(CHARLIE, DOT, 2_000_000_000_000),
		])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_conversion_price((HDX, DOT), EmaPrice::new(1_000_000_000_000, 500_000_000_000))
		.with_tiers(vec![
			(
				DAI,
				Level::Tier0,
				FeeDistribution {
					referrer: Permill::from_percent(50),
					trader: Permill::from_percent(20),
					external: Permill::from_percent(30),
				},
			),
			(
				DOT,
				Level::Tier0,
				FeeDistribution {
					referrer: Permill::from_percent(50),
					trader: Permill::from_percent(20),
					external: Permill::from_percent(30),
				},
			),
			(
				DAI,
				Level::Tier1,
				FeeDistribution {
					referrer: Permill::from_float(0.03),
					trader: Permill::from_float(0.01),
					external: Permill::from_float(0.002),
				},
			),
			(
				DOT,
				Level::Tier1,
				FeeDistribution {
					referrer: Permill::from_float(0.03),
					trader: Permill::from_float(0.01),
					external: Permill::from_float(0.002),
				},
			),
			(
				HDX,
				Level::Tier0,
				FeeDistribution {
					referrer: Permill::from_percent(20),
					trader: Permill::from_percent(10),
					external: Permill::from_percent(70),
				},
			),
			(
				HDX,
				Level::Tier1,
				FeeDistribution {
					referrer: Permill::from_percent(30),
					trader: Permill::from_percent(10),
					external: Permill::from_percent(60),
				},
			),
		])
		.with_tier_volumes(volumes)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone()));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code.clone()));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(CHARLIE), code,));

			// TRADES — each trade mints shares AND bumps accumulator via on_hdx_deposited
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000));
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(BOB),
				DAI,
				HDX,
				1_000_000_000_000_000_000
			));
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(CHARLIE),
				HDX,
				DOT,
				1_000_000_000_000
			));

			// Assert shares (same as before — share minting logic unchanged)
			let alice_shares = ReferrerShares::<Test>::get(ALICE);
			assert_eq!(alice_shares, 3_000_000_000);
			let bob_shares = TraderShares::<Test>::get(BOB);
			assert_eq!(bob_shares, 1_000_000_000);
			let charlie_shares = TraderShares::<Test>::get(CHARLIE);
			assert_eq!(charlie_shares, 500_000_000);
			let total_shares = TotalShares::<Test>::get();
			assert_eq!(total_shares, alice_shares + bob_shares + charlie_shares);

			// Verify pot has HDX deposited
			let pot_balance = Tokens::free_balance(HDX, &Referrals::pot_account_id());
			assert!(pot_balance > 0, "Pot should have HDX after trades");

			// CLAIMS — shares burned on claim, TotalShares decremented

			// CHARLIE claim
			let charlie_balance_before = Tokens::free_balance(HDX, &CHARLIE);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(CHARLIE)));
			let charlie_received = Tokens::free_balance(HDX, &CHARLIE) - charlie_balance_before;
			// Shares burned
			assert_eq!(TraderShares::<Test>::get(CHARLIE), 0);
			assert_eq!(ReferrerShares::<Test>::get(CHARLIE), 0);
			// TotalShares decremented
			let total_after_charlie = TotalShares::<Test>::get();
			assert_eq!(total_after_charlie, alice_shares + bob_shares);

			// BOB claim
			let bob_balance_before = Tokens::free_balance(HDX, &BOB);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(BOB)));
			let bob_received = Tokens::free_balance(HDX, &BOB) - bob_balance_before;
			assert_eq!(TraderShares::<Test>::get(BOB), 0);
			let total_after_bob = TotalShares::<Test>::get();
			assert_eq!(total_after_bob, alice_shares);

			// ALICE claim
			let alice_balance_before = Tokens::free_balance(HDX, &ALICE);
			assert_ok!(Referrals::claim_rewards(RuntimeOrigin::signed(ALICE)));
			let alice_received = Tokens::free_balance(HDX, &ALICE) - alice_balance_before;
			assert_eq!(ReferrerShares::<Test>::get(ALICE), 0);

			// All shares burned
			assert_eq!(TotalShares::<Test>::get(), 0);

			// Total distributed should not exceed pot
			let total_distributed = charlie_received + bob_received + alice_received;
			assert!(
				total_distributed <= pot_balance,
				"distributed {} > pot {}",
				total_distributed,
				pot_balance
			);
			assert!(total_distributed > 0, "should have distributed something");

			// Referrer level should have increased
			let (level, total) = Referrer::<Test>::get(ALICE).unwrap();
			assert_ne!(level, Level::None);
			assert_eq!(total, alice_received);
		});
}
