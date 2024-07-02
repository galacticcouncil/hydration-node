use super::*;
use crate::tests::mock::{ExtBuilder, Omnipool, HDX, LP1, NATIVE_AMOUNT, ONE};
use frame_support::assert_noop;
use sp_runtime::FixedU128;

#[test]
fn sell_asset_for_hub_asset_not_allowed_by_default() {
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
fn sell_asset_for_hub_asset_should_fail_when_account_is_not_allowed_to_buy_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, HDX, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::BUY | Tradability::SELL
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, LRNA, 100 * ONE, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn buy_hub_asset_should_fail_when_account_is_not_allowed_to_buy_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, HDX, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::BUY | Tradability::SELL
			));

			assert_noop!(
				Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, LRNA, 100 * ONE, 0),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn sell_asset_for_hub_asset_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, HDX, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::BUY | Tradability::SELL
			));

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), HDX, LRNA, 100 * ONE, 0),);

			pretty_assertions::assert_eq!(Tokens::free_balance(HDX, &LP1), 1900_000_000_000_000);
			pretty_assertions::assert_eq!(Tokens::free_balance(LRNA, &LP1), 99009900990099);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				10100000000000000
			);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				10400990099009901
			);
			assert_pool_state!(
				10400990099009901,
				20801980198019802,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10100000000000000,
					hub_reserve: 9900990099009901,
					shares: 10000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn buy_hub_asset_should_work() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), 0, NATIVE_AMOUNT),
			(Omnipool::protocol_account(), 2, 1000 * ONE),
			(LP1, HDX, 2000 * ONE),
		])
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::BUY | Tradability::SELL
			));

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				LRNA,
				HDX,
				100 * ONE,
				u128::MAX
			),);

			pretty_assertions::assert_eq!(Tokens::free_balance(HDX, &LP1), 1898_989_898_989_898);
			pretty_assertions::assert_eq!(Tokens::free_balance(LRNA, &LP1), 100 * ONE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(HDX, &Omnipool::protocol_account()),
				10101010101010102
			);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(LRNA, &Omnipool::protocol_account()),
				10400000000000000
			);
			assert_pool_state!(
				10_400 * ONE,
				20_800 * ONE,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10101010101010102,
					hub_reserve: 9900000000000000,
					shares: 10000000000000000,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn buy_hub_asset_and_sell_hub_asset_should_match_direct_trade() {
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
				LRNA,
				Tradability::BUY | Tradability::SELL
			));
			let liq_added = 400 * ONE;
			assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), 100, liq_added));

			let sell_amount = 50 * ONE;

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, LRNA, sell_amount, 0,),);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				200,
				LRNA,
				47808764940238,
				u128::MAX
			),);

			// ensure nothing left
			pretty_assertions::assert_eq!(Tokens::free_balance(LRNA, &LP1), 0);

			pretty_assertions::assert_eq!(Tokens::free_balance(100, &LP1), 550000000000000);
			pretty_assertions::assert_eq!(Tokens::free_balance(200, &LP1), 47808764940238);
			pretty_assertions::assert_eq!(Tokens::free_balance(LRNA, &Omnipool::protocol_account()), 13360 * ONE);
			pretty_assertions::assert_eq!(Tokens::free_balance(100, &Omnipool::protocol_account()), 2450 * ONE);
			pretty_assertions::assert_eq!(
				Tokens::free_balance(200, &Omnipool::protocol_account()),
				1952191235059762
			);

			assert_pool_state!(
				13_360 * ONE,
				26_720 * ONE,
				SimpleImbalance {
					value: 63597602795242u128, //TODO: this changed!
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
