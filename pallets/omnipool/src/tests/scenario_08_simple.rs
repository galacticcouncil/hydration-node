use super::*;

/// Auto-generated test
#[test]
fn sell_fee_test() {
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
		.with_initial_pool(
			1000 * ONE,
			NATIVE_AMOUNT,
			FixedU128::from_float(0.5),
			FixedU128::from(1),
		)
		.build()
		.execute_with(|| {
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

			assert_balance_approx!(Omnipool::protocol_account(), 0, 100000000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 2000000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 14260000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 2560000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1938322315391001u128, 10);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 10);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 10);
			assert_balance_approx!(LP2, 100, 550000000000000u128, 10);
			assert_balance_approx!(LP2, 200, 18014179710851u128, 10);
			assert_balance_approx!(LP3, 100, 1890000000000000u128, 10);
			assert_balance_approx!(LP3, 200, 343663504898149u128, 10);

			assert_asset_state!(
				2,
				AssetState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 1000000000000000,
					tvl: 1000000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_asset_state!(
				0,
				AssetState {
					reserve: 10000000000000000,
					hub_reserve: 10019499999999999,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 10000000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetState {
					reserve: 2560000000000000,
					hub_reserve: 1462500000000001,
					shares: 2400000000000000,
					protocol_shares: 2000000000000000,
					tvl: 3120000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetState {
					reserve: 1938322315391001,
					hub_reserve: 2277999999999998,
					shares: 2000000000000000,
					protocol_shares: 2000000000000000,
					tvl: 2000000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_pool_state!(
				14259999999999998,
				29120000000000000,
				SimpleImbalance {
					value: 0,
					negative: true
				}
			);
		});
}
