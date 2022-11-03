use super::*;

use crate::AssetDetail;
use crate::{
	add_omnipool_token, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_sharetoken_is_added_to_omnipool_as_another_asset,
	assert_that_stableswap_subpool_is_created_with_poolinfo, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::AssetState;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pallet_stableswap::types::PoolInfo;
use pretty_assertions::assert_eq;
use sp_runtime::BoundedVec;

#[test]
fn create_subpool_should_fail_when_called_by_non_origin() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
			assert_noop!(
				OmnipoolSubpools::create_subpool(
					mock::Origin::none(),
					share_asset_as_pool_id,
					ASSET_3,
					ASSET_4,
					Permill::from_percent(10),
					100u16,
					Permill::from_percent(0),
					Permill::from_percent(0),
				),
				BadOrigin
			);
		});
}

#[test]
fn create_subpool_should_fail_when_called_by_user() {
	let alice = 99;
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
			assert_noop!(
				OmnipoolSubpools::create_subpool(
					mock::Origin::signed(alice),
					share_asset_as_pool_id,
					ASSET_3,
					ASSET_4,
					Permill::from_percent(10),
					100u16,
					Permill::from_percent(0),
					Permill::from_percent(0),
				),
				BadOrigin
			);
		});
}

#[test]
fn create_subpool_should_fail_when_asset_a_does_not_exist_in_omnipool() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_4);

			//Act
			assert_noop!(
				OmnipoolSubpools::create_subpool(
					Origin::root(),
					share_asset_as_pool_id,
					ASSET_3,
					ASSET_4,
					Permill::from_percent(10),
					100u16,
					Permill::from_percent(0),
					Permill::from_percent(0),
				),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn create_subpool_should_fail_when_asset_b_does_not_exist_in_omnipool() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);

			//Act
			assert_noop!(
				OmnipoolSubpools::create_subpool(
					Origin::root(),
					share_asset_as_pool_id,
					ASSET_3,
					ASSET_4,
					Permill::from_percent(10),
					100u16,
					Permill::from_percent(0),
					Permill::from_percent(0),
				),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn create_subpool_should_work_when_single_pool_is_created() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 3000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//Act
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

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();

			//Assert that liquidity is moved from omnipool account to subpool
			let balance_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let balance_4 = Tokens::free_balance(ASSET_4, &pool_account);
			assert_eq!(balance_3, 2000 * ONE);
			assert_eq!(balance_4, 3000 * ONE);

			let balance_3 = Tokens::free_balance(ASSET_3, &omnipool_account);
			let balance_4 = Tokens::free_balance(ASSET_4, &omnipool_account);
			let balance_shares = Tokens::free_balance(share_asset_as_pool_id, &omnipool_account);
			assert_eq!(balance_3, 0);
			assert_eq!(balance_4, 0);
			assert_eq!(balance_shares, 3250 * ONE);

			assert_that_stableswap_subpool_is_created_with_poolinfo!(
				share_asset_as_pool_id,
				PoolInfo {
					assets: vec![ASSET_3, ASSET_4].try_into().unwrap(),
					amplification: 100,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0),
				}
			);

			assert_that_sharetoken_is_added_to_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 3250 * ONE,
					hub_reserve: 3250 * ONE,
					shares: 3250 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_3,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 2000 * ONE,
					hub_reserve: 1300 * ONE,
					share_tokens: 1300 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_4,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 3000 * ONE,
					hub_reserve: 1950 * ONE,
					share_tokens: 1950 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_3);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_4);

			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id).is_some());

			//TODO: ask Martin - ask martin how to change the procol shares so we have more meaningfull data
			//Once it is done, make sure that all the mutations are killed
		});
}

#[test]
fn create_subpool_should_work_when_multiple_pools_are_created() {
	let share_asset_as_pool_id1: AssetId = 7;
	let share_asset_as_pool_id2: AssetId = 8;
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(ASSET_6)
		.with_registered_asset(share_asset_as_pool_id1)
		.with_registered_asset(share_asset_as_pool_id2)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);
			add_omnipool_token!(ASSET_5);
			add_omnipool_token!(ASSET_6);

			//Act
			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id1,
				ASSET_3,
				ASSET_4,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				share_asset_as_pool_id2,
				ASSET_5,
				ASSET_6,
				Permill::from_percent(10),
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let pool_account2 = AccountIdConstructor::from_assets(&vec![ASSET_5, ASSET_6], None);
			let omnipool_account = Omnipool::protocol_account();

			//Assert that liquidity is moved from omnipool account to subpool
			let subpool_balance_of_asset_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let subpool_balance_of_asset_4 = Tokens::free_balance(ASSET_4, &pool_account);
			assert_eq!(subpool_balance_of_asset_3, 2000 * ONE);
			assert_eq!(subpool_balance_of_asset_4, 3000 * ONE);

			let omnipool_balance_of_asset3 = Tokens::free_balance(ASSET_3, &omnipool_account);
			let omnipool_balance_of_asset4 = Tokens::free_balance(ASSET_4, &omnipool_account);
			assert_eq!(omnipool_balance_of_asset3, 0);
			assert_eq!(omnipool_balance_of_asset4, 0);

			let balance_shares = Tokens::free_balance(share_asset_as_pool_id1, &omnipool_account);
			assert_eq!(balance_shares, 3250 * ONE);

			let subpool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &pool_account2);
			let subpool_balance_of_asset_6 = Tokens::free_balance(ASSET_6, &pool_account2);
			assert_eq!(subpool_balance_of_asset_5, 4000 * ONE);
			assert_eq!(subpool_balance_of_asset_6, 5000 * ONE);

			let omnipool_balance_of_asset_5 = Tokens::free_balance(ASSET_5, &omnipool_account);
			let omnipool_balance_of_asset_6 = Tokens::free_balance(ASSET_6, &omnipool_account);
			assert_eq!(omnipool_balance_of_asset_5, 0);
			assert_eq!(omnipool_balance_of_asset_6, 0);

			let balance_shares = Tokens::free_balance(share_asset_as_pool_id2, &omnipool_account);
			assert_eq!(balance_shares, 5850 * ONE);

			assert_that_stableswap_subpool_is_created_with_poolinfo!(
				share_asset_as_pool_id1,
				PoolInfo {
					assets: vec![ASSET_3, ASSET_4].try_into().unwrap(),
					amplification: 100,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0),
				}
			);

			assert_that_stableswap_subpool_is_created_with_poolinfo!(
				share_asset_as_pool_id2,
				PoolInfo {
					assets: vec![ASSET_5, ASSET_6].try_into().unwrap(),
					amplification: 100,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0),
				}
			);

			assert_that_sharetoken_is_added_to_omnipool_as_another_asset!(
				share_asset_as_pool_id1,
				AssetReserveState::<Balance> {
					reserve: 3250 * ONE,
					hub_reserve: 3250 * ONE,
					shares: 3250 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);

			assert_that_sharetoken_is_added_to_omnipool_as_another_asset!(
				share_asset_as_pool_id2,
				AssetReserveState::<Balance> {
					reserve: 5850 * ONE,
					hub_reserve: 5850 * ONE,
					shares: 5850 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_3,
				share_asset_as_pool_id1,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 2000 * ONE,
					hub_reserve: 1300 * ONE,
					share_tokens: 1300 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_4,
				share_asset_as_pool_id1,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 3000 * ONE,
					hub_reserve: 1950 * ONE,
					share_tokens: 1950 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				share_asset_as_pool_id2,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 4000 * ONE,
					hub_reserve: 2600 * ONE,
					share_tokens: 2600 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_6,
				share_asset_as_pool_id2,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_3);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_4);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_6);

			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id1).is_some());
			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id2).is_some());
		});
}

#[test]
fn create_subpool_should_fail_created_with_same_asset() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);

			//Act
			assert_noop!(
				OmnipoolSubpools::create_subpool(
					Origin::root(),
					share_asset_as_pool_id,
					ASSET_3,
					ASSET_3,
					Permill::from_percent(10),
					100u16,
					Permill::from_percent(0),
					Permill::from_percent(0),
				),
				pallet_stableswap::Error::<Test>::SameAssets
			);
		});
}
