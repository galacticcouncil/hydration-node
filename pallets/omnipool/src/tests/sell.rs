use super::*;
use frame_support::assert_noop;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

#[test]
fn simple_sell_works() {
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
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			assert_eq!(Tokens::free_balance(100, &LP1), 550000000000000);
			assert_eq!(Tokens::free_balance(200, &LP1), 47808764940238);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1952191235059762
			);

			assert_pool_state!(
				13_360 * ONE,
				26_720 * ONE,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2450 * ONE,
					hub_reserve: 1_528_163_265_306_123,
					shares: 2400 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1952191235059762,
					hub_reserve: 1331836734693877,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn sell_with_insufficient_balance_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 10000 * ONE, 0),
			Error::<Test>::InsufficientBalance
		);
	});
}
#[test]
fn sell_insufficient_amount_fails() {
	ExtBuilder::default()
		.with_min_trade_amount(5 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 200, ONE, 0),
				Error::<Test>::InsufficientTradingAmount
			);
		});
}

#[test]
fn hub_asset_buy_not_allowed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, HDX, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, LRNA, 100 * ONE, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn selling_assets_not_in_pool_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, HDX, 1000 * ONE),
			(LP1, 1000, 1000 * ONE),
			(LP1, 2000, 1000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_registered_asset(100)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 1000, HDX, 50 * ONE, 10 * ONE),
				Error::<Test>::AssetNotFound
			);
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, 1000, 50 * ONE, 10 * ONE),
				Error::<Test>::AssetNotFound
			);
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 1000, 2000, 50 * ONE, 10 * ONE),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn sell_limit_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 1000 * ONE),
				Error::<Test>::BuyLimitNotReached
			);
		});
}

#[test]
fn sell_hub_asset_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::one(), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, HDX, 50 * ONE, 1000 * ONE),
				Error::<Test>::BuyLimitNotReached
			);
		});
}

#[test]
fn sell_hub_works() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(LP2),
				100,
				400000000000000
			));

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP3),
				1,
				200,
				50000000000000,
				10000000000000
			));

			assert_balance_approx!(Omnipool::protocol_account(), 0, NATIVE_AMOUNT, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 2, 1_000_000_000_000_000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 1, 13410000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 100, 2400000000000000u128, 1);
			assert_balance_approx!(Omnipool::protocol_account(), 200, 1925925925925925u128, 1);
			assert_balance_approx!(LP1, 100, 3000000000000000u128, 1);
			assert_balance_approx!(LP1, 200, 3000000000000000u128, 1);
			assert_balance_approx!(LP2, 100, 600000000000000u128, 1);
			assert_balance_approx!(LP3, 100, 1000000000000000u128, 1);
			assert_balance_approx!(LP3, 1, 50000000000000u128, 1);
			assert_balance_approx!(LP3, 200, 74074074074074u128, 1);

			assert_asset_state!(
				2,
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 0,
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
					protocol_shares: 0,
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
					reserve: 1925925925925926,
					hub_reserve: 1350000000000000,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(
				13410000000000000,
				26820000000000000,
				SimpleImbalance {
					value: 99813571961223,
					negative: true
				}
			);
		});
}

#[test]
fn sell_not_allowed_asset_fails() {
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
				RuntimeOrigin::root(),
				100,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				100,
				Tradability::BUY
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE),
				Error::<Test>::NotAllowed
			);
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				100,
				Tradability::SELL
			));

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE));

			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::FROZEN
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE),
				Error::<Test>::NotAllowed
			);

			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::SELL
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE),
				Error::<Test>::NotAllowed
			);

			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::BUY
			));

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 50 * ONE, 10 * ONE));
		});
}

#[test]
fn simple_sell_with_fee_works() {
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
		.with_token(100, FixedU128::one(), LP2, 2000 * ONE)
		.with_token(200, FixedU128::one(), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			let fee = Permill::from_percent(10);
			let fee = Permill::from_percent(100).checked_sub(&fee).unwrap();

			let expected_zero_fee = 47_619_047_619_047u128;
			let expected_10_percent_fee = fee.mul_floor(expected_zero_fee);

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			assert_eq!(Tokens::free_balance(100, &LP1), 950_000_000_000_000);
			assert_eq!(Tokens::free_balance(200, &LP1), expected_10_percent_fee);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				2000000000000000 - expected_10_percent_fee,
			);
		});
}

#[test]
fn sell_hub_asset_should_fail_when_asset_out_is_not_allowed_to_buy() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				200,
				Tradability::SELL | Tradability::ADD_LIQUIDITY
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP3), 1, 200, 50000000000000, 10000000000000),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_should_fail_when_trading_same_assets() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP3), 100, 100, 10 * ONE, 10000000000000),
				Error::<Test>::SameAssetTradeNotAllowed
			);
		});
}

#[test]
fn sell_should_work_when_trading_native_asset() {
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
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				HDX,
				200,
				sell_amount,
				min_limit
			));

			assert_eq!(Tokens::free_balance(HDX, &LP1), 950000000000000);
			assert_eq!(Tokens::free_balance(200, &LP1), 53_471_964_352_023);
			assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				NATIVE_AMOUNT + sell_amount
			);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1946528035647977
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
					reserve: 1946528035647977,
					hub_reserve: 1339800995024876,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10050000000000000,
					hub_reserve: 9960199004975124,
					shares: 10000 * ONE,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn sell_imbalance() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000000000000000),
			(LP1, 200, 5000000000000000),
			(LP2, 100, 1000000000000000),
			(LP3, 100, 1000000000000000),
			(LP3, 1, 100000000000000),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_protocol_fee(Permill::from_percent(20))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.with_token(200, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(LP2),
				100,
				400000000000000
			));

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP3),
				1,
				200,
				50000000000000,
				10000000000000
			));

			assert_pool_state!(
				13410000000000000,
				26820000000000000,
				SimpleImbalance {
					value: 99813571961223,
					negative: true
				}
			);

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), 200, 100, 1000000000000, 1,));
		});
}

#[test]
fn sell_should_fail_when_exceeds_max_in_ratio() {
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
		.with_max_in_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 1000 * ONE, 0u128),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn sell_should_fail_when_exceeds_max_out_ratio() {
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
		.with_token(100, FixedU128::from_float(1.00), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from_float(1.00), LP3, 100 * ONE)
		.with_max_out_ratio(3)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, 1000 * ONE, 0u128),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}

#[test]
fn sell_lrna_should_fail_when_exceeds_max_in_ratio() {
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
				Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 100, 1000 * ONE, 0u128),
				Error::<Test>::MaxInRatioExceeded
			);
		});
}

#[test]
fn sell_lrna_should_fail_when_exceeds_max_out_ratio() {
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
				Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 100, 1500 * ONE, 0u128),
				Error::<Test>::MaxOutRatioExceeded
			);
		});
}
