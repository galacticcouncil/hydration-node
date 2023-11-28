use super::mock::*;
use crate::types::AssetPair;
use crate::*;
use frame_support::assert_ok;

#[test]
fn get_liquidity_behind_shares_should_return_both_assets_value_when_pool_exists() {
	let asset_a = ACA;
	let asset_b = DOT;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, 1_000 * ONE), (ALICE, asset_b, 1_000 * ONE)])
		.build()
		.execute_with(|| {
			//arange
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				100 * ONE,
				asset_b,
				10 * ONE
			));

			let pair_account = XYK::get_pair_id(AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			});

			let share_token = XYK::share_token(pair_account);

			let shares_amount = Currency::free_balance(share_token, &ALICE);

			assert_eq!(
				XYK::get_liquidity_behind_shares(asset_a, asset_b, shares_amount).unwrap(),
				(100 * ONE, 10 * ONE)
			);

			assert_eq!(
				XYK::get_liquidity_behind_shares(asset_b, asset_a, shares_amount).unwrap(),
				(10 * ONE, 100 * ONE)
			);

			assert_eq!(
				XYK::get_liquidity_behind_shares(asset_b, asset_a, shares_amount / 2).unwrap(),
				(5 * ONE, 50 * ONE)
			);

			assert_eq!(
				XYK::get_liquidity_behind_shares(asset_a, asset_b, shares_amount / 2).unwrap(),
				(50 * ONE, 5 * ONE)
			);
		});
}
