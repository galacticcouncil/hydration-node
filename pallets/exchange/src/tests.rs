use super::*;
use crate::mock::*;
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};
use primitives::Price;
use sp_runtime::FixedPointNumber;

const ENDOWED_AMOUNT: u128 = 1_000_000_000_000_000;

/// HELPER FOR INITIALIZING POOLS
fn initialize_pool(asset_a: u32, asset_b: u32, user: u64, amount: u128, price: Price) {
	assert_ok!(AMMModule::create_pool(
		Origin::signed(user),
		asset_a,
		asset_b,
		amount,
		price
	));

	let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
	let share_token = AMMModule::share_token(pair_account);

	let amount_b = price.saturating_mul_int(amount);

	// Check users state
	assert_eq!(Currency::free_balance(asset_a, &user), ENDOWED_AMOUNT - amount);
	assert_eq!(Currency::free_balance(asset_b, &user), ENDOWED_AMOUNT - amount_b);

	// Check initial state of the pool
	assert_eq!(Currency::free_balance(asset_a, &pair_account), amount);
	assert_eq!(Currency::free_balance(asset_b, &pair_account), amount_b);

	// Check pool shares
	assert_eq!(Currency::free_balance(share_token, &user), amount * amount_b);
}

#[test]
fn sell_test_pool_finalization_states() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			false
		));
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			20_000_000_000_000_000_000_000_000_000
		);

		// Balance should not change yet
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100_000_000_000_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_003_972_238_015_089);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_000_996_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances
		// TODO: CHECK IF RIGHT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101_004_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198_027_761_984_911);

		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			20_000_000_000_000_000_000_000_000_000
		);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_standard() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			false
		));
		assert_ok!(Exchange::buy(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_003_972_238_015_089);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_000_996_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 101_004_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 198_027_761_984_911);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost
		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_inverse_standard() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			4_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances  -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_001_992_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_001_986_138_378_978);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 996_000_000_000_000);

		// Check final pool balances  -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 99_013_861_621_022);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 202_008_000_000_000);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_exact_match() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			2_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_001_996_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_000_998_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100002000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200004000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_single_eth_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_a,
			asset_b,
			2_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1_001_899_978_143_094);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 998_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1003913878975647);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 103_000_000_000_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 194_186_142_881_259);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_single_dot_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;
		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			2_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1_000_496_522_353_457);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999_000_000_000_000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1_000_978_388_447_963);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 998_000_000_000_000);

		// Check final pool balances -> SEEMS LEGIT
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 98_525_089_198_580);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 203_000_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_single_multiple_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let user_5 = FERDIE;
		let user_6 = GEORGE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_5),
			asset_b,
			asset_a,
			1_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_6),
			asset_b,
			asset_a,
			2_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 5);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1001996000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000498000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 999000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1001991034974081);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100002512534115);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200012965025919);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_group_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1002480000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 995000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001702547361447);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018915629970700);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 105817452638553);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 189084370029300);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}
#[test]
fn sell_without_pool_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 100, false),
			Error::<Test>::TokenPoolNotFound
		);
	});
}

#[test]
fn sell_more_than_owner_should_not_work() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(AMMModule::create_pool(
			Origin::signed(ALICE),
			HDX,
			ETH,
			200_000,
			Price::from(2)
		));

		assert_noop!(
			Exchange::sell(Origin::signed(ALICE), HDX, ETH, 1000_000_000_000_000u128, false),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn sell_test_mixed_buy_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 996968733621972);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001480000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018630903108670);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111551266378028);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179369096891330);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn discount_tests_no_discount() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			false
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 996968733621972);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1005000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001480000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 990000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1018630903108670);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 111551266378028);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 179369096891330);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn discount_tests_with_discount() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let user_4 = DAVE;
		let asset_a = ETH;
		let asset_b = DOT;

		let pool_amount = 100_000_000_000_000;
		let initial_price = Price::from(2);

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);

		initialize_pool(asset_a, asset_b, user_1, pool_amount, initial_price);

		// Make sell intentions
		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			5_000_000_000_000,
			true
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			3_000_000_000_000,
			true
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_4),
			asset_a,
			asset_b,
			10_000_000_000_000,
			true
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1001480000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 997000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_4), 998500000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_4), 1002994000000000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100020000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 200006000000000);

		assert_eq!(Currency::free_balance(HDX, &user_1), 1000000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_2), 1000000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_3), 1000000000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}
