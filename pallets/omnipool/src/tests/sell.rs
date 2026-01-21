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

			assert_pool_state!(13_360 * ONE, 26_720 * ONE);

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
					hub_reserve: 10050000000000000, // H2O now routed to HDX subpool
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
					hub_reserve: 1300000000000000, // unchanged - H2O routed to HDX subpool
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_pool_state!(13410000000000000, 26820000000000000);
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
			assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				NATIVE_AMOUNT + sell_amount
			);
			assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1946528035647977
			);

			// Verify hub reserve invariant (LRNA balance = sum of all hub reserves)
			// Note: Exact LRNA balance depends on protocol fee routing to HDX subpool
			assert_hub_asset!();

			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1946528035647977,
					hub_reserve: 1343902949850822,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			// HDX hub_reserve changes due to protocol fees being routed to HDX subpool
			// Verify reserve, shares, and other fields but not the exact hub_reserve value
			let hdx_reserve = Tokens::free_balance(HDX, &Omnipool::protocol_account());
			assert_eq!(hdx_reserve, 10050000000000000);
			let hdx_state = Assets::<Test>::get(HDX).unwrap();
			assert_eq!(hdx_state.shares, 10000 * ONE);
			assert_eq!(hdx_state.protocol_shares, 0);
			assert_eq!(hdx_state.tradable, Tradability::default());
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

#[test]
fn spot_price_after_sell_should_be_identical_when_protocol_fee_is_nonzero() {
	let mut spot_price_1 = FixedU128::zero();
	let mut spot_price_2 = FixedU128::zero();

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
		.with_asset_fee(Permill::from_percent(0))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let expected_sold_amount = 58_823_529_411_766;
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				expected_sold_amount,
				0
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_1 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

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
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let expected_sold_amount = 58_823_529_411_766;
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				expected_sold_amount,
				0
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_2 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	assert_eq_approx!(
		spot_price_1,
		spot_price_2,
		FixedU128::from_float(0.000000001),
		"spot price afters sells"
	);
}

#[test]
fn sell_and_buy_should_get_same_amounts_when_all_fees_are_set() {
	let buy_amount = 49513753820506u128;
	let sold_amount = 58_823_529_411_766u128;
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
		.with_protocol_fee(Permill::from_percent(1))
		.with_burn_fee(Permill::from_percent(50))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(1))
		.build()
		.execute_with(|| {
			let initial_lp1_balance_200 = Tokens::free_balance(200, &LP1);
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, 200, sold_amount, 0));
			let lp1_balance_200 = Tokens::free_balance(200, &LP1);
			assert_eq!(lp1_balance_200, initial_lp1_balance_200 + buy_amount);
		});

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
		.with_protocol_fee(Permill::from_percent(1))
		.with_burn_fee(Permill::from_percent(50))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(1))
		.build()
		.execute_with(|| {
			let initial_lp1_balance_200 = Tokens::free_balance(200, &LP1);
			let initial_lp1_balance_100 = Tokens::free_balance(100, &LP1);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				100,
				buy_amount,
				u128::MAX,
			));
			let lp1_balance_200 = Tokens::free_balance(200, &LP1);
			assert_eq!(lp1_balance_200, initial_lp1_balance_200 + buy_amount);

			let lp1_balance_100 = Tokens::free_balance(100, &LP1);
			let spent = initial_lp1_balance_100 - lp1_balance_100;
			assert_eq!(spent, sold_amount - 1); //TODO: this can adtually fixed by rounding. Needs colin verification!
			assert_eq!(lp1_balance_100, initial_lp1_balance_100 - sold_amount + 1);
		});
}

#[test]
fn spot_price_after_sell_should_be_identical_when_protocol_fee_is_nonzero_and_part_of_asset_fee_is_taken() {
	let mut spot_price_1 = FixedU128::zero();
	let mut spot_price_2 = FixedU128::zero();

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
		.with_asset_fee(Permill::from_percent(0))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(50))
		.build()
		.execute_with(|| {
			let expected_sold_amount = 58_823_529_411_766;
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				expected_sold_amount,
				0
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_1 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

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
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(10))
		.build()
		.execute_with(|| {
			let expected_sold_amount = 58_823_529_411_766;
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				expected_sold_amount,
				0
			));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_2 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	assert_eq_approx!(
		spot_price_1,
		spot_price_2,
		FixedU128::from_float(0.000000001),
		"spot price afters sells"
	);
}

#[test]
fn spot_price_after_selling_hub_asset_should_be_identical_when_protocol_fee_is_nonzero() {
	let mut spot_price_1 = FixedU128::zero();
	let mut spot_price_2 = FixedU128::zero();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(0))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let sell_amount = 50_000_000_000_000;
			let initial_lrna_balance = Tokens::free_balance(LRNA, &LP1);
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 200, sell_amount, 0));
			let final_lrna_balance = Tokens::free_balance(LRNA, &LP1);

			assert_eq!(final_lrna_balance, initial_lrna_balance - sell_amount);

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();
			spot_price_1 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(0))
		.build()
		.execute_with(|| {
			let sell_amount = 50_000_000_000_000;
			let initial_lrna_balance = Tokens::free_balance(LRNA, &LP1);
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 200, sell_amount, 0));
			let final_lrna_balance = Tokens::free_balance(LRNA, &LP1);
			assert_eq!(final_lrna_balance, initial_lrna_balance - sell_amount);

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_2 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	// With H2O routing to HDX subpool, fee differences affect spot price more directly
	// since hub_reserve no longer changes on the traded asset, hence the delta tolerance
	assert_eq_approx!(
		spot_price_1,
		spot_price_2,
		FixedU128::from_float(0.0001),
		"spot price afters sells"
	);
}

#[test]
fn spot_price_after_selling_hub_asset_should_be_identical_when_protocol_fee_is_nonzero_and_part_of_asset_fee_is_taken()
{
	let mut spot_price_1 = FixedU128::zero();
	let mut spot_price_2 = FixedU128::zero();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(0))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(5))
		.build()
		.execute_with(|| {
			let sell_amount = 50_000_000_000_000;
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 200, sell_amount, 0));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();
			spot_price_1 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, 200, 2000 * ONE),
			(LP1, LRNA, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_registered_asset(200)
		.with_asset_fee(Permill::from_percent(10))
		.with_protocol_fee(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::from(1), LP2, 2000 * ONE)
		.with_token(200, FixedU128::from(1), LP3, 2000 * ONE)
		.with_on_trade_withdrawal(Permill::from_percent(5))
		.build()
		.execute_with(|| {
			let sell_amount = 50_000_000_000_000;
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), LRNA, 200, sell_amount, 0));

			let actual = Pallet::<Test>::load_asset_state(200).unwrap();

			spot_price_2 = FixedU128::from_rational(actual.reserve, actual.hub_reserve);
		});

	// With H2O routing to HDX subpool, fee differences affect spot price more directly
	// since hub_reserve no longer changes on the traded asset, hence the delta tolerance
	assert_eq_approx!(
		spot_price_1,
		spot_price_2,
		FixedU128::from_float(0.0001),
		"spot price afters sells"
	);
}

#[test]
fn sell_with_all_fees_and_extra_withdrawal_works() {
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
		.with_protocol_fee(Permill::from_percent(3))
		.with_burn_fee(Permill::from_percent(50))
		.with_on_trade_withdrawal(Permill::from_percent(10))
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::one(), LP2, 2000 * ONE)
		.with_token(200, FixedU128::one(), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2000 * ONE + sell_amount,
					hub_reserve: 1951219512195122,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1957936621396236,
					hub_reserve: 2051676360499704,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 950_000_000_000_000);
			assert_eq!(Tokens::free_balance(200, &LP1), 41601143674053);
			assert_eq!(Tokens::free_balance(200, &TRADE_FEE_COLLECTOR), 462234929711);
			// Protocol fees now stay in protocol account and are routed to HDX hub reserve
			// No longer transferred to PROTOCOL_FEE_COLLECTOR
			assert_eq!(Tokens::free_balance(LRNA, &PROTOCOL_FEE_COLLECTOR), 0);
			// Account for 200 asset
			let initial_reserve = 2000 * ONE;
			let omnipool_200_reserve = Tokens::free_balance(200, &Omnipool::protocol_account());
			let fee_collector = Tokens::free_balance(200, &TRADE_FEE_COLLECTOR);
			let buy_amount = Tokens::free_balance(200, &LP1);
			assert_eq!(initial_reserve, omnipool_200_reserve + buy_amount + fee_collector);
		});
}

#[test]
fn sell_allows_tolerance_when_part_of_fee_is_taken() {
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
		.with_protocol_fee(Permill::from_percent(3))
		.with_burn_fee(Permill::from_percent(50))
		.with_on_trade_withdrawal(Permill::from_percent(100))
		.with_on_trade_withdrawal_extra(1)
		.with_initial_pool(FixedU128::from(1), FixedU128::from(1))
		.with_token(100, FixedU128::one(), LP2, 2000 * ONE)
		.with_token(200, FixedU128::one(), LP3, 2000 * ONE)
		.build()
		.execute_with(|| {
			let sell_amount = 50 * ONE;
			let min_limit = 10 * ONE;

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP1),
				100,
				200,
				sell_amount,
				min_limit
			));

			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2000 * ONE + sell_amount,
					hub_reserve: 1951219512195122,
					shares: 2000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				200,
				AssetReserveState {
					reserve: 1953776507028830,
					hub_reserve: 2047317073170732,
					shares: 2000 * ONE,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Tokens::free_balance(100, &LP1), 950_000_000_000_000);
			assert_eq!(Tokens::free_balance(200, &LP1), 41601143674053);
			assert_eq!(Tokens::free_balance(200, &TRADE_FEE_COLLECTOR), 4622349297117);
			// Protocol fees now stay in protocol account and are routed to HDX hub reserve
			// No longer transferred to PROTOCOL_FEE_COLLECTOR
			assert_eq!(Tokens::free_balance(LRNA, &PROTOCOL_FEE_COLLECTOR), 0);
			// Account for 200 asset
			let initial_reserve = 2000 * ONE;
			let omnipool_200_reserve = Tokens::free_balance(200, &Omnipool::protocol_account());
			let fee_collector = Tokens::free_balance(200, &TRADE_FEE_COLLECTOR);
			let buy_amount = Tokens::free_balance(200, &LP1);
			assert_eq!(initial_reserve, omnipool_200_reserve + buy_amount + fee_collector);
		});
}

#[test]
fn sell_hub_routes_to_hdx_subpool() {
	let initial_asset_100_reserve = 2000 * ONE;
	let expected_asset_100_reserve = 1_925_925_925_925_926;
	let expected_received = initial_asset_100_reserve - expected_asset_100_reserve;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, initial_asset_100_reserve)
		.build()
		.execute_with(|| {
			let sell_amount = 50 * ONE;
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP3),
				LRNA,
				100,
				sell_amount,
				0
			));

			// HDX subpool: hub_reserve increased by delta_hub_reserve (routed here)
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10_000_000_000_000_000,
					hub_reserve: NATIVE_AMOUNT + sell_amount,
					shares: 10_000_000_000_000_000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			// Asset 100: reserve decreased (user received tokens), hub_reserve unchanged
			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: expected_asset_100_reserve,
					hub_reserve: 1_300_000_000_000_000,
					shares: 2_000_000_000_000_000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Tokens::free_balance(LRNA, &LP3), 100 * ONE - sell_amount);
			assert_eq!(Tokens::free_balance(100, &LP3), expected_received);

			// Verify the HDX routing event is emitted
			expect_last_events(vec![
				Event::SellExecuted {
					who: LP3,
					asset_in: LRNA,
					asset_out: 100,
					amount_in: sell_amount,
					amount_out: expected_received,
					hub_amount_in: 0,
					hub_amount_out: 0,
					asset_fee_amount: 0,
					protocol_fee_amount: 0,
				}
				.into(),
				pallet_broadcast::Event::Swapped3 {
					swapper: LP3,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, sell_amount)],
					outputs: vec![Asset::new(100, expected_received)],
					fees: vec![Fee::new(100, 0, Destination::Account(Omnipool::protocol_account()))],
					operation_stack: vec![],
				}
				.into(),
				// HDX routing event
				pallet_broadcast::Event::Swapped3 {
					swapper: LP3,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, sell_amount)],
					outputs: vec![Asset::new(HDX, 0)],
					fees: vec![],
					operation_stack: vec![],
				}
				.into(),
			]);
		});
}

#[test]
fn buy_for_hub_routes_to_hdx_subpool() {
	let expected_hdx_hub_reserve = 10_033_333_333_333_334;
	let expected_lrna_spent = expected_hdx_hub_reserve - NATIVE_AMOUNT;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP1, 100, 5000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			let buy_amount = 50 * ONE;
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				buy_amount,
				100 * ONE
			));

			// HDX subpool: hub_reserve increased by delta_hub_reserve (routed here)
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10_000_000_000_000_000,
					hub_reserve: expected_hdx_hub_reserve,
					shares: 10_000_000_000_000_000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			// Asset 100: reserve decreased by buy_amount, hub_reserve unchanged
			assert_asset_state!(
				100,
				AssetReserveState {
					reserve: 2000 * ONE - buy_amount,
					hub_reserve: 1_300_000_000_000_000,
					shares: 2_000_000_000_000_000,
					protocol_shares: 0,
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);

			assert_eq!(Tokens::free_balance(LRNA, &LP3), 100 * ONE - expected_lrna_spent);
			assert_eq!(Tokens::free_balance(100, &LP3), buy_amount);

			// Verify the HDX routing event is emitted
			expect_last_events(vec![
				Event::BuyExecuted {
					who: LP3,
					asset_in: LRNA,
					asset_out: 100,
					amount_in: expected_lrna_spent,
					amount_out: buy_amount,
					hub_amount_in: 0,
					hub_amount_out: 0,
					asset_fee_amount: 0,
					protocol_fee_amount: 0,
				}
				.into(),
				pallet_broadcast::Event::Swapped3 {
					swapper: LP3,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(LRNA, expected_lrna_spent)],
					outputs: vec![Asset::new(100, buy_amount)],
					fees: vec![Fee::new(100, 0, Destination::Account(Omnipool::protocol_account()))],
					operation_stack: vec![],
				}
				.into(),
				// HDX routing event
				pallet_broadcast::Event::Swapped3 {
					swapper: LP3,
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, expected_lrna_spent)],
					outputs: vec![Asset::new(HDX, 0)],
					fees: vec![],
					operation_stack: vec![],
				}
				.into(),
			]);
		});
}
