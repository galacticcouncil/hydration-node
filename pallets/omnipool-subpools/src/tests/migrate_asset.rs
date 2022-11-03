use super::*;

use crate::{
	add_omnipool_token, assert_that_asset_is_migrated_to_omnipool_subpool,
	assert_that_asset_is_not_present_in_omnipool, assert_that_sharetoken_in_omnipool_as_another_asset, AssetDetail,
	Error,
};
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;

#[test]
fn migrate_asset_to_subpool_should_work_when_subpool_exists() {
	let share_asset_as_pool_id: AssetId = 6;
	ExtBuilder::default()
		.with_registered_asset(ASSET_3)
		.with_registered_asset(ASSET_4)
		.with_registered_asset(ASSET_5)
		.with_registered_asset(share_asset_as_pool_id)
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 3000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_5, 4000 * ONE))
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

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				share_asset_as_pool_id,
				ASSET_5,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4, ASSET_5], None);
			let omnipool_account = Omnipool::protocol_account();
			let subpool = Stableswap::get_pool(share_asset_as_pool_id).unwrap();

			assert_eq!(subpool.assets.to_vec(), vec![ASSET_3, ASSET_4, ASSET_5]);

			//Assert that liquidty has been moved
			let balance_a = Tokens::free_balance(ASSET_3, &pool_account);
			let balance_b = Tokens::free_balance(ASSET_4, &pool_account);
			let balance_c = Tokens::free_balance(ASSET_5, &pool_account);
			assert_eq!(balance_a, 2000 * ONE);
			assert_eq!(balance_b, 3000 * ONE);
			assert_eq!(balance_c, 4000 * ONE);

			let balance_a = Tokens::free_balance(ASSET_3, &omnipool_account);
			let balance_b = Tokens::free_balance(ASSET_4, &omnipool_account);
			let balance_c = Tokens::free_balance(ASSET_5, &omnipool_account);
			assert_eq!(balance_a, 0);
			assert_eq!(balance_b, 0);
			assert_eq!(balance_c, 0);

			//Assert that share has been deposited to omnipool
			let balance_shares = Tokens::free_balance(share_asset_as_pool_id, &omnipool_account);
			assert_eq!(balance_shares, 5850 * ONE);

			assert_that_sharetoken_in_omnipool_as_another_asset!(
				share_asset_as_pool_id,
				AssetReserveState::<Balance> {
					reserve: 5850 * ONE,
					hub_reserve: 5850 * ONE,
					shares: 5850 * ONE,
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
					shares: 4000 * ONE,
					hub_reserve: 2600 * ONE,
					share_tokens: 2600 * ONE,
				}
			);

			assert_that_asset_is_not_present_in_omnipool!(ASSET_5);

			//TODO: Adjust test to have non-zero protocol shares
		});
}

#[test]
fn migrate_asset_to_subpool_should_fail_when_subpool_does_not_exist() {}

//TODO: add tests for multiple pools with multiple assets, max number of assets
//TODO: at the end, mutation testing
