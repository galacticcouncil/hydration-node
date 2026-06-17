//! v3-vs-v4 differential accounting. Score is the solver's own objective and is
//! value-blind across assets/decimals, so absolute scores across heterogeneous
//! scenarios aren't directly comparable — we report the per-scenario *relative*
//! score delta (basis points), alongside the cleaner resolved-count and
//! trade-count comparisons and the coverage cases where only one solver finds a
//! solution at all.

use crate::SolverIntent;
use ice_support::{IntentData, Solution};
use std::collections::BTreeMap;

pub struct DiffRecord {
	pub bps: i64,
	pub seed: u64,
	pub archetype: &'static str,
	pub v3_score: u128,
	pub v4_score: u128,
	pub v3_res: usize,
	pub v4_res: usize,
	pub v3_tr: usize,
	pub v4_tr: usize,
}

#[derive(Default, Clone, Copy)]
struct ArchAgg {
	compared: u64,
	sum_bps: i128,
	better: u64,
	worse: u64,
	tie: u64,
	res_fewer: u64,
	res_more: u64,
	tr_fewer: u64,
	tr_more: u64,
}

#[derive(Default)]
pub struct DiffReport {
	records: Vec<DiffRecord>,
	v3_only: u64,
	v4_only: u64,
	both_none: u64,
	per_arch: BTreeMap<&'static str, ArchAgg>,
}

/// (v4 − v3) / v3 in basis points, clamped so a near-zero v3 baseline can't make
/// one scenario dominate the aggregate.
fn rel_bps(v3: u128, v4: u128) -> i64 {
	if v3 == 0 {
		return if v4 == 0 { 0 } else { 1_000_000 };
	}
	let raw = (v4 as i128 - v3 as i128) * 10_000 / v3 as i128;
	raw.clamp(-1_000_000, 1_000_000) as i64
}

impl DiffReport {
	pub fn record(&mut self, seed: u64, archetype: &'static str, v3: Option<&Solution>, v4: Option<&Solution>) {
		match (v3, v4) {
			(Some(a), Some(b)) => {
				let bps = rel_bps(a.score, b.score);
				let e = self.per_arch.entry(archetype).or_default();
				e.compared += 1;
				e.sum_bps += bps as i128;
				match b.score.cmp(&a.score) {
					std::cmp::Ordering::Greater => e.better += 1,
					std::cmp::Ordering::Less => e.worse += 1,
					std::cmp::Ordering::Equal => e.tie += 1,
				}
				match b.resolved_intents.len().cmp(&a.resolved_intents.len()) {
					std::cmp::Ordering::Less => e.res_fewer += 1,
					std::cmp::Ordering::Greater => e.res_more += 1,
					std::cmp::Ordering::Equal => {}
				}
				match b.trades.len().cmp(&a.trades.len()) {
					std::cmp::Ordering::Less => e.tr_fewer += 1,
					std::cmp::Ordering::Greater => e.tr_more += 1,
					std::cmp::Ordering::Equal => {}
				}
				self.records.push(DiffRecord {
					bps,
					seed,
					archetype,
					v3_score: a.score,
					v4_score: b.score,
					v3_res: a.resolved_intents.len(),
					v4_res: b.resolved_intents.len(),
					v3_tr: a.trades.len(),
					v4_tr: b.trades.len(),
				});
			}
			(Some(_), None) => self.v3_only += 1,
			(None, Some(_)) => self.v4_only += 1,
			(None, None) => self.both_none += 1,
		}
	}

	pub fn print(&self) {
		let n = self.records.len();
		println!("\n========== v3-vs-v4 DIFFERENTIAL ==========");
		if n == 0 {
			println!("no scenarios where both solvers produced a solution");
			println!("coverage: v3-only {} | v4-only {} | both-none {}", self.v3_only, self.v4_only, self.both_none);
			return;
		}

		let (mut wins, mut losses, mut ties) = (0u64, 0u64, 0u64);
		let (mut res_more, mut res_fewer, mut res_eq) = (0u64, 0u64, 0u64);
		let (mut tr_fewer, mut tr_more, mut tr_eq) = (0u64, 0u64, 0u64);
		let (mut sum_v3_res, mut sum_v4_res, mut sum_v3_tr, mut sum_v4_tr) = (0u64, 0u64, 0u64, 0u64);
		let mut sum_bps: i128 = 0;
		for r in &self.records {
			match r.v4_score.cmp(&r.v3_score) {
				std::cmp::Ordering::Greater => wins += 1,
				std::cmp::Ordering::Less => losses += 1,
				std::cmp::Ordering::Equal => ties += 1,
			}
			match r.v4_res.cmp(&r.v3_res) {
				std::cmp::Ordering::Greater => res_more += 1,
				std::cmp::Ordering::Less => res_fewer += 1,
				std::cmp::Ordering::Equal => res_eq += 1,
			}
			match r.v4_tr.cmp(&r.v3_tr) {
				std::cmp::Ordering::Less => tr_fewer += 1,
				std::cmp::Ordering::Greater => tr_more += 1,
				std::cmp::Ordering::Equal => tr_eq += 1,
			}
			sum_v3_res += r.v3_res as u64;
			sum_v4_res += r.v4_res as u64;
			sum_v3_tr += r.v3_tr as u64;
			sum_v4_tr += r.v4_tr as u64;
			sum_bps += r.bps as i128;
		}

		let mut bps: Vec<i64> = self.records.iter().map(|r| r.bps).collect();
		bps.sort_unstable();
		let pct = |p: f64| bps[((n as f64 - 1.0) * p) as usize];
		let mean = sum_bps / n as i128;

		println!("compared scenarios (both solved): {n}");
		println!("coverage: v3-only {} | v4-only {} | both-none {}", self.v3_only, self.v4_only, self.both_none);
		println!(
			"SCORE   v4 wins {} ({:.1}%) | v4 loses {} ({:.1}%) | tie {} ({:.1}%)",
			wins,
			100.0 * wins as f64 / n as f64,
			losses,
			100.0 * losses as f64 / n as f64,
			ties,
			100.0 * ties as f64 / n as f64,
		);
		println!(
			"SCORE   relative delta bps (v4 vs v3): mean {} | p10 {} | median {} | p90 {}",
			mean,
			pct(0.10),
			pct(0.50),
			pct(0.90),
		);
		println!(
			"RESOLVED v4 more {} | fewer {} | equal {} | totals v3={} v4={}",
			res_more, res_fewer, res_eq, sum_v3_res, sum_v4_res,
		);
		println!(
			"TRADES   v4 fewer {} | more {} | equal {} | totals v3={} v4={}",
			tr_fewer, tr_more, tr_eq, sum_v3_tr, sum_v4_tr,
		);

		println!("\nby archetype (compared | score W/L/T | resolved v4 fewer/more | trades v4 fewer/more):");
		for (arch, a) in &self.per_arch {
			println!(
				"  {:<14} {:>6} | {:>5}/{:>5}/{:>5} | res {:>4}/{:<4} | tr {:>4}/{:<4}",
				arch, a.compared, a.better, a.worse, a.tie, a.res_fewer, a.res_more, a.tr_fewer, a.tr_more,
			);
		}

		let mut worst: Vec<&DiffRecord> = self.records.iter().filter(|r| r.v4_score < r.v3_score).collect();
		worst.sort_by_key(|r| r.bps);
		println!("\nworst v4 regressions (replay with FUZZ_SCENARIO_SEED=<seed> FUZZ_MAX_INTENTS=50):");
		for r in worst.iter().take(15) {
			println!(
				"  bps {:>8} | {:<14} seed={} | v3_score={} v4_score={} | res {}->{} tr {}->{}",
				r.bps, r.archetype, r.seed, r.v3_score, r.v4_score, r.v3_res, r.v4_res, r.v3_tr, r.v4_tr,
			);
		}
		println!("===========================================\n");
	}
}

/// Verbose side-by-side dump of one scenario (for replaying a worst-case seed).
pub fn dump_scenario_detail(seed: u64, archetype: &str, intents: &[SolverIntent], v3: Option<&Solution>, v4: Option<&Solution>) {
	println!("\n----- scenario seed={seed} archetype={archetype} ({} intents) -----", intents.len());
	for (i, it) in intents.iter().enumerate() {
		if let IntentData::Swap(s) = &it.data {
			println!(
				"  in[{i}] id={} {}->{} amount_in={} min_out={} partial={:?}",
				it.id, s.asset_in, s.asset_out, s.amount_in, s.amount_out, s.partial
			);
		}
	}
	for (label, sol) in [("v3", v3), ("v4", v4)] {
		match sol {
			None => println!("  {label}: NO SOLUTION"),
			Some(s) => {
				println!("  {label}: score={} resolved={} trades={}", s.score, s.resolved_intents.len(), s.trades.len());
				for r in s.resolved_intents.iter() {
					if let IntentData::Swap(sd) = &r.data {
						println!("       id={} {}->{} in={} out={}", r.id, sd.asset_in, sd.asset_out, sd.amount_in, sd.amount_out);
					}
				}
			}
		}
	}
	if let (Some(a), Some(b)) = (v3, v4) {
		println!("  delta: v4-v3 score = {} ({} bps)", b.score as i128 - a.score as i128, rel_bps(a.score, b.score));
	}
}
