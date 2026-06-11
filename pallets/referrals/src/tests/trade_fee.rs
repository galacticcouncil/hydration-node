use crate::tests::*;
use pretty_assertions::assert_eq;

#[test]
fn process_trade_fee_should_increased_referrer_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::zero(),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = ReferrerShares::<Test>::get(ALICE);
			assert_eq!(shares, 5_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_increased_trader_shares() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = TraderShares::<Test>::get(BOB);
			assert_eq!(shares, 2_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_increased_total_share_issuance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let shares = TotalShares::<Test>::get();
			assert_eq!(shares, 2_000_000_000 + 5_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_fail_when_taken_amount_is_greater_than_fee_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(70),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
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
		.with_endowed_accounts(vec![(ALICE, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_trader_shares(vec![(BOB, 1_000_000_000_000)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code,));
			// Assert
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(ALICE),
				HDX,
				DAI,
				1_000_000_000_000,
			));
			let shares = ReferrerShares::<Test>::get(ALICE);
			assert_eq!(shares, 0);
		});
}

#[test]
fn process_trade_fee_should_add_asset_to_asset_list() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000,));
			// Assert
			let asset = PendingConversions::<Test>::get(DAI);
			assert_eq!(asset, Some(()));
		});
}

#[test]
fn process_trade_fee_should_not_add_reward_asset_to_asset_list() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, HDX, 2_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(50),
				trader: Permill::from_percent(20),
			},
		)])
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), DAI, HDX, 1_000_000_000_000,));
			// Assert
			let asset = PendingConversions::<Test>::get(HDX);
			assert_eq!(asset, None);
		});
}

#[test]
fn process_trade_fee_should_not_store_zero_trader_reward_in_storage() {
	let mut none_rewards = HashMap::new();
	none_rewards.insert(
		Level::None,
		FeeDistribution {
			referrer: Default::default(),
			trader: Default::default(),
		},
	);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_global_tier_rewards(none_rewards)
		.build()
		.execute_with(|| {
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000));
			// Assert
			assert_eq!(TraderShares::<Test>::try_get(BOB), Err(()));
		});
}

#[test]
fn process_trade_fee_should_take_nothing_to_pot_when_no_code_linked() {
	let mut none_rewards = HashMap::new();
	none_rewards.insert(
		Level::None,
		FeeDistribution {
			referrer: Default::default(),
			trader: Default::default(),
		},
	);

	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_global_tier_rewards(none_rewards)
		.build()
		.execute_with(|| {
			let source_before = Tokens::free_balance(DAI, &BOB);
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000));
			// Assert: unlinked trade mints nothing and consumes nothing — the slice stays with
			// the source, the pot receives no DAI.
			assert_eq!(Tokens::free_balance(DAI, &Referrals::pot_account_id()), 0);
			assert_eq!(Tokens::free_balance(DAI, &BOB), source_before);
		});
}

#[test]
fn process_trade_fee_should_reward_all_parties_based_on_global_config_when_asset_not_set_explicitly() {
	let mut global_rewards = HashMap::new();
	global_rewards.insert(
		Level::None,
		FeeDistribution {
			referrer: Default::default(),
			trader: Default::default(),
		},
	);
	global_rewards.insert(
		Level::Tier0,
		FeeDistribution {
			referrer: Permill::from_percent(5),
			trader: Permill::from_percent(5),
		},
	);
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, HDX, 2_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_global_tier_rewards(global_rewards)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(
				RuntimeOrigin::signed(BOB),
				DAI,
				HDX,
				1_000_000_000_000_000_000
			));
			// Assert
			let referrer_shares = ReferrerShares::<Test>::get(ALICE);
			assert_eq!(referrer_shares, 500_000_000);
			let trader_shares = TraderShares::<Test>::get(BOB);
			assert_eq!(trader_shares, 500_000_000);
			let shares = TotalShares::<Test>::get();
			assert_eq!(shares, 1_000_000_000);
		});
}

#[test]
fn process_trade_fee_should_use_configured_asset_instead_of_global_when_set() {
	let mut global_rewards = HashMap::new();
	global_rewards.insert(
		Level::None,
		FeeDistribution {
			referrer: Default::default(),
			trader: Default::default(),
		},
	);
	global_rewards.insert(
		Level::Tier0,
		FeeDistribution {
			referrer: Permill::from_percent(5),
			trader: Permill::from_percent(5),
		},
	);
	ExtBuilder::default()
		.with_endowed_accounts(vec![(BOB, DAI, 2_000_000_000_000_000_000)])
		.with_conversion_price((HDX, DAI), EmaPrice::new(1_000_000_000_000, 1_000_000_000_000_000_000))
		.with_tiers(vec![(
			DAI,
			Level::Tier0,
			FeeDistribution {
				referrer: Permill::from_percent(10),
				trader: Permill::from_percent(5),
			},
		)])
		.with_global_tier_rewards(global_rewards)
		.build()
		.execute_with(|| {
			// ARRANGE
			let code: ReferralCode<<Test as Config>::CodeLength> = b"BALLS69".to_vec().try_into().unwrap();
			assert_ok!(Referrals::register_code(RuntimeOrigin::signed(ALICE), code.clone(),));
			assert_ok!(Referrals::link_code(RuntimeOrigin::signed(BOB), code));
			// Act
			assert_ok!(MockAmm::trade(RuntimeOrigin::signed(BOB), HDX, DAI, 1_000_000_000_000));
			// Assert
			let referrer_shares = ReferrerShares::<Test>::get(ALICE);
			assert_eq!(referrer_shares, 1_000_000_000);
			let trader_shares = TraderShares::<Test>::get(BOB);
			assert_eq!(trader_shares, 500_000_000);
			let shares = TotalShares::<Test>::get();
			assert_eq!(shares, 1_000_000_000 + 500_000_000);
		});
}
