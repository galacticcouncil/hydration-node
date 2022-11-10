use super::*;

use crate::{
	add_omnipool_token, assert_balance, assert_stableswap_pool_assets,
	assert_that_asset_is_migrated_to_omnipool_subpool, assert_that_asset_is_not_present_in_omnipool,
	assert_that_sharetoken_in_omnipool_as_another_asset, create_subpool, AssetDetail, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;

#[test]
fn migrate_asset_to_subpool_should_work_when_subpool_exists() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			//Assert that liquidity has been moved
			let pool_account_old = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			assert_balance!(pool_account_old, ASSET_3, 0);
			assert_balance!(pool_account_old, ASSET_4, 0);
			assert_balance!(pool_account_old, ASSET_5, 0);

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			let omnipool_account = Omnipool::protocol_account();
			let subpool = Stableswap::get_pool(SHARE_ASSET_AS_POOL_ID).unwrap();

			assert_eq!(subpool.assets.to_vec(), vec![ASSET_3, ASSET_4, ASSET_5]);

			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(pool_account, ASSET_5, 5000 * ONE);

			assert_balance!(&omnipool_account, ASSET_3, 0);
			assert_balance!(&omnipool_account, ASSET_4, 0);
			assert_balance!(&omnipool_account, ASSET_5, 0);

			//Assert that share has been deposited to omnipool
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, 7800 * ONE);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 7800 * ONE,
					hub_reserve: 7800 * ONE,
					shares: 7800 * ONE,
					protocol_shares: 0,
					cap: 1_500_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				SHARE_ASSET_AS_POOL_ID,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);
		});
}

#[test]
fn migrate_asset_should_recalculate_protocol_shares_when_protocol_has_some_shares() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, 1000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_5, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			//We need to add liquidty then sacrificing it because we want to have some protocol shares for having meaningfull tests
			let position_id: u32 = Omnipool::next_position_id();
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_5,
				100 * ONE
			));
			assert_ok!(Omnipool::sacrifice_position(Origin::signed(ALICE), position_id));

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			//Assert
			assert_that_sharetoken_in_omnipool_as_another_asset!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 7865 * ONE,
					hub_reserve: 7865 * ONE,
					shares: 7865 * ONE,
					protocol_shares: 65 * ONE,
					cap: 1_500_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_subpool_does_not_exist() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), SHARE_ASSET_AS_POOL_ID, ASSET_5,),
				Error::<Test>::SubpoolNotFound
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_token_does_not_exist() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let non_existing_token = 99;
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), SHARE_ASSET_AS_POOL_ID, non_existing_token),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_called_from_non_origin() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::none(), SHARE_ASSET_AS_POOL_ID, ASSET_5),
				BadOrigin
			);
		});
}

fn migrate_asset_to_subpool_should_fail_when_called_by_normal_user() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act and assert
			let alice = 99;
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(
					mock::Origin::signed(alice),
					SHARE_ASSET_AS_POOL_ID,
					ASSET_5
				),
				BadOrigin
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_work_when_migrating_multiple_assets() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 6000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_7, 7000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);
			add_omnipool_token!(ASSET_7);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_6,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_7,
			));

			//Assert
			let pool_account =
				AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7], None);
			let omnipool_account = Omnipool::protocol_account();

			assert_stableswap_pool_assets!(
				SHARE_ASSET_AS_POOL_ID,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);

			//Assert that liquidty has been moved
			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(pool_account, ASSET_5, 5000 * ONE);
			assert_balance!(pool_account, ASSET_6, 6000 * ONE);
			assert_balance!(pool_account, ASSET_7, 7000 * ONE);

			assert_balance!(&omnipool_account, ASSET_3, 0);
			assert_balance!(&omnipool_account, ASSET_4, 0);
			assert_balance!(&omnipool_account, ASSET_5, 0);
			assert_balance!(&omnipool_account, ASSET_6, 0);
			assert_balance!(&omnipool_account, ASSET_7, 0);

			//Assert that share has been deposited to omnipool
			assert_balance!(&omnipool_account, SHARE_ASSET_AS_POOL_ID, 16250 * ONE);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				SHARE_ASSET_AS_POOL_ID,
				AssetReserveState::<Balance> {
					reserve: 16250 * ONE,
					hub_reserve: 16250 * ONE,
					shares: 16250 * ONE,
					protocol_shares: 0,
					cap: 3_500_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				SHARE_ASSET_AS_POOL_ID,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_6,
				SHARE_ASSET_AS_POOL_ID,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 6000 * ONE,
					hub_reserve: 3900 * ONE,
					share_tokens: 3900 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_7,
				SHARE_ASSET_AS_POOL_ID,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 7000 * ONE,
					hub_reserve: 4550 * ONE,
					share_tokens: 4550 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_6);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_7);
		});
}

#[test]
fn migrate_asset_to_subpool_should_update_subpool_when_called_consequently() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 6000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_7, 7000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);
			add_omnipool_token!(ASSET_7);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_4]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_4, ASSET_5]);

			//Assert that the liquidty is moved from old pool account
			let old_pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			assert_balance!(old_pool_account, ASSET_3, 0);
			assert_balance!(old_pool_account, ASSET_4, 0);
			assert_balance!(old_pool_account, ASSET_5, 0);

			//Assert that liquidty is moved to new pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(pool_account, ASSET_5, 5000 * ONE);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_6,
			));

			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6]);

			//Assert that the liquidity is moved from old pool account
			let old_pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			assert_balance!(old_pool_account, ASSET_3, 0);
			assert_balance!(old_pool_account, ASSET_4, 0);
			assert_balance!(old_pool_account, ASSET_5, 0);
			assert_balance!(old_pool_account, ASSET_6, 0);

			//Assert that liquidity is moved to new pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6], None);
			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(pool_account, ASSET_5, 5000 * ONE);
			assert_balance!(pool_account, ASSET_6, 6000 * ONE);
		});
}

#[test]
fn migrate_asset_to_subpool_should_sort_the_assets_in_subpool() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 6000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_7, 7000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);
			add_omnipool_token!(ASSET_7);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_5, ASSET_3);

			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_5]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_7,
			));

			//Assert
			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_5, ASSET_7]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_4,
			));

			//Assert
			assert_stableswap_pool_assets!(SHARE_ASSET_AS_POOL_ID, vec![ASSET_3, ASSET_4, ASSET_5, ASSET_7]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_6,
			));

			//Assert
			assert_stableswap_pool_assets!(
				SHARE_ASSET_AS_POOL_ID,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_doing_more_migration_than_max_pool_assets() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(ASSET_8)
		.with_registered_asset(SHARE_ASSET_AS_POOL_ID)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 6000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_7, 7000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_8, 8000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);
			add_omnipool_token!(ASSET_7);
			add_omnipool_token!(ASSET_8);

			create_subpool!(SHARE_ASSET_AS_POOL_ID, ASSET_3, ASSET_4);

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_5,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_6,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				SHARE_ASSET_AS_POOL_ID,
				ASSET_7,
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), SHARE_ASSET_AS_POOL_ID, ASSET_8),
				pallet_stableswap::Error::<Test>::MaxAssetsExceeded
			);

			//Post condition
			assert_stableswap_pool_assets!(
				SHARE_ASSET_AS_POOL_ID,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);
		});
}

//TODO: add tests for multiple pools with multiple assets,
//TODO: at the end, mutation testing
