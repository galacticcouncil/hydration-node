//! Standalone ICE solver fuzzing / property-soak utility. Run manually, not in
//! CI. Loads the same `mainnet_apr` snapshot the integration tests use, then
//! continuously generates scenarios, solves them, and (Tier 2) submits the
//! solutions through the pallet — asserting solution invariants throughout.
//!
//! Configure via environment variables:
//!   FUZZ_SECONDS    soak duration in seconds (default 60; 0 = use FUZZ_ITERS)
//!   FUZZ_ITERS      fixed scenario count (default 0 = use FUZZ_SECONDS)
//!   FUZZ_SEED       run seed for reproducibility (default 0 = derive from time)
//!   FUZZ_TIER       solver | submit | both          (default both)
//!   FUZZ_SOLVER     v3 | v4 | diff                  (default v4)
//!   FUZZ_MAX_INTENTS  max intents per scenario       (default 30)
//!   FUZZ_MAX_SLIP_PCT slip-fee cap percent           (default 5)
//!   FUZZ_KEEP_GOING   1 = don't stop on first failure (default 0)
//!   FUZZ_SNAPSHOT     snapshot path override
//!   FUZZ_OUT          quarantine dir for failure fixtures (default ./fuzz-findings)
//!
//! Examples:
//!   FUZZ_SECONDS=600 FUZZ_TIER=both FUZZ_SOLVER=diff \
//!     cargo run -p ice-solver-bench --release --bin ice-fuzz
//!   FUZZ_SEED=12345 FUZZ_ITERS=1 cargo run -p ice-solver-bench --bin ice-fuzz

use ice_solver_bench::fuzz::harness::{run, Config, Tier};
use ice_solver_bench::fuzz::SolverSel;
use sp_runtime::Permill;
use std::time::{SystemTime, UNIX_EPOCH};

fn env_str(key: &str, default: &str) -> String {
	std::env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_u64(key: &str, default: u64) -> u64 {
	std::env::var(key).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

fn main() {
	let default_snapshot = concat!(
		env!("CARGO_MANIFEST_DIR"),
		"/../../integration-tests/snapshots/ice/mainnet_apr"
	);

	let tier = match env_str("FUZZ_TIER", "both").to_lowercase().as_str() {
		"solver" => Tier::Solver,
		"submit" => Tier::Submit,
		_ => Tier::Both,
	};
	let solver = match env_str("FUZZ_SOLVER", "v4").to_lowercase().as_str() {
		"v3" => SolverSel::V3,
		"diff" => SolverSel::Diff,
		_ => SolverSel::V4,
	};

	let seed = match env_u64("FUZZ_SEED", 0) {
		0 => SystemTime::now()
			.duration_since(UNIX_EPOCH)
			.map(|d| d.as_nanos() as u64)
			.unwrap_or(0x1234_5678)
			.max(1),
		s => s,
	};

	// Replay one exact scenario from a worst-list seed (forces a single verbose
	// solver iteration so it works regardless of FUZZ_SECONDS/FUZZ_ITERS).
	let scenario_seed = std::env::var("FUZZ_SCENARIO_SEED").ok().and_then(|v| v.parse::<u64>().ok());
	let replay = scenario_seed.is_some();

	let cfg = Config {
		snapshot: env_str("FUZZ_SNAPSHOT", default_snapshot),
		seconds: if replay { 0 } else { env_u64("FUZZ_SECONDS", 60) },
		iters: if replay { 1 } else { env_u64("FUZZ_ITERS", 0) },
		seed,
		tier: if replay { Tier::Solver } else { tier },
		solver: if replay { SolverSel::Diff } else { solver },
		max_intents: env_u64("FUZZ_MAX_INTENTS", 30) as usize,
		fee: Permill::zero(), // overridden with the pallet's live fee at runtime
		max_slip: Permill::from_percent(env_u64("FUZZ_MAX_SLIP_PCT", 5) as u32),
		stop_on_fail: env_u64("FUZZ_KEEP_GOING", 0) == 0,
		check_determinism: true,
		verbose: replay || env_u64("FUZZ_VERBOSE", 0) == 1,
		scenario_seed,
		report_every: env_u64("FUZZ_REPORT_EVERY", 200),
		quarantine_dir: env_str("FUZZ_OUT", "./fuzz-findings"),
	};

	run(cfg);
}
