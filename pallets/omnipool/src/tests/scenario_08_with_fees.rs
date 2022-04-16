use super::*;

/// Auto-generated test
#[test]
fn fee_test_buy_sell() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, 100000000000000000),
			(Omnipool::protocol_account(), 2, 2000000000000000),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 2000000000000000),
			(LP3, 200, 300000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee((1, 10))
		.with_protocol_fee((2, 10))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::root(),
				2,
				1000000000000000,
				FixedU128::from_float(0.5)
			));

			assert_ok!(Omnipool::add_token(
				Origin::root(),
				0,
				10000000000000000,
				FixedU128::from(1)
			));

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				100,
				2000000000000000,
				FixedU128::from_float(0.65)
			));

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				200,
				2000000000000000,
				FixedU128::from_float(1.1)
			));
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 400000000000000));

			assert_ok!(Omnipool::sell(
				Origin::signed(LP3),
				100,
				200,
				110000000000000,
				10000000000000
			));

			assert_ok!(Omnipool::sell(
				Origin::signed(LP2),
				100,
				200,
				50000000000000,
				10000000000000
			));

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP3), 200, 200000000000000));

			assert_ok!(Omnipool::buy(
				Origin::signed(LP3),
				200,
				100,
				300000000000000,
				100000000000000000
			));

			assert_ok!(Omnipool::remove_liquidity(Origin::signed(LP3), 3, 200000000000000));

			check_balance_approx!(Omnipool::protocol_account(), 0, 100000000000000000u128, 10);
			check_balance_approx!(Omnipool::protocol_account(), 2, 2000000000000000u128, 10);
			check_balance_approx!(Omnipool::protocol_account(), 1, 14182282238540066u128, 10);
			check_balance_approx!(Omnipool::protocol_account(), 100, 4243052260380435u128, 10);
			check_balance_approx!(Omnipool::protocol_account(), 200, 1671684145777546u128, 10);
			check_balance_approx!(LP1, 100, 3000000000000000u128, 10);
			check_balance_approx!(LP1, 200, 3000000000000000u128, 10);
			check_balance_approx!(LP2, 100, 550000000000000u128, 10);
			check_balance_approx!(LP2, 200, 18014179710851u128, 10);
			check_balance_approx!(LP3, 100, 206947739619565u128, 10);
			check_balance_approx!(LP3, 200, 610301674511603u128, 10);
			check_balance_approx!(LP3, 1, 42897803510764u128, 10);

			check_asset_state!(
				2,
				AssetState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 1000000000000000,
					tvl: 1000000000000000
				}
			);

			check_asset_state!(
				0,
				AssetState {
					reserve: 10000000000000000,
					hub_reserve: 10135523267202731,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 10000000000000000
				}
			);

			check_asset_state!(
				100,
				AssetState {
					reserve: 4243052260380435,
					hub_reserve: 882383663986338,
					shares: 2400000000000000,
					protocol_shares: 2000000000000000,
					tvl: 3120000000000000
				}
			);

			check_asset_state!(
				200,
				AssetState {
					reserve: 1671684145777546,
					hub_reserve: 2707273110861761,
					shares: 2006364027707802,
					protocol_shares: 2000000000000000,
					tvl: 5414546221723522
				}
			);

			check_state!(
				14182282238540066, // hub liquidity
				32534546221723522, // tvl
				SimpleImbalance {
					value: 0,
					negative: true
				}
			);
		});
}
