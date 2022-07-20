use super::*;

/// Auto-generated test
#[test]
fn fee_test_buy_sell() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 2000000000000000),
			(LP3, 200, 300000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(20))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.1), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
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

			assert_balance_approx!(Omnipool::protocol_account(), 0, NATIVE_AMOUNT, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 1000000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 14182282238540066u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 4243052260380446u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1671684145777546u128, 10);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 10);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 10);
			assert_balance_approx!(LP2, 100, 550000000000000u128, 10);
			assert_balance_approx!(LP2, 200, 18014179710851u128, 10);
			assert_balance_approx!(LP3, 100, 206947739619554u128, 10);
			assert_balance_approx!(LP3, 200, 610301674511603u128, 10);
			assert_balance_approx!(LP3, 1, 42897803510764u128, 10);

			assert_asset_state!(
				2,
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 1000000000000000,
					tvl: 1000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				0,
				AssetReserveState {
					reserve: 10000000000000000,
					hub_reserve: 10135523267202732,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 20000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 4243052260380451,
					hub_reserve: 882383663986335,
					shares: 2400000000000000,
					protocol_shares: 2000000000000000,
					tvl: 3120000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1671684145777543,
					hub_reserve: 2707273110861765,
					shares: 2006364027707802,
					protocol_shares: 2000000000000000,
					tvl: 5414546221723530,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(
				14182282238540070, // hub liquidity
				29534546221723530,
				SimpleImbalance {
					value: 0,
					negative: true
				}
			);
		});
}
