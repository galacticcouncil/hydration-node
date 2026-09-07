//! Solution invariants — the oracle. An optimizer has no cheap ground-truth
//! optimum, so we assert properties every *valid* solution must satisfy
//! regardless of optimality. The conservation rule mirrors the pallet's
//! `settle_matched_fees`; the limit-respect rule is something the chain does
//! NOT re-check on submit (`pallet-ice` lib.rs `NOTE`), so a fuzzer is the only
//! thing that catches a solver shortchanging a user.

use crate::SolverIntent;
use ice_support::{Solution, MAX_NUMBER_OF_RESOLVED_INTENTS, MAX_NUMBER_OF_SOLUTION_TRADES};
use primitives::{AssetId, Balance};
use sp_runtime::Permill;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Violation {
	pub kind: &'static str,
	pub detail: String,
}

impl Violation {
	fn new(kind: &'static str, detail: impl Into<String>) -> Self {
		Violation {
			kind,
			detail: detail.into(),
		}
	}
}

/// Check a solver-produced solution against the originally submitted intents.
/// `conservation` runs the per-asset value-conservation replica; pass `false`
/// in Tier 2 where the pallet's own check on submit is authoritative.
pub fn check_solution(
	originals: &[SolverIntent],
	solution: &Solution,
	fee: Permill,
	conservation: bool,
) -> Vec<Violation> {
	let mut v = Vec::new();

	if solution.resolved_intents.len() > MAX_NUMBER_OF_RESOLVED_INTENTS as usize {
		v.push(Violation::new(
			"bounds_resolved",
			format!("{}", solution.resolved_intents.len()),
		));
	}
	if solution.trades.len() > MAX_NUMBER_OF_SOLUTION_TRADES as usize {
		v.push(Violation::new("bounds_trades", format!("{}", solution.trades.len())));
	}

	let by_id: BTreeMap<u128, &SolverIntent> = originals.iter().map(|i| (i.id, i)).collect();
	let mut seen: BTreeSet<u128> = BTreeSet::new();

	for r in solution.resolved_intents.iter() {
		if !seen.insert(r.id) {
			v.push(Violation::new("duplicate_resolved", format!("id={}", r.id)));
		}
		let Some(orig) = by_id.get(&r.id) else {
			v.push(Violation::new("unknown_id", format!("id={}", r.id)));
			continue;
		};

		let orig_in = orig.data.amount_in();
		let res_in = r.data.amount_in();
		if orig.data.is_partial() {
			if res_in == 0 || res_in > orig_in {
				v.push(Violation::new(
					"partial_amount_in",
					format!("id={} resolved_in={} original_in={}", r.id, res_in, orig_in),
				));
			}
		} else if res_in != orig_in {
			v.push(Violation::new(
				"nonpartial_amount_in",
				format!("id={} resolved_in={} original_in={}", r.id, res_in, orig_in),
			));
		}

		// Limit respect: the user must receive at least their minimum (pro-rata
		// for partials). `surplus` returns None when the limit is breached.
		if orig.data.surplus(&r.data).is_none() {
			v.push(Violation::new(
				"limit_violated",
				format!(
					"id={} got_out={} min_out={}",
					r.id,
					r.data.amount_out(),
					orig.data.amount_out()
				),
			));
		}
	}

	if conservation {
		v.extend(check_conservation(solution, fee));
	}
	v
}

/// Per-asset conservation, mirroring the pallet: for each asset X the holding
/// residual `intent_in + pool_out − intent_out − pool_in` must cover
/// `fee · matched`, where `matched = intent_in − pool_in`.
fn check_conservation(solution: &Solution, fee: Permill) -> Vec<Violation> {
	// (intent_in, intent_out, pool_in, pool_out)
	let mut flow: BTreeMap<AssetId, (Balance, Balance, Balance, Balance)> = BTreeMap::new();

	for r in solution.resolved_intents.iter() {
		flow.entry(r.data.asset_in()).or_default().0 += r.data.amount_in();
		flow.entry(r.data.asset_out()).or_default().1 += r.data.amount_out();
	}
	for t in solution.trades.iter() {
		if let (Some(first), Some(last)) = (t.route.first(), t.route.last()) {
			flow.entry(first.asset_in).or_default().2 += t.amount_in;
			flow.entry(last.asset_out).or_default().3 += t.amount_out;
		}
	}

	// Per-asset integer floor-rounding accumulates a few units, and this replica
	// runs on the solver's *claimed* amounts whereas the pallet's authoritative
	// check (Tier 2) runs on actual re-executed amounts. Allow a small
	// batch-scaled slack; a genuine conservation break is orders of magnitude
	// larger than the number of floor operations in a batch.
	let slack = (solution.resolved_intents.len() + solution.trades.len()) as i128 + 4;
	let mut v = Vec::new();
	for (asset, (intent_in, intent_out, pool_in, pool_out)) in flow {
		let matched = intent_in.saturating_sub(pool_in);
		let expected_fee = fee.mul_floor(matched) as i128;
		let residual = (intent_in as i128 + pool_out as i128) - (intent_out as i128 + pool_in as i128);
		if residual + slack < expected_fee {
			v.push(Violation::new(
				"conservation",
				format!("asset={asset} residual={residual} expected_fee={expected_fee} slack={slack}"),
			));
		}
	}
	v
}
