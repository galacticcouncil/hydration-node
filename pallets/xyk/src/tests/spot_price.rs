use super::mock::*;
use crate::types::{AssetPair, Price};
use crate::XYKSpotPrice;
use crate::*;
use frame_support::assert_ok;
use frame_support::dispatch::RawOrigin;
use hydradx_traits::pools::SpotPriceProvider;

#[test]
fn spot_price_provider_should_return_correct_price_when_pool_exists() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial = 99_000_000_000_000u128;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial), (ALICE, asset_b, initial)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial,
				asset_b,
				39_600_000_000_000
			));

			let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

			assert_eq!(price, Some(Price::from_float(2.5))); // 99_000 / 39_600 = 2.5
		});
}

#[test]
fn spot_price_provider_should_return_none_when_pool_does_not_exist() {
	let asset_a = ACA;
	let asset_b = DOT;

	ExtBuilder::default().build().execute_with(|| {
		let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

		assert_eq!(price, None);
	});
}

#[test]
fn spot_price_provider_should_return_none_when_asset_reserve_is_zero() {
	let asset_a = ACA;
	let asset_b = DOT;

	let initial = 99_000_000_000_000u128;

	ExtBuilder::default()
		.with_accounts(vec![(ALICE, asset_a, initial), (ALICE, asset_b, initial)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				initial,
				asset_b,
				39_600_000_000_000
			));

			let pool_account = XYK::get_pair_id(AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			});

			// Force the pool balance to be zero in this test
			assert_ok!(Currency::set_balance(
				RawOrigin::Root.into(),
				pool_account,
				asset_b,
				0u128,
				0u128
			));

			let price = XYKSpotPrice::<Test>::spot_price(asset_a, asset_b);

			assert_eq!(price, None);
		});
}
