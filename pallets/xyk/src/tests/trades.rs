pub use super::mock::*;
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::AMM as AmmPool;
use orml_traits::MultiCurrency;

use crate::types::AssetPair;

#[test]
fn sell_test() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000_000,
			asset_b,
			600_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			456_444_678,
			1000000000000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363483591788);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636516408212);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a,
				asset_b,
				initial_shares_amount: 600000000000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::SellExecuted {
				who: ALICE,
				asset_in: asset_a,
				asset_out: asset_b,
				amount: 456444678,
				sale_price: 1363483591788,
				fee_asset: asset_b,
				fee_amount: 2732432046,
				pool: pair_account,
			}
			.into(),
		]);
	});
}

#[test]
fn work_flow_happy_path_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = HDX;
		let asset_b = ACA;

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		// Check initial balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 0);

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			350_000_000_000,
			asset_b,
			14_000_000_000_000,
		));

		// User 1 really tries!
		assert_noop!(
			XYK::add_liquidity(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				800_000_000_000_000_000,
				100
			),
			Error::<Test>::InsufficientAssetBalance
		);

		// Total liquidity
		assert_eq!(XYK::total_liquidity(pair_account), 350_000_000_000);

		let share_token = XYK::share_token(pair_account);

		// Check balance after add liquidity for user 1 and user 2

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_000_000_000_000_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 0);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 350_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 14_000_000_000_000);

		// User 2 adds liquidity
		let current_b_balance = Currency::free_balance(asset_b, &user_2);
		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			300_000_000_000,
			current_b_balance
		));

		assert_eq!(XYK::total_liquidity(pair_account), 650_000_000_000);

		// Check balance after add liquidity for user 1 and user 2
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_700_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 988_000_000_000_000 - 1); // - 1 because of liquidity_in rounds up in favor of pool

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 26_000_000_000_001);

		// User 2 SELLs
		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			216_666_666_666,
			100_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_650_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_483_333_333_334);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 994_486_999_999_986);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 866_666_666_666);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 19_513_000_000_014);

		// User 1 SELLs
		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			288_888_888_888,
			100_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_361_111_111_112);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 990_868_493_499_997);

		let user_2_original_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_original_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_original_balance_1, 999_483_333_333_334);
		assert_eq!(user_2_original_balance_2, 994_486_999_999_986);

		assert_eq!(Currency::free_balance(share_token, &user_1), 350_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 300_000_000_000);

		// User 2 removes liquidity

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			10_000
		));

		let user_2_remove_1_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_remove_1_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_remove_1_balance_1, 999_483_333_351_111);
		assert_eq!(user_2_remove_1_balance_2, 994_487_000_225_286);
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_990_000);

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_b,
			asset_a,
			10_000
		));

		let user_2_remove_2_balance_1 = Currency::free_balance(asset_a, &user_2);
		let user_2_remove_2_balance_2 = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_2_remove_2_balance_1, 999_483_333_368_888);
		assert_eq!(user_2_remove_2_balance_2, 994_487_000_450_586);
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_980_000);

		// The two removes should be equal (this could slip by 1 because of rounding error)

		assert_eq!(
			user_2_remove_1_balance_1 - user_2_original_balance_1,
			user_2_remove_2_balance_1 - user_2_remove_1_balance_1
		);

		assert_eq!(
			user_2_remove_1_balance_2 - user_2_original_balance_2,
			user_2_remove_2_balance_2 - user_2_remove_1_balance_2
		);

		assert_eq!(XYK::total_liquidity(pair_account), 649_999_980_000);

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			18_000
		));
		assert_eq!(Currency::free_balance(share_token, &user_2), 299_999_962_000);

		assert_eq!(XYK::total_liquidity(pair_account), 649_999_962_000);

		expect_events(vec![
			Event::PoolCreated {
				who: user_1,
				asset_a,
				asset_b,
				initial_shares_amount: 350_000_000_000,
				share_token,
				pool: pair_account,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: share_token,
				who: 2,
				amount: 300000000000,
			}
			.into(),
			Event::LiquidityAdded {
				who: user_2,
				asset_a,
				asset_b,
				amount_a: 300_000_000_000,
				amount_b: 12_000_000_000_001,
			}
			.into(),
		]);
	});
}

#[test]
fn sell_with_correct_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000_000_000_000u128),
		(BOB, HDX, 1_000_000_000_000_000u128),
		(ALICE, ACA, 1_000_000_000_000_000u128),
		(BOB, ACA, 1_000_000_000_000_000u128),
		(ALICE, DOT, 1_000_000_000_000_000u128),
		(BOB, DOT, 1_000_000_000_000_000u128),
	];

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = ACA;
		let asset_b = HDX;

		// Verify initial balances
		assert_eq!(Currency::free_balance(asset_a, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_b, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_000_000_000_000_000);

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			10_000_000,
			asset_b,
			2_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999990000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999998000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2000000000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 2000000000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			100_000,
			1_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10100000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1980237622);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999989900000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999998019762378);
		expect_events(vec![
			Event::PoolCreated {
				who: user_1,
				asset_a,
				asset_b,
				initial_shares_amount: 2000000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::SellExecuted {
				who: user_1,
				asset_in: asset_a,
				asset_out: asset_b,
				amount: 100_000,
				sale_price: 19_762_378,
				fee_asset: asset_b,
				fee_amount: 39_602,
				pool: pair_account,
			}
			.into(),
		]);
	});
}

#[test]
fn sell_without_sufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			1_000_000_000,
			asset_b,
			1_000_000_000,
		));

		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(user),
			BOB,
			ACA,
			999_998_999_999_999
		));

		assert_noop!(
			XYK::sell(RuntimeOrigin::signed(user), ACA, DOT, 1_000, 100, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn sell_without_sufficient_discount_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			1_000_000_000_000,
			asset_b,
			1_000_000_000_000,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			1_000_000_000_000,
			HDX,
			1_000_000_000_000,
		));

		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(user),
			BOB,
			HDX,
			998_999_999_999_999
		));

		assert_noop!(
			XYK::sell(RuntimeOrigin::signed(user), ACA, DOT, 1_000_000_000, 100, true),
			Error::<Test>::InsufficientNativeCurrencyBalance
		);
	});
}

#[test]
fn buy_without_sufficient_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			1_000_000_000,
			asset_b,
			1_000_000_000,
		));

		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(user),
			BOB,
			ACA,
			999_998_999_999_999
		));

		assert_noop!(
			XYK::buy(RuntimeOrigin::signed(user), DOT, ACA, 1_000, 10_000, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn buy_without_sufficient_discount_balance_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			1_000_000_000_000,
			asset_b,
			1_000_000_000_000,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_b,
			1_000_000_000_000,
			HDX,
			1_000_000_000_000,
		));

		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(user),
			BOB,
			HDX,
			998_999_999_999_999
		));

		assert_noop!(
			XYK::buy(
				RuntimeOrigin::signed(user),
				DOT,
				ACA,
				1_000_000_000,
				10_000_000_000,
				true
			),
			Error::<Test>::InsufficientNativeCurrencyBalance
		);
	});
}

#[test]
fn single_buy_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000,
			asset_b,
			640_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_800_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_360_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640_000_000_000);

		assert_ok!(XYK::buy(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			6_666_666,
			1_000_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_806_666_666);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_337_886_898_839);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 193_333_334);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 662_113_101_161);

		expect_events(vec![
			Event::PoolCreated {
				who: user_1,
				asset_a,
				asset_b,
				initial_shares_amount: 640_000_000_000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::BuyExecuted {
				who: user_1,
				asset_out: asset_a,
				asset_in: asset_b,
				amount: 6_666_666,
				buy_price: 22_068_963_235,
				fee_asset: asset_b,
				fee_amount: 44_137_926,
				pool: pair_account,
			}
			.into(),
		]);
	});
}

#[test]
fn create_pool_with_insufficient_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::create_pool(RuntimeOrigin::signed(ALICE), ACA, 500, HDX, 1_600_000),
			Error::<Test>::InsufficientLiquidity
		);

		assert_noop!(
			XYK::create_pool(RuntimeOrigin::signed(ALICE), ACA, 5000, HDX, 500),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn add_liquidity_to_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				HDX,
				ACA,
				200_000_000_000_000_000,
				600_000_000
			),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn remove_zero_liquidity_from_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(ALICE), HDX, ACA, 100),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn sell_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::sell(RuntimeOrigin::signed(ALICE), HDX, DOT, 456_444_678, 1_000_000, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_sell_with_no_native_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			ACA,
			1000,
			DOT,
			3_200_000
		));

		assert_noop!(
			XYK::sell(RuntimeOrigin::signed(ALICE), ACA, DOT, 456_444_678, 1_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn buy_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::buy(
				RuntimeOrigin::signed(ALICE),
				HDX,
				DOT,
				456_444_678,
				1_000_000_000,
				false
			),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_buy_with_no_native_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			ACA,
			10_000,
			DOT,
			32_000_000
		));

		assert_noop!(
			XYK::buy(RuntimeOrigin::signed(ALICE), ACA, DOT, 1000, 1_000_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn money_in_sell_money_out_should_leave_the_same_balance() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		let user_1_balance_a_before = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_before = Currency::free_balance(asset_b, &user_1);

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000_000,
			asset_b,
			600_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			456_444_678,
			1000000000000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363483591788);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636516408212);

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			600000000000000
		));

		let user_1_balance_a_after = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_after = Currency::free_balance(asset_b, &user_1);

		assert_eq!(user_1_balance_a_before, user_1_balance_a_after);
		assert_eq!(user_1_balance_b_before, user_1_balance_b_after);
	});
}

#[test]
fn money_in_money_out_should_leave_the_same_balance_for_both_accounts() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = HDX;
		let asset_b = DOT;

		let user_1_balance_a_before = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_before = Currency::free_balance(asset_b, &user_1);
		let user_2_balance_a_before = Currency::free_balance(asset_a, &user_2);
		let user_2_balance_b_before = Currency::free_balance(asset_b, &user_2);

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			100_000_000,
			asset_b,
			1_000_000_000_000,
		));

		let asset_pair = AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		};

		let pair_account = XYK::get_pair_id(asset_pair);
		let share_token = XYK::share_token(pair_account);

		assert!(XYK::exists(asset_pair));

		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			100_000_000,
			1_100_000_000_000
		));

		assert_eq!(Currency::free_balance(share_token, &user_1), 100_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 100_000_000);

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000
		));

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user_2),
			asset_a,
			asset_b,
			100_000_000
		));

		assert_eq!(XYK::total_liquidity(pair_account), 0);

		let user_1_balance_a_after = Currency::free_balance(asset_a, &user_1);
		let user_1_balance_b_after = Currency::free_balance(asset_b, &user_1);
		let user_2_balance_a_after = Currency::free_balance(asset_a, &user_2);
		let user_2_balance_b_after = Currency::free_balance(asset_b, &user_2);

		assert_eq!(user_1_balance_a_before, user_1_balance_a_after);
		assert_eq!(user_1_balance_b_before, user_1_balance_b_after);
		assert_eq!(user_2_balance_a_before, user_2_balance_a_after);
		assert_eq!(user_2_balance_b_before, user_2_balance_b_after);

		assert!(!XYK::exists(asset_pair));
	});
}

#[test]
fn sell_test_not_reaching_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000_000,
			asset_b,
			600_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_noop!(
			XYK::sell(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000_000_000,
				false,
			),
			Error::<Test>::AssetAmountNotReachedLimit
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);
	});
}

#[test]
fn buy_test_exceeding_max_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000_000,
			asset_b,
			600_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_noop!(
			XYK::buy(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000,
				false,
			),
			Error::<Test>::AssetAmountExceededLimit
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);
	});
}

#[test]
fn single_buy_more_than_ratio_out_should_not_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000,
			asset_b,
			640_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_999_800_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_360_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 640_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640_000_000_000);

		assert_noop!(
			XYK::buy(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				66_666_667,
				1_000_000_000_000,
				false,
			),
			Error::<Test>::MaxOutRatioExceeded
		);
	});
}

#[test]
fn single_buy_more_than_ratio_in_should_not_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			100_000_000_000,
			asset_b,
			100_000_000_000
		));

		assert_noop!(
			XYK::buy(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				33_333_333_333,
				1_000_000_000_000,
				false,
			),
			Error::<Test>::MaxInRatioExceeded
		);
	});
}

#[test]
fn single_sell_more_than_ratio_in_should_not_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			200_000_000_000,
			asset_b,
			600_000_000_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999_800_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400_000_000_000_000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 600_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600_000_000_000_000);

		assert_noop!(
			XYK::sell(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				66_666_666_667,
				10_000_000,
				false,
			),
			Error::<Test>::MaxInRatioExceeded
		);
	});
}

#[test]
fn single_sell_more_than_ratio_out_should_not_work() {
	ExtBuilder::default().with_max_out_ratio(5).build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user_1),
			asset_a,
			100_000_000_000,
			asset_b,
			100_000_000_000
		));

		assert_noop!(
			XYK::sell(
				RuntimeOrigin::signed(user_1),
				asset_a,
				asset_b,
				33_333_333_333,
				10_000_000,
				false,
			),
			Error::<Test>::MaxOutRatioExceeded
		);
	});
}

#[test]
fn sell_with_low_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::sell(RuntimeOrigin::signed(ALICE), HDX, DOT, 1, 1_000_000, false),
			Error::<Test>::InsufficientTradingAmount
		);
	});
}

#[test]
fn buy_with_low_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::buy(RuntimeOrigin::signed(ALICE), HDX, DOT, 1, 1_000_000, false),
			Error::<Test>::InsufficientTradingAmount
		);
	});
}

#[test]
fn buy_with_excesive_amount_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(RuntimeOrigin::signed(ALICE), HDX, 10_000, DOT, 10_000,));

		assert_noop!(
			XYK::buy(RuntimeOrigin::signed(ALICE), HDX, DOT, 20_000, 1_000_000, false),
			Error::<Test>::InsufficientPoolAssetBalance
		);
	});
}
