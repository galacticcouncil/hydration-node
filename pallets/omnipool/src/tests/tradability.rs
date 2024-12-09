use super::*;
use frame_support::assert_noop;

#[test]
fn sell_asset_tradable_state_should_work_when_hub_asset_new_state_contains_sell_or_buy() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::SELL
			));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::BUY
			));
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				LRNA,
				Tradability::SELL | Tradability::BUY
			));
		});
}
#[test]
fn sell_asset_tradable_state_should_fail_when_hub_asset_new_state_contains_liquidity_operations() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL | Tradability::ADD_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::SELL | Tradability::REMOVE_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
			assert_noop!(
				Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					LRNA,
					Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
				),
				Error::<Test>::InvalidHubAssetTradableState
			);
		});
}
