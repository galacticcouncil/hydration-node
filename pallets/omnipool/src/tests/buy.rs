use super::*;
use frame_support::assert_noop;
use pretty_assertions::assert_eq;
use test_case::test_case;

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
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Arrange
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			// Act
			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, buy_amount, max_limit));

			// Assert
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
				AssetReserveState {
					reserve: 2452401746724892,
					hub_reserve: 1526666666666666,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950 * ONE,
					hub_reserve: 1333333333333334,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
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
		.with_token(100, FixedU128::from(1), LP2, 500 * ONE)
		.build()
		.execute_with(|| {
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
		.with_token(100, FixedU128::from(1), LP2, 500 * ONE)
		.build()
		.execute_with(|| {
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
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradability::SELL
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradability::BUY
			));

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE));

			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradability::BUY
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 50 * ONE, 100 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				100,
				Tradability::SELL
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
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
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
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 1000000000000000,
					cap: DEFAULT_WEIGHT_CAP,
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
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2400000000000000,
					hub_reserve: 1560000000000000,
					shares: 2400000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950000000000000,
					hub_reserve: 1333333333333334,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(
				13393333333333334,
				26786666666666668,
				SimpleImbalance {
					value: 661295833333341,
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
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let token_amount = 2000 * ONE;

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

#[test]
fn buy_should_fail_when_buying_more_than_in_pool() {
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
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Act
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 200, 100, 3000 * ONE, 100 * ONE),
				Error::<Test>::InsufficientLiquidity
			);
		});
}

#[test]
fn buy_for_hub_asset_should_fail_when_asset_out_is_not_allowed_to_sell() {
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
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				Origin::root(),
				200,
				Tradability::SELL | Tradability::ADD_LIQUIDITY
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(LP3), 200, 1, 50_000_000_000_000, 50_000_000_000_000),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_for_hub_asset_should_fail_when_limit_exceeds() {
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
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP3), 200, 1, 20_000_000_000_000, 30_000_000_000_000),
				Error::<Test>::SellLimitExceeded
			);
		});
}

#[test]
fn buy_should_fail_when_trading_same_asset() {
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
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP3), 200, 200, 50_000_000_000_000, 100_000_000_000),
				Error::<Test>::SameAssetTradeNotAllowed
			);
		});
}

#[test]
fn buy_should_work_when_trading_native_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
			(LP1, HDX, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(20))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(Origin::signed(LP1), 100, liq_added));

			let buy_amount = 50 * ONE;
			let max_limit = 100 * ONE;

			assert_ok!(Omnipool::buy(Origin::signed(LP1), 200, HDX, buy_amount, max_limit));

			assert_eq!(Tokens::free_balance(HDX, &LP1), 953354861858628);
			assert_eq!(Tokens::free_balance(200, &LP1), buy_amount);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				10046645138141372
			);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1950000000000000
			);

			let hub_reserves: Vec<Balance> = Assets::<Test>::iter().map(|v| v.1.hub_reserve).collect();

			assert_pool_state!(
				hub_reserves.iter().sum::<Balance>(),
				26_720 * ONE,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1950000000000000,
					hub_reserve: 1337142857142858,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10046645138141372,
					hub_reserve: 9962857142857142,
					shares: 10000 * ONE,
					protocol_shares: 10000000000000000,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn buy_should_fail_when_exceeds_max_out_ratio() {
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
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP3, 2000 * ONE)
		.with_max_out_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, 200, 1000 * ONE, 0u128),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}

#[test]
fn buy_should_fail_when_exceeds_max_in_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, 200, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.00), LP3, 500 * ONE)
		.with_max_in_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, 200, 200 * ONE, Balance::MAX),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn buy_for_lrna_should_fail_when_exceeds_max_in_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_max_in_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, LRNA, 1000 * ONE, Balance::MAX),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn buy_for_lrna_should_fail_when_exceeds_max_out_ratio() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, LRNA, 1500 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_max_out_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::buy(Origin::signed(LP1), 100, LRNA, 1500 * ONE, Balance::MAX),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}

#[test_case(0)]
#[test_case(ONE)]
#[test_case(100 * ONE)]
fn buy_should_work_when_trade_volume_limit_not_exceeded(diff_from_min_limit: Balance) {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 10000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, DOT, 2000000 * ONE),
			(TRADER, AUSD, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.8), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = TEN_PERCENT.mul_floor(initial_dot_amount) - diff_from_min_limit;

			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				AUSD,
				buy_amount,
				Balance::MAX
			));
		});
}

#[test]
fn buy_should_fail_when_trade_volume_max_limit_exceeded() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 10000 * ONE;
	let initial_ausd_amount = 10000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, DOT, 2000000 * ONE),
			(TRADER, AUSD, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, initial_ausd_amount)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = TEN_PERCENT.mul_floor(initial_dot_amount);

			//Act and assert
			//Asset_in amount would be 1250_000_000_000_002 in a successful trade, but it fails due to limit
			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, AUSD, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MaxTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_should_fail_when_consequent_trades_exceed_trade_volume_max_limit() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 10000 * ONE;
	let initial_ausd_amount = 10000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, DOT, 2000000 * ONE),
			(TRADER, AUSD, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, initial_ausd_amount)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = FIVE_PERCENT.mul_floor(initial_dot_amount);

			//Act and assert
			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				AUSD,
				buy_amount,
				Balance::MAX
			));

			let buy_amount_to_exceed_limit = buy_amount + ONE;

			assert_noop!(
				Omnipool::buy(
					Origin::signed(TRADER),
					DOT,
					AUSD,
					buy_amount_to_exceed_limit,
					Balance::MAX
				),
				pallet_circuit_breaker::Error::<Test>::MaxTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_should_fail_when_trade_volume_min_limit_exceeded() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 10000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, DOT, 2000000 * ONE),
			(TRADER, AUSD, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.8), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = TEN_PERCENT.mul_floor(initial_dot_amount) + ONE;

			//Act and assert
			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, AUSD, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MinTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_should_fail_when_consequent_trades_exceed_trade_volume_min_limit() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 10000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, DOT, 2000000 * ONE),
			(TRADER, AUSD, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.8), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = FIVE_PERCENT.mul_floor(initial_dot_amount);

			//Act and assert
			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				AUSD,
				buy_amount,
				Balance::MAX
			));

			let buy_amount_to_exceed_limit = buy_amount + ONE;

			assert_noop!(
				Omnipool::buy(
					Origin::signed(TRADER),
					DOT,
					AUSD,
					buy_amount_to_exceed_limit,
					Balance::MAX
				),
				pallet_circuit_breaker::Error::<Test>::MinTradeVolumePerBlockReached
			);
		});
}

#[test_case(ONE)]
#[test_case(10 * ONE)]
#[test_case(100 * ONE)]
fn buy_asset_for_hub_asset_should_work_when_trade_volume_limit_not_exceeded(buy_amount: Balance) {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 100000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, LRNA, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let lrna_balance_in_omnipool = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			assert_eq!(lrna_balance_in_omnipool, 82000 * ONE);

			//Act and assert
			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				LRNA,
				buy_amount,
				Balance::MAX
			),);
		});
}

#[test]
fn buy_asset_for_hub_asset_should_fail_when_trade_volume_max_limit_exceeded() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 100000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, LRNA, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = 13500 * ONE;
			let lrna_balance_in_omnipool = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
			assert_eq!(lrna_balance_in_omnipool, 82000 * ONE);

			//Act and assert
			//Amount_in would be 10_144_508_670_520_232 during successful trade, but it fails as it is more than 10% of `lrna_balance_in_omnipool`
			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, LRNA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MaxTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_asset_for_hub_asset_should_fail_when_consequent_trades_exceed_trade_volume_max_limit() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 100000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, LRNA, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(1.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let lrna_balance_in_omnipool = Tokens::free_balance(DOT, &Omnipool::protocol_account());
			let buy_amount = FIVE_PERCENT.mul_floor(lrna_balance_in_omnipool);

			//Act and assert
			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				LRNA,
				buy_amount,
				Balance::MAX
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, LRNA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MaxTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_asset_for_hub_asset_should_fail_when_trade_volume_min_limit_exceeded() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 100000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, LRNA, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = TEN_PERCENT.mul_floor(initial_dot_amount) + ONE;

			//Act and assert
			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, LRNA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MinTradeVolumePerBlockReached
			);
		});
}

#[test]
fn buy_asset_for_hub_asset_should_fail_when_consequent_trades_exceed_trade_volume_min_limit() {
	const DOT: AssetId = 100;
	const AUSD: AssetId = 200;
	const TRADER: u64 = 11u64;

	let initial_dot_amount = 100000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, DOT, 2000000 * ONE),
			(LP1, AUSD, 2000000 * ONE),
			(TRADER, LRNA, 2000000 * ONE),
		])
		.with_registered_asset(DOT)
		.with_registered_asset(AUSD)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(DOT, FixedU128::from_float(0.65), LP1, initial_dot_amount)
		.with_token(AUSD, FixedU128::from_float(0.65), LP1, 10000 * ONE)
		.with_max_trade_volume_limit_per_block(TEN_PERCENT)
		.build()
		.execute_with(|| {
			let buy_amount = FIVE_PERCENT.mul_floor(initial_dot_amount) + ONE;

			//Act and assert
			assert_ok!(Omnipool::buy(
				Origin::signed(TRADER),
				DOT,
				LRNA,
				buy_amount,
				Balance::MAX
			));

			assert_noop!(
				Omnipool::buy(Origin::signed(TRADER), DOT, LRNA, buy_amount, Balance::MAX),
				pallet_circuit_breaker::Error::<Test>::MinTradeVolumePerBlockReached
			);
		});
}
