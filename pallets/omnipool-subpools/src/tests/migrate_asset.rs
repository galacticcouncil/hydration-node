use super::*;

use crate::{
	add_omnipool_token, assert_stableswap_pool_assets, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_sharetoken_in_omnipool_as_another_asset, AssetDetail,
	Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;

#[test]
fn migrate_asset_to_subpool_should_work_when_subpool_exists() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			let omnipool_account = Omnipool::protocol_account();
			let subpool = Stableswap::get_pool(share_asset_as_pool_id).unwrap();

			assert_eq!(subpool.assets.to_vec(), vec![ASSET_3, ASSET_4, ASSET_5]);

			//Assert that liquidty has been moved
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 3000 * ONE);
			assert_eq!(subpool_balance_of_asset_4, 4000 * ONE);
			assert_eq!(subpool_balance_of_asset_5, 5000 * ONE);

			let omnipool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &omnipool_account);
			let omnipool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &omnipool_account);
			let omnipool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &omnipool_account);
			assert_eq!(omnipool_balance_of_asset_3, 0);
			assert_eq!(omnipool_balance_of_asset_4, 0);
			assert_eq!(omnipool_balance_of_asset_5, 0);

			//Assert that share has been deposited to omnipool
			let balance_shares = Tokens::free_balance(share_asset_as_pool_id, &omnipool_account);
			assert_eq!(balance_shares, 7800 * ONE);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 7800 * ONE,
					hub_reserve: 7800 * ONE,
					shares: 7800 * ONE,
					protocol_shares: 0,
					cap: 1_100_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);

			//TODO: Adjust test to have non-zero protocol shares
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_subpool_does_not_exist() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), share_asset_as_pool_id, ASSET_5,),
				Error::<Test>::SubpoolNotFound
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_token_does_not_exist() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act and assert
			let non_existing_token = 99;
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), share_asset_as_pool_id, non_existing_token),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_called_from_non_origin() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::none(), share_asset_as_pool_id, ASSET_5),
				BadOrigin
			);
		});
}

fn migrate_asset_to_subpool_should_fail_when_called_by_normal_user() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act and assert
			let alice = 99;
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(
					mock::Origin::signed(alice),
					share_asset_as_pool_id,
					ASSET_5
				),
				BadOrigin
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_work_when_migrating_multiple_assets() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 20;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_6,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_7,
			));

			//Assert
			let pool_account =
				AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7], None);
			let omnipool_account = Omnipool::protocol_account();

			assert_stableswap_pool_assets!(
				share_asset_as_pool_id,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);

			//Assert that liquidty has been moved
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			let subpool_balance_of_asset_6 = Tokens::free_balance(ASSET_6, &pool_account);
			let subpool_balance_of_asset_7 = Tokens::free_balance(ASSET_7, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 3000 * ONE);
			assert_eq!(subpool_balance_of_asset_4, 4000 * ONE);
			assert_eq!(subpool_balance_of_asset_5, 5000 * ONE);
			assert_eq!(subpool_balance_of_asset_6, 6000 * ONE);
			assert_eq!(subpool_balance_of_asset_7, 7000 * ONE);

			let omnipool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &omnipool_account);
			let omnipool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &omnipool_account);
			let omnipool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &omnipool_account);
			let omnipool_balance_of_asset_6 = Tokens::free_balance(ASSET_6, &omnipool_account);
			let omnipool_balance_of_asset_7 = Tokens::free_balance(ASSET_7, &omnipool_account);
			assert_eq!(omnipool_balance_of_asset_3, 0);
			assert_eq!(omnipool_balance_of_asset_4, 0);
			assert_eq!(omnipool_balance_of_asset_5, 0);
			assert_eq!(omnipool_balance_of_asset_6, 0);
			assert_eq!(omnipool_balance_of_asset_7, 0);

			//Assert that share has been deposited to omnipool
			let balance_shares = Tokens::free_balance(share_asset_as_pool_id, &omnipool_account);
			assert_eq!(balance_shares, 16250 * ONE);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 16250 * ONE,
					hub_reserve: 16250 * ONE,
					shares: 16250 * ONE,
					protocol_shares: 0,
					cap: 3_100_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_6,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 6000 * ONE,
					hub_reserve: 3900 * ONE,
					share_tokens: 3900 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_7,
				share_asset_as_pool_id,
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

			//TODO: Adjust test to have non-zero protocol shares
		});
}

#[test]
fn migrate_asset_to_subpool_should_update_subpool_when_called_consequently() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 20;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_4]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
			));

			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_4, ASSET_5]);

			//Assert that the liquidty is moved from old pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 0);
			assert_eq!(subpool_balance_of_asset_4, 0);
			assert_eq!(subpool_balance_of_asset_5, 0);

			//Assert that liquidty is moved to new pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 3000 * ONE);
			assert_eq!(subpool_balance_of_asset_4, 4000 * ONE);
			assert_eq!(subpool_balance_of_asset_5, 5000 * ONE);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_6,
			));

			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6]);

			//Assert that the liquidty is moved from old pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 0);
			assert_eq!(subpool_balance_of_asset_4, 0);
			assert_eq!(subpool_balance_of_asset_5, 0);

			//Assert that liquidty is moved to new pool account
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6], None);
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 3000 * ONE);
			assert_eq!(subpool_balance_of_asset_4, 4000 * ONE);
			assert_eq!(subpool_balance_of_asset_5, 5000 * ONE);
		});
}

#[test]
fn migrate_asset_to_subpool_should_sort_the_assets_in_subpool() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 20;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
				ASSET_3,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_5]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_7,
			));

			//Assert
			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_5, ASSET_7]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_4,
			));

			//Assert
			assert_stableswap_pool_assets!(share_asset_as_pool_id, vec![ASSET_3, ASSET_4, ASSET_5, ASSET_7]);

			//Act
			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_6,
			));

			//Assert
			assert_stableswap_pool_assets!(
				share_asset_as_pool_id,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_doing_more_migration_than_max_pool_assets() {
	//Arrange
	let share_asset_as_pool_id: AssetId = 20;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(ASSET_7)
		.with_registered_asset(ASSET_8)
		.with_registered_asset(share_asset_as_pool_id)
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

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_6,
			));

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_7,
			));

			//Act and assert
			assert_noop!(
				OmnipoolSubpools::migrate_asset_to_subpool(Origin::root(), share_asset_as_pool_id, ASSET_8),
				pallet_stableswap::Error::<Test>::MaxAssetsExceeded
			);

			//Assert
			assert_stableswap_pool_assets!(
				share_asset_as_pool_id,
				vec![ASSET_3, ASSET_4, ASSET_5, ASSET_6, ASSET_7]
			);
		});
}

//TODO: add tests for multiple pools with multiple assets,
//TODO: at the end, mutation testing
