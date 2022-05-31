use super::*;
use frame_support::assert_noop;

#[test]
fn simple_buy_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(Origin::signed(LP2), 100, token_amount, token_price,));

			assert_ok!(Omnipool::add_token(Origin::signed(LP3), 200, token_amount, token_price,));

			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			assert_eq!(Tokens::free_balance(100, &LP1), 600 * ONE);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 2000 * ONE);

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, buy_amount, max_limit));

			assert_eq!(Tokens::free_balance(100, &LP1), 547598253275108);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				2452401746724892
			);
			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), 1950 * ONE);

			assert_pool_state!(13_360 * ONE, 26_720 * ONE, SimpleImbalance::default());

			assert_asset_state!(
				100,
				AssetState {
					reserve: 2452401746724892,
					hub_reserve: 1526666666666666,
					shares: 2400 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 3120 * ONE,
					tradable: Tradable::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetState {
					reserve: 1950 * ONE,
					hub_reserve: 1333333333333334,
					shares: 2000 * ONE,
					protocol_shares: 2000 * ONE,
					tvl: 2600 * ONE,
					tradable: Tradable::default(),
				}
			);
		});
}

#[test]
fn hub_asset_buy_fails() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
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
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(Origin::signed(LP2), 100, 500 * ONE, Price::from(1)));

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
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(Origin::signed(LP2), 100, 500 * ONE, Price::from(1)));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, HDX, 100 * ONE, 10 * ONE),
				Error::<Test>::SellLimitExceeded
			);
		});
}

#[test]
fn buy_not_allowed_assets_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from_float(0.65);

			assert_ok!(Omnipool::add_token(Origin::signed(LP2), 100, token_amount, token_price,));

			assert_ok!(Omnipool::add_token(Origin::signed(LP3), 200, token_amount, token_price,));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradable::Frozen
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradable::SellOnly
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradable::BuyOnly
			));

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradable::Frozen
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradable::BuyOnly
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradable::SellOnly
			));

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE));
		});
}

#[test]
fn buy_for_hub_asset_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100_000_000_000_000),
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
				FixedU128::from_float(0.65)
			));
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP2), 100, 400000000000000));

			assert_ok!(Omnipool::buy(
				Origin::signed(LP3),
				200,
				1,
				50_000_000_000_000,
				50_000_000_000_000
			));

			assert_balance_approx!(Omnipool::protocol_account(), 0, 10000000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 1000000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 13393333333333334u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 2400000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1950000000000000u128, 1);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 1);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 1);
			assert_balance_approx!(LP2, 100, 600000000000000u128, 1);
			assert_balance_approx!(LP3, 100, 1000000000000000u128, 1);
			assert_balance_approx!(LP3, 1, 66_666_666_666_667u128, 1);
			assert_balance_approx!(LP3, 200, 50000000000000u128, 1);

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
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 20000000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetState {
					reserve: 2400000000000000,
					hub_reserve: 1560000000000000,
					shares: 2400000000000000,
					protocol_shares: 2000000000000000,
					tvl: 3120000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetState {
					reserve: 1950000000000000,
					hub_reserve: 1333333333333334,
					shares: 2000000000000000,
					protocol_shares: 2000000000000000,
					tvl: 2600000000000000,
					tradable: Tradable::default(),
				}
			);

			assert_pool_state!(
				13393333333333334,
				26720000000000000,
				SimpleImbalance {
					value: 65833333333334,
					negative: true
				}
			);
		});
}

#[test]
fn simple_buy_with_fee_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;
			let token_price = FixedU128::from(1);

			assert_ok!(Omnipool::add_token(Origin::signed(LP2), 100, token_amount, token_price,));

			assert_ok!(Omnipool::add_token(Origin::signed(LP3), 200, token_amount, token_price,));

			assert_eq!(Tokens::free_balance(200, &LP1), 0u128);

			assert_eq!(Tokens::free_balance(200, &Omnipool::protocol_account()), token_amount);

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			let expected_zero_fee: Balance = 52_631_578_947_370;
			let expected_10_percent_fee: Balance = 58_823_529_411_766;

			assert!(expected_zero_fee < expected_10_percent_fee); // note: dont make much sense as values are constants, but good to see the diff for further verification

			let expect_sold_amount = expected_10_percent_fee;

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, buy_amount, max_limit));

			assert_eq!(Tokens::free_balance(100, &LP1), 1000 * ONE - expect_sold_amount);

			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);

			assert_eq!(
				Tokens::free_balance(100, &Omnipool::protocol_account()),
				token_amount + expect_sold_amount
			);
		});
}
