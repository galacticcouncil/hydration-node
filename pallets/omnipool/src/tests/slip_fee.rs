use super::*;
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;

use crate::types::SlipFeeConfig;

#[test]
fn set_slip_fee_works() {
	ExtBuilder::default().build().execute_with(|| {
		assert!(SlipFee::<Test>::get().is_none());

		assert_ok!(Omnipool::set_slip_fee(
			RuntimeOrigin::root(),
			Some(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(5),
			})
		));

		let config = SlipFee::<Test>::get().unwrap();
		assert_eq!(config.max_slip_fee, Permill::from_percent(5));

		// Disable
		assert_ok!(Omnipool::set_slip_fee(RuntimeOrigin::root(), None));
		assert!(SlipFee::<Test>::get().is_none());
	});
}

#[test]
fn set_slip_fee_unauthorized_fails() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			Omnipool::set_slip_fee(
				RuntimeOrigin::signed(LP1),
				Some(SlipFeeConfig {
					max_slip_fee: Permill::from_percent(5),
				})
			),
			sp_runtime::DispatchError::BadOrigin
		);
	});
}

#[test]
fn sell_with_slip_fee_disabled_is_noop() {
	// Run a sell without slip fees and verify the output matches
	let mut amount_without_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// SlipFee is None by default
			assert!(SlipFee::<Test>::get().is_none());

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_without_slip = Tokens::free_balance(HDX, &LP1);
		});

	let mut amount_with_slip_disabled = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			// Explicitly no slip fee
			assert!(SlipFee::<Test>::get().is_none());

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_with_slip_disabled = Tokens::free_balance(HDX, &LP1);
		});

	assert_eq!(amount_without_slip, amount_with_slip_disabled);
}

#[test]
fn sell_with_slip_fee_enabled_reduces_output() {
	let mut amount_without_slip = 0u128;
	let mut amount_with_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_without_slip = Tokens::free_balance(HDX, &LP1);
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_with_slip = Tokens::free_balance(HDX, &LP1);
		});

	// With slip fees, output should be less
	assert!(
		amount_with_slip < amount_without_slip,
		"Slip fee should reduce output: without_slip={} with_slip={}",
		amount_without_slip,
		amount_with_slip
	);
}

#[test]
fn buy_with_slip_fee_enabled_increases_cost() {
	let mut cost_without_slip = 0u128;
	let mut cost_with_slip = 0u128;

	let buy_amount = 10 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let before = Tokens::free_balance(100, &LP1);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				HDX,
				100,
				buy_amount,
				u128::MAX
			));
			let after = Tokens::free_balance(100, &LP1);
			cost_without_slip = before - after;
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			let before = Tokens::free_balance(100, &LP1);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP1),
				HDX,
				100,
				buy_amount,
				u128::MAX
			));
			let after = Tokens::free_balance(100, &LP1);
			cost_with_slip = before - after;
		});

	assert!(
		cost_with_slip > cost_without_slip,
		"Slip fee should increase buy cost: without_slip={} with_slip={}",
		cost_without_slip,
		cost_with_slip
	);
}

#[test]
fn sell_hub_asset_with_slip_reduces_output() {
	let mut amount_without_slip = 0u128;
	let mut amount_with_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, 100, 50 * ONE, 0));
			amount_without_slip = Tokens::free_balance(100, &LP3);
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 100 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, 100, 50 * ONE, 0));
			amount_with_slip = Tokens::free_balance(100, &LP3);
		});

	assert!(
		amount_with_slip < amount_without_slip,
		"Hub sell with slip should produce less: without={} with={}",
		amount_without_slip,
		amount_with_slip
	);
}

#[test]
fn buy_for_hub_asset_with_slip_increases_cost() {
	let mut cost_without_slip = 0u128;
	let mut cost_with_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let before = Tokens::free_balance(LRNA, &LP3);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				10 * ONE,
				u128::MAX
			));
			let after = Tokens::free_balance(LRNA, &LP3);
			cost_without_slip = before - after;
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			let before = Tokens::free_balance(LRNA, &LP3);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				10 * ONE,
				u128::MAX
			));
			let after = Tokens::free_balance(LRNA, &LP3);
			cost_with_slip = before - after;
		});

	assert!(
		cost_with_slip > cost_without_slip,
		"Hub buy with slip should cost more: without={} with={}",
		cost_without_slip,
		cost_with_slip
	);
}

#[test]
fn consecutive_trades_have_increasing_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 5000 * ONE),
			(LP1, 100, 5000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			let before_first = Tokens::free_balance(HDX, &LP1);
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			let after_first = Tokens::free_balance(HDX, &LP1);
			let output_first = after_first - before_first;

			// Second trade should produce less output due to accumulated delta
			let before_second = Tokens::free_balance(HDX, &LP1);
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			let after_second = Tokens::free_balance(HDX, &LP1);
			let output_second = after_second - before_second;

			assert!(
				output_second < output_first,
				"Second trade should get less output: first={} second={}",
				output_first,
				output_second
			);
		});
}

#[test]
fn block_reset_clears_tracking() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 5000 * ONE),
			(LP1, 100, 5000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			// First trade creates deltas
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));

			// Verify deltas are non-zero
			assert_ne!(SlipFeeDelta::<Test>::get(100), 0);
			assert_ne!(SlipFeeDelta::<Test>::get(HDX), 0);
			assert!(SlipFeeLrnaAtBlockStart::<Test>::get(100).is_some());
			assert!(SlipFeeLrnaAtBlockStart::<Test>::get(HDX).is_some());

			// Simulate end of block
			<Omnipool as Hooks<u64>>::on_finalize(System::block_number());

			// Deltas should be cleared
			assert_eq!(SlipFeeDelta::<Test>::get(100), 0);
			assert_eq!(SlipFeeDelta::<Test>::get(HDX), 0);
			assert!(SlipFeeLrnaAtBlockStart::<Test>::get(100).is_none());
			assert!(SlipFeeLrnaAtBlockStart::<Test>::get(HDX).is_none());
		});
}

#[test]
fn delta_tracking_is_correct() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 5000 * ONE),
			(LP1, 100, 5000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			// Get the initial hub reserves for Q0 snapshot
			let asset_100_state = Omnipool::load_asset_state(100).unwrap();
			let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
			let q0_100 = asset_100_state.hub_reserve;
			let q0_hdx = hdx_state.hub_reserve;

			// Sell 100 -> HDX
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));

			// Q0 snapshots should match the initial hub reserves
			assert_eq!(SlipFeeLrnaAtBlockStart::<Test>::get(100).unwrap(), q0_100);
			assert_eq!(SlipFeeLrnaAtBlockStart::<Test>::get(HDX).unwrap(), q0_hdx);

			// Delta for sell asset (100) should be negative (LRNA left the pool)
			let delta_100 = SlipFeeDelta::<Test>::get(100);
			assert!(delta_100 < 0, "Sell-side delta should be negative: {}", delta_100);

			// Delta for buy asset (HDX) should be positive (LRNA entered the pool)
			let delta_hdx = SlipFeeDelta::<Test>::get(HDX);
			assert!(delta_hdx > 0, "Buy-side delta should be positive: {}", delta_hdx);
		});
}

#[test]
fn q0_snapshot_is_lazy_and_persistent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 5000 * ONE),
			(LP1, 100, 5000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50),
			});

			// Before any trade, no snapshot exists
			assert!(SlipFeeLrnaAtBlockStart::<Test>::get(100).is_none());

			let initial_state = Omnipool::load_asset_state(100).unwrap();
			let initial_hub_reserve = initial_state.hub_reserve;

			// First trade snapshots Q0
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));

			let q0 = SlipFeeLrnaAtBlockStart::<Test>::get(100).unwrap();
			assert_eq!(q0, initial_hub_reserve);

			// After trade, hub_reserve has changed
			let after_state = Omnipool::load_asset_state(100).unwrap();
			assert_ne!(after_state.hub_reserve, initial_hub_reserve);

			// Second trade should still use the same Q0 (not the updated hub_reserve)
			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));

			let q0_still = SlipFeeLrnaAtBlockStart::<Test>::get(100).unwrap();
			assert_eq!(q0_still, initial_hub_reserve, "Q0 should remain from first snapshot");
		});
}

#[test]
fn max_cap_is_applied() {
	// Use a very small max_slip_fee cap and a large trade
	// Compare against a high cap to verify the cap limits the fee
	let mut amount_with_low_cap = 0u128;
	let mut amount_with_high_cap = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_parts(1000), // 0.1% cap
			});

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_with_low_cap = Tokens::free_balance(HDX, &LP1);
		});

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP1, 100, 1000 * ONE),
		])
		.with_registered_asset(100)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(50), // 50% cap (effectively uncapped for this trade)
			});

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP1), 100, HDX, 50 * ONE, 0));
			amount_with_high_cap = Tokens::free_balance(HDX, &LP1);
		});

	// With low cap, slip fee is limited, so output should be higher than with high cap
	assert!(
		amount_with_low_cap > amount_with_high_cap,
		"Low cap should produce more output (less fee): low_cap={} high_cap={}",
		amount_with_low_cap,
		amount_with_high_cap
	);
}

// ========== Hub asset trade verification tests ==========
//
// Pool setup: asset 100 at price 0.65 with 2000*ONE reserve → hub_reserve = 1300*ONE
// Asset fee = 0.25% (non-zero to test interaction with slip)
//
// For sell_hub (first trade in block):
//   slip_rate = |delta_q| / (Q0 + delta_q) where delta_q = amount (LRNA entering pool)
//   e.g. sell 50*ONE LRNA: slip_rate = 50 / (1300 + 50) ≈ 3.703%

#[test]
fn sell_hub_asset_with_fee_and_slip_verification() {
	let sell_amount = 50 * ONE;
	let asset_fee = Permill::from_rational(25u32, 10000u32); // 0.25%

	// Run 1: slip disabled, asset_fee enabled
	let mut output_no_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 200 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(asset_fee)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			assert!(SlipFee::<Test>::get().is_none());

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, 100, sell_amount, 0));
			output_no_slip = Tokens::free_balance(100, &LP3);
		});

	// Run 2: slip enabled (100% cap = uncapped), same asset_fee
	let mut output_with_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 200 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(asset_fee)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(100),
			});

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, 100, sell_amount, 0));
			output_with_slip = Tokens::free_balance(100, &LP3);
		});

	assert!(
		output_with_slip < output_no_slip,
		"Slip should reduce output: no_slip={} with_slip={}",
		output_no_slip,
		output_with_slip
	);

	// Verify the slip fee impact roughly matches the expected rate.
	// Q0 = 1300*ONE, delta_q = 50*ONE → slip_rate ≈ 50/1350 ≈ 3.703%
	// The reduction in output should be approximately slip_rate of the no-slip output.
	let reduction = output_no_slip - output_with_slip;
	let reduction_pct_x1000 = reduction * 1000 / output_no_slip; // e.g. 37 = 3.7%
															  // Expected ~37 (3.7%). Allow ±10 tolerance for AMM non-linearity.
	assert!(
		(27..=47).contains(&reduction_pct_x1000),
		"Slip reduction should be ~3.7%: actual={}‰ (no_slip={}, with_slip={}, reduction={})",
		reduction_pct_x1000,
		output_no_slip,
		output_with_slip,
		reduction
	);
}

#[test]
fn buy_for_hub_asset_with_fee_and_slip_verification() {
	let buy_amount = 10 * ONE;
	let asset_fee = Permill::from_rational(25u32, 10000u32); // 0.25%

	// Run 1: slip disabled
	let mut cost_no_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(asset_fee)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			let before = Tokens::free_balance(LRNA, &LP3);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				buy_amount,
				u128::MAX
			));
			let after = Tokens::free_balance(LRNA, &LP3);
			cost_no_slip = before - after;
		});

	// Run 2: slip enabled
	let mut cost_with_slip = 0u128;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(asset_fee)
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(100),
			});

			let before = Tokens::free_balance(LRNA, &LP3);
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				buy_amount,
				u128::MAX
			));
			let after = Tokens::free_balance(LRNA, &LP3);
			cost_with_slip = before - after;
		});

	assert!(
		cost_with_slip > cost_no_slip,
		"Slip should increase buy cost: no_slip={} with_slip={}",
		cost_no_slip,
		cost_with_slip
	);

	// Verify the extra cost roughly matches expected slip rate.
	// Q0 = 1300*ONE. Buying 10 tokens requires ~6.5*ONE LRNA (rough AMM estimate).
	// slip_rate ≈ d_net / (Q0 + d_net). For d_net ≈ 6.5: rate ≈ 6.5/1306.5 ≈ 0.5%
	// The increase in cost should be approximately that percentage of the no-slip cost.
	let extra_cost = cost_with_slip - cost_no_slip;
	let extra_pct_x10000 = extra_cost * 10000 / cost_no_slip; // e.g. 50 = 0.5%
														   // Expected somewhere in the range of 0.3%-1.0%. Be generous with tolerance due to AMM non-linearity.
	assert!(
		(20..=150).contains(&extra_pct_x10000),
		"Slip cost increase should be small for this trade size: actual={}bp (no_slip={}, with_slip={}, extra={})",
		extra_pct_x10000,
		cost_no_slip,
		cost_with_slip,
		extra_cost
	);
}

#[test]
fn sell_hub_asset_does_not_take_more_lrna_than_specified() {
	let sell_amount = 50 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 200 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(Permill::from_rational(25u32, 10000u32))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(100),
			});

			let lrna_before = Tokens::free_balance(LRNA, &LP3);

			assert_ok!(Omnipool::sell(RuntimeOrigin::signed(LP3), LRNA, 100, sell_amount, 0));

			let lrna_after = Tokens::free_balance(LRNA, &LP3);
			let lrna_spent = lrna_before - lrna_after;

			// The user must spend exactly sell_amount, not more
			assert_eq!(
				lrna_spent, sell_amount,
				"User should spend exactly the specified amount: spent={} specified={}",
				lrna_spent, sell_amount
			);
		});
}

#[test]
fn buy_for_hub_asset_charges_exactly_slip_fee_on_top() {
	// Verify that buy-for-hub charges d_net + slip_buy_amount from user.
	// Compare the LRNA cost (user balance change) against the math layer result.
	let buy_amount = 10 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, 1000 * ONE),
			(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			(LP2, 100, 2000 * ONE),
			(LP3, LRNA, 500 * ONE),
		])
		.with_registered_asset(100)
		.with_asset_fee(Permill::from_rational(25u32, 10000u32))
		.with_initial_pool(FixedU128::from_float(0.5), FixedU128::from(1))
		.with_token(100, FixedU128::from_float(0.65), LP2, 2000 * ONE)
		.build()
		.execute_with(|| {
			SlipFee::<Test>::put(SlipFeeConfig {
				max_slip_fee: Permill::from_percent(100),
			});

			// Snapshot state before trade for manual math verification
			let asset_state = Omnipool::load_asset_state(100).unwrap();
			let q0 = asset_state.hub_reserve;

			let lrna_before = Tokens::free_balance(LRNA, &LP3);

			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(LP3),
				100,
				LRNA,
				buy_amount,
				u128::MAX
			));

			let lrna_after = Tokens::free_balance(LRNA, &LP3);
			let lrna_spent = lrna_before - lrna_after;

			// Compute expected cost using the math layer directly
			let slip = hydra_dx_math::omnipool::types::HubTradeSlipFees {
				asset_hub_reserve: q0,
				asset_delta: 0,
				max_slip_fee: Permill::from_percent(100),
			};
			let math_result = hydra_dx_math::omnipool::calculate_buy_for_hub_asset_state_changes(
				&(&asset_state).into(),
				buy_amount,
				Permill::from_rational(25u32, 10000u32),
				Some(&slip),
			)
			.unwrap();

			let expected_cost = *math_result.asset.delta_hub_reserve + math_result.fee.protocol_fee;

			assert_eq!(
				lrna_spent, expected_cost,
				"User LRNA cost should equal d_net + slip: spent={} d_net={} slip={} expected={}",
				lrna_spent, *math_result.asset.delta_hub_reserve, math_result.fee.protocol_fee, expected_cost
			);
		});
}
