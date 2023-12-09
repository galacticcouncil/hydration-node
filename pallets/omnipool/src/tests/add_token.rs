use super::*;
use frame_support::assert_noop;

#[test]
fn add_token_works() {
	ExtBuilder::default()
		.with_registered_asset(1000)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 1_000, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_price = FixedU128::from_float(0.65);

			let token_amount = 2000 * ONE;

			assert_ok!(Omnipool::add_token(
				RuntimeOrigin::root(),
				1_000,
				token_price,
				Permill::from_percent(100),
				LP1
			));

			assert_pool_state!(11_800 * ONE, 23_600 * ONE, SimpleImbalance::default());

			assert_asset_state!(
				1_000,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1300 * ONE,
					shares: token_amount,
					protocol_shares: Balance::zero(),
					cap: DEFAULT_WEIGHT_CAP,
					tradable: Tradability::default(),
				}
			)
		});
}

#[test]
fn add_non_registered_asset_fails() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					2_000,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetNotRegistered
			);
		});
}

#[test]
fn add_token_with_zero_price_fails() {
	ExtBuilder::default()
		.with_registered_asset(1000)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_price = FixedU128::from(0);

			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					1_000,
					token_price,
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::InvalidInitialAssetPrice
			);
		});
}

#[test]
fn cannot_add_existing_asset() {
	ExtBuilder::default()
		.with_registered_asset(1000)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1000, FixedU128::from_float(0.5), LP1, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					1_000,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					DAI,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					HDX,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetAlreadyAdded
			);
		});
}

#[test]
fn add_token_with_insufficient_balance_fails() {
	ExtBuilder::default()
		.add_endowed_accounts((LP3, 1_000, 100 * ONE))
		.with_registered_asset(1000)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_token(
					RuntimeOrigin::root(),
					1_000,
					FixedU128::from_float(0.6),
					Permill::from_percent(100),
					LP3
				),
				Error::<Test>::MissingBalance
			);
		});
}

#[test]
fn update_weight_cap_of_native_stable_asset_should_work_when_pool_is_initialized() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_weight_cap(
				RuntimeOrigin::root(),
				HDX,
				Permill::from_rational(1u32, 100000u32),
			));
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10000000000000000,
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 0,
					cap: 10_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
			assert_ok!(Omnipool::set_asset_weight_cap(
				RuntimeOrigin::root(),
				DAI,
				Permill::from_percent(2u32),
			));
			assert_asset_state!(
				DAI,
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 0,
					cap: 20_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
		});
}
