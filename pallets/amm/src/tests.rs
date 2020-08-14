use super::*;
use crate::mock::{calculate_sale_price, Currency, ExtBuilder, Origin, Test, ACA, ALICE, AMM, BOB, DOT, HDX};
use frame_support::{assert_noop, assert_ok};

#[test]
fn create_pool_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000_000,
			10
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 900000000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 0);
		assert_eq!(
			Currency::free_balance(share_token, &ALICE),
			100000000000000000000000000000
		);
		assert_eq!(AMM::total_liquidity(&pair_account), 100000000000000000000000000000);
	});
}

#[test]
fn create_same_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user = ALICE;
		let asset_id = ACA;

		assert_ok!(AMM::create_pool(Origin::signed(user), asset_id, HDX, 100, 2));
		assert_noop!(
			AMM::create_pool(Origin::signed(user), asset_id, HDX, 100, 2),
			Error::<Test>::TokenPoolAlreadyExists
		);
	});
}

#[test]
fn create_pool_and_add_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			HDX,
			DOT,
			150_000_000_000,
			2
		));

		assert_ok!(AMM::add_liquidity(
			Origin::signed(ALICE),
			HDX,
			DOT,
			2_000_000_000,
			5_000_000_000
		));
	});
}

#[test]
fn overflow_should_panic() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::create_pool(
				Origin::signed(ALICE),
				HDX,
				DOT,
				u128::MAX,
				2
			),
			Error::<Test>::CreatePoolAssetAmountInvalid
		);
	});
}

#[test]
fn add_liquidity_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user = ALICE;
		let asset_b = HDX;
		let asset_a = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			10_000
		));

		assert_ok!(AMM::add_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			400_000,
			4_000_000_000
		));

		let pair_account = AMM::get_pair_id(&asset_b, &asset_a);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1004000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 100400000000000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 100400000000000000000);
	});
}

#[test]
fn add_liquidity_as_another_user_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user = ALICE;
		let asset_hdx = HDX;
		let asset_id = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_id,
			asset_hdx,
			100_000_000,
			10_000
		));
		assert_ok!(AMM::add_liquidity(
			Origin::signed(user),
			asset_id,
			asset_hdx,
			400_000,
			1_000_000_000_000
		));

		let pair_account = AMM::get_pair_id(&asset_hdx, &asset_id);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_hdx, &pair_account), 1004000000000);
		assert_eq!(Currency::free_balance(asset_id, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_id, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 100400000000000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 100400000000000000000);

		assert_ok!(AMM::add_liquidity(
			Origin::signed(BOB),
			asset_id,
			asset_hdx,
			1_000_000,
			1_000_000_000_000
		));

		assert_eq!(Currency::free_balance(asset_hdx, &pair_account), 1014000000000);
		assert_eq!(Currency::free_balance(asset_id, &pair_account), 101400000);
		assert_eq!(Currency::free_balance(asset_id, &user), 999999899600000);
		assert_eq!(Currency::free_balance(asset_id, &BOB), 999999999000000);
		assert_eq!(Currency::free_balance(share_token, &user), 100400000000000000000);
		assert_eq!(Currency::free_balance(share_token, &BOB), 1000000000000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 101400000000000000000);
	});
}

#[test]
fn remove_liquidity_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			10_000
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(share_token, &user), 100000000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000000);

		assert_ok!(AMM::remove_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			355_000_000_000
		));

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 999999996450);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900000000);

		assert_eq!(Currency::free_balance(share_token, &user), 99999999645000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 99999999645000000000);
	});
}

#[test]
fn add_liquidity_more_than_owner_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMM::create_pool(Origin::signed(ALICE), HDX, ACA, 200_000_000, 3000000));

		assert_eq!(Currency::free_balance(ACA, &ALICE), 400000000000000);

		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn add_zero_liquidity_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMM::create_pool(Origin::signed(ALICE), HDX, ACA, 100, 1));

		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 0, 0),
			Error::<Test>::CannotAddZeroLiquidity
		);

		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 100, 0),
			Error::<Test>::CannotAddZeroLiquidity
		);
	});
}

#[test]
fn remove_zero_liquidity_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 0),
			Error::<Test>::CannotRemoveLiquidityWithZero
		);
	});
}

#[test]
fn sell_test() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			3000
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);
		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			120000000000000000000000000
		);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);

		assert_ok!(AMM::sell(Origin::signed(user_1), asset_a, asset_b, 456_444_678, false));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363489802256);
		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			120000000000000000000000000
		);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636510197744);
	});
}

#[test]
fn work_flow_happy_path_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = HDX;
		let asset_b = ACA;

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);

		// Check initial balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 0);

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			350_000_000_000,
			40
		));

		// User 1 really tries!
		assert_noop!(
			AMM::add_liquidity(Origin::signed(user_1), asset_a, asset_b, 800_000_000_000_00_0000, 100),
			Error::<Test>::InsufficientAssetBalance
		);

		// Total liquidity
		assert_eq!(AMM::total_liquidity(&pair_account), 4900000000000000000000000);

		let share_token = AMM::share_token(pair_account);

		// Check balance after add liquidity for user 1 and user 2

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999650000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 4900000000000000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 0);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 350000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 14000000000000);

		// User 2 adds liquidity
		let current_b_balance = Currency::free_balance(asset_b, &user_2);
		assert_ok!(AMM::add_liquidity(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			300_000_000_000,
			current_b_balance
		));

		assert_eq!(AMM::total_liquidity(&pair_account), 9100000000000000000000000);

		// Check balance after add liquidity for user 1 and user 2
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999650000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999700000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 988000000000000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 4900000000000000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 4200000000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 650000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 26000000000000);

		// User 2 SELLs

		let asset_a_reserve = Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = Currency::free_balance(asset_b, &pair_account);

		assert_ok!(AMM::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			499700000000000,
			false
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999650000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 500000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1013966156043470);

		assert_eq!(Currency::free_balance(share_token, &user_1), 4900000000000000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 4200000000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 500350000000000);
		assert_eq!(
			Currency::free_balance(asset_b, &pair_account),
			asset_b_reserve - calculate_sale_price(asset_a_reserve, asset_b_reserve, 499700000000000)
		);

		// User 1 SELLs

		let asset_a_reserve = Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = Currency::free_balance(asset_b, &pair_account);

		assert_ok!(AMM::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			899650000000000,
			false
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 986021732802781);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 500000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1013966156043470);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1400000000000000);
		assert_eq!(
			Currency::free_balance(asset_b, &pair_account),
			asset_b_reserve - calculate_sale_price(asset_a_reserve, asset_b_reserve, 899650000000000)
		);

		assert_eq!(Currency::free_balance(share_token, &user_1), 4900000000000000000000000);
		assert_eq!(Currency::free_balance(share_token, &user_2), 4200000000000000000000000);

		// User 2 removes liquidity

		assert_ok!(AMM::remove_liquidity(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			120000000000000000
		));

		assert_eq!(Currency::free_balance(asset_a, &user_2), 500000018461538);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1013966156043629);
		assert_eq!(Currency::free_balance(share_token, &user_2), 4199999880000000000000000);

		assert_ok!(AMM::remove_liquidity(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			119999988000000000000000
		));

		assert_eq!(Currency::free_balance(asset_a, &user_2), 518461555076922);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1013966315751135);
		assert_eq!(Currency::free_balance(share_token, &user_2), 4079999892000000000000000);

		assert_eq!(AMM::total_liquidity(&pair_account), 8979999892000000000000000);

		assert_ok!(AMM::remove_liquidity(Origin::signed(user_2), asset_a, asset_b, 18000));
		assert_eq!(Currency::free_balance(share_token, &user_2), 4079999891999999999982000);

		assert_eq!(AMM::total_liquidity(&pair_account), 8979999891999999999982000);
	});
}

#[test]
fn sell_with_correct_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1000_000_000_000_000u128),
		(BOB, HDX, 1000_000_000_000_000u128),
		(ALICE, ACA, 1000_000_000_000_000u128),
		(BOB, ACA, 1000_000_000_000_000u128),
		(ALICE, DOT, 1000_000_000_000_000u128),
		(BOB, DOT, 1000_000_000_000_000u128),
	];

	ExtBuilder::default().with_accounts(accounts).build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let asset_a = ACA;
		let asset_b = HDX;

		// Verify initial balances
		assert_eq!(Currency::free_balance(asset_a, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_b, &user_1), 1_000_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_000_000_000_000_000);

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			10_000_000,
			200
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		let asset_a_reserve = Currency::free_balance(asset_a, &pair_account);
		let asset_b_reserve = Currency::free_balance(asset_b, &pair_account);
		let user_asset_b_amount = Currency::free_balance(asset_b, &user_1);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999990000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999998000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2000000000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 20000000000000000);

		assert_ok!(AMM::sell(Origin::signed(user_1), asset_a, asset_b, 100_000, false));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 10100000);
		assert_eq!(
			Currency::free_balance(asset_b, &pair_account),
			asset_b_reserve - calculate_sale_price(asset_a_reserve, asset_b_reserve, 100_000)
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999989900000);
		assert_eq!(
			Currency::free_balance(asset_b, &user_1),
			user_asset_b_amount + calculate_sale_price(asset_a_reserve, asset_b_reserve, 100_000)
		);
	});
}
#[test]
fn discount_sell_fees_should_work() {
	let accounts = vec![
		(ALICE, HDX, 1_000_000u128),
		(BOB, HDX, 1_000_000u128),
		(ALICE, ACA, 1_000_000u128),
		(BOB, ACA, 1_000_000u128),
		(ALICE, DOT, 1_000_000u128),
		(BOB, DOT, 1_000u128),
	];

	ExtBuilder::default().with_accounts(accounts).build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(Origin::signed(user_1), asset_a, HDX, 5_000, 2));
		assert_ok!(AMM::create_pool(Origin::signed(user_1), asset_a, asset_b, 1000, 2));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let hdx_pair_account = AMM::get_pair_id(&asset_a, &HDX);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2000);
		assert_eq!(Currency::free_balance(asset_a, &hdx_pair_account), 5000);
		assert_eq!(Currency::free_balance(HDX, &hdx_pair_account), 10000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 994_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998_000);
		assert_eq!(Currency::free_balance(HDX, &user_1), 990_000);

		assert_ok!(AMM::sell(Origin::signed(user_1), asset_a, asset_b, 10_000, true));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 11000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 181);
		assert_eq!(Currency::free_balance(asset_a, &hdx_pair_account), 5000);
		assert_eq!(Currency::free_balance(HDX, &hdx_pair_account), 10000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 984_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_819);
		assert_eq!(Currency::free_balance(HDX, &user_1), 989_986);
	});
}

#[test]
fn single_buy_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			3200
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999800000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999360000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640000000000);

		assert_ok!(AMM::buy(Origin::signed(user_1), asset_a, asset_b, 100_000_000, false));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999900000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998717434869739);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1282565130261);
	});
}

#[test]
fn single_buy_with_discount_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			3200
		));

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			HDX,
			5_000_0000_000,
			2
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999949800000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999360000000000);
		assert_eq!(Currency::free_balance(HDX, &user_1), 999900000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640000000000);

		assert_ok!(AMM::buy(Origin::signed(user_1), asset_a, asset_b, 100_000_000, true));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999949900000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998719103372360); // compare to values in previous test to see difference!
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1280896627640);
		assert_eq!(Currency::free_balance(HDX, &user_1), 999899999860000);
	});
}

#[test]
fn create_pool_with_zero_liquidity_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::create_pool(Origin::signed(ALICE), ACA, HDX, 0, 3200),
			Error::<Test>::CannotCreatePoolWithZeroLiquidity
		);

		assert_noop!(
			AMM::create_pool(Origin::signed(ALICE), ACA, HDX, 10, 0),
			Error::<Test>::CannotCreatePoolWithZeroInitialPrice
		);
	});
}

#[test]
fn add_liquidity_to_non_existing_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn remove_zero_liquidity_from_non_existing_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 100),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn sell_with_non_existing_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::sell(Origin::signed(ALICE), HDX, DOT, 456_444_678, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_sell_with_no_hdx_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMM::create_pool(Origin::signed(ALICE), ACA, DOT, 100, 3200));

		assert_noop!(
			AMM::sell(Origin::signed(ALICE), ACA, DOT, 456_444_678, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn buy_with_non_existing_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			AMM::buy(Origin::signed(ALICE), HDX, DOT, 456_444_678, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_buy_with_no_hdx_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMM::create_pool(Origin::signed(ALICE), ACA, DOT, 100, 3200));

		assert_noop!(
			AMM::buy(Origin::signed(ALICE), ACA, DOT, 10, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}
