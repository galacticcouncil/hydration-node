use super::*;
use crate::mock::*;
use frame_support::traits::OnFinalize;
use frame_support::{assert_noop, assert_ok};
use primitives::Price;

#[test]
fn sell_test() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_2),
			asset_a,
			asset_b,
			2_000_000_000_000,
			false
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			1_000_000_000_000,
			false
		));

		assert_eq!(
			Exchange::get_intentions((asset_a, asset_b)).len() + Exchange::get_intentions((asset_b, asset_a)).len(),
			2
		);

		// Balance should not change yet
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 998000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000999999064353);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000499000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999000000000000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1501000200000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1335647);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_a, asset_b)), 0);
	});
}

#[test]
fn sell_test_case_two() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intention
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 4000, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999999000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000001987);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000001996);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999996000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199004);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 402013);

		// TODO: check if final transferred balances add up to initial balance
		// No tokens should be created or lost

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

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

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 2000, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);
		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999999000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000001987);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000001003);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999998000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199_997);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_013);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

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

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_a, asset_b, 2000, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Balance should not change yet
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999999000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000001948);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 999999999998000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000000000003953);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 203_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 394_099);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

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

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_b, asset_a, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 2000, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 2);

		// Balance should not change yet
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances
		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000498);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999999999999000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000989);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999998000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 198_513);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 403_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_single_multiple_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_1), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_b, asset_a, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_1), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_b, asset_a, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 2000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 2000, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 6);

		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999798000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999603981);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000001000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999999999998000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000001996);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999996000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 199_004);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 402_019);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn sell_test_group_sells() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_1), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::sell(Origin::signed(user_2), asset_b, asset_a, 500, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 300, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);
		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999799000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999601992);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000000000000250);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 999999999999500);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000152);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999999700);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_598);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 398_808);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

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
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_1), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::buy(Origin::signed(user_2), asset_b, asset_a, 500, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 300, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);
		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000_000_000_000_000u128);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 1000_000_000_000_000u128);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999799000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999601991);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999999747);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000500);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000150);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999999700);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 201_103);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 397_809);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn discount_tests_no_discount() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000,
			Price::from(2)
		));

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_2),
			asset_a,
			HDX,
			200_000,
			Price::from(2)
		));

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_3),
			asset_b,
			HDX,
			200_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400_000);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		// Make sell intentions
		assert_ok!(Exchange::sell(Origin::signed(user_1), asset_a, asset_b, 1000, false));
		assert_ok!(Exchange::buy(Origin::signed(user_2), asset_b, asset_a, 500, false));
		assert_ok!(Exchange::sell(Origin::signed(user_3), asset_b, asset_a, 300, false));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Balance should not change yet

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999800_000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999600_000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999800000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999800000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200_000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 999999999799000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 999999999601991);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 999999999799747);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000500);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000150);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 999999999799700);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 201_103);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 397_809);

		assert_eq!(Currency::free_balance(share_token, &user_1), 80_000_000_000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}

#[test]
fn discount_tests_with_discount() {
	ExtBuilder::default().build().execute_with(|| {
		let user_1 = ALICE;
		let user_2 = BOB;
		let user_3 = CHARLIE;
		let asset_a = ETH;
		let asset_b = DOT;

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			200_000_000_000_000,
			Price::from(2)
		));

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_2),
			asset_a,
			HDX,
			200_000_000_000_000,
			Price::from(2)
		));

		assert_ok!(AMMModule::create_pool(
			Origin::signed(user_3),
			asset_b,
			HDX,
			200_000_000_000_000,
			Price::from(2)
		));

		let pair_account = AMMModule::get_pair_id(&asset_a, &asset_b);
		let share_token = AMMModule::share_token(pair_account);

		// Check initial state of the pool
		assert_eq!(Currency::free_balance(asset_a, &user_1), 800000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 400000000000000);

		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			80000000000000000000000000000
		);

		// Make sell intentions
		assert_ok!(Exchange::sell(
			Origin::signed(user_1),
			asset_a,
			asset_b,
			100_000_000_000_000,
			true
		));
		assert_ok!(Exchange::buy(
			Origin::signed(user_2),
			asset_b,
			asset_a,
			500_000_000,
			true
		));
		assert_ok!(Exchange::sell(
			Origin::signed(user_3),
			asset_b,
			asset_a,
			300_000_000,
			true
		));

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 3);

		// Balance should not change yet

		assert_eq!(Currency::free_balance(HDX, &user_1), 1000000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_2), 600000000000000);
		assert_eq!(Currency::free_balance(HDX, &user_3), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_1), 800000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 600000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 800000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000000000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 800000000000000);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 200000000000000);

		// Finalize block
		<Exchange as OnFinalize<u64>>::on_finalize(9);

		// Check final account balances

		assert_eq!(Currency::free_balance(asset_a, &user_1), 700000000000000);
		assert_eq!(Currency::free_balance(asset_b, &user_1), 733271262753542);

		assert_eq!(Currency::free_balance(asset_a, &user_2), 799999437237099);
		assert_eq!(Currency::free_balance(asset_b, &user_2), 1000000500000000);

		assert_eq!(Currency::free_balance(asset_a, &user_3), 1000000149700000);
		assert_eq!(Currency::free_balance(asset_b, &user_3), 799999700000000);

		// Check final pool balances
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 300000413062901);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 266728537246458);

		assert_eq!(
			Currency::free_balance(share_token, &user_1),
			80000000000000000000000000000
		);

		assert_eq!(Currency::free_balance(HDX, &user_1), 999860000210000);
		assert_eq!(Currency::free_balance(HDX, &user_2), 599999999300000);
		assert_eq!(Currency::free_balance(HDX, &user_3), 600000000000000);

		assert_eq!(Exchange::get_intentions_count((asset_b, asset_a)), 0);
	});
}
