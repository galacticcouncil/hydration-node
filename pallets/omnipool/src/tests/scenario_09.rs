use super::*;

/// Auto-generated test
#[test]
fn complex_scenario_works() {
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
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
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

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP3), 200, 200000000000000));

			assert_ok!(Omnipool::buy(
				Origin::signed(LP3),
				200,
				100,
				300000000000000,
				100000000000000000
			));

			assert_ok!(Omnipool::remove_liquidity(Origin::signed(LP3), 3, 200000000000000));

			assert_ok!(Omnipool::sell(
				Origin::signed(LP3),
				1,
				200,
				20000000000000,
				10000000000000
			));

			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 500000000000000));

			assert_balance_approx!(Omnipool::protocol_account(), 0, NATIVE_AMOUNT, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 1000000000000000u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 14356887226495360u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 4089236949625567u128, 10);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1638588974363038u128, 10);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 10);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 10);
			assert_balance_approx!(LP2, 100, 50000000000000u128, 10);
			assert_balance_approx!(LP2, 200, 24596656872852u128, 10);
			assert_balance_approx!(LP3, 100, 860763050374432u128, 10);
			assert_balance_approx!(LP3, 200, 636814368764109u128, 10);
			assert_balance_approx!(LP3, 1, 20634322079393u128, 10);

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
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 20000000000000000,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 4089236949625565,
					hub_reserve: 1188430684479242,
					shares: 2734332900513906,
					protocol_shares: 2000000000000000,
					tvl: 2376861368958484,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1638588974363041,
					hub_reserve: 2709090864095513,
					shares: 2008863636363636,
					protocol_shares: 2000000000000000,
					tvl: 5378181728191026,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(
				14356887226495363,
				28755043097149510,
				SimpleImbalance {
					value: 40259835553610,
					negative: true
				}
			);
		});
}
