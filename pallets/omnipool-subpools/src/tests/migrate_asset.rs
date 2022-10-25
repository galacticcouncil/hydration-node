use super::*;

use pallet_omnipool::types::{AssetReserveState, Tradability};

#[test]
fn migrate_asset_to_subpool_should_work_when_subpool_exists() {
	ExtBuilder::default()
		.with_registered_asset(b"1000".to_vec())
		.with_registered_asset(b"2000".to_vec())
		.with_registered_asset(b"3000".to_vec())
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 4, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 5, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			let token_price = FixedU128::from_float(0.65);

			let token_amount = 2000 * ONE;

			assert_ok!(Omnipool::add_token(
				Origin::root(),
				3,
				token_price,
				Permill::from_percent(100),
				LP1
			));
			assert_ok!(Omnipool::add_token(
				Origin::root(),
				4,
				token_price,
				Permill::from_percent(100),
				LP1
			));
			assert_ok!(Omnipool::add_token(
				Origin::root(),
				5,
				token_price,
				Permill::from_percent(100),
				LP1
			));
			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				3,
				4,
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));
			let asset_a: AssetId = 3;
			let asset_b: AssetId = 4;
			let asset_c: AssetId = 5;
			let pool_id: AssetId = 6;

			assert_ok!(OmnipoolSubpools::migrate_asset_to_subpool(
				Origin::root(),
				pool_id,
				asset_c,
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![3, 4], None);
			let omnipool_account = Omnipool::protocol_account();

			let subpool = Stableswap::get_pool(pool_id).unwrap();

			assert_eq!(subpool.assets.to_vec(), vec![asset_a, asset_b, asset_c]);

			let balance_a = Tokens::free_balance(asset_a, &pool_account);
			let balance_b = Tokens::free_balance(asset_b, &pool_account);
			let balance_c = Tokens::free_balance(asset_c, &pool_account);

			assert_eq!(balance_a, 2000 * ONE);
			assert_eq!(balance_b, 2000 * ONE);
			assert_eq!(balance_c, 2000 * ONE);
			let balance_a = Tokens::free_balance(asset_a, &omnipool_account);
			let balance_b = Tokens::free_balance(asset_b, &omnipool_account);
			let balance_c = Tokens::free_balance(asset_c, &omnipool_account);
			let balance_shares = Tokens::free_balance(pool_id, &omnipool_account);

			assert_eq!(balance_a, 0);
			assert_eq!(balance_b, 0);
			assert_eq!(balance_c, 0);
			assert_eq!(balance_shares, 3900 * ONE);
			assert_err!(
				Omnipool::load_asset_state(asset_c),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);

			let pool_asset = Omnipool::load_asset_state(pool_id).unwrap();
			assert_eq!(
				pool_asset,
				AssetReserveState::<Balance> {
					reserve: 3900 * ONE,
					hub_reserve: 3900 * ONE,
					shares: 3900 * ONE,
					protocol_shares: 0,
					cap: 3_000_000_000_000_000_000,
					tradable: Tradability::default(),
				}
			);
		});
}
