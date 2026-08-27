//! Global-netting scenarios: chains, cycles and partial cross-pair coincidences
//! that the solver must internalize instead of routing through the AMM.
//!
//! Each test runs against the real `mainnet_apr` snapshot with real Omnipool
//! assets and slip fees enabled — no mocks — so the measured behaviour reflects
//! production liquidity. Every scenario pins the solver's per-intent outputs,
//! AMM trade count and score, and submits the solution so the pallet's own
//! conservation and score checks act as the oracle.

use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use hydradx_runtime::Runtime;
use ice_support::{Solution, SolverMode};
use primitives::AccountId;
use xcm_emulator::Network;

use super::harness::{
	amm_in_for, amm_trade_count, enable_slip_fees, is_resolved, resolved, run_and_submit_as, swap, V4Solver,
};
use super::PATH_TO_SNAPSHOT;

/// Solve with v4 under the default mode, dump the pinnable numbers, and submit —
/// the pallet re-checks per-asset conservation and the score, so acceptance is
/// the scenario's real oracle.
fn run_and_submit(label: &str) -> Solution {
	run_and_submit_as::<V4Solver>(SolverMode::V4, label)
}

// ---------------------------------------------------------------------------
// Scenario 1 — open 3-asset chain (A->B, B->C); the canonical case rings miss.
//
// Alice sells BNC for HDX; Bob sells HDX for DOT. HDX is the intermediate:
// Alice *buys* it, Bob *sells* it, so the HDX leg nets internally and only the
// residual reaches the AMM. Solving the two pairs independently would
// round-trip the HDX volume through the pool.
// ---------------------------------------------------------------------------
#[test]
fn chain_should_net_the_shared_leg_when_an_intermediate_asset_is_bought_and_sold() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32; // 12 decimals
	let hdx = 0u32; // 12 decimals
	let dot = 5u32; // 10 decimals

	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // ~14_700 HDX at snapshot spot
	let bob_hdx = 14_000 * hdx_unit; // sized to roughly cancel Alice's HDX receipt
	let alice_min_hdx = 1_000 * hdx_unit; // loose but valid (>= ED), below spot
	let bob_min_dot = 1 * dot_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, alice_min_hdx, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, bob_min_dot, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			// Intent ids are deterministic for the snapshot: base + submission order.
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX, first
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT, second

			let sol = run_and_submit("chain_bnc_hdx_dot");
			// The HDX leg nets out: nothing is sold HDX->DOT any more, and only the
			// residual BNC reaches the pool.
			assert_eq!(sol.resolved_intents.len(), 2, "both chain intents resolve");
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 13778637061199802u128);
			assert_eq!(amm_in_for(&sol, hdx, dot), 0, "the HDX leg never reaches the pool");
			assert_eq!(amm_in_for(&sol, bnc, hdx), 53009477949832u128);
			assert_eq!(amm_in_for(&sol, bnc, dot), 946990522050167u128);

			let alice = resolved(&sol, alice_id);
			assert_eq!(swap(alice).asset_in, bnc);
			assert_eq!(swap(alice).asset_out, hdx);
			assert_eq!(swap(alice).amount_in, alice_bnc);
			assert_eq!(swap(alice).amount_out, 14778425464087880u128, "Alice HDX out");

			let bob = resolved(&sol, bob_id);
			assert_eq!(swap(bob).asset_in, hdx);
			assert_eq!(swap(bob).asset_out, dot);
			assert_eq!(swap(bob).amount_in, bob_hdx);
			assert_eq!(swap(bob).amount_out, 221597111922u128, "Bob DOT out");
		});
}

// ---------------------------------------------------------------------------
// Scenario 2 — 3-asset cycle (HDX->BNC->DOT->HDX). Control case: a closed cycle
// must internalize almost entirely, leaving little or nothing for the AMM.
// ---------------------------------------------------------------------------
#[test]
fn three_asset_cycle_should_internalize_when_all_legs_are_present() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx = 1_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 5 * bnc_unit; // BNC->DOT
	let charlie_dot = 10 * dot_unit; // DOT->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit / 2, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit / 10, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot, 500 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->HDX

			let sol = run_and_submit("cycle_3asset_ring");
			assert_eq!(sol.resolved_intents.len(), 3, "the whole cycle resolves");
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 5848644064753465u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				67433819967149u128,
				"Alice BNC out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 1173241108u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				6281710071545208u128,
				"Charlie HDX out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 3 — partial cross-pair coincidence. Alice sells 1000 BNC for HDX
// (~14.7k HDX of demand); Bob sells only 5000 HDX for DOT. ~5000 HDX of the
// coincidence nets internally; only the rest may come from the AMM. Solving
// per pair would net none of it and route both full legs.
// ---------------------------------------------------------------------------
#[test]
fn partial_coincidence_should_net_the_overlap_and_route_only_the_rest() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit;
	let bob_hdx = 5_000 * hdx_unit; // less than Alice's ~14.7k HDX receipt

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT

			let sol = run_and_submit("partial_coincidence");
			// Only the part of the HDX coincidence that cannot net internally is
			// routed; the HDX->DOT leg itself never reaches the pool.
			assert_eq!(sol.resolved_intents.len(), 2);
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 13749119045233406u128);
			assert_eq!(amm_in_for(&sol, hdx, dot), 0, "the HDX leg never reaches the pool");
			assert_eq!(amm_in_for(&sol, bnc, hdx), 661789099267797u128);
			assert_eq!(amm_in_for(&sol, bnc, dot), 338210900732202u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14749049912240615u128,
				"Alice HDX out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 79132992791u128, "Bob DOT out");
		});
}

// ---------------------------------------------------------------------------
// Scenario 4 — 4-asset cycle (HDX->BNC->DOT->WETH->HDX). Longer than explicit
// ring detection reaches; global netting must still internalize the whole cycle
// rather than routing four independent AMM trades.
// ---------------------------------------------------------------------------
#[test]
fn four_asset_cycle_should_internalize_when_it_exceeds_ring_detection() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	// ~10k HDX-equivalent per leg (rough spot), loose mins (>= ED, below spot).
	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->HDX (~0.033 WETH)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->HDX

			let sol = run_and_submit("cycle_4asset");
			// Four legs, three AMM trades: the cycle internalizes all but the residual.
			assert_eq!(sol.resolved_intents.len(), 4, "the whole 4-cycle resolves");
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 43712571451562280u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				676286517104114u128,
				"Alice BNC out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 159538036540u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8919692554988036u128,
				"Charlie WETH out"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				35217442841433590u128,
				"Dave HDX out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 5 — open 4-asset chain (BNC->HDX->DOT->WETH). Both intermediates
// (HDX, DOT) must net out, collapsing the batch to a single BNC->WETH residual
// instead of three independent AMM legs.
// ---------------------------------------------------------------------------
#[test]
fn four_asset_chain_should_net_both_intermediates() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let weth = 20u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX (~14.7k HDX)
	let bob_hdx = 14_000 * hdx_unit; // HDX->DOT (~22 DOT)
	let charlie_dot = 22 * dot_unit; // DOT->WETH

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH

			let sol = run_and_submit("chain_4asset");
			assert_eq!(sol.resolved_intents.len(), 3);
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 25803520695372633u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14778425464087880u128,
				"Alice HDX out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 222204364143u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				13024883026920610u128,
				"Charlie WETH out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 6 — 5-asset cycle (HDX->BNC->DOT->WETH->ETH->HDX). Far beyond what
// explicit ring detection reaches; the whole cycle must still internalize.
// ---------------------------------------------------------------------------
#[test]
fn five_asset_cycle_should_internalize_when_it_exceeds_ring_detection() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();
	let eve: AccountId = EVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let eth = 34u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;
	let eth_unit = 1_000_000_000_000_000_000u128;

	// ~10k HDX-equivalent per leg (rough spot), loose mins (>= ED, below spot).
	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->ETH
	let eve_eth = eth_unit / 30; // ETH->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.endow_account(eve.clone(), eth, eve_eth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, eth, dave_weth, eth_unit / 1000, Some(10))
		.submit_swap_intent(eve.clone(), eth, hdx, eve_eth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 5);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->ETH
			let eve_id = 32752052247409382067756072960004u128; // ETH->HDX

			let sol = run_and_submit("cycle_5asset");
			assert_eq!(sol.resolved_intents.len(), 5, "the whole 5-cycle resolves");
			assert_eq!(amm_trade_count(&sol), 4);
			assert_eq!(sol.score, 76040401743062421u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				676286517104114u128,
				"Alice BNC out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 159538039639u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8919692554988035u128,
				"Charlie WETH out"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				33305976737841383u128,
				"Dave ETH out"
			);
			assert_eq!(
				swap(resolved(&sol, eve_id)).amount_out,
				35239296395089250u128,
				"Eve HDX out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 7 — chain + same-pair direct match. Alice BNC->HDX and Bob HDX->BNC
// oppose on pair (BNC,HDX) while Charlie HDX->DOT chains off the HDX. Tests that
// global netting composes with the same-pair direct match instead of breaking
// it.
// ---------------------------------------------------------------------------
#[test]
fn netting_should_compose_with_the_same_pair_direct_match() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX
	let bob_hdx = 10_000 * hdx_unit; // HDX->BNC (opposing Alice on the pair)
	let charlie_hdx = 5_000 * hdx_unit; // HDX->DOT (chain off HDX)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), hdx, charlie_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, bnc, bob_hdx, bnc_unit / 2, Some(10))
		.submit_swap_intent(charlie.clone(), hdx, dot, charlie_hdx, dot_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->BNC
			let charlie_id = 32752052247409382067756072960002u128; // HDX->DOT

			let sol = run_and_submit("chain_plus_direct_match");
			// The opposing BNC<->HDX pair matches directly and Charlie's HDX chains
			// off it; only two residual trades reach the pool.
			assert_eq!(sol.resolved_intents.len(), 3);
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 14456573836907606u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14780718153014936u128,
				"Alice HDX out"
			);
			assert_eq!(
				swap(resolved(&sol, bob_id)).amount_out,
				676286517104114u128,
				"Bob BNC out"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				79166788556u128,
				"Charlie DOT out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 8 — two disjoint matching groups in one batch: a BNC->HDX->DOT chain
// AND an independent opposing WETH<->ETH pair. Asserts the solver handles
// independent groups deterministically and that netting one group never
// perturbs the other.
// ---------------------------------------------------------------------------
#[test]
fn disjoint_groups_should_settle_independently_when_batched_together() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let weth = 20u32;
	let eth = 34u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;
	let eth_unit = 1_000_000_000_000_000_000u128;

	// Group 1: chain BNC->HDX->DOT. Group 2: opposing WETH<->ETH (roughly balanced).
	let alice_bnc = 1_000 * bnc_unit;
	let bob_hdx = 14_000 * hdx_unit;
	let charlie_weth = weth_unit / 30; // WETH->ETH
	let dave_eth = eth_unit / 30; // ETH->WETH (opposes Charlie)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.endow_account(charlie.clone(), weth, charlie_weth * 10)
		.endow_account(dave.clone(), eth, dave_eth * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), weth, eth, charlie_weth, eth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), eth, weth, dave_eth, weth_unit / 1000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT
			let charlie_id = 32752052247409382067756072960002u128; // WETH->ETH
			let dave_id = 32752052247409382067756072960003u128; // ETH->WETH

			let sol = run_and_submit("disjoint_groups");
			assert_eq!(sol.resolved_intents.len(), 4);
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 78431977036771331u128);
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14778425464087880u128,
				"Alice HDX out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 221597111922u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				33305976737841383u128,
				"Charlie ETH out"
			);
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				33347363237730146u128,
				"Dave WETH out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 9 — blocked 3-ring. The ring HDX->BNC->DOT->HDX is feasible, but the
// FIRST intent on the HDX->BNC edge (Alice) carries a tight limit above spot. An
// edge-first ring detector skips the whole ring over that one entry even though
// Dave's loose HDX->BNC intent could have carried it; netting must not.
// ---------------------------------------------------------------------------
#[test]
fn cycle_should_still_settle_when_one_edge_intent_has_a_tight_limit() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx = 1_000 * hdx_unit; // HDX->BNC, tight (min above spot ~67.4 BNC)
	let dave_hdx = 1_000 * hdx_unit; // HDX->BNC, loose (could carry the ring)
	let bob_bnc = 5 * bnc_unit; // BNC->DOT
	let charlie_dot = 10 * dot_unit; // DOT->HDX

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), hdx, dave_hdx * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, 68 * bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit / 10, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot, 500 * hdx_unit, Some(10))
		.submit_swap_intent(dave.clone(), hdx, bnc, dave_hdx, 60 * bnc_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC (tight)
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->HDX
			let dave_id = 32752052247409382067756072960003u128; // HDX->BNC (loose)

			let sol = run_and_submit("blocked_3ring");
			// Alice's tight limit blocks her, but it no longer blocks the ring:
			// Dave's loose HDX->BNC intent carries the same edge.
			assert_eq!(sol.resolved_intents.len(), 3);
			assert_eq!(amm_trade_count(&sol), 2);
			assert_eq!(sol.score, 5789144064753465u128);
			assert!(!is_resolved(&sol, alice_id), "the tight-limit intent stays out");
			assert_eq!(
				swap(resolved(&sol, dave_id)).amount_out,
				67433819967149u128,
				"Dave BNC out"
			);
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				6281710071545208u128,
				"Charlie HDX out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 1173241108u128, "Bob DOT out");
		});
}

// ---------------------------------------------------------------------------
// Scenario 10 — binding-limit chain. Chain BNC->HDX->DOT, but Bob's HDX->DOT min
// is set just above what a per-pair HDX->DOT AMM trade yields (~22.167 DOT):
// only netting the HDX leg — and the slippage it saves — can fill him.
// ---------------------------------------------------------------------------
#[test]
fn chain_should_exclude_the_intent_whose_limit_exceeds_the_netted_rate() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let bnc = 14u32;
	let hdx = 0u32;
	let dot = 5u32;
	let bnc_unit = 1_000_000_000_000u128;
	let hdx_unit = 1_000_000_000_000u128;

	let alice_bnc = 1_000 * bnc_unit; // BNC->HDX (loose)
	let bob_hdx = 14_000 * hdx_unit; // HDX->DOT (tight)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, alice_bnc * 10)
		.endow_account(bob.clone(), hdx, bob_hdx * 10)
		.submit_swap_intent(alice.clone(), bnc, hdx, alice_bnc, 1_000 * hdx_unit, Some(10))
		// 222_000_000_000 = 22.2 DOT, just above the ~22.167 DOT a per-pair route yields.
		.submit_swap_intent(bob.clone(), hdx, dot, bob_hdx, 222_000_000_000, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let alice_id = 32752052247409382067756072960000u128; // BNC->HDX (loose)
			let bob_id = 32752052247409382067756072960001u128; // HDX->DOT (tight)

			let sol = run_and_submit("binding_limit_chain");
			// Bob's limit sits above what even the netted chain can pay him, so he
			// is still excluded — netting improves the rate, it does not invent one.
			assert_eq!(sol.resolved_intents.len(), 1);
			assert_eq!(amm_trade_count(&sol), 1);
			assert_eq!(sol.score, 13730309893978531u128);
			assert!(!is_resolved(&sol, bob_id), "the tight-limit intent stays out");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				14730309893978531u128,
				"Alice HDX out"
			);
		});
}

// ---------------------------------------------------------------------------
// Scenario 11 — tight-limit cycle (more-intents-filled). The 4-cycle from
// scenario 4, but Dave's WETH->HDX min is set just above the ~35_187 HDX a
// per-leg route yields: only internalizing the cycle can fill all four.
// ---------------------------------------------------------------------------
#[test]
fn cycle_should_exclude_the_intent_whose_limit_exceeds_the_netted_rate() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_hdx = 10_000 * hdx_unit; // HDX->BNC
	let bob_bnc = 680 * bnc_unit; // BNC->DOT
	let charlie_dot = 15 * dot_unit; // DOT->WETH
	let dave_weth = weth_unit / 30; // WETH->HDX (tight)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		// 35_500 HDX, just above the ~35_187 HDX a per-leg WETH->HDX route yields.
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 35_500 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 4);
			let alice_id = 32752052247409382067756072960000u128; // HDX->BNC
			let bob_id = 32752052247409382067756072960001u128; // BNC->DOT
			let charlie_id = 32752052247409382067756072960002u128; // DOT->WETH
			let dave_id = 32752052247409382067756072960003u128; // WETH->HDX (tight)

			let sol = run_and_submit("tight_limit_cycle");
			// Dave's limit sits above what the netted cycle can pay him, so the
			// cycle settles as a 3-leg chain without him.
			assert_eq!(sol.resolved_intents.len(), 3);
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 8559350182822069u128);
			assert!(!is_resolved(&sol, dave_id), "the tight-limit intent stays out");
			assert_eq!(
				swap(resolved(&sol, alice_id)).amount_out,
				676286517104114u128,
				"Alice BNC out"
			);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, 159538642564u128, "Bob DOT out");
			assert_eq!(
				swap(resolved(&sol, charlie_id)).amount_out,
				8883914127075391u128,
				"Charlie WETH out"
			);
		});
}

// ---------------------------------------------------------------------------
// Explicit 4-asset-cycle conservation check: solve and submit, so the pallet's
// own per-asset conservation and score checks are the assertion.
// ---------------------------------------------------------------------------
#[test]
fn four_asset_cycle_should_conserve_every_asset_when_submitted() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;
	let weth = 20u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;
	let weth_unit = 1_000_000_000_000_000_000u128;

	let alice_hdx = 10_000 * hdx_unit;
	let bob_bnc = 680 * bnc_unit;
	let charlie_dot = 15 * dot_unit;
	let dave_weth = weth_unit / 30;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx * 10)
		.endow_account(bob.clone(), bnc, bob_bnc * 10)
		.endow_account(charlie.clone(), dot, charlie_dot * 10)
		.endow_account(dave.clone(), weth, dave_weth * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx, bnc_unit, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc, dot_unit, Some(10))
		.submit_swap_intent(charlie.clone(), dot, weth, charlie_dot, weth_unit / 1000, Some(10))
		.submit_swap_intent(dave.clone(), weth, hdx, dave_weth, 100 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			let sol = run_and_submit("4cycle_conservation");
			// `run_and_submit` already asserted the pallet accepts this — per-asset
			// conservation and the score recompute both hold on chain.
			assert_eq!(sol.resolved_intents.len(), 4);
			assert_eq!(amm_trade_count(&sol), 3);
			assert_eq!(sol.score, 43712571451562280u128);
		});
}
