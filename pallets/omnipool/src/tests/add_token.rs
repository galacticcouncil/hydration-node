use super::*;
use frame_support::assert_noop;

#[test]
fn initialize_pool_works() {
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

			assert_ok!(Omnipool::initialize_pool(Origin::root(), stable_price, native_price));

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
					tradable: Tradability::default(),
				}
			);
		});
}
#[test]
fn already_initialized_pool_fails() {
	ExtBuilder::default()
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(Origin::root(), stable_price, native_price),
				Error::<Test>::AssetAlreadyAdded
			);
		});
}

#[test]
fn initialize_pool_without_stable_balance_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(Origin::root(), stable_price, native_price),
				Error::<Test>::MissingBalance
			);
		});
}

#[test]
fn initialize_pool_without_native_balance_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from_float(0.5);
			let native_price = FixedU128::from_float(1.5);

			assert_noop!(
				Omnipool::initialize_pool(Origin::root(), stable_price, native_price),
				Error::<Test>::MissingBalance
			);
		});
}
#[test]
fn initialize_pool_with_zero_stable_price_fails() {
	ExtBuilder::default().build().execute_with(|| {
		let stable_price = FixedU128::from(0);
		let native_price = FixedU128::from(1);

		assert_noop!(
			Omnipool::initialize_pool(Origin::root(), stable_price, native_price),
			Error::<Test>::InvalidInitialAssetPrice
		);
	});
}

#[test]
fn initialize_pool_with_zero_native_price_fails() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)])
		.build()
		.execute_with(|| {
			let stable_price = FixedU128::from(1);
			let native_price = FixedU128::from(0);

			assert_noop!(
				Omnipool::initialize_pool(Origin::root(), stable_price, native_price),
				Error::<Test>::InvalidInitialAssetPrice
			);
		});
}

#[test]
fn add_token_works() {
	ExtBuilder::default()
		.with_registered_asset(1000)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_price = FixedU128::from_float(0.65);

			let token_amount = 2000 * ONE;

			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				1_000,
				token_amount,
				token_price,
				LP1
			));

			assert_pool_state!(11_800 * ONE, 23_600 * ONE, SimpleImbalance::default());

			assert_asset_state!(
				1_000,
				AssetReserveState {
					reserve: token_amount,
					hub_reserve: 1300 * ONE,
					shares: token_amount,
					protocol_shares: token_amount,
					tvl: 2600000000000000,
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
				Omnipool::add_token(Origin::signed(LP1), 2_000, 2000 * ONE, FixedU128::from_float(0.5), LP1),
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
				Omnipool::add_token(Origin::signed(LP1), 1_000, 100 * ONE, token_price, LP1),
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
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::add_token(
				Origin::signed(LP1),
				1_000,
				2000 * ONE,
				FixedU128::from_float(0.5),
				LP1
			));

			assert_noop!(
				Omnipool::add_token(Origin::signed(LP1), 1_000, 2000 * ONE, FixedU128::from_float(0.5), LP1),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(Origin::signed(LP1), DAI, 2000 * ONE, FixedU128::from_float(0.5), LP1),
				Error::<Test>::AssetAlreadyAdded
			);
			assert_noop!(
				Omnipool::add_token(Origin::signed(LP1), HDX, 2000 * ONE, FixedU128::from_float(0.5), LP1),
				Error::<Test>::AssetAlreadyAdded
			);
		});
}

#[test]
fn first_assset_must_be_hub_asset() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::add_token(Origin::signed(LP1), HDX, 2000 * ONE, FixedU128::from_float(0.5), LP1),
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
				Omnipool::add_token(Origin::signed(LP3), 1_000, 1000 * ONE, FixedU128::from_float(0.6), LP3),
				orml_tokens::Error::<Test>::BalanceTooLow
			);
		});
}
