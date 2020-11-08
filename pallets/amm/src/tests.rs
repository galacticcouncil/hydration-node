use super::*;
pub use crate::mock::{
	calculate_sale_price, Currency, ExtBuilder, Origin, System, Test, TestEvent, ACA, ALICE, AMM, BOB, DOT, HDX,
};
use frame_support::{assert_noop, assert_ok};
use primitives::traits::AMM as AMMPool;

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}

fn last_events(n: usize) -> Vec<TestEvent> {
	system::Module::<Test>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

fn expect_events(e: Vec<TestEvent>) {
	assert_eq!(last_events(e.len()), e);
}

#[test]
fn create_pool_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000_000,
			Price::from(10)
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

		expect_events(vec![RawEvent::CreatePool(
			ALICE,
			asset_a,
			asset_b,
			100000000000000000000000000000,
		)
		.into()]);
	});
}

#[test]
fn create_same_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_id = ACA;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_id,
			HDX,
			100,
			Price::from(2)
		));
		assert_noop!(
			AMM::create_pool(Origin::signed(user), asset_id, HDX, 100, Price::from(2)),
			Error::<Test>::TokenPoolAlreadyExists
		);
		expect_events(vec![RawEvent::CreatePool(ALICE, asset_id, HDX, 20000).into()]);
	});
}

#[test]
fn add_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_b = HDX;
		let asset_a = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		assert_ok!(AMM::add_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			400_000,
			1_000_000_000_000
		));

		let pair_account = AMM::get_pair_id(&asset_b, &asset_a);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1004000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 100400000000000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 100400000000000000000);

		expect_events(vec![
			RawEvent::CreatePool(ALICE, asset_a, asset_b, 100000000000000000000).into(),
			RawEvent::AddLiquidity(ALICE, asset_a, asset_b, 400000, 4000000000).into(),
		]);
	});
}

#[test]
fn add_liquidity_as_another_user_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_hdx = HDX;
		let asset_id = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_id,
			asset_hdx,
			100_000_000,
			Price::from(10_000)
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

		expect_events(vec![
			RawEvent::CreatePool(ALICE, asset_id, asset_hdx, 100000000000000000000).into(),
			RawEvent::AddLiquidity(ALICE, asset_id, asset_hdx, 400000, 4000000000).into(),
			RawEvent::AddLiquidity(BOB, asset_id, asset_hdx, 1000000, 10000000000).into(),
		]);
	});
}

#[test]
fn remove_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
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

		expect_events(vec![
			RawEvent::CreatePool(ALICE, asset_a, asset_b, 100000000000000000000).into(),
			RawEvent::RemoveLiquidity(ALICE, asset_a, asset_b, 355000000000).into(),
		]);
	});
}

#[test]
fn add_liquidity_more_than_owner_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			HDX,
			ACA,
			200_000_000,
			Price::from(3000000)
		));

		assert_eq!(Currency::free_balance(ACA, &ALICE), 400000000000000);

		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn add_zero_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(AMM::create_pool(Origin::signed(ALICE), HDX, ACA, 100, Price::from(1)));

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
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 0),
			Error::<Test>::CannotRemoveLiquidityWithZero
		);
	});
}

#[test]
fn sell_test() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
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

		assert_ok!(AMM::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			456_444_678,
			1000000000000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999799543555322);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 401363489802256);
		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			120000000000000000000000000
		);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200456444678);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 598636510197744);

		expect_events(vec![
			RawEvent::CreatePool(ALICE, asset_a, asset_b, 120000000000000000000000000).into(),
			RawEvent::Sell(ALICE, asset_a, asset_b, 456444678, 1363489802256).into(),
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
			Price::from(40)
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
			10000000000000,
			false,
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
			10000000000,
			false,
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

		expect_events(vec![
			RawEvent::CreatePool(user_1, asset_a, asset_b, 4900000000000000000000000).into(),
			RawEvent::AddLiquidity(user_2, asset_a, asset_b, 300000000000, 12000000000000).into(),
			RawEvent::Sell(user_2, asset_a, asset_b, 499700000000000, 25966156043470).into(),
			RawEvent::Sell(ALICE, asset_a, asset_b, 899650000000000, 21732802781).into(),
			RawEvent::RemoveLiquidity(user_2, asset_a, asset_b, 120000000000000000).into(),
			RawEvent::RemoveLiquidity(user_2, asset_b, asset_a, 119999988000000000000000).into(),
			RawEvent::RemoveLiquidity(user_2, asset_a, asset_b, 18000).into(),
		]);
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

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			10_000_000,
			Price::from(200)
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

		assert_ok!(AMM::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000,
			1_000_000,
			false,
		));

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
		expect_events(vec![
			RawEvent::CreatePool(user_1, asset_a, asset_b, 20000000000000000).into(),
			RawEvent::Sell(user_1, asset_a, asset_b, 100000, 19762768).into(),
		]);
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

	let mut ext: sp_io::TestExternalities = ExtBuilder::default().with_accounts(accounts).build();
	ext.execute_with(|| System::set_block_number(1));
	ext.execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			HDX,
			5_000,
			Price::from(2)
		));
		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			1000,
			Price::from(2)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let hdx_pair_account = AMM::get_pair_id(&asset_a, &HDX);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 2000);
		assert_eq!(Currency::free_balance(asset_a, &hdx_pair_account), 5000);
		assert_eq!(Currency::free_balance(HDX, &hdx_pair_account), 10000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 994_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998_000);
		assert_eq!(Currency::free_balance(HDX, &user_1), 990_000);

		assert_ok!(AMM::sell(Origin::signed(user_1), asset_a, asset_b, 10_000, 1_500, true,));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 11000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 181);
		assert_eq!(Currency::free_balance(asset_a, &hdx_pair_account), 5000);
		assert_eq!(Currency::free_balance(HDX, &hdx_pair_account), 10000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 984_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999_819);
		assert_eq!(Currency::free_balance(HDX, &user_1), 989_986);

		expect_events(vec![
			RawEvent::CreatePool(user_1, asset_a, HDX, 50000000).into(),
			RawEvent::CreatePool(user_1, asset_a, asset_b, 2000000).into(),
			RawEvent::Sell(user_1, asset_a, asset_b, 10000, 1819).into(),
		]);
	});
}

#[test]
fn single_buy_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			Price::from(3200)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999800000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999360000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640000000000);

		assert_ok!(AMM::buy(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000,
			1_000_000_000_000,
			false,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999900000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998717434869739);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1282565130261);

		expect_events(vec![
			RawEvent::CreatePool(user_1, asset_a, asset_b, 128000000000000000000).into(),
			RawEvent::Buy(user_1, asset_a, asset_b, 100000000, 642565130261).into(),
		]);
	});
}

#[test]
fn single_buy_with_discount_should_work() {
	new_test_ext().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000,
			Price::from(3200)
		));

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			HDX,
			5_000_0000_000,
			Price::from(2)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999949800000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999360000000000);
		assert_eq!(Currency::free_balance(HDX, &user_1), 999900000000000);
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 640000000000);

		assert_ok!(AMM::buy(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000,
			1_000_000_000_000,
			true,
		));

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999949900000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 998719103372360); // compare to values in previous test to see difference!
		assert_eq!(Currency::free_balance(share_token, &user_1), 128000000000000000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1280896627640);
		assert_eq!(Currency::free_balance(HDX, &user_1), 999899999860000);

		expect_events(vec![
			RawEvent::CreatePool(user_1, asset_a, asset_b, 128000000000000000000).into(),
			RawEvent::CreatePool(user_1, asset_a, HDX, 5000000000000000000000).into(),
			RawEvent::Buy(user_1, asset_a, asset_b, 100000000, 640896627640).into(),
		]);
	});
}

#[test]
fn create_pool_with_zero_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::create_pool(Origin::signed(ALICE), ACA, HDX, 0, Price::from(3200)),
			Error::<Test>::CannotCreatePoolWithZeroLiquidity
		);

		assert_noop!(
			AMM::create_pool(Origin::signed(ALICE), ACA, HDX, 10, Price::from(0)),
			Error::<Test>::CannotCreatePoolWithZeroInitialPrice
		);
	});
}

#[test]
fn add_liquidity_to_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::add_liquidity(Origin::signed(ALICE), HDX, ACA, 200_000_000_000_000_000, 600_000_000),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn remove_zero_liquidity_from_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::remove_liquidity(Origin::signed(ALICE), HDX, ACA, 100),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn sell_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::sell(Origin::signed(ALICE), HDX, DOT, 456_444_678, 1_000_000, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_sell_with_no_hdx_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			ACA,
			DOT,
			100,
			Price::from(3200)
		));

		assert_noop!(
			AMM::sell(Origin::signed(ALICE), ACA, DOT, 456_444_678, 1_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn buy_with_non_existing_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			AMM::buy(Origin::signed(ALICE), HDX, DOT, 456_444_678, 1_000_000_000, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn discount_buy_with_no_hdx_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			ACA,
			DOT,
			100,
			Price::from(3200)
		));

		assert_noop!(
			AMM::buy(Origin::signed(ALICE), ACA, DOT, 10, 1_000_000_000, true),
			Error::<Test>::CannotApplyDiscount
		);
	});
}

#[test]
fn create_pool_small_fixed_point_amount_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000_000,
			Price::from_fraction(0.00001)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 900000000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 999999000000000);
		assert_eq!(Currency::free_balance(share_token, &ALICE), 100000000000000000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 100000000000000000000000);

		expect_events(vec![RawEvent::CreatePool(
			ALICE,
			asset_a,
			asset_b,
			100000000000000000000000,
		)
		.into()]);
	});
}

#[test]
fn create_pool_fixed_point_amount_should_work() {
	new_test_ext().execute_with(|| {
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(AMM::create_pool(
			Origin::signed(ALICE),
			asset_a,
			asset_b,
			100_000_000_000,
			Price::from_fraction(4560.234543)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		let share_token = AMM::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 456023454299999);
		assert_eq!(Currency::free_balance(asset_a, &ALICE), 999900000000000);
		assert_eq!(Currency::free_balance(asset_b, &ALICE), 543976545700001);
		assert_eq!(Currency::free_balance(share_token, &ALICE), 45602345429999900000000000);
		assert_eq!(AMM::total_liquidity(&pair_account), 45602345429999900000000000);

		expect_events(vec![RawEvent::CreatePool(
			ALICE,
			asset_a,
			asset_b,
			45602345429999900000000000,
		)
		.into()]);
	});
}

#[test]
fn destry_pool_on_remove_liquidity_and_recreate_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		let pair_account = AMM::get_pair_id(&asset_a, &asset_b);
		assert_eq!(AMM::exists(asset_a, asset_b), true);

		assert_ok!(AMM::remove_liquidity(
			Origin::signed(user),
			asset_a,
			asset_b,
			100000000000000000000
		));

		assert_eq!(AMM::total_liquidity(&pair_account), 0);

		assert_eq!(AMM::exists(asset_a, asset_b), false);

		// It should be possible to recreate the pool again

		assert_ok!(AMM::create_pool(
			Origin::signed(user),
			asset_a,
			asset_b,
			100_000_000,
			Price::from(10_000)
		));

		expect_events(vec![
			RawEvent::CreatePool(user, asset_a, asset_b, 100000000000000000000).into(),
			RawEvent::RemoveLiquidity(user, asset_a, asset_b, 100000000000000000000).into(),
			RawEvent::PoolDestroyed(user, asset_a, asset_b).into(),
			RawEvent::CreatePool(user, asset_a, asset_b, 100000000000000000000).into(),
		]);
	});
}

#[test]
fn create_pool_with_same_assets_should_not_be_allowed() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;

		assert_noop!(
			AMM::create_pool(Origin::signed(user), asset_a, asset_a, 100_000_000, Price::from(10_000)),
			Error::<Test>::CannotCreatePoolWithSameAssets
		);
	})
}

#[test]
fn sell_test_exceeding_max_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let asset_a = ACA;
		let asset_b = DOT;

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
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

		assert_noop!(
			AMM::sell(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000_000_000,
				false,
			),
			Error::<Test>::AssetBalanceLimitExceeded
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

		assert_ok!(AMM::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000,
			Price::from(3000)
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

		assert_noop!(
			AMM::buy(
				Origin::signed(user_1),
				asset_a,
				asset_b,
				456_444_678,
				1_000_000_000,
				false,
			),
			Error::<Test>::AssetBalanceLimitExceeded
		);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999800000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 400000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 600000000000000);
	});
}

#[test]
fn test_calculate_sell_price() {
	ExtBuilder::default().build().execute_with(|| {
		let sell_reserve: Balance = 10000000000000;
		let buy_reserve: Balance = 100000;
		let sell_amount: Balance = 100000000000;
		let result = AMM::calculate_sell_price(sell_reserve, buy_reserve, sell_amount);
		assert_ok!(result);
		assert_eq!(result.unwrap(), 991);
	});
}

#[test]
fn test_calculate_sell_price_invalid() {
	ExtBuilder::default().build().execute_with(|| {
		let sell_reserve: Balance = 0;
		let buy_reserve: Balance = 1000;
		let sell_amount: Balance = 0;
		let result = AMM::calculate_sell_price(sell_reserve, buy_reserve, sell_amount);
		assert_noop!(result, Error::<Test>::SellAssetAmountInvalid);
	});
}

#[test]
fn test_calculate_buy_price_insufficient_pool_balance() {
	ExtBuilder::default().build().execute_with(|| {
		let sell_reserve: Balance = 10000000000000;
		let buy_reserve: Balance = 100000;
		let buy_amount: Balance = 100000000000;
		let result = AMM::calculate_buy_price(sell_reserve, buy_reserve, buy_amount);
		assert_noop!(result, Error::<Test>::InsufficientPoolAssetBalance);
	});
}

#[test]
fn test_calculate_buy_price() {
	ExtBuilder::default().build().execute_with(|| {
		let sell_reserve: Balance = 10000000000000;
		let buy_reserve: Balance = 10000000;
		let buy_amount: Balance = 1000000;
		let result = AMM::calculate_buy_price(sell_reserve, buy_reserve, buy_amount);
		assert_ok!(result);
		assert_eq!(result.unwrap(), 1111111111112);
	});
}
