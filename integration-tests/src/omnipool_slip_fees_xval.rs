#![cfg(test)]
#![allow(clippy::identity_op)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, Omnipool, RuntimeOrigin};
use hydradx_traits::fee::GetDynamicFee;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::SlipFeeConfig;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

const MAX_SLIP_FEE: Permill = Permill::from_percent(5);

fn enable_slip_fees() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: MAX_SLIP_FEE,
		})
	));
}

/// Assert that `actual` is within `tolerance` of `expected`.
fn assert_approx(actual: u128, expected: u128, tolerance: u128, label: &str) {
	let diff = if actual >= expected {
		actual - expected
	} else {
		expected - actual
	};
	assert!(
		diff <= tolerance,
		"{}: rust={} python={} diff={} tolerance={}",
		label,
		actual,
		expected,
		diff,
		tolerance
	);
}

// Tolerance: for values > 10^15 (DAI amounts), use relative 0.01% of value.
// For smaller values (HDX amounts), use absolute 10_000.
fn tol(val: u128) -> u128 {
	let relative = val / 10_000; // 0.01%
	relative.max(10_000)
}

// ============================================================
// Pool state dump (for documentation / debugging)
// ============================================================

#[test]
fn dump_pool_state_and_fees() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let hdx_state = Omnipool::load_asset_state(HDX).unwrap();
		let dai_state = Omnipool::load_asset_state(DAI).unwrap();

		let hdx_reserve = Currencies::free_balance(HDX, &Omnipool::protocol_account());
		let dai_reserve = Currencies::free_balance(DAI, &Omnipool::protocol_account());

		println!("=== POOL STATE ===");
		println!("HDX reserve:      {}", hdx_reserve);
		println!("HDX hub_reserve:  {}", hdx_state.hub_reserve);
		println!("DAI reserve:      {}", dai_reserve);
		println!("DAI hub_reserve:  {}", dai_state.hub_reserve);

		let (hdx_asset_fee, hdx_protocol_fee) =
			pallet_dynamic_fees::UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((HDX, hdx_state.reserve));
		let (dai_asset_fee, dai_protocol_fee) =
			pallet_dynamic_fees::UpdateAndRetrieveFees::<hydradx_runtime::Runtime>::get((DAI, dai_state.reserve));

		println!("HDX asset_fee={:?} protocol_fee={:?}", hdx_asset_fee, hdx_protocol_fee);
		println!("DAI asset_fee={:?} protocol_fee={:?}", dai_asset_fee, dai_protocol_fee);
	});
}

// ============================================================
// Cross-validation scenarios
// Python reference: test_slip_fee_integration_xval.py
// All trades within a single block (no dynamic fee changes).
// ============================================================

// Scenario 1: Single sell (100 HDX -> DAI)
#[test]
fn xval_single_sell() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let sell_amount = 100 * UNITS;

		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));

		let dai_received = Currencies::free_balance(DAI, &trader) - dai_before;

		let py_dai_received: u128 = 2661140647206267099;
		assert_approx(
			dai_received,
			py_dai_received,
			tol(py_dai_received),
			"single sell dai_received",
		);
	});
}

// Scenario 2: Single buy (buy 100 DAI with HDX)
#[test]
fn xval_single_buy() {
	let buy_amount = 100 * UNITS;

	// Baseline: no slip fees
	TestNet::reset();
	let mut cost_no_slip = 0u128;
	Hydra::execute_with(|| {
		init_omnipool();
		let trader = AccountId::from(BOB);
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			buy_amount,
			u128::MAX,
		));
		cost_no_slip = hdx_before - hydradx_runtime::Balances::free_balance(&trader);
	});

	// With slip fees
	TestNet::reset();
	let mut hdx_spent = 0u128;
	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			buy_amount,
			u128::MAX,
		));

		hdx_spent = hdx_before - hydradx_runtime::Balances::free_balance(&trader);
	});

	let py_hdx_spent: u128 = 3756583451;
	assert_approx(hdx_spent, py_hdx_spent, tol(py_hdx_spent), "single buy hdx_spent");

	// Slip fees must increase cost vs baseline
	assert!(
		hdx_spent > cost_no_slip,
		"Slip fee should increase buy cost: with_slip={} no_slip={}",
		hdx_spent,
		cost_no_slip
	);
}

// Scenario 3: Multiple sells - same direction (50 HDX->DAI, 50 HDX->DAI)
#[test]
fn xval_multiple_sells_same_direction() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		// Trade 1
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			50 * UNITS,
			0u128,
		));
		let trade1_output = Currencies::free_balance(DAI, &trader) - dai_before;

		// Trade 2 (accumulated delta)
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			50 * UNITS,
			0u128,
		));
		let trade2_output = Currencies::free_balance(DAI, &trader) - dai_before;

		let py_trade1: u128 = 1330783469624669360;
		let py_trade2: u128 = 1330463859019908636;

		assert_approx(trade1_output, py_trade1, tol(py_trade1), "sells_same trade1");
		assert_approx(trade2_output, py_trade2, tol(py_trade2), "sells_same trade2");

		// Second trade should get less due to accumulated slip
		assert!(trade2_output < trade1_output, "Second trade should get less output");
	});
}

// Scenario 4: Multiple sells - opposite direction (100 HDX->DAI, 100 DAI->HDX)
#[test]
fn xval_multiple_sells_opposite_direction() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));

		// Trade 1: sell 100 HDX -> DAI
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			100 * UNITS,
			0u128,
		));
		let trade1_output = Currencies::free_balance(DAI, &trader) - dai_before;

		// Trade 2: sell 100 DAI -> HDX
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			100 * UNITS,
			0u128,
		));
		let trade2_output = hydradx_runtime::Balances::free_balance(&trader) - hdx_before;

		let py_trade1: u128 = 2661140647206267099;
		let py_trade2: u128 = 3734684507;

		assert_approx(trade1_output, py_trade1, tol(py_trade1), "sells_opp trade1");
		assert_approx(trade2_output, py_trade2, tol(py_trade2), "sells_opp trade2");
	});
}

// Scenario 5: Multiple buys - same direction (buy 50 DAI, buy 50 DAI)
#[test]
fn xval_multiple_buys_same_direction() {
	// Baseline: no slip fees
	TestNet::reset();
	let mut no_slip_total = 0u128;
	Hydra::execute_with(|| {
		init_omnipool();
		let trader = AccountId::from(BOB);
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX
		));
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX
		));
		no_slip_total = hdx_before - hydradx_runtime::Balances::free_balance(&trader);
	});

	// With slip fees
	TestNet::reset();
	let mut trade1_cost = 0u128;
	let mut trade2_cost = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		// Trade 1
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX,
		));
		trade1_cost = hdx_before - hydradx_runtime::Balances::free_balance(&trader);

		// Trade 2
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX,
		));
		trade2_cost = hdx_before - hydradx_runtime::Balances::free_balance(&trader);
	});

	let py_trade1: u128 = 1878291714;
	let py_trade2: u128 = 1878291731;

	assert_approx(trade1_cost, py_trade1, tol(py_trade1), "buys_same trade1");
	assert_approx(trade2_cost, py_trade2, tol(py_trade2), "buys_same trade2");

	// Second buy should cost more
	assert!(trade2_cost > trade1_cost, "Second buy should cost more");

	// Slip fees must increase total cost vs baseline
	let slip_total = trade1_cost + trade2_cost;
	assert!(
		slip_total > no_slip_total,
		"Slip fees should increase total buy cost: with_slip={} no_slip={}",
		slip_total,
		no_slip_total
	);
}

// Scenario 6: Multiple buys - opposite direction (buy 50 DAI, buy 50 HDX)
#[test]
fn xval_multiple_buys_opposite_direction() {
	// Baseline: no slip fees
	TestNet::reset();
	let mut no_slip_trade1 = 0u128;
	let mut no_slip_trade2 = 0u128;
	Hydra::execute_with(|| {
		init_omnipool();
		let trader = AccountId::from(BOB);
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX
		));
		no_slip_trade1 = hdx_before - hydradx_runtime::Balances::free_balance(&trader);
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			50 * UNITS,
			u128::MAX
		));
		no_slip_trade2 = dai_before - Currencies::free_balance(DAI, &trader);
	});

	// With slip fees
	TestNet::reset();
	let mut trade1_cost = 0u128;
	let mut trade2_cost = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));

		// Trade 1: buy 50 DAI with HDX
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX,
		));
		trade1_cost = hdx_before - hydradx_runtime::Balances::free_balance(&trader);

		// Trade 2: buy 50 HDX with DAI
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			50 * UNITS,
			u128::MAX,
		));
		trade2_cost = dai_before - Currencies::free_balance(DAI, &trader);
	});

	let py_trade1: u128 = 1878291714;
	let py_trade2: u128 = 1339230498425223571;

	assert_approx(trade1_cost, py_trade1, tol(py_trade1), "buys_opp trade1");
	assert_approx(trade2_cost, py_trade2, tol(py_trade2), "buys_opp trade2");

	// Slip fees must increase costs vs baseline
	assert!(
		trade1_cost > no_slip_trade1,
		"Slip fee should increase trade1 cost: with_slip={} no_slip={}",
		trade1_cost,
		no_slip_trade1
	);
	assert!(
		trade2_cost > no_slip_trade2,
		"Slip fee should increase trade2 cost: with_slip={} no_slip={}",
		trade2_cost,
		no_slip_trade2
	);
}

// Scenario 7: Mixed sells and buys - mixed directions
#[test]
fn xval_mixed_trades() {
	// Baseline: first trade without slip fees
	TestNet::reset();
	let mut t1_no_slip = 0u128;
	Hydra::execute_with(|| {
		init_omnipool();
		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			100 * UNITS,
			0u128,
		));
		t1_no_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// With slip fees
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));

		// Trade 1: sell 100 HDX -> DAI
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			100 * UNITS,
			0u128,
		));
		let t1 = Currencies::free_balance(DAI, &trader) - dai_before;

		// Trade 2: buy 50 DAI with HDX
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			50 * UNITS,
			u128::MAX,
		));
		let t2 = hdx_before - hydradx_runtime::Balances::free_balance(&trader);

		// Trade 3: sell 200 DAI -> HDX
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			200 * UNITS,
			0u128,
		));
		let t3 = hydradx_runtime::Balances::free_balance(&trader) - hdx_before;

		// Trade 4: buy 30 HDX with DAI
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			30 * UNITS,
			u128::MAX,
		));
		let t4 = dai_before - Currencies::free_balance(DAI, &trader);

		// Trade 5: sell 50 HDX -> DAI
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			50 * UNITS,
			0u128,
		));
		let t5 = Currencies::free_balance(DAI, &trader) - dai_before;

		let py_t1: u128 = 2661140647206267099;
		let py_t2: u128 = 1879194238;
		let py_t3: u128 = 7469369037;
		let py_t4: u128 = 803280698075868829;
		let py_t5: u128 = 1330336597671390699;

		assert_approx(t1, py_t1, tol(py_t1), "mixed trade1 sell HDX->DAI");
		assert_approx(t2, py_t2, tol(py_t2), "mixed trade2 buy DAI w/ HDX");
		assert_approx(t3, py_t3, tol(py_t3), "mixed trade3 sell DAI->HDX");
		assert_approx(t4, py_t4, tol(py_t4), "mixed trade4 buy HDX w/ DAI");
		assert_approx(t5, py_t5, tol(py_t5), "mixed trade5 sell HDX->DAI");

		// Slip fees must reduce trade 1 output vs no-slip baseline
		assert!(
			t1 < t1_no_slip,
			"Slip fee should reduce trade1 output: no_slip={} with_slip={}",
			t1_no_slip,
			t1
		);
	});
}

// ============================================================
// LRNA (hub asset) sell/buy scenarios
// Hub trades have NO sell-side slip, NO protocol (LRNA) fee — only buy-side slip + asset fee.
// ============================================================

// Scenario 8: Sell LRNA -> DAI (fresh block)
#[test]
fn xval_sell_lrna_for_dai() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let sell_amount = 100 * UNITS;

		let dai_before = Currencies::free_balance(DAI, &trader);
		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			sell_amount,
			0u128,
		));

		let dai_received = Currencies::free_balance(DAI, &trader) - dai_before;
		let lrna_spent = lrna_before - Currencies::free_balance(LRNA, &trader);

		let py_dai_received: u128 = 2035714285714285714285;
		assert_approx(
			dai_received,
			py_dai_received,
			tol(py_dai_received),
			"sell_lrna->dai received",
		);
		assert_eq!(lrna_spent, sell_amount, "should spend exactly sell_amount LRNA");
	});
}

// Scenario 9: Sell LRNA -> HDX (fresh block)
#[test]
fn xval_sell_lrna_for_hdx() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let sell_amount = 100 * UNITS;

		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			HDX,
			sell_amount,
			0u128,
		));

		let hdx_received = hydradx_runtime::Balances::free_balance(&trader) - hdx_before;

		let py_hdx_received: u128 = 72728633265704192;
		assert_approx(
			hdx_received,
			py_hdx_received,
			tol(py_hdx_received),
			"sell_lrna->hdx received",
		);
	});
}

// Scenario 10: Buy DAI with LRNA (fresh block)
#[test]
fn xval_buy_dai_with_lrna() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let buy_amount = 100 * UNITS;

		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			LRNA,
			buy_amount,
			u128::MAX,
		));

		let lrna_spent = lrna_before - Currencies::free_balance(LRNA, &trader);

		let py_lrna_cost: u128 = 4511278;
		assert_approx(lrna_spent, py_lrna_cost, tol(py_lrna_cost), "buy_dai lrna_cost");
	});
}

// Scenario 11: Buy HDX with LRNA (fresh block)
#[test]
fn xval_buy_hdx_with_lrna() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let buy_amount = 100 * UNITS;

		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			LRNA,
			buy_amount,
			u128::MAX,
		));

		let lrna_spent = lrna_before - Currencies::free_balance(LRNA, &trader);

		let py_lrna_cost: u128 = 120476926186;
		assert_approx(lrna_spent, py_lrna_cost, tol(py_lrna_cost), "buy_hdx lrna_cost");
	});
}

// Scenario 12: Sell HDX->DAI then sell LRNA->DAI (accumulated delta on DAI)
#[test]
fn xval_sell_lrna_for_dai_with_prior_delta() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		// Trade 1: sell 100 HDX -> DAI (builds positive delta on DAI)
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			100 * UNITS,
			0u128,
		));
		let t1_dai = Currencies::free_balance(DAI, &trader) - dai_before;

		// Trade 2: sell 100 LRNA -> DAI (with prior positive delta on DAI → higher slip)
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			100 * UNITS,
			0u128,
		));
		let t2_dai = Currencies::free_balance(DAI, &trader) - dai_before;

		let py_t1: u128 = 2661140647206267099;
		let py_t2: u128 = 2035402018544312661680;

		assert_approx(t1_dai, py_t1, tol(py_t1), "sell_hdx->dai (trade1)");
		assert_approx(t2_dai, py_t2, tol(py_t2), "sell_lrna->dai with delta (trade2)");

		// With prior delta, trade2 should receive LESS than a fresh sell_lrna->dai
		// (because positive delta on DAI means more hub entered → higher buy-side slip)
		let py_fresh: u128 = 2035714285714285714285;
		assert!(
			t2_dai < py_fresh,
			"Prior delta should reduce sell_lrna output: {} < {}",
			t2_dai,
			py_fresh
		);
	});
}

// Scenario 13: Sell HDX->DAI then buy DAI with LRNA (accumulated delta on DAI)
#[test]
fn xval_buy_dai_with_lrna_after_prior_sell() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		// Trade 1: sell 100 HDX -> DAI (builds positive delta on DAI)
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			100 * UNITS,
			0u128,
		));
		let t1_dai = Currencies::free_balance(DAI, &trader) - dai_before;

		// Trade 2: buy 100 DAI with LRNA (with prior positive delta on DAI)
		let lrna_before = Currencies::free_balance(LRNA, &trader);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			LRNA,
			100 * UNITS,
			u128::MAX,
		));
		let t2_lrna_cost = lrna_before - Currencies::free_balance(LRNA, &trader);

		let py_t1: u128 = 2661140647206267099;
		let py_t2_cost: u128 = 4511999;

		assert_approx(t1_dai, py_t1, tol(py_t1), "sell_hdx->dai (trade1)");
		assert_approx(t2_lrna_cost, py_t2_cost, tol(py_t2_cost), "buy_dai with lrna (trade2)");

		// With prior delta, buy should cost MORE than fresh
		let py_fresh_cost: u128 = 4511278;
		assert!(
			t2_lrna_cost >= py_fresh_cost,
			"Prior delta should increase buy_lrna cost: {} >= {}",
			t2_lrna_cost,
			py_fresh_cost
		);
	});
}
