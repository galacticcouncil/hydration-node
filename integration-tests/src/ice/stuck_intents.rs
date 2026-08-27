//! Regression cover for the testnet "stuck intents" state
//! (`snapshots/stuck_intents`, block 747957), replaying the production path:
//! `solver_input()` → v4 solve → `validate_unsigned` → `submit_solution`.
//!
//! What was stuck: `get_valid_intents` handed the solver a DCA intent's *hard*
//! limit as `amount_out` while `validate_dca_intent_resolve` enforced
//! `max(oracle_floor, hard_limit)`. Two DCA intents here have a hard limit of
//! 1 HDX and an oracle floor of ~298_552 HDX, so the solver kept resolving them
//! at the batch rate (~284_179 HDX) — 4.8% under the floor — and the chain kept
//! rejecting the whole solution, taking the other nine intents with it.
//!
//! The floor is not caused by a stale oracle: standing alone this trade quotes
//! ~306_282 HDX, comfortably above the floor. It is the batch's own price
//! impact — 11_343 units of 222 sold into one leg — and v4 pays every intent in
//! a direction the same rate. Skipping the DCA is the correct answer for a 3%
//! slippage tolerance; the solver just had no way to know.
//!
//! `solver_intents()` now ships the floor beside the intent, admission-only, so
//! the solver excludes what it cannot pay. `amount_out` still carries the stored
//! hard limit, keeping `compute_surplus` — and the score the chain re-derives in
//! `submit_solution` — reproducible from storage alone.
//!
//! The snapshot is a 147 MB gitignored scrape, so these tests are `#[ignore]`d:
//!
//! ```sh
//! cargo test -p runtime-integration-tests --locked ice::stuck_intents -- --ignored --nocapture
//! ```

use crate::polkadot_test_net::hydradx_run_to_next_block;
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::pallet_prelude::{TransactionSource, ValidateUnsigned};
use frame_support::traits::Time;
use hydradx_runtime::{Runtime, RuntimeOrigin, System, Timestamp};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::registry::Inspect as RegistryInspect;
use ice_solver::v4::Solver as IceSolver;
use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Solution};
use sp_core::U256;
use std::collections::BTreeMap;

const SNAPSHOT: &str = "snapshots/stuck_intents";

/// The two DCA intents (222 → HDX) that jam every solution in this snapshot.
const STUCK_DCA: [IntentId; 2] = [32933704124705019147633819648673, 32933704899468270243434987520674];

/// `amount_out` the pallet hands the solver for `STUCK_DCA` — the user's hard limit.
const DCA_HARD_LIMIT: Balance = 1_000_000_000_000;
/// `amount_out` floor `validate_dca_intent_resolve` actually enforces: the
/// oracle-derived estimate less the intent's 3% slippage tolerance.
const DCA_ORACLE_FLOOR: Balance = 298_552_547_494_476_084;
/// What the solver pays those intents: clears the hard limit by 5 orders of
/// magnitude, misses the enforced oracle floor by 4.8%.
const DCA_RESOLVED_OUT: Balance = 284_179_020_151_218_420;

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;
type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

fn driver() -> crate::driver::HydrationTestDriver {
	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT)
}

fn ed(asset: AssetId) -> Balance {
	hydradx_runtime::AssetRegistry::existential_deposit(asset).unwrap_or(Balance::MAX)
}

fn report_snapshot_state() {
	println!("=== snapshot state ===");
	println!("block:     {:?}", System::block_number());
	println!("timestamp: {:?}", Timestamp::now());
	println!("fee:       {:?}", pallet_ice::ProtocolFee::<Runtime>::get());

	let events = System::events();
	println!("events in state: {}", events.len());
	for record in events.iter() {
		match &record.event {
			hydradx_runtime::RuntimeEvent::Intent(e) => println!("  Intent: {e:?}"),
			hydradx_runtime::RuntimeEvent::ICE(e) => println!("  ICE: {e:?}"),
			_ => {}
		}
	}

	let all: Vec<_> = pallet_intent::Intents::<Runtime>::iter().collect();
	println!("intents in storage: {}", all.len());
	for (id, intent) in all.iter() {
		println!("  #{id}");
		println!("    data:     {:?}", intent.data);
		println!("    deadline: {:?}", intent.deadline);
		println!(
			"    ed_in: {} (asset {}), ed_out: {} (asset {})",
			ed(intent.data.asset_in()),
			intent.data.asset_in(),
			ed(intent.data.asset_out()),
			intent.data.asset_out()
		);
		if let IntentData::Dca(ref dca) = intent.data {
			let floor = pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca);
			println!(
				"    solver sees amount_out {}, resolution enforces {} (gap {})",
				dca.amount_out,
				floor,
				floor.saturating_sub(dca.amount_out)
			);
		}
	}

	let valid = pallet_intent::Pallet::<Runtime>::get_valid_intents();
	println!("valid intents (solver input): {}", valid.len());
	for (id, intent) in valid.iter() {
		println!("  #{id}: {:?}", intent.data);
	}
	println!("=== end snapshot state ===");
}

fn dump_solution(solution: &Solution) {
	println!("=== solution ===");
	println!("score: {}", solution.score);
	println!("resolved intents: {}", solution.resolved_intents.len());
	for ri in solution.resolved_intents.iter() {
		println!(
			"  #{}: {} of {} -> {} of {} (partial: {})",
			ri.id,
			ri.data.amount_in(),
			ri.data.asset_in(),
			ri.data.amount_out(),
			ri.data.asset_out(),
			ri.data.is_partial()
		);
	}
	println!("trades: {}", solution.trades.len());
	for t in solution.trades.iter() {
		println!(
			"  {:?} in {} out {} route {:?}",
			t.direction, t.amount_in, t.amount_out, t.route
		);
	}
	println!("=== end solution ===");
}

/// Runs the exact node path: `solver_input()` (the runtime API the OCW calls)
/// feeding the v4 solver. `exclude` drops intents before solving; `blind` drops
/// the admission floors, reproducing the pre-fix behaviour.
fn solve_inner(exclude: &[IntentId], blind: bool) -> Solution {
	let (intents, _encoded_state, _eds, min_outs, fee, _mode) =
		pallet_ice::Pallet::<Runtime>::solver_input().expect("snapshot should yield solver input");
	let intents: Vec<Intent> = intents.into_iter().filter(|i| !exclude.contains(&i.id)).collect();
	let min_outs: BTreeMap<IntentId, Balance> = if blind {
		BTreeMap::new()
	} else {
		min_outs.into_iter().collect()
	};
	println!(
		"solving {} intents, fee {fee:?}, {} admission floors",
		intents.len(),
		min_outs.len()
	);

	let state =
		<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::initial_state();
	let solution = Solver::solve_with_limits(intents, min_outs, state, fee).expect("solver should produce a solution");
	dump_solution(&solution);
	solution
}

fn solve_excluding(exclude: &[IntentId]) -> Solution {
	solve_inner(exclude, false)
}

fn solve() -> Solution {
	solve_inner(&[], false)
}

/// Solve without the admission floors — what the solver saw before the fix.
fn solve_blind() -> Solution {
	solve_inner(&[], true)
}

/// Mirrors every gate in `pallet_ice::validate_unsigned_solution`, returning one
/// message per failing gate — the pallet collapses them all into
/// `InvalidTransaction::Call`, which is why the testnet failure was opaque.
fn diagnose(solution: &Solution) -> Vec<String> {
	let mut problems = Vec::new();
	let mut exec_prices: BTreeMap<(AssetId, AssetId), (Balance, Balance)> = BTreeMap::new();
	let mut score: u128 = 0;

	for ri in solution.resolved_intents.iter() {
		let id = ri.id;
		let resolve = &ri.data;

		if resolve.amount_in() < ed(resolve.asset_in()) {
			problems.push(format!(
				"intent {id}: InvalidAmount — amount_in {} < ED {} (asset {})",
				resolve.amount_in(),
				ed(resolve.asset_in()),
				resolve.asset_in()
			));
		}
		if resolve.amount_out() < ed(resolve.asset_out()) {
			problems.push(format!(
				"intent {id}: InvalidAmount — amount_out {} < ED {} (asset {})",
				resolve.amount_out(),
				ed(resolve.asset_out()),
				resolve.asset_out()
			));
		}

		let Some(intent) = pallet_intent::Pallet::<Runtime>::get_intent(id) else {
			problems.push(format!("intent {id}: IntentNotFound"));
			continue;
		};

		match pallet_intent::Pallet::<Runtime>::compute_surplus(&intent, resolve) {
			Some(surplus) => score = score.saturating_add(surplus),
			None => problems.push(format!("intent {id}: surplus underflow — resolve pays below the limit")),
		}

		if let Err(e) = pallet_intent::Pallet::<Runtime>::validate_resolve(&intent, resolve) {
			let detail = match intent.data {
				IntentData::Dca(ref dca) => format!(
					" — DCA: resolved out {} vs hard limit {} vs enforced oracle floor {}; resolved in {} vs per-trade in {}",
					resolve.amount_out(),
					dca.amount_out,
					pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca),
					resolve.amount_in(),
					dca.amount_in
				),
				IntentData::Swap(ref swap) => format!(
					" — Swap: resolved {} -> {} vs intent {} -> {} (partial: {:?})",
					resolve.amount_in(),
					resolve.amount_out(),
					swap.amount_in,
					swap.amount_out,
					swap.partial
				),
			};
			problems.push(format!("intent {id}: validate_resolve failed: {e:?}{detail}"));
		}

		let key = (resolve.asset_in(), resolve.asset_out());
		let (n, d) = *exec_prices
			.entry(key)
			.or_insert((resolve.amount_out(), resolve.amount_in()));
		let expected = U256::from(resolve.amount_in())
			.saturating_mul(U256::from(n))
			.checked_div(U256::from(d))
			.unwrap_or_default();
		let expected_out = if expected > U256::from(u128::MAX) {
			u128::MAX
		} else {
			expected.low_u128()
		};
		if expected_out.abs_diff(resolve.amount_out()) > 1 {
			problems.push(format!(
				"intent {id}: PriceInconsistency — pair ({}, {}) clears at {n}/{d}, amount_in {} implies out {}, resolved out {} (diff {})",
				key.0,
				key.1,
				resolve.amount_in(),
				expected_out,
				resolve.amount_out(),
				expected_out.abs_diff(resolve.amount_out())
			));
		}
	}

	if score != solution.score {
		problems.push(format!(
			"ScoreMismatch — solution claims {}, recomputed {}",
			solution.score, score
		));
	}

	problems
}

fn assert_accepted(solution: &Solution) {
	let problems = diagnose(solution);
	for p in problems.iter() {
		println!("REJECT: {p}");
	}

	let call = pallet_ice::Call::<Runtime>::submit_solution {
		solution: solution.clone(),
	};
	let validity = pallet_ice::Pallet::<Runtime>::validate_unsigned(TransactionSource::Local, &call);
	println!("validate_unsigned: {validity:?}");

	assert!(
		problems.is_empty(),
		"solution fails on-chain validation:\n{}",
		problems.join("\n")
	);
	assert!(
		validity.is_ok(),
		"validate_unsigned rejected the solution: {validity:?}"
	);
}

/// Every DCA intent in storage with the floor `validate_dca_intent_resolve`
/// would enforce right now. The floor is never persisted — it is recomputed
/// from `ShortOraclePrice` on each read — so this is only ever a point-in-time
/// value of the state it is evaluated against.
fn dca_floors() -> Vec<(IntentId, Balance)> {
	let mut floors: Vec<(IntentId, Balance)> = pallet_intent::Intents::<Runtime>::iter()
		.filter_map(|(id, intent)| match intent.data {
			IntentData::Dca(ref dca) => Some((id, pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca))),
			IntentData::Swap(_) => None,
		})
		.collect();
	floors.sort_by_key(|(id, _)| *id);
	floors
}

/// Splits the DCA shortfall into its two possible causes: an oracle that has
/// drifted away from the pool, versus the price impact of the batch itself.
/// Compares, for 222 → HDX, the router spot rate, the oracle rate behind the
/// floor, and the rate the solution actually clears at.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn dca_shortfall_should_be_attributed_to_oracle_drift_or_batch_price_impact() {
	use hydradx_traits::router::{RouteProvider, RouterT};

	driver().execute(|| {
		let (asset_in, asset_out) = (222u32, 0u32);
		let amount_in = 263_157_894_736_842_105_263u128;

		let route = <hydradx_runtime::Router as RouteProvider<AssetId>>::get_route(
			hydradx_traits::router::AssetPair::new(asset_in, asset_out),
		);
		let spot = <hydradx_runtime::Router as RouterT<_, _, _, _, _>>::calculate_sell_trade_amounts(&route, amount_in)
			.expect("route should quote")
			.last()
			.expect("route should have a hop")
			.amount_out;

		// Blind solve: post-fix the solver excludes this intent, so the rate it
		// *would* have been paid only exists without the floors.
		let solution = solve_blind();
		let cleared = solution
			.resolved_intents
			.iter()
			.find(|ri| ri.id == STUCK_DCA[0])
			.expect("blind solve should resolve the stuck DCA")
			.data
			.amount_out();

		println!("=== 222 -> 0, amount_in {amount_in} ===");
		println!("  standalone AMM quote:  {spot}");
		println!("  enforced oracle floor: {DCA_ORACLE_FLOOR}");
		println!("  batch clearing amount: {cleared}");
		println!(
			"  floor vs standalone quote: {:.2}%",
			(DCA_ORACLE_FLOOR as f64 / spot as f64 - 1.0) * 100.0
		);
		println!(
			"  batch vs standalone quote: {:.2}%",
			(cleared as f64 / spot as f64 - 1.0) * 100.0
		);

		// If the floor is reachable standing alone, the shortfall is the batch's
		// own price impact, not a stale oracle.
		assert!(
			spot >= DCA_ORACLE_FLOOR,
			"a standalone trade also misses the floor — the oracle itself is off the pool"
		);
	});
}

/// What the score is actually made of. `submit_solution` enforces
/// `solution.score == exec_score` as an exact equality, so every input to it has
/// to be reproducible from storage — which is the argument for keeping DCA
/// surplus anchored on the stored hard limit. Measures what that costs: the
/// score per intent, and what it would become if DCA surplus were measured
/// against the (unstored) oracle floor instead.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn score_composition_should_be_measured_against_stored_and_oracle_limits() {
	driver().execute(|| {
		let solution = solve_excluding(&STUCK_DCA);

		let mut on_hard_limit: u128 = 0;
		let mut on_floor: u128 = 0;
		println!("=== score decomposition ===");
		for ri in solution.resolved_intents.iter() {
			let intent = pallet_intent::Pallet::<Runtime>::get_intent(ri.id).expect("intent in storage");
			let surplus = pallet_intent::Pallet::<Runtime>::compute_surplus(&intent, &ri.data).expect("surplus");
			on_hard_limit = on_hard_limit.saturating_add(surplus);

			let (kind, floor_surplus) = match intent.data {
				IntentData::Dca(ref dca) => {
					let floor = pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca);
					("DCA ", ri.data.amount_out().saturating_sub(floor))
				}
				IntentData::Swap(_) => ("swap", surplus),
			};
			on_floor = on_floor.saturating_add(floor_surplus);

			println!(
				"  {kind} #{}: asset_out {:>7}, surplus {:>24} ({:>5.1}% of score)",
				ri.id,
				ri.data.asset_out(),
				surplus,
				surplus as f64 / solution.score as f64 * 100.0
			);
		}

		println!("  score on stored hard limits (today): {on_hard_limit}");
		println!("  score on oracle floors:              {on_floor}");
		assert_eq!(on_hard_limit, solution.score);

		let top = solution
			.resolved_intents
			.iter()
			.map(|ri| {
				let intent = pallet_intent::Pallet::<Runtime>::get_intent(ri.id).expect("intent in storage");
				pallet_intent::Pallet::<Runtime>::compute_surplus(&intent, &ri.data).expect("surplus")
			})
			.max()
			.expect("solution is non-empty");
		println!(
			"  largest single intent share of score: {:.1}%",
			top as f64 / solution.score as f64 * 100.0
		);
	});
}

/// Why the batch rate is so far under the floor: the DCA's 263 units ride on the
/// AMM leg of a sell 43x its size, and v4 pays every intent in a direction the
/// same rate. Quotes both sizes through the router in isolation, so the number
/// is the pool's own price impact with no solver logic involved.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn batch_price_impact_should_explain_the_gap_between_floor_and_cleared_rate() {
	use hydradx_traits::router::{RouteProvider, RouterT};

	driver().execute(|| {
		let dca_size = 263_157_894_736_842_105_263u128;
		// `amount_in` of the solution's 222 -> 0 omnipool leg.
		let batch_size = 11_343_167_479_964_213_018_025u128;

		let route = <hydradx_runtime::Router as RouteProvider<AssetId>>::get_route(
			hydradx_traits::router::AssetPair::new(222, 0),
		);
		let quote = |amount_in: Balance| -> Balance {
			<hydradx_runtime::Router as RouterT<_, _, _, _, _>>::calculate_sell_trade_amounts(&route, amount_in)
				.expect("route should quote")
				.last()
				.expect("route should have a hop")
				.amount_out
		};

		let dca_alone = quote(dca_size);
		let batch_alone = quote(batch_size);
		let state = hydradx_runtime::Omnipool::load_asset_state(222).expect("222 should be in the omnipool");

		// HDX per whole unit of 222, both assets at their own decimals.
		let rate = |amount_out: Balance, amount_in: Balance| amount_out as f64 / (amount_in as f64 / 1e18) / 1e12;

		println!("=== 222 -> 0 price impact ===");
		println!(
			"  omnipool reserve of 222: {} ({:.0} units)",
			state.reserve,
			state.reserve as f64 / 1e18
		);
		println!(
			"  DCA alone   {:>8.0} units -> {dca_alone:>26} ({:.1} HDX/unit)",
			dca_size as f64 / 1e18,
			rate(dca_alone, dca_size)
		);
		println!(
			"  batch alone {:>8.0} units -> {batch_alone:>26} ({:.1} HDX/unit)",
			batch_size as f64 / 1e18,
			rate(batch_alone, batch_size)
		);
		println!(
			"  batch is {:.1}% of the 222 reserve, and costs {:.2}% vs the DCA-sized trade",
			batch_size as f64 / state.reserve as f64 * 100.0,
			(rate(batch_alone, batch_size) / rate(dca_alone, dca_size) - 1.0) * 100.0
		);
		println!(
			"  floor {DCA_ORACLE_FLOOR} = {:.1} HDX/unit",
			rate(DCA_ORACLE_FLOOR, dca_size)
		);

		// The whole gap is the pool: a batch-sized sell quoted on its own already
		// pays less per unit than the DCA-sized sell does.
		assert!(
			rate(batch_alone, batch_size) < rate(DCA_ORACLE_FLOOR, dca_size),
			"batch-sized sell clears the floor on its own — the gap is not price impact"
		);
	});
}

/// Where the 7.4% actually comes from. A sell of 0.8% of a pool's reserve should
/// not cost anywhere near that, so this dumps the pool state, the fee config and
/// the full size-vs-rate curve for 222 → HDX.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn omnipool_should_charge_price_impact_proportional_to_trade_size_for_222() {
	use hydradx_traits::router::{RouteProvider, RouterT};

	driver().execute(|| {
		let route = <hydradx_runtime::Router as RouteProvider<AssetId>>::get_route(
			hydradx_traits::router::AssetPair::new(222, 0),
		);
		let quote = |amount_in: Balance| -> Balance {
			<hydradx_runtime::Router as RouterT<_, _, _, _, _>>::calculate_sell_trade_amounts(&route, amount_in)
				.expect("route should quote")
				.last()
				.expect("route should have a hop")
				.amount_out
		};

		let s222 = hydradx_runtime::Omnipool::load_asset_state(222).expect("222 in omnipool");
		let shdx = hydradx_runtime::Omnipool::load_asset_state(0).expect("HDX in omnipool");
		println!("=== omnipool state ===");
		println!("  222: reserve {} hub_reserve {}", s222.reserve, s222.hub_reserve);
		println!("  HDX: reserve {} hub_reserve {}", shdx.reserve, shdx.hub_reserve);
		println!("  slip fee: {:?}", pallet_omnipool::SlipFee::<Runtime>::get());
		println!(
			"  dynamic fee 222: {:?}",
			pallet_dynamic_fees::AssetFee::<Runtime>::get(222)
		);
		println!(
			"  dynamic fee HDX: {:?}",
			pallet_dynamic_fees::AssetFee::<Runtime>::get(0)
		);

		println!("=== size vs rate (HDX per unit of 222) ===");
		let mut baseline = 0f64;
		for units in [1u128, 10, 100, 263, 1_000, 5_000, 11_343] {
			let amount_in = units * 1_000_000_000_000_000_000;
			let out = quote(amount_in);
			let rate = out as f64 / units as f64 / 1e12;
			if baseline == 0.0 {
				baseline = rate;
			}
			println!(
				"  {units:>6} units -> {out:>26} = {rate:>8.1} HDX/unit ({:>6.2}% vs 1 unit, {:.3}% of reserve)",
				(rate / baseline - 1.0) * 100.0,
				amount_in as f64 / s222.reserve as f64 * 100.0
			);
		}

		// Marginal price with no fee and no impact, straight from the two hub legs.
		let spot = (s222.hub_reserve as f64 / (s222.reserve as f64 / 1e18))
			* ((shdx.reserve as f64 / 1e12) / shdx.hub_reserve as f64);

		let batch = 11_343u128 * 1_000_000_000_000_000_000;
		let with_slip = quote(batch);
		assert_ok!(hydradx_runtime::Omnipool::set_slip_fee(RuntimeOrigin::root(), None));
		let without_slip = quote(batch);

		let per_unit = |out: Balance| out as f64 / 11_343.0 / 1e12;
		println!("=== 11343-unit sell decomposed (HDX per unit of 222) ===");
		println!("  pool spot, no fee no impact: {spot:>8.1}");
		println!(
			"  with slip fee off:           {:>8.1} ({:.2}% vs spot)",
			per_unit(without_slip),
			(per_unit(without_slip) / spot - 1.0) * 100.0
		);
		println!(
			"  with slip fee on (actual):   {:>8.1} ({:.2}% vs spot)",
			per_unit(with_slip),
			(per_unit(with_slip) / spot - 1.0) * 100.0
		);
		println!(
			"  slip fee alone costs:        {:.2}%",
			(per_unit(with_slip) / per_unit(without_slip) - 1.0) * 100.0
		);
		println!(
			"  enforced floor:              {:>8.1} (spot - 3%)",
			DCA_ORACLE_FLOOR as f64 / (263_157_894_736_842_105_263f64 / 1e18) / 1e12
		);
	});
}

/// Fixture guard: the snapshot must carry the stuck intents, or every test below
/// passes vacuously.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn snapshot_should_expose_sixteen_valid_intents_of_seventeen_stored() {
	driver().execute(|| {
		report_snapshot_state();

		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 17);
		assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 16);
	});
}

/// The contract the fix rests on: `amount_out` stays the stored hard limit — so
/// `compute_surplus`, and therefore the score, is unchanged — while the floor
/// resolution enforces travels beside it.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn solver_intents_should_carry_the_enforced_floor_beside_the_stored_hard_limit() {
	driver().execute(|| {
		let (valid, floors) = pallet_intent::Pallet::<Runtime>::solver_intents();

		for id in STUCK_DCA {
			let (_, presented) = valid
				.iter()
				.find(|(i, _)| *i == id)
				.unwrap_or_else(|| panic!("intent {id} should be in the solver input"));
			let stored = pallet_intent::Pallet::<Runtime>::get_intent(id)
				.unwrap_or_else(|| panic!("intent {id} should be in storage"));
			let IntentData::Dca(ref dca) = stored.data else {
				panic!("intent {id} should be a DCA intent");
			};
			let (_, floor) = floors
				.iter()
				.find(|(i, _)| *i == id)
				.unwrap_or_else(|| panic!("intent {id} should carry an admission floor"));

			assert_eq!(presented.data.amount_out(), DCA_HARD_LIMIT);
			assert_eq!(*floor, DCA_ORACLE_FLOOR);
			assert_eq!(
				*floor,
				pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca)
			);
		}

		// Swap intents carry no floor — their own `amount_out` binds.
		for (id, intent) in valid.iter() {
			if matches!(intent.data, IntentData::Swap(_))
				&& !matches!(
					pallet_intent::Pallet::<Runtime>::get_intent(*id).map(|i| i.data),
					Some(IntentData::Dca(_))
				) {
				assert!(
					!floors.iter().any(|(i, _)| i == id),
					"swap intent {id} should not carry a floor"
				);
			}
		}
	});
}

/// With the floor in hand the solver excludes what it cannot pay, instead of
/// resolving it below the floor and having the chain reject the whole batch.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn solver_should_exclude_dca_intents_it_cannot_pay_the_enforced_floor() {
	driver().execute(|| {
		let solution = solve();

		for id in STUCK_DCA {
			assert!(
				!solution.resolved_intents.iter().any(|ri| ri.id == id),
				"intent {id} cannot be paid its floor at the batch rate and must be excluded"
			);
		}

		// Every DCA that *is* resolved clears the floor the chain will enforce.
		for ri in solution.resolved_intents.iter() {
			let intent = pallet_intent::Pallet::<Runtime>::get_intent(ri.id).expect("intent in storage");
			let IntentData::Dca(ref dca) = intent.data else {
				continue;
			};
			let floor = pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(dca);
			assert!(
				ri.data.amount_out() >= floor,
				"resolved DCA {} pays {} against floor {floor}",
				ri.id,
				ri.data.amount_out()
			);
		}
	});
}

/// Regression guard for why the floor is plumbed at all: strip it and the solver
/// resolves the two DCAs 4.8% under the floor, which the chain rejects — taking
/// the other nine intents down with it.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn solver_without_admission_floors_should_produce_a_solution_the_chain_rejects() {
	driver().execute(|| {
		let solution = solve_blind();

		for id in STUCK_DCA {
			let resolved = solution
				.resolved_intents
				.iter()
				.find(|ri| ri.id == id)
				.unwrap_or_else(|| panic!("blind solve should resolve intent {id}"));
			assert_eq!(resolved.data.amount_out(), DCA_RESOLVED_OUT);
			assert!(DCA_RESOLVED_OUT < DCA_ORACLE_FLOOR);
		}

		let problems = diagnose(&solution);
		for p in problems.iter() {
			println!("REJECT: {p}");
		}
		assert_eq!(problems.len(), 2, "both stuck DCAs should fail validation");

		let call = pallet_ice::Call::<Runtime>::submit_solution { solution };
		assert!(
			pallet_ice::Pallet::<Runtime>::validate_unsigned(TransactionSource::Local, &call).is_err(),
			"blind solution must be rejected"
		);
	});
}

/// The gate the testnet trips: `validate_unsigned` rejects the whole solution
/// because of those two intents.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn solver_solution_should_pass_unsigned_validation_on_stuck_intents_snapshot() {
	driver().execute(|| {
		report_snapshot_state();
		let solution = solve();
		assert_accepted(&solution);
	});
}

/// `Pallet::run` is what the node calls: it solves, self-validates, and returns
/// `None` when the result would be rejected. `None` here *is* the stuck state —
/// the node holds a solution it never submits.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn ice_run_should_return_a_submittable_call_on_stuck_intents_snapshot() {
	driver().execute(|| {
		let call = pallet_ice::Pallet::<Runtime>::run(
			System::block_number(),
			|intents: Vec<Intent>,
			 limits: Vec<(ice_support::IntentId, ice_support::Balance)>,
			 state: CombinedSimulatorState| {
				Solver::solve_with_limits(
					intents,
					limits.into_iter().collect(),
					state,
					pallet_ice::ProtocolFee::<Runtime>::get(),
				)
				.ok()
			},
		);

		assert!(
			call.is_some(),
			"run() rejected its own solution — the node never submits and every intent stays stuck"
		);
	});
}

/// End-to-end, the way the node does it: solve on block N, submit on N+1.
/// `submit_solution` re-runs `validate_resolve` inside `intent_resolved`, so a
/// hand-crafted submission is rejected too — the funds are safe, just frozen.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn submit_solution_should_succeed_on_stuck_intents_snapshot() {
	driver().execute(|| {
		let solution = solve();

		hydradx_run_to_next_block();

		let result = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
		println!("submit_solution: {result:?}");
		assert_ok!(result);
	});
}

/// The floor is not stored, and the two contexts that use it do not read it at
/// the same height: the node solves against block N's state while
/// `submit_solution` executes in N+1, and `EmaOracle::get_updated_entry`
/// fast-forwards to `current_block - 1`. So any fix that hands the solver the
/// floor is only sound if the value survives that one-block shift.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn dca_oracle_floor_should_be_stable_between_solve_block_and_execution_block() {
	driver().execute(|| {
		let at_solve = dca_floors();
		let solve_block = System::block_number();

		hydradx_run_to_next_block();

		let at_execution = dca_floors();
		let execution_block = System::block_number();

		println!("=== oracle floor drift, block {solve_block} -> {execution_block} ===");
		for ((id, solve), (_, exec)) in at_solve.iter().zip(at_execution.iter()) {
			println!(
				"  #{id}: {solve} -> {exec} (delta {}{})",
				if exec >= solve { "+" } else { "-" },
				exec.abs_diff(*solve)
			);
		}

		assert_eq!(
			at_solve, at_execution,
			"floor moved between the block the solver reads and the block that enforces it"
		);
	});
}

/// Proof that the two DCA intents are the sole blocker: drop them from the
/// solver input and the remaining intents solve, validate and execute against
/// the same state. This is the behaviour a solver that could see the oracle
/// floor would produce on its own.
#[test]
#[ignore = "needs the gitignored snapshots/stuck_intents scrape"]
fn submit_solution_should_succeed_when_unfillable_dca_intents_are_excluded() {
	driver().execute(|| {
		let solution = solve_excluding(&STUCK_DCA);

		assert!(
			!solution.resolved_intents.iter().any(|ri| STUCK_DCA.contains(&ri.id)),
			"excluded intents must not reappear"
		);
		assert_accepted(&solution);

		hydradx_run_to_next_block();
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution
		));
	});
}
