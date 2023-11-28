use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn process_trade_fee_should_increased_referrer_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_tiers(vec![(
			DAI,
			Level::Novice,
			Tier {
				referrer: Permill::from_percent(50),
				trader: Permill::zero(),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = Shares::<Test>::get(&ALICE);
			assert_eq!(shares, 5_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_increased_trader_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_tiers(vec![(
			DAI,
			Level::Novice,
			Tier {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = Shares::<Test>::get(&BOB);
			assert_eq!(shares, 2_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_increased_total_share_issuance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_tiers(vec![(
			DAI,
			Level::Novice,
			Tier {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = TotalShares::<Test>::get();
			assert_eq!(shares, 2_000_000_000 + 5_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_fail_when_taken_amount_is_greated_than_fee_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_tiers(vec![(
			DAI,
			Level::Novice,
			Tier {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(70),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_noop!(
				MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,),
				Error::<Test>::IncorrectRewardCalculation
			);
		});
}

#[test]
fn process_trade_should_not_increase_shares_when_trader_does_not_have_linked_account() {
	ExtBuilder::default()
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_shares(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			// Assert
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(ALICE),
				HDX,
				DAI,
				1_000_000_000_000,
			));
			let shares = Shares::<Test>::get(&ALICE);
			assert_eq!(shares, 0);
		});
}

#[test]
fn process_trade_fee_should_add_asset_to_asset_list() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price(
			(HDX, DAI),
			FixedU128::from_rational(1_000_000_000_000, 1_000_000_000_000_000_000),
		)
		.with_tiers(vec![(
			DAI,
			Level::Novice,
			Tier {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			assert_ok!(Referrals::register_code(
				RuntimeOrigin::signed(ALICE),
				b"BALLS69".to_vec(),
				ALICE
			));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), b"BALLS69".to_vec()));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let asset = Assets::<Test>::get(DAI);
			assert_eq!(asset, Some(()));
		});
}
