//! Omnipool slip fee level per asset: the ICE simulator must price a lowered asset exactly as
//! the pallet executes it, and the node-side fill must read the same storage the pallet writes.

use super::harness::{enable_slip_fees, set_solver_mode, CombinedSimulatorState, V4Solver};
use super::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::{TestNet, ALICE, BOB};
use amm_simulator::omnipool::Simulator as OmnipoolSimulator;
use frame_support::assert_ok;
use hydradx_runtime::{ice_simulator_provider, Currencies, Omnipool, Runtime, RuntimeOrigin, System};
use hydradx_traits::amm::AmmSimulator;
use ice_support::{Balance, SolverMode};
use orml_traits::MultiCurrency;
use primitives::AccountId;
use sp_runtime::Percent;
use std::collections::BTreeMap;
use std::sync::Arc;
use xcm_emulator::Network;

type Sim = OmnipoolSimulator<ice_simulator_provider::Omnipool<Runtime>>;

const HDX: u32 = 0;
const H2O: u32 = 1;
const DOT: u32 = 5;
const BNC: u32 = 14;
const VDOT: u32 = 15;
const VDOT_AMOUNT: Balance = 1_000 * 10_000_000_000;
const BNC_AMOUNT: Balance = 1_000 * 1_000_000_000_000;

fn set_level(asset_id: u32, level: u8) {
	assert_ok!(Omnipool::set_asset_slip_fee_level(
		RuntimeOrigin::root(),
		asset_id,
		Percent::from_percent(level)
	));
}

fn endow(asset_id: u32, amount: Balance) {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		ALICE.into(),
		asset_id,
		amount as i128,
	));
}

/// Sells vDOT for BNC `count` times in one block, each simulated on the snapshot the previous
/// simulation returned and then executed. Returns (simulated, executed) per sell.
fn sell_vdot_for_bnc(count: usize) -> Vec<(Balance, Balance)> {
	endow(VDOT, VDOT_AMOUNT * count as Balance);
	let alice: AccountId = ALICE.into();
	let mut state = Sim::snapshot();
	let mut outs = Vec::new();
	for _ in 0..count {
		let (next, simulated) =
			<Sim as AmmSimulator>::simulate_sell(VDOT, BNC, VDOT_AMOUNT, 0, &state).expect("sell should simulate");
		state = next;
		let before = Currencies::free_balance(BNC, &alice);
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(alice.clone()),
			VDOT,
			BNC,
			VDOT_AMOUNT,
			0
		));
		outs.push((simulated.amount_out, Currencies::free_balance(BNC, &alice) - before));
	}
	outs
}

#[test]
fn simulate_sell_should_match_pallet_when_no_level_is_set() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		assert!(Sim::snapshot().slip_fee_levels.is_empty());
		let outs = sell_vdot_for_bnc(1);
		assert_eq!(outs[0].0, outs[0].1);
	});
}

#[test]
fn simulate_sell_should_match_pallet_when_asset_in_level_is_lowered() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 50);
		let outs = sell_vdot_for_bnc(1);
		assert_eq!(outs[0].0, outs[0].1);
	});
}

#[test]
fn simulate_sell_should_match_pallet_when_both_levels_are_lowered() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 50);
		set_level(BNC, 10);
		let outs = sell_vdot_for_bnc(1);
		assert_eq!(outs[0].0, outs[0].1);
	});
}

#[test]
fn simulate_sell_should_match_pallet_when_level_is_zero() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 0);
		set_level(BNC, 0);
		let outs = sell_vdot_for_bnc(1);
		assert_eq!(outs[0].0, outs[0].1);
	});
}

#[test]
fn simulate_buy_should_match_pallet_when_level_is_lowered() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 50);
		set_level(BNC, 10);
		endow(VDOT, 10 * VDOT_AMOUNT);
		let alice: AccountId = ALICE.into();
		let (_, simulated) = <Sim as AmmSimulator>::simulate_buy(VDOT, BNC, BNC_AMOUNT, Balance::MAX, &Sim::snapshot())
			.expect("buy should simulate");
		let before = Currencies::free_balance(VDOT, &alice);
		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(alice.clone()),
			BNC,
			VDOT,
			BNC_AMOUNT,
			Balance::MAX
		));
		assert_eq!(simulated.amount_in, before - Currencies::free_balance(VDOT, &alice));
	});
}

#[test]
fn simulate_sell_should_match_pallet_when_lowered_asset_is_traded_twice_in_one_block() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 50);
		let outs = sell_vdot_for_bnc(2);
		assert_eq!(outs[0].0, outs[0].1);
		// The trade-fee hooks' cut of the first sell's BNC fee leaves the pool and is not in the
		// snapshot; a level applied twice would move the second sell by far more than that.
		assert_eq!(outs[1], (61_457_096_803_937_468, 61_457_084_211_222_172));
	});
}

#[test]
fn simulate_sell_should_keep_raw_q0_in_snapshot_when_asset_level_is_lowered() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(VDOT, 50);
		endow(VDOT, VDOT_AMOUNT);
		let (after_first, _) = <Sim as AmmSimulator>::simulate_sell(VDOT, BNC, VDOT_AMOUNT, 0, &Sim::snapshot())
			.expect("sell should simulate");
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			VDOT,
			BNC,
			VDOT_AMOUNT,
			0
		));
		// A scaled Q₀ here would make the next trade apply the level twice.
		assert_eq!(
			after_first.slip_fee_hubreserve_at_block_start.get(&VDOT).copied(),
			pallet_omnipool::SlipFeeHubReserveAtBlockStart::<Runtime>::get(VDOT)
		);
	});
}

#[test]
fn simulate_sell_should_match_pallet_when_hub_asset_is_sold_twice_for_lowered_asset() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		set_level(HDX, 50);
		let amount: Balance = 25 * 1_000_000_000_000;
		endow(H2O, 2 * amount);
		let alice: AccountId = ALICE.into();
		let mut state = Sim::snapshot();
		let mut outs = Vec::new();
		for _ in 0..2 {
			let (next, simulated) =
				<Sim as AmmSimulator>::simulate_sell(H2O, HDX, amount, 0, &state).expect("sell should simulate");
			state = next;
			let before = Currencies::free_balance(HDX, &alice);
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(alice.clone()),
				H2O,
				HDX,
				amount,
				0
			));
			outs.push((simulated.amount_out, Currencies::free_balance(HDX, &alice) - before));
		}
		// As above, the hooks' cut of the first sell's HDX fee is not in the snapshot.
		assert_eq!(
			outs,
			vec![
				(88_925_378_620_228_963, 88_925_378_620_228_963),
				(88_861_664_010_275_986, 88_861_664_002_059_478),
			]
		);
		assert_eq!(
			state.slip_fee_hubreserve_at_block_start.get(&HDX).copied(),
			pallet_omnipool::SlipFeeHubReserveAtBlockStart::<Runtime>::get(HDX)
		);
	});
}

#[test]
fn fill_slip_fee_levels_should_match_snapshot_levels_when_read_from_raw_storage() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		set_level(VDOT, 50);
		set_level(BNC, 0);
		let snapshot = Sim::snapshot();
		assert_eq!(
			snapshot.slip_fee_levels,
			BTreeMap::from([(BNC, Percent::zero()), (VDOT, Percent::from_percent(50))])
		);

		let mut refilled = snapshot.clone();
		refilled.slip_fee_levels.clear();
		refilled.fill_slip_fee_levels::<Runtime>(frame_support::storage::unhashed::get_raw);
		assert_eq!(refilled.slip_fee_levels, snapshot.slip_fee_levels);
	});
}

/// A client that does not know the levels prices every asset at the full slip fee. Two intents
/// buying DOT, one selling a lowered asset: the solution is built without the levels and must
/// still execute on chain, where the lowered level applies.
#[test]
fn solution_should_execute_when_snapshot_has_no_slip_fee_levels() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let bnc_amount: Balance = 10_000 * 1_000_000_000_000;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), VDOT, VDOT_AMOUNT * 10)
		.endow_account(bob.clone(), BNC, bnc_amount * 10)
		.submit_swap_intent(alice.clone(), VDOT, DOT, VDOT_AMOUNT, 10_000_000_000, Some(10))
		.submit_swap_intent(bob.clone(), BNC, DOT, bnc_amount, 100_000_000, Some(10))
		.execute(|| {
			enable_slip_fees();
			set_level(VDOT, 0);
			set_solver_mode(SolverMode::V4);

			let call = pallet_ice::Pallet::<Runtime>::run(
				System::block_number(),
				|intents: Vec<ice_support::Intent>,
				 limits: Vec<(ice_support::IntentId, ice_support::Balance)>,
				 mut state: CombinedSimulatorState| {
					Arc::make_mut(&mut state.0).slip_fee_levels.clear();
					V4Solver::solve_with_limits(
						intents,
						limits.into_iter().collect(),
						state,
						pallet_ice::ProtocolFee::<Runtime>::get(),
					)
					.ok()
				},
			)
			.expect("solver must produce a solution");
			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("expected submit_solution call");
			};
			assert_eq!(solution.resolved_intents.len(), 2);
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution
			));
		});
}
