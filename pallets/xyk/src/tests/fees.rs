pub use super::mock::*;
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::AMM as AmmPool;
use orml_traits::MultiCurrency;

use crate::types::AssetPair;

#[test]
fn fee_calculation() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(XYK::calculate_fee(100_000), Ok(200));
		assert_eq!(XYK::calculate_fee(10_000), Ok(20));

		assert_eq!(XYK::calculate_discounted_fee(9_999), Ok(0));
		assert_eq!(XYK::calculate_discounted_fee(10_000), Ok(7));
		assert_eq!(XYK::calculate_discounted_fee(100_000), Ok(70));
	});
	ExtBuilder::default()
		.with_exchange_fee((10, 1000))
		.with_discounted_fee((10, 1000))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100_000), Ok(1_000));
			assert_eq!(XYK::calculate_fee(10_000), Ok(100));

			assert_eq!(XYK::calculate_discounted_fee(999), Ok(0));
			assert_eq!(XYK::calculate_discounted_fee(1_000), Ok(10));
			assert_eq!(XYK::calculate_discounted_fee(10_000), Ok(100));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 0))
		.build()
		.execute_with(|| {
			assert_eq!(XYK::calculate_fee(100000), Ok(0));
		});

	ExtBuilder::default()
		.with_exchange_fee((10, 1))
		.build()
		.execute_with(|| {
			assert_noop!(XYK::calculate_fee(u128::MAX), Error::<Test>::FeeAmountInvalid);
		});
}

#[test]
fn get_fee_should_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			HDX,
			1_000_000_000,
			DOT,
			2_000_000_000,
		));

		// existing pool
		let fee = XYK::get_fee(&HDX_DOT_POOL_ID);
		assert_eq!(fee, (2, 1_000));
		// non existing pool
		let fee = XYK::get_fee(&1_234);
		assert_eq!(fee, (2, 1_000));
	});
}

#[test]
fn discount_sell_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000_000_000_000u128),
		(ALICE, ACA, 1_000_000_000_000_000u128),
		(ALICE, DOT, 1_000_000_000_000_000u128),
	];

	let asset_a = ACA;
	let asset_b = DOT;

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts.clone()).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 998_000_000_000_000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_500,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 399_999_980_013_994);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 798_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_019_986_006);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 997_999_999_972_014);

		expect_events(vec![Event::SellExecuted {
			who: ALICE,
			asset_in: asset_a,
			asset_out: asset_b,
			amount: 10_000_000,
			sale_price: 19_986_006,
			fee_asset: asset_b,
			fee_amount: 13_993,
			pool: pair_account,
		}
		.into()]);
	});

	// 0.1% discount fee
	let mut ext: sp_io::TestExternalities = ExtBuilder::default()
		.with_accounts(accounts.clone())
		.with_discounted_fee((10, 10_000))
		.build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 998_000_000_000_000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_500,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 399_999_980_019_991);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 798_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_019_980_009);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 997_999_999_960_020);

		expect_events(vec![Event::SellExecuted {
			who: ALICE,
			asset_in: asset_a,
			asset_out: asset_b,
			amount: 10_000_000,
			sale_price: 19_980_009,
			fee_asset: asset_b,
			fee_amount: 19_990,
			pool: pair_account,
		}
		.into()]);
	});

	// zero discount fee
	let mut ext: sp_io::TestExternalities = ExtBuilder::default()
		.with_accounts(accounts)
		.with_discounted_fee((0, 0))
		.build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_500,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 399_999_980_000_001);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 798_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_019_999_999);

		expect_events(vec![Event::SellExecuted {
			who: ALICE,
			asset_in: asset_a,
			asset_out: asset_b,
			amount: 10_000_000,
			sale_price: 19_999_999,
			fee_asset: asset_b,
			fee_amount: 0,
			pool: pair_account,
		}
		.into()]);
	});
}

#[test]
fn discount_buy_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000_000_000_000u128),
		(ALICE, ACA, 1_000_000_000_000_000u128),
		(ALICE, DOT, 1_000_000_000_000_000u128),
	];

	let asset_a = ACA;
	let asset_b = DOT;

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts.clone()).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 998_000_000_000_000);

		assert_ok!(XYK::buy(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_000_000_000_000,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_020_014_002);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 599_999_979_985_998); // compare to values in previous test to see difference!
		assert_eq!(Currency::free_balance(HDX, &ALICE), 997_999_999_972_000);

		expect_events(vec![Event::BuyExecuted {
			who: ALICE,
			asset_out: asset_a,
			asset_in: asset_b,
			amount: 10_000_000,
			buy_price: 20_000_002,
			fee_asset: asset_b,
			fee_amount: 14_000,
			pool: pair_account,
		}
		.into()]);
	});

	// 0.1% discount fee
	let mut ext: sp_io::TestExternalities = ExtBuilder::default()
		.with_accounts(accounts.clone())
		.with_discounted_fee((10, 10_000))
		.build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let native_pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: HDX,
		});

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &ALICE), 998_000_000_000_000);

		assert_ok!(XYK::buy(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_000_000_000_000,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_020_020_002);
		assert_eq!(Currency::free_balance(asset_a, &native_pair_account), 1_000_000_000_000);
		assert_eq!(Currency::free_balance(HDX, &native_pair_account), 2_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 599_999_979_979_998); // compare to values in previous test to see difference!
		assert_eq!(Currency::free_balance(HDX, &ALICE), 997_999_999_960_000);

		expect_events(vec![Event::BuyExecuted {
			who: ALICE,
			asset_out: asset_a,
			asset_in: asset_b,
			amount: 10_000_000,
			buy_price: 20_000_002,
			fee_asset: asset_b,
			fee_amount: 20_000,
			pool: pair_account,
		}
		.into()]);
	});

	// zero discount fee
	let mut ext: sp_io::TestExternalities = ExtBuilder::default()
		.with_accounts(accounts)
		.with_discounted_fee((0, 0))
		.build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			1_000_000_000_000,
			HDX,
			2_000_000_000_000,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			200_000_000_000_000,
			asset_b,
			400_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 600_000_000_000_000);

		assert_ok!(XYK::buy(
			RuntimeOrigin::signed(ALICE),
			asset_a,
			asset_b,
			10_000_000,
			1_000_000_000,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199_999_990_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000_020_000_002);

		assert_eq!(Currency::free_balance(asset_a, &ALICE), 799_000_010_000_000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 599_999_979_999_998);

		expect_events(vec![Event::BuyExecuted {
			who: ALICE,
			asset_out: asset_a,
			asset_in: asset_b,
			amount: 10_000_000,
			buy_price: 20_000_002,
			fee_asset: asset_b,
			fee_amount: 0,
			pool: pair_account,
		}
		.into()]);
	});
}
