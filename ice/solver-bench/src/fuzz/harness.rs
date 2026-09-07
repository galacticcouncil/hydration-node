//! The soak loops. Tier 1 runs the solver in-memory against captured state and
//! checks invariants; Tier 2 submits intents + solution through the pallet and
//! relies on the on-chain re-check (conservation + `score == exec_score`) plus
//! a limit-respect check the chain skips. Every scenario is reproducible from
//! its printed seed; failures dump a SCALE-hex fixture for a regression test.

use super::gen;
use super::oracle::{self, Violation};
use super::rng::{scenario_seed, Rng};
use super::{SolverV4, State};
use crate::{get_initial_state, load_snapshot, SolverIntent};
use codec::Encode;
use ice_support::{IntentData, Partial, Solution, SwapData};
use primitives::{AccountId, AssetId, Balance};
use sp_runtime::Permill;
use std::time::Instant;

type Runtime = hydradx_runtime::Runtime;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tier {
	Solver,
	Submit,
	Both,
}

pub struct Config {
	pub snapshot: String,
	pub seconds: u64,
	pub iters: u64,
	pub seed: u64,
	pub tier: Tier,
	pub max_intents: usize,
	pub fee: Permill,
	pub max_slip: Permill,
	pub stop_on_fail: bool,
	pub check_determinism: bool,
	pub verbose: bool,
	/// Replay one exact scenario by its per-scenario seed, bypassing the
	/// run-seed/iteration mixing.
	pub scenario_seed: Option<u64>,
	pub report_every: u64,
	pub quarantine_dir: String,
}

impl Config {
	fn deadline_reached(&self, start: Instant, iter: u64) -> bool {
		if self.iters > 0 && iter >= self.iters {
			return true;
		}
		if self.seconds > 0 && start.elapsed().as_secs() >= self.seconds {
			return true;
		}
		false
	}
}

#[derive(Default)]
struct Stats {
	scenarios: u64,
	solved: u64,
	no_solution: u64,
	panics: u64,
	failures: u64,
	submit_ok: u64,
	submit_rejected: u64,
}

impl Stats {
	fn print(&self, label: &str, start: Instant) {
		let secs = start.elapsed().as_secs_f64().max(0.001);
		println!(
			"[{label}] {:>8} scen ({:.0}/s) | solved {} no_sol {} | submit ok {} rej {} | panics {} | FAIL {}",
			self.scenarios,
			self.scenarios as f64 / secs,
			self.solved,
			self.no_solution,
			self.submit_ok,
			self.submit_rejected,
			self.panics,
			self.failures,
		);
	}
}

enum SolveResult {
	Solved(Solution),
	NoSolution,
	Panicked(String),
}

fn panic_msg(p: Box<dyn std::any::Any + Send>) -> String {
	if let Some(s) = p.downcast_ref::<&str>() {
		(*s).to_string()
	} else if let Some(s) = p.downcast_ref::<String>() {
		s.clone()
	} else {
		"<non-string panic>".to_string()
	}
}

fn solve(intents: &[SolverIntent], state: &State, fee: Permill) -> SolveResult {
	let res = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		SolverV4::solve(intents.to_vec(), state.clone(), fee).ok()
	}));
	match res {
		Ok(Some(s)) => SolveResult::Solved(s),
		Ok(None) => SolveResult::NoSolution,
		Err(p) => SolveResult::Panicked(panic_msg(p)),
	}
}

/// Spot output for `amount_in` of `asset_in → asset_out`, via a single-intent
/// probe (self-consistent with the production solver's own pricing).
fn quote(state: &State, fee: Permill, asset_in: AssetId, asset_out: AssetId, amount_in: Balance) -> Option<Balance> {
	let probe = vec![SolverIntent {
		id: u128::MAX,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: 1,
			partial: Partial::No,
		}),
	}];
	std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
		SolverV4::solve(probe, state.clone(), fee).ok()
	}))
	.ok()
	.flatten()
	.and_then(|s| s.resolved_intents.first().map(|r| r.data.amount_out()))
}

pub fn run(mut cfg: Config) {
	// Solver panics are caught and reported as violations; silence the default
	// hook so caught panics don't spam the soak output with backtraces.
	std::panic::set_hook(Box::new(|_| {}));
	let mut ext = load_snapshot(&cfg.snapshot);
	ext.execute_with(|| seed_pot_and_fees(cfg.max_slip));
	// Solve with the pallet's live protocol fee so Tier-2 submissions match the
	// on-chain validation exactly.
	cfg.fee = ext.execute_with(pallet_ice::ProtocolFee::<Runtime>::get);

	println!(
		"ice-fuzz | tier={:?} seed={} max_intents={} fee={:?} | {}",
		cfg.tier,
		cfg.seed,
		cfg.max_intents,
		cfg.fee,
		if cfg.seconds > 0 {
			format!("{}s", cfg.seconds)
		} else {
			format!("{} iters", cfg.iters)
		}
	);

	match cfg.tier {
		Tier::Solver => run_tier1(&mut ext, &cfg),
		Tier::Submit => run_tier2(&mut ext, &cfg),
		Tier::Both => {
			run_tier1(&mut ext, &cfg);
			run_tier2(&mut ext, &cfg);
		}
	}
}

/// One-time chain prep: seed the fee-processor pot to ≥ED HDX (else sub-ED fee
/// takes hit `Token(BelowMinimum)`) and enable slip fees, matching the
/// integration-test driver.
fn seed_pot_and_fees(max_slip: Permill) {
	use frame_support::assert_ok;
	use orml_traits::MultiCurrency;

	let pot = pallet_fee_processor::Pallet::<Runtime>::pot_account_id();
	let ed = <Runtime as pallet_balances::Config>::ExistentialDeposit::get();
	if <hydradx_runtime::Currencies as MultiCurrency<AccountId>>::free_balance(0, &pot) < ed {
		assert_ok!(hydradx_runtime::Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			pot,
			0,
			ed as i128,
		));
	}
	assert_ok!(pallet_omnipool::Pallet::<Runtime>::set_slip_fee(
		hydradx_runtime::RuntimeOrigin::root(),
		Some(pallet_omnipool::types::SlipFeeConfig { max_slip_fee: max_slip }),
	));
}

fn run_tier1(ext: &mut frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block>, cfg: &Config) {
	ext.execute_with(|| {
		let state = get_initial_state();
		let q = |ai, ao, amt| quote(&state, cfg.fee, ai, ao, amt);

		let mut stats = Stats::default();
		let start = Instant::now();
		let mut iter = 0u64;
		while !cfg.deadline_reached(start, iter) {
			let seed = cfg.scenario_seed.unwrap_or_else(|| scenario_seed(cfg.seed, iter));
			let mut r = Rng::new(seed);
			let scenario = gen::scenario(&mut r, cfg.max_intents);
			let realized = gen::realize(&scenario.specs, false, &q);
			let intents = gen::to_solver_intents(&realized);
			stats.scenarios += 1;

			let violations = tier1_check(cfg, &intents, &state, &mut stats);
			if !violations.is_empty() {
				stats.failures += 1;
				report_failure("tier1", seed, scenario.archetype, &intents, &violations, cfg);
				if cfg.stop_on_fail {
					break;
				}
			}

			iter += 1;
			if iter % cfg.report_every == 0 {
				stats.print("tier1", start);
			}
		}
		stats.print("tier1 DONE", start);
	});
}

fn tier1_check(cfg: &Config, intents: &[SolverIntent], state: &State, stats: &mut Stats) -> Vec<Violation> {
	let mut violations = Vec::new();
	let mut solution: Option<Solution> = None;

	match solve(intents, state, cfg.fee) {
		SolveResult::Panicked(msg) => {
			stats.panics += 1;
			violations.push(Violation {
				kind: "solver_panic",
				detail: msg,
			});
		}
		SolveResult::NoSolution => stats.no_solution += 1,
		SolveResult::Solved(sol) => {
			stats.solved += 1;
			violations.extend(oracle::check_solution(intents, &sol, cfg.fee, true));
			if cfg.verbose {
				println!(
					"  solved: {} of {} intents, {} trades, score {}",
					sol.resolved_intents.len(),
					intents.len(),
					sol.trades.len(),
					sol.score,
				);
			}
			solution = Some(sol);
		}
	}

	// Determinism — the same input must give a byte-identical solution
	// (collators must agree).
	if cfg.check_determinism {
		if let Some(sol) = &solution {
			if let SolveResult::Solved(again) = solve(intents, state, cfg.fee) {
				if sol.encode() != again.encode() {
					violations.push(Violation {
						kind: "nondeterministic",
						detail: "two different solutions for identical input".into(),
					});
				}
			}
		}
	}

	violations
}

fn run_tier2(ext: &mut frame_remote_externalities::RemoteExternalities<hydradx_runtime::Block>, cfg: &Config) {
	// State for quoting limits — pools are unchanged between scenarios because
	// each scenario's storage writes are rolled back.
	let state = ext.execute_with(get_initial_state);
	let mut stats = Stats::default();
	let start = Instant::now();
	let mut iter = 0u64;

	while !cfg.deadline_reached(start, iter) {
		let seed = scenario_seed(cfg.seed, iter);
		let mut r = Rng::new(seed);
		let scenario = gen::scenario(&mut r, cfg.max_intents);
		let corrupt_test = r.chance(1, 8);
		stats.scenarios += 1;

		// realize() quotes via the solver, so it must run inside the externality.
		let (intents_hex, violations) =
			ext.execute_with(|| tier2_scenario(cfg, seed, &scenario.specs, &state, corrupt_test, &mut stats));

		if !violations.is_empty() {
			stats.failures += 1;
			report_failure_hex("tier2", seed, scenario.archetype, intents_hex, &violations, cfg);
			if cfg.stop_on_fail {
				break;
			}
		}

		iter += 1;
		if iter % cfg.report_every == 0 {
			stats.print("tier2", start);
		}
	}
	stats.print("tier2 DONE", start);
}

/// Submit `realized` as real intents, run the solver, submit the solution, and
/// check it executes. All storage writes are rolled back at the end so the next
/// scenario starts from the pristine snapshot. Returns (intents-hex, violations).
fn tier2_scenario(
	cfg: &Config,
	seed: u64,
	specs: &[gen::IntentSpec],
	state: &State,
	corrupt_test: bool,
	stats: &mut Stats,
) -> (String, Vec<Violation>) {
	use frame_support::storage::{with_transaction, TransactionOutcome};
	use ice_support::{IntentDataInput, SwapParams};
	use orml_traits::MultiCurrency;

	let outcome: Result<(String, Vec<Violation>), sp_runtime::DispatchError> = with_transaction(|| {
		// Start each scenario from a clean intent set (the snapshot may carry
		// pending intents); rolled back with everything else.
		crate::clear_intent_storage();
		let realized = gen::realize(specs, true, &|ai, ao, amt| quote(state, cfg.fee, ai, ao, amt));
		let mut violations = Vec::new();

		// Endow a fresh account per intent and submit it; skip any the pallet
		// rejects at validation (out of scope for the solver oracle).
		for (i, ri) in realized.iter().enumerate() {
			if ri.asset_in == ri.asset_out {
				continue;
			}
			let who: AccountId = account_from(seed, i);
			let endow = ri.amount_in.saturating_mul(2).min(i128::MAX as u128) as i128;
			let _ = hydradx_runtime::Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				who.clone(),
				ri.asset_in,
				endow,
			);
			let input = pallet_intent::types::IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: ri.asset_in,
					asset_out: ri.asset_out,
					amount_in: ri.amount_in,
					amount_out: ri.amount_out,
					partial: ri.partial,
				}),
				deadline: None,
				on_resolved: None,
			};
			let _ = pallet_intent::Pallet::<Runtime>::submit_intent(hydradx_runtime::RuntimeOrigin::signed(who), input);
		}

		let valid = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		let originals: Vec<SolverIntent> = valid
			.iter()
			.map(|(id, si)| SolverIntent {
				id: *id,
				data: si.data.clone(),
			})
			.collect();
		let intents_hex = hex::encode(originals.encode());

		// Solver panics are swallowed here (Tier 1 is the panic oracle) — a
		// panic just yields no solution and a clean rollback.
		let fee = cfg.fee;
		let call =
			pallet_ice::Pallet::<Runtime>::run(hydradx_runtime::System::block_number(), move |ints, limits, st| {
				std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
					SolverV4::solve_with_limits(ints, limits.into_iter().collect(), st, fee).ok()
				}))
				.ok()
				.flatten()
			});

		let Some(pallet_ice::Call::submit_solution { solution, .. }) = call else {
			stats.no_solution += 1;
			return TransactionOutcome::Rollback(Ok((intents_hex, violations)));
		};

		// Limit respect — the chain does NOT re-check this on submit.
		violations.extend(oracle::check_solution(&originals, &solution, cfg.fee, false));

		if corrupt_test {
			// Over-pay one resolved intent by 1 unit; the pallet must reject it
			// (conservation / score mismatch).
			let mut bad = solution.clone();
			if let Some(first) = bad.resolved_intents.get_mut(0) {
				if let IntentData::Swap(s) = &mut first.data {
					s.amount_out = s.amount_out.saturating_add(1);
				}
			}
			let res = pallet_ice::Pallet::<Runtime>::submit_solution(hydradx_runtime::RuntimeOrigin::none(), bad);
			if res.is_ok() {
				violations.push(Violation {
					kind: "validator_accepted_corrupt",
					detail: "submit_solution accepted a solution overpaying a user by 1".into(),
				});
			}
		} else {
			match pallet_ice::Pallet::<Runtime>::submit_solution(hydradx_runtime::RuntimeOrigin::none(), solution) {
				Ok(_) => stats.submit_ok += 1,
				Err(e) => {
					stats.submit_rejected += 1;
					violations.push(Violation {
						kind: "submit_rejected",
						detail: format!("pallet rejected a solver solution: {e:?}"),
					});
				}
			}
		}

		// Suppress the unused warning for the orml import on builds where the
		// trait method isn't otherwise referenced.
		let _ = <hydradx_runtime::Currencies as MultiCurrency<AccountId>>::free_balance;

		TransactionOutcome::Rollback(Ok((intents_hex, violations)))
	});

	outcome.unwrap_or_else(|_| (String::new(), vec![]))
}

fn account_from(seed: u64, i: usize) -> AccountId {
	let mut bytes = [0u8; 32];
	bytes[0..8].copy_from_slice(&seed.to_le_bytes());
	bytes[8..16].copy_from_slice(&(i as u64).to_le_bytes());
	bytes[16] = 0xF0;
	bytes[17] = 0x0D;
	sp_runtime::AccountId32::new(bytes)
}

fn report_failure(
	label: &str,
	seed: u64,
	archetype: &str,
	intents: &[SolverIntent],
	violations: &[Violation],
	cfg: &Config,
) {
	report_failure_hex(
		label,
		seed,
		archetype,
		hex::encode(intents.to_vec().encode()),
		violations,
		cfg,
	);
}

fn report_failure_hex(
	label: &str,
	seed: u64,
	archetype: &str,
	intents_hex: String,
	violations: &[Violation],
	cfg: &Config,
) {
	println!("\n===== {label} VIOLATION =====");
	println!("seed       : {seed}");
	println!("archetype  : {archetype}");
	for v in violations {
		println!("  [{}] {}", v.kind, v.detail);
	}
	let path = format!("{}/fuzz_{label}_{seed}.hex", cfg.quarantine_dir);
	if std::fs::create_dir_all(&cfg.quarantine_dir).is_ok() {
		let body = format!("# seed={seed} archetype={archetype}\n{intents_hex}\n");
		if std::fs::write(&path, body).is_ok() {
			println!("fixture    : {path}");
		}
	}
	println!("============================\n");
}
