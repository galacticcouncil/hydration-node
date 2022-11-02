use super::*;

use crate::AssetDetail;
use crate::{
	add_omnipool_token, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_stableswap_subpool_is_created_with_poolinfo, Error,
};
use frame_support::error::BadOrigin;
use pallet_omnipool::types::AssetState;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pallet_stableswap::types::PoolInfo;
use pretty_assertions::assert_eq;
use sp_runtime::BoundedVec;

//TODO: Dani - add integration tests for creating pool, adding liq, and trading in it

//use withRegAddress like here  https://github.com/galacticcouncil/HydraDX-node/blob/cf2958f29717387154c28db98f4c4f6a2cc5c8da/pallets/omnipool/src/tests/buy.rs#L15

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

			assert_that_asset_is_not_present_in_omnipool!(ASSET_3);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_4);

			assert_that_stableswap_subpool_is_created_with_poolinfo!(
				share_asset_as_pool_id,
				PoolInfo {
					assets: vec![ASSET_3, ASSET_4].try_into().unwrap(),
					amplification: 100,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0),
				}
			);

			let pool_asset = Omnipool::load_asset_state(share_asset_as_pool_id).unwrap();
			assert_eq!(
				pool_asset,
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

			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id).is_some());

			//TODO: add test for having multiple pools multiple assets
			//TODO: add test for adding a subpool with the same asset as an existing one

			//TODO: ask Martin - change from 2000 for 2nd asset to something else to make the test more meaninhgufll, othewise the asset details are the same
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

			assert_that_asset_is_not_present_in_omnipool!(ASSET_3);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_4);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);
			assert_that_asset_is_not_present_in_omnipool!(ASSET_6);

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

			let pool_asset = Omnipool::load_asset_state(share_asset_as_pool_id1).unwrap();
			assert_eq!(
				pool_asset,
				AssetReserveState::<Balance> {
					reserve: 3250 * ONE,
					hub_reserve: 3250 * ONE,
					shares: 3250 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);
			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id1).is_some());

			let pool_asset = Omnipool::load_asset_state(share_asset_as_pool_id2).unwrap();
			assert_eq!(
				pool_asset,
				AssetReserveState::<Balance> {
					reserve: 5850 * ONE,
					hub_reserve: 5850 * ONE,
					shares: 5850 * ONE,
					protocol_shares: 0,
					cap: 1_000_000_000_000_000_00,
					tradable: Tradability::default(),
				}
			);
			assert!(OmnipoolSubpools::subpools(share_asset_as_pool_id2).is_some());

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
		});
}

#[macro_export]
macro_rules! add_omnipool_token {
	($asset_id:expr) => {
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			$asset_id,
			FixedU128::from_float(0.65),
			Permill::from_percent(100),
			LP1
		));
	};
}

#[macro_export]
macro_rules! assert_that_asset_is_not_present_in_omnipool {
	($asset_id:expr) => {
		assert_err!(
			Omnipool::load_asset_state($asset_id),
			pallet_omnipool::Error::<Test>::AssetNotFound
		);
	};
}

#[macro_export]
macro_rules! assert_that_asset_is_migrated_to_omnipool_subpool {
	($asset:expr, $pool_id:expr, $asset_details:expr) => {
		let migrate_asset = OmnipoolSubpools::migrated_assets($asset);

		assert!(
			migrate_asset.is_some(),
			"Asset '{}' can not be found in omnipool subpools migrated asset storage",
			$asset
		);
		assert_eq!(
			migrate_asset.unwrap(),
			($pool_id, $asset_details),
			"asset details for asset `{}` is not as expected",
			$asset
		);
	};
}

#[macro_export]
macro_rules! assert_that_stableswap_subpool_is_created_with_poolinfo {
	($pool_id:expr, $pool_info:expr) => {
		let stableswapPool = Stableswap::pools($pool_id);
		assert!(
			stableswapPool.is_some(),
			"subpool with id {} is not found in stableswap pools",
			$pool_id
		);
		assert_eq!(
			stableswapPool.unwrap(),
			$pool_info,
			"subpool with id {} has different PoolInfo than expected",
			$pool_id
		);
	};
}
