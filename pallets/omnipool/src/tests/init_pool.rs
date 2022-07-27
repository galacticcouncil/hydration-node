use super::*;
use frame_support::assert_noop;

#[test]
fn initialize_pool_should_work_when_called_first_time_with_correct_params() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 100 * ONE),
			(Omnipool::protocol_account(), HDX, 200 * ONE),
		])
		.build()
		.execute_with(|| {
			let stable_amount = 100 * ONE;
			let native_amount = 200 * ONE;

			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			// ACT
			assert_ok!(Omnipool::initialize_pool(
				Origin::root(),
				stable_price,
				native_price,
				Permill::from_percent(50),
				Permill::from_percent(50)
			));

			// ASSERT
			// - pool state
			// - native and stable asset states
			// - correct balances
			assert_pool_state!(
				stable_price.checked_mul_int(stable_amount).unwrap()
					+ native_price.checked_mul_int(native_amount).unwrap(),
				native_price.checked_mul_int(native_amount).unwrap()
					* (stable_amount / stable_price.checked_mul_int(stable_amount).unwrap())
					+ stable_amount,
				SimpleImbalance::default()
			);

			assert_asset_state!(
				DAI,
				AssetReserveState {
					reserve: 100000000000000,
					hub_reserve: 50000000000000,
					shares: 100000000000000,
					protocol_shares: 100000000000000,
					tvl: 100000000000000,
					cap: 500_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 200000000000000,
					hub_reserve: 300000000000000,
					shares: 200000000000000,
					protocol_shares: 200000000000000,
					tvl: 600000000000000,
					cap: 500_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_balance!(Omnipool::protocol_account(), DAI, stable_amount);
			assert_balance!(Omnipool::protocol_account(), HDX, native_amount);

			assert_eq!(HubAssetTradability::<Test>::get(), Tradability::SELL);
		});
}

#[test]
fn initialize_pool_should_fail_when_already_initialized() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				),
				Error::<Test>::AssetAlreadyAdded
			);
		});
}

#[test]
fn initialize_pool_should_fail_when_stable_funds_missing_in_pool_account() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				),
				Error::<Test>::MissingBalance
			);
		});
}

#[test]
fn initialize_pool_should_fail_when_native_funds_missing_in_pool_account() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				),
				Error::<Test>::MissingBalance
			);
		});
}

#[test]
fn initialize_pool_should_fail_when_stable_price_is_zero() {
	ExtBuilder::default().build().execute_with(|| {
		let stable_price = FixedU128::from(0);
		let native_price = FixedU128::from(1);

		assert_noop!(
			Omnipool::initialize_pool(
				Origin::root(),
				stable_price,
				native_price,
				Permill::from_percent(100),
				Permill::from_percent(100)
			),
			Error::<Test>::InvalidInitialAssetPrice
		);
	});
}

#[test]
fn initialize_pool_should_fail_when_native_price_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from(1);
			let native_price = FixedU128::from(0);

			assert_noop!(
				Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				),
				Error::<Test>::InvalidInitialAssetPrice
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
				Origin::root(),
				HDX,
				Permill::from_rational(1u32, 100000u32),
			));
			assert_asset_state!(
				HDX,
				AssetReserveState {
					reserve: 10000000000000000,
					hub_reserve: 10000000000000000,
					shares: 10000000000000000,
					protocol_shares: 10000000000000000,
					tvl: 20000000000000000,
					cap: 10_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
			assert_ok!(Omnipool::set_asset_weight_cap(
				Origin::root(),
				DAI,
				Permill::from_percent(2u32),
			));
			assert_asset_state!(
				DAI,
				AssetReserveState {
					reserve: 1000000000000000,
					hub_reserve: 500000000000000,
					shares: 1000000000000000,
					protocol_shares: 1000000000000000,
					tvl: 1000000000000000,
					cap: 20_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
		});
}
