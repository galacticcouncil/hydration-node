use super::*;

use pallet_omnipool::types::{AssetReserveState, Tradability};

#[test]
fn create_subpool_should_work() {
	ExtBuilder::default()
		.with_registered_asset(b"1000".to_vec())
		.with_registered_asset(b"2000".to_vec())
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 3, 2000 * ONE))
		.add_endowed_accounts((Omnipool::protocol_account(), 4, 2000 * ONE))
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
			assert_ok!(OmnipoolSubpools::create_subpool(
				Origin::root(),
				3,
				4,
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			let pool_account = AccountIdConstructor::from_assets(&vec![3, 4], None);
			let omnipool_account = Omnipool::protocol_account();

			let balance_3 = Tokens::free_balance(3, &pool_account);
			let balance_4 = Tokens::free_balance(4, &pool_account);

			assert_eq!(balance_3, 2000 * ONE);
			assert_eq!(balance_4, 2000 * ONE);

			let balance_3 = Tokens::free_balance(3, &omnipool_account);
			let balance_4 = Tokens::free_balance(4, &omnipool_account);
			let balance_shares = Tokens::free_balance(5, &omnipool_account);

			assert_eq!(balance_3, 0);
			assert_eq!(balance_4, 0);
			assert_eq!(balance_shares, 2600 * ONE);

			assert_err!(
				Omnipool::load_asset_state(3),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);
			assert_err!(
				Omnipool::load_asset_state(4),
				pallet_omnipool::Error::<Test>::AssetNotFound
			);

			let pool_asset = Omnipool::load_asset_state(5).unwrap();
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
		});
}
