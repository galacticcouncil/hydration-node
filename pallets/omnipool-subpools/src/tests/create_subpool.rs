use super::*;

use crate::AssetDetail;
use pallet_omnipool::types::{AssetReserveState, Tradability};
use pretty_assertions::assert_eq;

//TODO: Dani - add integration tests for creating pool, adding liq, and trading in it

#[test]
fn create_subpool_should_work_when_single_pool_is_created() {
	//Arrange
	ExtBuilder::default()
		.with_registered_asset(b"1000".to_vec())
		.with_registered_asset(b"2000".to_vec())
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), ASSET_4, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			add_omnipool_token(ASSET_3);
			add_omnipool_token(ASSET_4);

			//Act
			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				ASSET_3,
				ASSET_4,
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			//Assert
			let pool_account = AccountIdConstructor::from_assets(&vec![ASSET_3, ASSET_4], None);
			let omnipool_account = Omnipool::protocol_account();
			let pool_id: AssetId = 5;

			let balance_3 = Tokens::free_balance(ASSET_3, &pool_account);
			let balance_4 = Tokens::free_balance(ASSET_4, &pool_account);

			assert_eq!(balance_3, 2000 * ONE);
			assert_eq!(balance_4, 2000 * ONE);

			let balance_3 = Tokens::free_balance(ASSET_3, &omnipool_account);
			let balance_4 = Tokens::free_balance(ASSET_4, &omnipool_account);
			let balance_shares = Tokens::free_balance(pool_id, &omnipool_account);

			assert_eq!(balance_3, 0);
			assert_eq!(balance_4, 0);
			assert_eq!(balance_shares, 2600 * ONE);

			assert_that_asset_is_not_found_in_omnipool(ASSET_3);
			assert_that_asset_is_not_found_in_omnipool(ASSET_4);

			let pool_asset = Omnipool::load_asset_state(pool_id).unwrap();
			assert_eq!(
				pool_asset,
				AssetReserveState::<Balance> {
					reserve: 2600 * ONE,
					hub_reserve: 2600 * ONE,
					shares: 2600 * ONE,
					protocol_shares: 0,
					cap: 2_000_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);

			assert_that_asset_is_migrated_to_omnipool_subpool(
				ASSET_3,
				pool_id,
				AssetDetail {
					price: Default::default(),
					shares: 2000 * ONE,
					hub_reserve: 1300 * ONE,
					share_tokens: 1300 * ONE,
				},
			);

			assert_that_asset_is_migrated_to_omnipool_subpool(
				ASSET_4,
				pool_id,
				AssetDetail {
					price: Default::default(),
					shares: 2000 * ONE,
					hub_reserve: 1300 * ONE,
					share_tokens: 1300 * ONE,
				},
			);

			//TODO: ask Martin - change from 2000 for 2nd asset to something else to make the test more meaninhgufll, othewise the asset details are the same
		});
}

fn add_omnipool_token(asset_id: AssetId) {
	assert_ok!(Omnipool::add_token(
		Origin::root(),
		asset_id,
		FixedU128::from_float(0.65),
		Permill::from_percent(100),
		LP1
	));
}

fn assert_that_asset_is_not_found_in_omnipool(asset: AssetId) {
	assert_err!(
		Omnipool::load_asset_state(asset),
		pallet_omnipool::Error::<Test>::AssetNotFound
	);
}

fn assert_that_asset_is_migrated_to_omnipool_subpool(asset: AssetId, pool_id: AssetId, asset_details: AssetDetail) {
	let migrate_asset = OmnipoolSubpools::migrated_assets(asset);

	assert!(
		migrate_asset.is_some(),
		"Asset '{}' can not be found in omnipool subpools migrated asset storage",
		asset
	);
	assert_eq!(migrate_asset.unwrap(), (pool_id, asset_details));
}

//TODO: add test for having multiple pools multiple assets
