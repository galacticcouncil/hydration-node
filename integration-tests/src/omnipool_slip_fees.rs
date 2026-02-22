#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, Omnipool, Router, RuntimeOrigin};
use hydradx_traits::router::{PoolType, Trade};
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
fn sell_should_work_with_slip_fees_enabled() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let sell_amount = 10 * UNITS;

		// BOB has HDX from genesis; sell HDX for DAI
		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
		));

		let dai_received = Currencies::free_balance(DAI, &trader) - dai_before;
		assert!(dai_received > 0, "Should receive some DAI");
	});
}

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
		"Slip fee should reduce sell output: no_slip={} with_slip={}",
		output_no_slip,
		output_with_slip
	);
}

// ============================================================
// 2. Buy asset -> asset (omnipool direct)
// ============================================================

#[test]
fn buy_should_work_with_slip_fees_enabled() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let buy_amount = 10 * UNITS;

		// Give BOB enough DAI to buy HDX
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			trader.clone(),
			DAI,
			(10_000 * UNITS) as i128,
		));

		let dai_before = Currencies::free_balance(DAI, &trader);
		let hdx_before = hydradx_runtime::Balances::free_balance(&trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			buy_amount,
			u128::MAX,
		));

		let dai_spent = dai_before - Currencies::free_balance(DAI, &trader);
		assert!(dai_spent > 0, "Should spend some DAI");

		let hdx_received = hydradx_runtime::Balances::free_balance(&trader) - hdx_before;
		assert!(hdx_received >= buy_amount, "Should receive at least buy_amount HDX");
	});
}

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
		"Slip fee should increase buy cost: no_slip={} with_slip={}",
		cost_no_slip,
		cost_with_slip
	);
}

// ============================================================
// 3. Sell LRNA -> asset (sell hub asset)
// ============================================================

#[test]
fn sell_lrna_should_work_with_slip_fees_enabled() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let sell_amount = 100 * UNITS;

		let dai_before = Currencies::free_balance(DAI, &trader);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			sell_amount,
			0u128,
		));

		let dai_received = Currencies::free_balance(DAI, &trader) - dai_before;
		assert!(dai_received > 0, "Should receive DAI for selling LRNA");
	});
}

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
		"Slip fee should reduce LRNA sell output: no_slip={} with_slip={}",
		output_no_slip,
		output_with_slip
	);
}

// ============================================================
// 4. Buy asset with LRNA (buy for hub asset)
// ============================================================

#[test]
fn buy_with_lrna_should_work_with_slip_fees_enabled() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let buy_amount = 10 * UNITS;

		let lrna_before = Currencies::free_balance(LRNA, &trader);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			LRNA,
			buy_amount,
			u128::MAX,
		));

		let lrna_spent = lrna_before - Currencies::free_balance(LRNA, &trader);
		assert!(lrna_spent > 0, "Should spend some LRNA");

		let dai_balance = Currencies::free_balance(DAI, &trader);
		assert!(dai_balance >= buy_amount, "Should receive at least buy_amount DAI");
	});
}

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
		"Slip fee should increase LRNA buy cost: no_slip={} with_slip={}",
		cost_no_slip,
		cost_with_slip
	);
}

// ============================================================
// 5. Router: sell through omnipool matches direct sell
// ============================================================

#[test]
fn router_sell_matches_direct_sell_with_slip_fees() {
	let sell_amount = 100 * UNITS;

	// Run 1: direct omnipool sell
	TestNet::reset();
	let mut direct_output = 0u128;

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

		direct_output = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Run 2: router sell (single-hop through omnipool)
	TestNet::reset();
	let mut router_output = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(trader.clone()),
			HDX,
			DAI,
			sell_amount,
			0u128,
			trades.try_into().unwrap(),
		));

		router_output = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	assert_eq!(
		direct_output, router_output,
		"Router sell should match direct omnipool sell: direct={} router={}",
		direct_output, router_output
	);
}

// ============================================================
// 6. Router: buy through omnipool matches direct buy
// ============================================================

#[test]
fn router_buy_matches_direct_buy_with_slip_fees() {
	let buy_amount = 10 * UNITS;

	// Run 1: direct omnipool buy
	TestNet::reset();
	let mut direct_cost = 0u128;

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

		direct_cost = dai_before - Currencies::free_balance(DAI, &trader);
	});

	// Run 2: router buy (single-hop through omnipool)
	TestNet::reset();
	let mut router_cost = 0u128;

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

		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: DAI,
			asset_out: HDX,
		}];

		assert_ok!(Router::buy(
			RuntimeOrigin::signed(trader.clone()),
			DAI,
			HDX,
			buy_amount,
			u128::MAX,
			trades.try_into().unwrap(),
		));

		router_cost = dai_before - Currencies::free_balance(DAI, &trader);
	});

	assert_eq!(
		direct_cost, router_cost,
		"Router buy should match direct omnipool buy: direct={} router={}",
		direct_cost, router_cost
	);
}

// ============================================================
// 7. Router: sell LRNA through omnipool matches direct sell
// ============================================================

#[test]
fn router_sell_lrna_matches_direct_sell_with_slip_fees() {
	let sell_amount = 100 * UNITS;

	// Run 1: direct sell
	TestNet::reset();
	let mut direct_output = 0u128;

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

		direct_output = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	// Run 2: router sell
	TestNet::reset();
	let mut router_output = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let dai_before = Currencies::free_balance(DAI, &trader);

		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: LRNA,
			asset_out: DAI,
		}];

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			sell_amount,
			0u128,
			trades.try_into().unwrap(),
		));

		router_output = Currencies::free_balance(DAI, &trader) - dai_before;
	});

	assert_eq!(
		direct_output, router_output,
		"Router LRNA sell should match direct: direct={} router={}",
		direct_output, router_output
	);
}

// ============================================================
// 8. Router: buy with LRNA through omnipool matches direct buy
// ============================================================

#[test]
fn router_buy_with_lrna_matches_direct_buy_with_slip_fees() {
	let buy_amount = 10 * UNITS;

	// Run 1: direct buy
	TestNet::reset();
	let mut direct_cost = 0u128;

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

		direct_cost = lrna_before - Currencies::free_balance(LRNA, &trader);
	});

	// Run 2: router buy
	TestNet::reset();
	let mut router_cost = 0u128;

	Hydra::execute_with(|| {
		init_omnipool();
		enable_slip_fees();

		let trader = AccountId::from(BOB);
		let lrna_before = Currencies::free_balance(LRNA, &trader);

		let trades = vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: LRNA,
			asset_out: DAI,
		}];

		assert_ok!(Router::buy(
			RuntimeOrigin::signed(trader.clone()),
			LRNA,
			DAI,
			buy_amount,
			u128::MAX,
			trades.try_into().unwrap(),
		));

		router_cost = lrna_before - Currencies::free_balance(LRNA, &trader);
	});

	assert_eq!(
		direct_cost, router_cost,
		"Router LRNA buy should match direct: direct={} router={}",
		direct_cost, router_cost
	);
}
