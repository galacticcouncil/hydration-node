use super::mock::*;
use crate::*;

use proptest::prelude::*;

use frame_support::assert_ok;
use primitive_types::U256;
use sp_runtime::{FixedPointNumber, FixedU128};

const TOLERANCE: Balance = 1_000;

#[macro_export]
macro_rules! assert_eq_approx {
	( $x:expr, $y:expr, $z:expr, $r:expr) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\n left: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

fn asset_reserve() -> impl Strategy<Value = Balance> {
	1000 * ONE..10_000_000 * ONE
}

fn trade_amount() -> impl Strategy<Value = Balance> {
	ONE..100 * ONE
}

fn price() -> impl Strategy<Value = f64> {
	0.1f64..2f64
}

fn assert_asset_invariant(
	old_state: (Balance, Balance),
	new_state: (Balance, Balance),
	tolerance: FixedU128,
	desc: &str,
) {
	let new_s = U256::from(new_state.0) * U256::from(new_state.1);
	let s1 = new_s.integer_sqrt();

	let old_s = U256::from(old_state.0) * U256::from(old_state.1);
	let s2 = old_s.integer_sqrt();

	assert!(new_s >= old_s, "Invariant decreased for {desc}");

	let s1_u128 = Balance::try_from(s1).unwrap();
	let s2_u128 = Balance::try_from(s2).unwrap();

	let invariant = FixedU128::from((s1_u128, ONE)) / FixedU128::from((s2_u128, ONE));
	assert_eq_approx!(invariant, FixedU128::from(1u128), tolerance, desc);
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn add_liquidity(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		price in price(),
	) {
		let asset_a = HDX;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});
				let share_token = XYK::share_token(pool_account);

				let pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				let bob_balance_a = Currency::free_balance(asset_a, &BOB);
				let bob_balance_b = Currency::free_balance(asset_b, &BOB);

				let issuance = XYK::total_liquidity(pool_account);

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));

				let new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				let new_bob_balance_a = Currency::free_balance(asset_a, &BOB);
				let new_bob_balance_b = Currency::free_balance(asset_b, &BOB);

				let bob_shares = Currency::free_balance(share_token, &BOB);

				let p0 = FixedU128::from((pool_balance_a, pool_balance_b));
				let p1 = FixedU128::from((new_pool_balance_a, new_pool_balance_b));

				// Price should not change
				assert_eq_approx!(
					p0,
					p1,
					FixedU128::from_float(0.0000000001),
					"Price has changed after add liquidity"
				);

				// The following must hold when adding liquidity.
				// delta_S / S <= delta_X / X
				// delta_S / S <= delta_Y / Y
				// where S is total share issuance, X is asset a and Y is asset b

				let s = U256::from(issuance);
				let delta_s = U256::from(bob_shares);
				let delta_x = U256::from(bob_balance_a - new_bob_balance_a);
				let delta_y = U256::from(bob_balance_b - new_bob_balance_b);
				let x = U256::from(pool_balance_a);
				let y = U256::from(pool_balance_b);

				let left = delta_s * x;
				let right = s * delta_x;

				assert!(left <= right);

				let l = FixedU128::from((bob_shares, issuance));
				let r = FixedU128::from((bob_balance_a - new_bob_balance_a, pool_balance_a));

				let diff = r - l;

				assert!(diff <= FixedU128::from_float(0.000000001));

				let left = delta_s * y;
				let right = s * delta_y;

				assert!(left <= right);

				let l = FixedU128::from((bob_shares, issuance));
				let r = FixedU128::from((bob_balance_b - new_bob_balance_b, pool_balance_b));

				let diff = r - l;

				assert!(diff <= FixedU128::from_float(0.000000001));
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn remove_liquidity(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		price in price(),
	) {
		let asset_a = HDX;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});
				let share_token = XYK::share_token(pool_account);

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));
				let pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				let bob_balance_a = Currency::free_balance(asset_a, &BOB);
				let bob_balance_b = Currency::free_balance(asset_b, &BOB);

				let bob_shares = Currency::free_balance(share_token, &BOB);

				let issuance = XYK::total_liquidity(pool_account);

				assert_ok!(XYK::remove_liquidity(
						RuntimeOrigin::signed(BOB),
						asset_a,
						asset_b,
						bob_shares,
				));

				let new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				let new_bob_balance_a = Currency::free_balance(asset_a, &BOB);
				let new_bob_balance_b = Currency::free_balance(asset_b, &BOB);

				let p0 = FixedU128::from((pool_balance_a, pool_balance_b));
				let p1 = FixedU128::from((new_pool_balance_a, new_pool_balance_b));

				// Price should not change
				assert_eq_approx!(
					p0,
					p1,
					FixedU128::from_float(0.0000000001),
					"Price has changed after remove liquidity"
				);

				let s = U256::from(issuance);
				let delta_s = U256::from(bob_shares);
				let delta_x = U256::from(new_bob_balance_a - bob_balance_a);
				let delta_y = U256::from(new_bob_balance_b - bob_balance_b);
				let x = U256::from(pool_balance_a);
				let y = U256::from(pool_balance_b);

				let left = delta_s * x;
				let right = s * delta_x;

				assert!(left >= right);

				let l = FixedU128::from((bob_shares, issuance));
				let r = FixedU128::from((new_bob_balance_a - bob_balance_a, pool_balance_a));

				let diff = l - r;

				assert!(diff <= FixedU128::from_float(0.000000001));

				let left = delta_s * y;
				let right = s * delta_y;

				assert!(left >= right);

				let l = FixedU128::from((bob_shares, issuance));
				let r = FixedU128::from((new_bob_balance_b - bob_balance_b, pool_balance_b));

				let diff = l - r;

				assert!(diff <= FixedU128::from_float(0.000000001))
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_invariant(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		amount in trade_amount(),
		price in price(),
	) {
		let asset_a = HDX;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
				(CHARLIE, asset_a, amount),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));
				let pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				assert_ok!(XYK::sell(
						RuntimeOrigin::signed(CHARLIE),
						asset_a,
						asset_b,
						amount,
						0u128, // limit not interesting here,
						false,
				));

				let new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				 assert_asset_invariant((pool_balance_a, pool_balance_b),
					(new_pool_balance_a, new_pool_balance_b),
					FixedU128::from((TOLERANCE,ONE)),
					"sell"
				);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_invariant(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		amount in trade_amount(),
		price in price(),
	) {
		let asset_a = ACA;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity * 1000),
				(ALICE, HDX,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
				(CHARLIE, asset_a, amount * 1_000),
				(CHARLIE, HDX, amount * 1_000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));
				let pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				assert_ok!(XYK::buy(
						RuntimeOrigin::signed(CHARLIE),
						asset_b,
						asset_a,
						amount,
						u128::MAX, // limit not interesting here,
						false,
				));

				let new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				 assert_asset_invariant((pool_balance_a, pool_balance_b),
					(new_pool_balance_a, new_pool_balance_b),
					FixedU128::from((TOLERANCE,ONE)),
					"buy"
				);

			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn buy_invariant_with_discount(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		amount in trade_amount(),
		price in price(),
	) {
		let asset_a = ACA;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_discounted_fee((0,0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity * 1000),
				(ALICE, HDX,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
				(CHARLIE, asset_a, amount * 1_000),
				(CHARLIE, HDX, amount * 1_000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_b,
					10 * ONE,
					HDX,
					10 * ONE,
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));
				let _pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let _pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				assert_ok!(XYK::buy(
						RuntimeOrigin::signed(CHARLIE),
						asset_b,
						asset_a,
						amount,
						u128::MAX, // limit not interesting here,
						true,
				));

				let _new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let _new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				 assert_asset_invariant((_pool_balance_a, _pool_balance_b),
					(_new_pool_balance_a, _new_pool_balance_b),
					FixedU128::from((TOLERANCE,ONE)),
					"buy with discount"
				);
			});
	}
}

proptest! {
	#![proptest_config(ProptestConfig::with_cases(1000))]
	#[test]
	fn sell_invariant_with_discount(initial_liquidity in asset_reserve(),
		added_liquidity in asset_reserve(),
		amount in trade_amount(),
		price in price(),
	) {
		let asset_a = ACA;
		let asset_b = DOT;

		ExtBuilder::default()
			.with_exchange_fee((0, 0))
			.with_discounted_fee((0,0))
			.with_accounts(vec![
				(ALICE, asset_a,initial_liquidity * 1000),
				(ALICE, HDX,initial_liquidity),
				(ALICE, asset_b,initial_liquidity * 1000),
				(BOB, asset_a, added_liquidity),
				(BOB, asset_b, added_liquidity * 1_000_000),
				(CHARLIE, asset_a, amount * 1_000),
				(CHARLIE, HDX, amount * 1_000),
			])
			.build()
			.execute_with(|| {
				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					initial_liquidity,
					asset_b,
					FixedU128::from_float(price).saturating_mul_int(initial_liquidity),
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(ALICE),
					asset_a,
					10 * ONE,
					HDX,
					10 * ONE,
				));

				let pool_account = XYK::get_pair_id(AssetPair {
					asset_in: asset_a,
					asset_out: asset_b,
				});

				assert_ok!(XYK::add_liquidity(
					RuntimeOrigin::signed(BOB),
					asset_a,
					asset_b,
					added_liquidity,
					added_liquidity * 1_000_000, // do not care about the limit here
				));
				let _pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let _pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				assert_ok!(XYK::sell(
						RuntimeOrigin::signed(CHARLIE),
						asset_a,
						asset_b,
						amount,
						0u128, // limit not interesting here,
						true,
				));

				let _new_pool_balance_a = Currency::free_balance(asset_a, &pool_account);
				let _new_pool_balance_b = Currency::free_balance(asset_b, &pool_account);

				 assert_asset_invariant((_pool_balance_a, _pool_balance_b),
					(_new_pool_balance_a, _new_pool_balance_b),
					FixedU128::from((TOLERANCE,ONE)),
					"sell with discount"
				);
			});
	}
}
