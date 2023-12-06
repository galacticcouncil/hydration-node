pub use super::mock::*;
use crate::types::{AssetPair, Balance};
use crate::{Error, Event};
use frame_support::{assert_noop, assert_ok};
use hydradx_traits::AMM as AmmPool;
use orml_traits::MultiCurrency;

#[test]
fn add_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = DOT;
		let asset_b = HDX;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			65_400_000
		));
		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 65_400_000);
		assert_eq!(XYK::total_liquidity(pair_account), 65400000);

		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user),
			asset_a,
			asset_b,
			400_000,
			1_000_000_000_000
		));

		assert_eq!(Currency::free_balance(share_token, &user), 65661600);

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 65_661_601);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999899600000);
		assert_eq!(XYK::total_liquidity(pair_account), 65661600);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a,
				asset_b,
				initial_shares_amount: 65400000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::LiquidityAdded {
				who: ALICE,
				asset_a,
				asset_b,
				amount_a: 400000,
				amount_b: 261601,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_mints_correct_shares() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = DOT;
		let asset_b = HDX;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			65_400_000
		));

		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user),
			asset_b,
			asset_a,
			261600,
			1_000_000_000_000
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(share_token, &user), 65661600);
	});
}

#[test]
fn add_liquidity_as_another_user_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = ACA;
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_b,
			100_000_000,
			asset_a,
			1_000_000_000_000
		));
		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user),
			asset_b,
			asset_a,
			400_000,
			1_000_000_000_000
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1004000000001);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 100400000);
		assert_eq!(Currency::free_balance(asset_b, &user), 999999899600000);
		assert_eq!(Currency::free_balance(share_token, &user), 1004000000000);
		assert_eq!(XYK::total_liquidity(pair_account), 1004000000000);

		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(BOB),
			asset_b,
			asset_a,
			1_000_000,
			1_000_000_000_000
		));

		assert_eq!(Currency::free_balance(asset_a, &pair_account), 1014000000002);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 101400000);
		assert_eq!(Currency::free_balance(asset_b, &user), 999999899600000);
		assert_eq!(Currency::free_balance(asset_b, &BOB), 999999999000000);
		assert_eq!(Currency::free_balance(share_token, &user), 1004000000000);
		assert_eq!(Currency::free_balance(share_token, &BOB), 10000000000);
		assert_eq!(XYK::total_liquidity(pair_account), 1014000000000);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a: asset_b,
				asset_b: asset_a,
				initial_shares_amount: 1000000000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::LiquidityAdded {
				who: ALICE,
				asset_a: asset_b,
				asset_b: asset_a,
				amount_a: 400000,
				amount_b: 4000000001,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: share_token,
				who: 2,
				amount: 10000000000,
			}
			.into(),
			Event::LiquidityAdded {
				who: BOB,
				asset_a: asset_b,
				asset_b: asset_a,
				amount_a: 1000000,
				amount_b: 10000000001,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_should_work_when_limit_is_set_above_account_balance() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = DOT;
		let asset_b = HDX;
		let amount_b_max_limit = 2_000_000_000_000_000;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			100_000_000,
		));

		assert!(Currency::free_balance(asset_b, &user) < amount_b_max_limit);

		assert_ok!(XYK::add_liquidity(
			RuntimeOrigin::signed(user),
			asset_a,
			asset_b,
			400_000,
			amount_b_max_limit,
		));
	});
}

#[test]
fn remove_liquidity_should_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			1_000_000_000_000
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		assert_eq!(Currency::free_balance(share_token, &user), 100000000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900000000);
		assert_eq!(Currency::free_balance(asset_a, &pair_account), 100000000);
		assert_eq!(Currency::free_balance(asset_b, &pair_account), 1000000000000);

		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(user),
			asset_a,
			asset_b,
			355_000
		));

		assert_eq!(Currency::free_balance(asset_b, &pair_account), 996450000000);
		assert_eq!(Currency::free_balance(asset_a, &user), 999999900355000);

		assert_eq!(Currency::free_balance(share_token, &user), 99645000);
		assert_eq!(XYK::total_liquidity(pair_account), 99645000);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a,
				asset_b,
				initial_shares_amount: 100000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			Event::LiquidityRemoved {
				who: ALICE,
				asset_a,
				asset_b,
				shares: 355_000,
			}
			.into(),
		]);
	});
}

#[test]
fn remove_liquidity_without_shares_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			100_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);
		let shares = Currency::free_balance(share_token, &user);

		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(ALICE),
			BOB,
			share_token,
			shares
		));

		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(user), asset_a, asset_b, 355_000),
			Error::<Test>::InsufficientAssetBalance
		);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a,
				asset_b,
				initial_shares_amount: 100000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			orml_tokens::Event::Endowed {
				currency_id: share_token,
				who: BOB,
				amount: shares,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: share_token,
				from: ALICE,
				to: BOB,
				amount: shares,
			}
			.into(),
		]);
	});
}

// events in the following test do not occur during standard chain operation
#[test]
fn remove_liquidity_from_reduced_pool_should_not_work() {
	new_test_ext().execute_with(|| {
		let user = ALICE;
		let asset_a = HDX;
		let asset_b = DOT;

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(user),
			asset_a,
			100_000_000,
			asset_b,
			100_000_000,
		));

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		// remove some amount from the pool
		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(pair_account),
			BOB,
			asset_a,
			90_000_000
		));

		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(user), asset_a, asset_b, 200_000_000),
			Error::<Test>::InsufficientLiquidity
		);

		// return it back to the pool
		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(BOB),
			pair_account,
			asset_a,
			90_000_000
		));
		// do it again with asset_b
		assert_ok!(Currency::transfer(
			RuntimeOrigin::signed(pair_account),
			BOB,
			asset_b,
			90_000_000
		));

		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(user), asset_a, asset_b, 200_000_000),
			Error::<Test>::InsufficientLiquidity
		);

		let pair_account = XYK::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let share_token = XYK::share_token(pair_account);

		expect_events(vec![
			Event::PoolCreated {
				who: ALICE,
				asset_a,
				asset_b,
				initial_shares_amount: 100000000,
				share_token,
				pool: pair_account,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_a,
				from: pair_account,
				to: BOB,
				amount: 90_000_000,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_a,
				from: BOB,
				to: pair_account,
				amount: 90_000_000,
			}
			.into(),
			orml_tokens::Event::Transfer {
				currency_id: asset_b,
				from: pair_account,
				to: BOB,
				amount: 90_000_000,
			}
			.into(),
		]);
	});
}

#[test]
fn add_liquidity_more_than_owner_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			HDX,
			200_000_000,
			ACA,
			600_000_000_000_000,
		));

		assert_eq!(Currency::free_balance(ACA, &ALICE), 400_000_000_000_000);

		assert_noop!(
			XYK::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				HDX,
				ACA,
				200_000_000_000_000_000,
				600_000_000
			),
			Error::<Test>::InsufficientAssetBalance
		);

		assert_noop!(
			XYK::add_liquidity(
				RuntimeOrigin::signed(ALICE),
				HDX,
				ACA,
				600_000_000,
				200_000_000_000_000_000
			),
			Error::<Test>::InsufficientAssetBalance
		);
	});
}

#[test]
fn add_insufficient_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(RuntimeOrigin::signed(ALICE), HDX, 1000, ACA, 1500,));

		assert_noop!(
			XYK::add_liquidity(RuntimeOrigin::signed(ALICE), HDX, ACA, 0, 0),
			Error::<Test>::InsufficientTradingAmount
		);

		assert_noop!(
			XYK::add_liquidity(RuntimeOrigin::signed(ALICE), HDX, ACA, 1000, 0),
			Error::<Test>::ZeroLiquidity
		);

		assert_noop!(
			XYK::add_liquidity(RuntimeOrigin::signed(BOB), ACA, HDX, 1000, 2000),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn add_liquidity_exceeding_max_limit_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE),
			HDX,
			100_000_000_000_000,
			ACA,
			100_000_000_000_000,
		));

		assert_noop!(
			XYK::add_liquidity(RuntimeOrigin::signed(ALICE), HDX, ACA, 10_000_000, 1_000_000),
			Error::<Test>::AssetAmountExceededLimit
		);
	});
}
#[test]
fn remove_liquidity_should_respect_min_pool_limit() {
	new_test_ext().execute_with(|| {
		assert_ok!(XYK::create_pool(RuntimeOrigin::signed(ALICE), HDX, 1000, ACA, 1500,));

		assert_ok!(XYK::add_liquidity(RuntimeOrigin::signed(BOB), ACA, HDX, 2000, 2000));

		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(BOB), ACA, HDX, 500),
			Error::<Test>::InsufficientLiquidity
		);
	});
}

#[test]
fn remove_zero_liquidity_should_not_work() {
	new_test_ext().execute_with(|| {
		assert_noop!(
			XYK::remove_liquidity(RuntimeOrigin::signed(ALICE), HDX, ACA, 0),
			Error::<Test>::ZeroLiquidity
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
fn add_liquidity_overflow_work() {
	let user = ALICE;
	let asset_a = DOT;
	let asset_b = HDX;
	ExtBuilder::default()
		.with_accounts(vec![(ALICE, DOT, Balance::MAX), (ALICE, HDX, Balance::MAX)])
		.build()
		.execute_with(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(user),
				asset_a,
				100_000,
				asset_b,
				10_u128.pow(38)
			));

			assert_noop!(
				XYK::add_liquidity(
					RuntimeOrigin::signed(user),
					asset_a,
					asset_b,
					10_u128.pow(33),
					1_000_000_000_000
				),
				Error::<Test>::AddAssetAmountInvalid
			);
		});
}

#[test]
fn share_ratio_calculations_are_correct() {
	ExtBuilder::default()
		.with_exchange_fee((0, 0))
		.build()
		.execute_with(|| {
			let asset_a = HDX;
			let asset_b = DOT;

			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE),
				asset_a,
				100 * ONE,
				asset_b,
				65_440_000_000_000,
			));

			assert_eq!(Currency::free_balance(asset_a, &BOB), 1_000 * ONE);
			assert_eq!(Currency::free_balance(asset_b, &BOB), 1_000 * ONE);

			let balance_a = Currency::free_balance(asset_a, &BOB);
			let balance_b = Currency::free_balance(asset_b, &BOB);

			let bob_initial_balance = balance_a + balance_b;

			assert_eq!(bob_initial_balance, 2000 * ONE);

			assert_ok!(XYK::add_liquidity(
				RuntimeOrigin::signed(BOB),
				asset_b,
				asset_a,
				10 * ONE,
				200 * ONE
			));

			let pair_account = XYK::get_pair_id(AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			});
			let share_token = XYK::share_token(pair_account);

			let expected_shares = 15_281_173_594_132u128;

			assert_eq!(Currency::free_balance(share_token, &BOB), expected_shares);

			assert_ok!(XYK::sell(
				RuntimeOrigin::signed(CHARLIE),
				asset_a,
				asset_b,
				10 * ONE,
				0u128,
				false,
			));

			assert_ok!(XYK::remove_liquidity(
				RuntimeOrigin::signed(BOB),
				asset_a,
				asset_b,
				expected_shares
			));

			assert_eq!(Currency::free_balance(share_token, &BOB), 0);

			for _ in 0..10 {
				let balance_a = Currency::free_balance(asset_a, &BOB);
				let balance_b = Currency::free_balance(asset_b, &BOB);

				let bob_previous_balance = balance_a + balance_b;

				let balance_pool_a = Currency::free_balance(asset_a, &pair_account);
				let balance_pool_b = Currency::free_balance(asset_a, &pair_account);

				let initial_pool_liquidity = balance_pool_a + balance_pool_b;

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_b,
					asset_a,
					10 * ONE,
					200 * ONE
				));

				let shares = Currency::free_balance(share_token, &BOB);

				assert_ok!(XYK::remove_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					shares
				));
				let balance_a = Currency::free_balance(asset_a, &BOB);
				let balance_b = Currency::free_balance(asset_b, &BOB);
				let bob_new_balance = balance_a + balance_b;

				let balance_pool_a = Currency::free_balance(asset_a, &pair_account);
				let balance_pool_b = Currency::free_balance(asset_a, &pair_account);

				let total_pool_liquidity = balance_pool_a + balance_pool_b;

				assert!(bob_new_balance <= bob_previous_balance);
				assert!(initial_pool_liquidity <= total_pool_liquidity);
			}
		});
}
