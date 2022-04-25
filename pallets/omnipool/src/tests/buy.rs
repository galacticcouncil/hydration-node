use super::*;
use frame_support::assert_noop;

#[test]
fn simple_buy_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 100, 2000 * ONE),
			(Omnipool::protocol_account(), 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
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

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			assert_eq!(Tokens::free_balance(100, &LP1), 600 * ONE);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 2000 * ONE);

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, buy_amount, max_limit));

			assert_eq!(Tokens::free_balance(100, &LP1), 547598253275110);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				2452401746724890
			);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1950 * ONE);

			assert_pool_state!(13_360 * ONE, 27_320 * ONE, SimpleImbalance::default());

			assert_asset_state!(
				100,
				AssetState {
					reserve: 2452401746724890,
					hub_reserve: 1526666666666667,
					shares: 2400 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 3120 * ONE
				}
			);
			assert_asset_state!(
				200,
				AssetState {
					reserve: 1950 * ONE,
					hub_reserve: 1333333333333333,
					shares: 2000 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 2000 * ONE
				}
			);
		});
}

#[test]
fn hub_asset_buy_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::buy(Origin::signed(LP1), LRNA, HDX, 100 * ONE, 0),
			Error::<Test>::NotAllowed
		);
	});
}

#[test]
fn buy_insufficient_amount_fails() {
	ExtBuilder::default()
		.with_min_trade_amount(5 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), LRNA, HDX, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 1000, HDX, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);
		});
}

#[test]
fn buy_assets_not_in_pool_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::buy(Origin::signed(LP1), 1000, 2000, 100 * ONE, 0),
			Error::<Test>::AssetNotFound
		);

		assert_noop!(
			Omnipool::buy(Origin::signed(LP1), 2000, 1000, 100 * ONE, 0),
			Error::<Test>::AssetNotFound
		);
	});
}

#[test]
fn buy_with_insufficient_balance_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			assert_ok!(Omnipool::add_token(Origin::root(), 100, 500 * ONE, Price::from(1)));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, HDX, 100 * ONE, 10 * ONE),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn buy_exceeding_limit_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.build()
		.execute_with(|| {
			let dai_amount = 1000 * ONE;
			let price = FixedU128::from_float(0.5);
			init_omnipool(dai_amount, price);

			assert_ok!(Omnipool::add_token(Origin::root(), 100, 500 * ONE, Price::from(1)));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, HDX, 100 * ONE, 10 * ONE),
				Error::<Test>::SellLimitExceeded
			);
		});
}
