#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, Omnipool, RuntimeOrigin};
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

// ============================================================
// 1. Sell asset -> asset (omnipool direct)
// ============================================================

#[test]
fn sell_with_slip_fees_gives_less_output_than_without() {
	let sell_amount = 100 * UNITS;

	// Run 1: slip disabled
	TestNet::reset();
	let mut output_no_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));

		output_no_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Run 2: slip enabled
	TestNet::reset();
	let mut output_with_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));

		output_with_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	assert!(
		output_with_slip < output_no_slip,
		"Slip fee should reduce sell output: no_slip={output_no_slip} with_slip={output_with_slip}"
	);
}

// ============================================================
// 2. Buy asset -> asset (omnipool direct)
// ============================================================

#[test]
fn buy_with_slip_fees_costs_more_than_without() {
	let buy_amount = 10 * UNITS;

	// Run 1: slip disabled
	TestNet::reset();
	let mut cost_no_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));

		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			buy_amount,
			u128::MAX,
		));

		cost_no_slip = dai_before - Currencies::free_balance(DAI, &trader);
	});

	// Run 2: slip enabled
	TestNet::reset();
	let mut cost_with_slip = 0u128;

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

		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			buy_amount,
			u128::MAX,
		));

		cost_with_slip = dai_before - Currencies::free_balance(DAI, &trader);
	});

	assert!(
		cost_with_slip > cost_no_slip,
		"Slip fee should increase buy cost: no_slip={cost_no_slip} with_slip={cost_with_slip}"
	);
}

// ============================================================
// 3. Sell LRNA -> asset (sell hub asset)
// ============================================================

#[test]
fn sell_lrna_with_slip_fees_gives_less_output_than_without() {
	let sell_amount = 100 * UNITS;

	// Run 1: slip disabled
	TestNet::reset();
	let mut output_no_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			sell_amount,
			0u128,
		));

		output_no_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Run 2: slip enabled
	TestNet::reset();
	let mut output_with_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			sell_amount,
			0u128,
		));

		output_with_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	assert!(
		output_with_slip < output_no_slip,
		"Slip fee should reduce LRNA sell output: no_slip={output_no_slip} with_slip={output_with_slip}"
	);
}

// ============================================================
// 4. Buy asset with LRNA (buy for hub asset)
// ============================================================

#[test]
fn buy_with_lrna_with_slip_fees_costs_more_than_without() {
	let buy_amount = 10 * UNITS;

	// Run 1: slip disabled
	TestNet::reset();
	let mut cost_no_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);
		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			LRNA,
			buy_amount,
			u128::MAX,
		));

		cost_no_slip = lrna_before - Currencies::free_balance(LRNA, &trader);
	});

	// Run 2: slip enabled
	TestNet::reset();
	let mut cost_with_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			LRNA,
			buy_amount,
			u128::MAX,
		));

		cost_with_slip = lrna_before - Currencies::free_balance(LRNA, &trader);
	});

	assert!(
		cost_with_slip > cost_no_slip,
		"Slip fee should increase LRNA buy cost: no_slip={cost_no_slip} with_slip={cost_with_slip}"
	);
}

// ============================================================
// 5. Cross-block: deltas cleared between blocks
// ============================================================

#[test]
fn slip_fee_deltas_are_cleared_across_blocks() {
	let sell_amount = 100 * UNITS;

	// Baseline: no slip fees, single trade output
	TestNet::reset();
	let mut output_no_slip = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));

		output_no_slip = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// With slip fees: trade in block N, advance block, trade in block N+1
	TestNet::reset();
	let mut first_output = 0u128;
	let mut cleared_output = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		// Block N: first trade
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		first_output = Currencies::free_balance(DAI, &trader) - dai_before;

		// Advance to new block (clears slip fee deltas)
		go_to_block(11);

		// Block N+1: fresh trade with cleared deltas
		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		cleared_output = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Slip fees are active: first trade gives less than no-slip baseline
	assert!(
		first_output < output_no_slip,
		"Slip fee should reduce output: no_slip={output_no_slip} with_slip={first_output}"
	);

	// After delta clearing, output should be close to first trade (no accumulated penalty).
	// Pool state changed slightly from the first trade, but the slip fee restarts from zero.
	assert!(
		cleared_output >= first_output * 99 / 100,
		"Cleared trade should not suffer accumulated slip penalty: first={first_output} cleared={cleared_output}"
	);
}

// ============================================================
// 6. Sequential trades accumulate slip within a block
// ============================================================

#[test]
fn sequential_trades_accumulate_slip_within_block() {
	// Two same-direction trades in one block: the second should produce less
	// output than the first due to accumulated slip fee deltas.
	// We compare against a no-slip baseline to prove the extra reduction comes from slip fees.
	let sell_amount = 100 * UNITS;

	// Run 1: no slip fees — measure the drop from AMM state change alone
	TestNet::reset();
	let mut no_slip_first = 0u128;
	let mut no_slip_second = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();

		let trader = AccountId::from(BOB);

		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		no_slip_first = Currencies::free_balance(DAI, &trader) - dai_before;

		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		no_slip_second = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Run 2: with slip fees
	TestNet::reset();
	let mut slip_first = 0u128;
	let mut slip_second = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);

		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		slip_first = Currencies::free_balance(DAI, &trader) - dai_before;

		let dai_before = Currencies::free_balance(DAI, &trader);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));
		slip_second = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Basic: second trade gets less output in both cases
	assert!(
		slip_second < slip_first,
		"Second trade should get less output due to accumulated slip: first={slip_first} second={slip_second}"
	);

	// The drop between first and second trade should be LARGER with slip fees than without.
	// Without slip fees, the drop is purely from AMM pool state changes.
	// With slip fees, the drop includes accumulated slip on top of pool state changes.
	let no_slip_drop = no_slip_first - no_slip_second;
	let slip_drop = slip_first - slip_second;
	assert!(
		slip_drop > no_slip_drop,
		"Slip fees should cause a larger drop between sequential trades: slip_drop={slip_drop} no_slip_drop={no_slip_drop}"
	);
}
