use super::*;

use crate::AssetDetail;
use crate::{
	add_omnipool_token, assert_balance, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_sharetoken_in_omnipool_as_another_asset,
	assert_that_stableswap_subpool_is_created_with_poolinfo, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::AssetState;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pallet_stableswap::types::PoolInfo;
use pretty_assertions::assert_eq;
use sp_runtime::BoundedVec;

//TODO: use assert_balance macro in all tests

#[test]
fn create_subpool_should_fail_when_called_by_non_origin() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
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
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
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
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
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
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
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
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
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
			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);

			assert_balance!(&omnipool_account, ASSET_3, 0);
			assert_balance!(&omnipool_account, ASSET_4, 0);

			//Assert that share has been deposited to omnipool
			assert_balance!(&omnipool_account, share_asset_as_pool_id, 4550 * ONE);

			assert_that_stableswap_subpool_is_created_with_poolinfo!(
				share_asset_as_pool_id,
				PoolInfo {
					assets: vec![ASSET_3, ASSET_4].try_into().unwrap(),
					amplification: 100,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0),
				}
			);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 4550 * ONE,
					hub_reserve: 4550 * ONE,
					shares: 4550 * ONE,
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
					shares: 3000 * ONE,
					hub_reserve: 1950 * ONE,
					share_tokens: 1950 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_4,
				share_asset_as_pool_id,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 4000 * ONE,
					hub_reserve: 2600 * ONE,
					share_tokens: 2600 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_3);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_4);

			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id).is_some());
		});
}

#[test]
fn protocol_share_calculation_should_work_when_protocol_has_shares() {
	let share_asset_as_pool_id: AssetId = ASSET_5;

	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((ALICE, ASSET_3, 1000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token!(ASSET_3);
			add_omnipool_token!(ASSET_4);

			//We need to add liquidity then sacrificing it because we want to have some protocol shares for having meaningful test result to assert
			let position_id: u32 = Omnipool::next_position_id();
			assert_ok!(OmnipoolSubpools::add_liquidity(
				Origin::signed(ALICE),
				ASSET_3,
				100 * ONE
			));
			assert_ok!(Omnipool::sacrifice_position(Origin::signed(ALICE), position_id));

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
			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 4615 * ONE,
					hub_reserve: 4615 * ONE,
					shares: 4615 * ONE,
					protocol_shares: 130 * ONE,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);
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
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 4000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_6, 6000 * ONE))
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
			assert_balance!(pool_account, ASSET_3, 3000 * ONE);
			assert_balance!(pool_account, ASSET_4, 4000 * ONE);
			assert_balance!(&omnipool_account, ASSET_3, 0);
			assert_balance!(&omnipool_account, ASSET_4, 0);
			assert_balance!(&omnipool_account, share_asset_as_pool_id1, 4550 * ONE);

			assert_balance!(pool_account2, ASSET_5, 5000 * ONE);
			assert_balance!(pool_account2, ASSET_6, 6000 * ONE);
			assert_balance!(&omnipool_account, ASSET_5, 0);
			assert_balance!(&omnipool_account, ASSET_6, 0);
			assert_balance!(&omnipool_account, share_asset_as_pool_id2, 7150 * ONE);

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

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id1,
				AssetReserveState::<Balance> {
					reserve: 4550 * ONE,
					hub_reserve: 4550 * ONE,
					shares: 4550 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id2,
				AssetReserveState::<Balance> {
					reserve: 7150 * ONE,
					hub_reserve: 7150 * ONE,
					shares: 7150 * ONE,
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
					shares: 3000 * ONE,
					hub_reserve: 1950 * ONE,
					share_tokens: 1950 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_4,
				share_asset_as_pool_id1,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 4000 * ONE,
					hub_reserve: 2600 * ONE,
					share_tokens: 2600 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_5,
				share_asset_as_pool_id2,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 5000 * ONE,
					hub_reserve: 3250 * ONE,
					share_tokens: 3250 * ONE,
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool!(
				ASSET_6,
				share_asset_as_pool_id2,
				AssetDetail {
					price: FixedU128::from_float(0.65),
					shares: 6000 * ONE,
					hub_reserve: 3900 * ONE,
					share_tokens: 3900 * ONE,
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
