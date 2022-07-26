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
				Origin::root(),
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
					tvl: 2600000000000000,
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
					Origin::root(),
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
				Omnipool::add_token(Origin::root(), 1_000, token_price, Permill::from_percent(100), LP1),
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
					Origin::root(),
					1_000,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(
					Origin::root(),
					DAI,
					FixedU128::from_float(0.5),
					Permill::from_percent(100),
					LP1
				),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(
					Origin::root(),
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
fn first_assset_must_be_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::add_token(
				Origin::root(),
				HDX,
				FixedU128::from_float(0.5),
				Permill::from_percent(100),
				LP1
			),
			Error::<Test>::NoStableAssetInPool
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
					Origin::root(),
					1_000,
					FixedU128::from_float(0.6),
					Permill::from_percent(100),
					LP3
				),
				Error::<Test>::MissingBalance
			);
		});
}
