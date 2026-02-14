use super::*;
use crate::types::Tradability;
use frame_support::assert_noop;
use orml_traits::MultiCurrencyExtended;

#[test]
fn add_all_liquidity_works() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let ed = <Test as crate::Config>::Currency::minimum_balance(1_000);
			let expected_amount = 5000 * ONE - ed;

			let position_id = last_position_id();

			assert_ok!(Omnipool::add_all_liquidity(
				RuntimeOrigin::signed(LP1),
				1_000,
				Balance::MIN,
			));

			// LP1 should hold exactly the ED
			assert_balance!(LP1, 1_000, ed);

			// A position NFT was minted for LP1
			let minted = POSITIONS.with(|v| v.borrow().get(&position_id).copied());
			assert_eq!(minted, Some(LP1));

			// Pool received the full amount minus ED
			let state = Assets::<Test>::get(1_000).unwrap();
			assert_eq!(state.shares, 2000 * ONE + expected_amount);
		});
}

#[test]
fn add_all_liquidity_and_add_liquidity_with_limit_produce_same_result() {
	// Verify that add_all_liquidity gives the exact same pool state as
	// an equivalent explicit add_liquidity_with_limit call.
	let run = |use_all: bool| {
		ExtBuilder::default()
			.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
			.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
			.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
			.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
			.build()
			.execute_with(|| {
				let ed = <Test as crate::Config>::Currency::minimum_balance(1_000);
				let amount = 5000 * ONE - ed;

				if use_all {
					assert_ok!(Omnipool::add_all_liquidity(
						RuntimeOrigin::signed(LP1),
						1_000,
						Balance::MIN,
					));
				} else {
					assert_ok!(Omnipool::add_liquidity_with_limit(
						RuntimeOrigin::signed(LP1),
						1_000,
						amount,
						Balance::MIN,
					));
				}

				Assets::<Test>::get(1_000).unwrap()
			})
	};

	assert_eq!(run(true), run(false));
}

#[test]
fn add_all_liquidity_fails_when_balance_is_zero() {
	ExtBuilder::default()
		.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// LP3 has no balance of asset 1_000
			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP3), 1_000, Balance::MIN),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_balance_equals_ed() {
	ExtBuilder::default()
		.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let ed = <Test as crate::Config>::Currency::minimum_balance(1_000);
			// Give LP3 exactly the ED — after subtracting ED the amount becomes zero
			Tokens::update_balance(1_000, &LP3, ed as i128).unwrap();

			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP3), 1_000, Balance::MIN),
				Error::<Test>::InsufficientBalance
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_asset_not_in_pool() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP1), 1_000, Balance::MIN),
				Error::<Test>::AssetNotFound
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_add_liquidity_not_allowed() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 2000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::set_asset_tradable_state(
				RuntimeOrigin::root(),
				1_000,
				Tradability::SELL | Tradability::BUY | Tradability::REMOVE_LIQUIDITY,
			));

			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP1), 1_000, Balance::MIN),
				Error::<Test>::NotAllowed
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_weight_cap_exceeded() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.with_asset_weight_cap(Permill::from_float(0.1))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP1, 100 * ONE)
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP1), 1_000, Balance::MIN),
				Error::<Test>::AssetWeightCapExceeded
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_slippage_limit_not_met() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Drive price away from parity so shares < amount
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 1_000, DAI, 20 * ONE, 0));

			assert_noop!(
				Omnipool::add_all_liquidity(
					RuntimeOrigin::signed(LP1),
					1_000,
					u128::MAX, // unreachable minimum shares
				),
				Error::<Test>::SlippageLimit
			);
		});
}

#[test]
fn add_all_liquidity_fails_when_price_differs_too_much() {
	ExtBuilder::default()
		.add_endowed_accounts((LP1, 1_000, 5000 * ONE))
		.add_endowed_accounts((LP2, 1_000, 5000 * ONE))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(1_000, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.with_max_allowed_price_difference(Permill::from_percent(1))
		.with_external_price_adjustment((3, 100, false))
		.build()
		.execute_with(|| {
			assert_noop!(
				Omnipool::add_all_liquidity(RuntimeOrigin::signed(LP1), 1_000, Balance::MIN),
				Error::<Test>::PriceDifferenceTooHigh
			);
		});
}
