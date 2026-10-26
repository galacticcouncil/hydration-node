//! PoC: can the top-of-block ICE settlement slot be turned into a profit?
//!
//! The 2024 oracle-manipulation report (PR #870) was closed on pallet-dca by capping
//! the stability threshold, not by the min period. ICE re-exposes the same primitive —
//! a `submit_solution` settles at the top of the next block, before any signed tx — and
//! has no symmetric spot-vs-EMA guard. This test executes the pure single-attacker
//! round trip and *measures* the net P&L, so "is the guard urgent" is answered by a
//! pinned number rather than assumed:
//!
//!   block N:   attacker buys BNC with HDX on Omnipool (pumps BNC spot)
//!   block N+1: attacker's ICE swap intent sells the BNC back to HDX, settled first
//!   measure:   final HDX − initial HDX   (BNC fully round-tripped)
//!
//! A negative result is the finding: fees + round-trip slippage make the pure ICE
//! round trip unprofitable, so the min period is irrelevant to the attack and any
//! divergence guard is a separate, non-urgent design change. A positive result would
//! make this a live-exploit regression.

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE};
use frame_support::assert_ok;
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, Omnipool, RuntimeOrigin, System, Timestamp};
use ice_support::{Balance, SolverMode, SwapParams};
use orml_traits::MultiCurrency;
use pallet_intent::types::IntentInput;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::{AccountId, AssetId};
use xcm_emulator::Network;

use super::harness::{enable_slip_fees, run_and_submit_as, V4Solver};
use super::PATH_TO_SNAPSHOT;

const HDX: AssetId = 0;
const BNC: AssetId = 14;
const HDX_UNIT: Balance = 1_000_000_000_000;

/// HDX the attacker commits to the pump in block N.
const PUMP_IN: Balance = 500 * HDX_UNIT;

fn step() {
	hydradx_run_to_next_block();
	System::reset_events();
	Timestamp::set_timestamp(Timestamp::now() + MILLISECS_PER_BLOCK);
}

fn hdx_of(who: &AccountId) -> Balance {
	Currencies::free_balance(HDX, who)
}

fn bnc_of(who: &AccountId) -> Balance {
	Currencies::free_balance(BNC, who)
}

fn submit_sell_intent(who: &AccountId, asset_in: AssetId, amount_in: Balance, min_out: Balance) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
		IntentInput {
			data: ice_support::IntentDataInput::Swap(SwapParams {
				asset_in,
				asset_out: HDX,
				amount_in,
				amount_out: min_out,
				partial: false,
			}),
			deadline: None,
			on_resolved: None,
		}
	));
}

#[test]
fn ice_top_of_block_round_trip_should_report_net_pnl() {
	TestNet::reset();
	let attacker: AccountId = ALICE.into();
	let driver = HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(attacker.clone(), HDX, 2_000 * HDX_UNIT);
	driver.execute(|| {
		enable_slip_fees();

		// Off the snapshot block so the EMA integrates cleanly, matching dca_early.
		step();

		let start_hdx = hdx_of(&attacker);
		assert_eq!(bnc_of(&attacker), 0, "attacker starts with no BNC");

		// Block N: pump. Buy BNC with HDX on Omnipool.
		let bnc_before = bnc_of(&attacker);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(attacker.clone()),
			HDX,
			BNC,
			PUMP_IN,
			0,
		));
		let bnc_gained = bnc_of(&attacker) - bnc_before;
		assert!(bnc_gained > 0, "pump must yield BNC");

		// Dump the pumped BNC back to HDX. min_out = HDX ED (1 HDX) is the lowest a swap
		// intent accepts, so only an oracle guard — which swap intents lack — could block
		// it; the ~500 HDX coming back clears a 1 HDX floor trivially.
		submit_sell_intent(&attacker, BNC, bnc_gained, HDX_UNIT);

		// Block N+1: the solution settles top-of-block, selling BNC into the pool the
		// attacker just moved, before any arb tx.
		step();
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "oracle_divergence_dump");
		assert!(!sol.resolved_intents.is_empty(), "attacker intent must settle");

		// Block N+2: let the dust settle.
		step();

		let residual_bnc = bnc_of(&attacker);
		let final_hdx = hdx_of(&attacker);

		println!("// start_hdx   = {start_hdx}");
		println!("// pump_in     = {PUMP_IN}");
		println!("// bnc_gained  = {bnc_gained}");
		println!("// residual_bnc= {residual_bnc}");
		println!("// final_hdx   = {final_hdx}");
		if final_hdx >= start_hdx {
			println!("// NET PROFIT  = +{}", final_hdx - start_hdx);
		} else {
			println!("// NET LOSS    = -{}", start_hdx - final_hdx);
		}

		// The BNC must be fully round-tripped for `final - start` to be the whole P&L.
		assert_eq!(residual_bnc, 0, "intent must fully consume the pumped BNC");

		// Pinned on the mainnet_apr snapshot. A pure single-attacker round trip through
		// one pool pays the pool fee twice plus round-trip slippage, so the top-of-block
		// settlement slot creates no profit on its own: the min DCA period is irrelevant
		// to the oracle attack, and a divergence guard is a separate, non-urgent change.
		// A flip of this assertion (final >= start) would mean the slot became self-funding.
		assert_eq!(start_hdx, 2_000 * HDX_UNIT);
		assert_eq!(bnc_gained, 33_719_084_277_407);
		assert_eq!(final_hdx, 1_996_947_447_927_041);
		assert_eq!(
			start_hdx - final_hdx,
			3_052_552_072_959,
			"~3.05 HDX net loss (~0.61% of the pump)"
		);
	});
}
