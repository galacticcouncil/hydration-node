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

			check_state!(13_360 * ONE, 27_320 * ONE, SimpleImbalance::default());

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
