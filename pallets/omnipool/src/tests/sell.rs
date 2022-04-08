use super::*;

#[test]
fn simple_sell_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 100, 2000 * ONE),
			(Omnipool::protocol_account(), 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(Origin::root(), 100, token_amount, token_price,));

			assert_ok!(Omnipool::add_token(Origin::root(), 200, token_amount, token_price,));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(Origin::signed(LP1), 100, 200, sell_amount, min_limit));

			assert_eq!(Tokens::free_balance(100, &LP1), 550000000000000);
			assert_eq!(Tokens::free_balance(200, &LP1), 47808764940238);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1952191235059762
			);

			check_state!(
				13_360 * ONE,
				27_320 * ONE,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			check_asset_state!(
				100,
				AssetState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 3120 * ONE
				}
			);
			check_asset_state!(
				200,
				AssetState {
					reserve: 1952191235059762,
					hub_reserve: 1331836734693877,
					shares: 2000 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 2000 * ONE
				}
			);
		});
}

#[test]
fn sell_hub_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, 100000000000000000),
			(Omnipool::protocol_account(), 2, 2000000000000000),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
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
				FixedU128::from_float(0.65)
			));
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 400000000000000));

			assert_ok!(Omnipool::sell(
				Origin::signed(LP3),
				1,
				200,
				50000000000000,
				10000000000000
			));

			check_balance_approx!(Omnipool::protocol_account(), 0, 100000000000000000u128, 1);
			check_balance_approx!(Omnipool::protocol_account(), 2, 2000000000000000u128, 1);
			check_balance_approx!(Omnipool::protocol_account(), 1, 13410000000000000u128, 1);
			check_balance_approx!(Omnipool::protocol_account(), 100, 2400000000000000u128, 1);
			check_balance_approx!(Omnipool::protocol_account(), 200, 1925925925925925u128, 1);
			check_balance_approx!(LP1, 100, 3000000000000000u128, 1);
			check_balance_approx!(LP1, 200, 3000000000000000u128, 1);
			check_balance_approx!(LP2, 100, 600000000000000u128, 1);
			check_balance_approx!(LP3, 100, 1000000000000000u128, 1);
			check_balance_approx!(LP3, 1, 50000000000000u128, 1);
			check_balance_approx!(LP3, 200, 74074074074074u128, 1);

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
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 10000000000000000
				}
			);

			check_asset_state!(
				100,
				AssetState {
					reserve: 2400000000000000,
					hub_reserve: 1560000000000000,
					shares: 2400000000000000,
					protocol_shares: 2000000000000000,
					tvl: 3120000000000000
				}
			);

			check_asset_state!(
				200,
				AssetState {
					reserve: 1925925925925926,
					hub_reserve: 1350000000000000,
					shares: 2000000000000000,
					protocol_shares: 2000000000000000,
					tvl: 2000000000000000
				}
			);

			check_state!(
				13410000000000000,
				27320000000000000,
				SimpleImbalance {
					value: 98148148148148,
					negative: true
				}
			);
		});
}
