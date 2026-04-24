//! Ring trade detection and filling.
//!
//! Detects 3-asset cycles (A→B→C→A) in remaining flow graph and fills them
//! at the bottleneck volume using spot-price-consistent rates.
//! Ring trades avoid AMM interaction entirely — assets flow peer-to-peer around the cycle.

use crate::common::flow_graph::{FlowGraph, IntentEntry, MatchFill, Pair};
use crate::common::{calc_amount_out, mul_div};
use hydra_dx_math::types::Ratio;
use ice_support::{AssetId, Balance};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec;
use sp_std::vec::Vec;

/// A ring trade through 3 assets with fills on each edge.
#[derive(Debug, Clone)]
pub struct RingTrade {
	/// Three edges forming the cycle: (A→B), (B→C), (C→A)
	pub edges: Vec<(Pair, Vec<MatchFill>)>,
}

/// Detect and fill feasible 3-asset cycles (A→B→C→A) in the remaining flow graph.
///
/// Only detects 3-asset rings. Longer cycles (4+ assets) are not attempted.
///
/// Uses spot prices to compute fill rates (not limit prices).
/// Limit prices are only used for the feasibility check — ensuring each
/// participant receives at least their minimum.
///
/// Fills at bottleneck volume and repeats until no more rings are found.
pub fn detect_rings(graph: &mut FlowGraph, spot_prices: &BTreeMap<AssetId, Ratio>) -> Vec<RingTrade> {
	let mut rings = Vec::new();

	loop {
		let mut found = false;

		let pairs: Vec<Pair> = graph.keys().copied().collect();

		for &(a, b) in &pairs {
			let bc_pairs: Vec<AssetId> = pairs
				.iter()
				.filter(|&&(x, y)| x == b && y != a)
				.map(|&(_, y)| y)
				.collect();

			for c in bc_pairs {
				if !graph.contains_key(&(c, a)) {
					continue;
				}

				let ab_has_volume = graph
					.get(&(a, b))
					.map(|e| e.iter().any(|i| i.remaining_in > 0))
					.unwrap_or(false);
				let bc_has_volume = graph
					.get(&(b, c))
					.map(|e| e.iter().any(|i| i.remaining_in > 0))
					.unwrap_or(false);
				let ca_has_volume = graph
					.get(&(c, a))
					.map(|e| e.iter().any(|i| i.remaining_in > 0))
					.unwrap_or(false);

				if !ab_has_volume || !bc_has_volume || !ca_has_volume {
					continue;
				}

				// Get spot prices for all 3 assets
				let (Some(pa), Some(pb), Some(pc)) = (spot_prices.get(&a), spot_prices.get(&b), spot_prices.get(&c))
				else {
					continue;
				};

				// Feasibility: check that each edge's best intent can be satisfied at spot rate
				let ab_best = first_with_remaining(graph.get(&(a, b)).expect("edge (a,b) verified above"));
				let bc_best = first_with_remaining(graph.get(&(b, c)).expect("edge (b,c) verified above"));
				let ca_best = first_with_remaining(graph.get(&(c, a)).expect("edge (c,a) verified above"));

				let (ab_best, bc_best, ca_best) = match (ab_best, bc_best, ca_best) {
					(Some(ab), Some(bc), Some(ca)) => (ab, bc, ca),
					_ => continue,
				};

				// Check each intent's limit rate is met at spot: compare against the
				// intent's full `original_amount_in` → `min_amount_out` bound, which is
				// equivalent to asking "is spot_rate ≥ limit_rate?" independent of the
				// current `remaining_in` volume. Using `remaining_in` here would compare
				// a scaled-down output against an unscaled minimum, spuriously rejecting
				// partials whose cap is below their original `amount_in`.
				let ab_spot = calc_amount_out(ab_best.original_amount_in, pa, pb);
				let bc_spot = calc_amount_out(bc_best.original_amount_in, pb, pc);
				let ca_spot = calc_amount_out(ca_best.original_amount_in, pc, pa);

				let (Some(ab_out_at_spot), Some(bc_out_at_spot), Some(ca_out_at_spot)) = (ab_spot, bc_spot, ca_spot)
				else {
					continue;
				};

				if ab_out_at_spot < ab_best.min_amount_out
					|| bc_out_at_spot < bc_best.min_amount_out
					|| ca_out_at_spot < ca_best.min_amount_out
				{
					continue;
				}

				// Compute bottleneck: convert all edge volumes to asset A equivalent at spot
				let ab_vol_a = U256::from(ab_best.remaining_in);
				let bc_vol_a = calc_amount_out(bc_best.remaining_in, pb, pa)
					.map(U256::from)
					.unwrap_or(U256::zero());
				let ca_vol_a = calc_amount_out(ca_best.remaining_in, pc, pa)
					.map(U256::from)
					.unwrap_or(U256::zero());

				let bottleneck_a = ab_vol_a.min(bc_vol_a).min(ca_vol_a);
				if bottleneck_a.is_zero() {
					continue;
				}

				let bottleneck_a_128: Balance = bottleneck_a.try_into().unwrap_or(0);
				if bottleneck_a_128 == 0 {
					continue;
				}

				// Fill amounts at spot rates
				// AB: input = bottleneck_a of A, output = calc_amount_out(bottleneck_a, pa, pb) of B
				let ab_amount_in = bottleneck_a_128;
				let ab_amount_out = calc_amount_out(ab_amount_in, pa, pb).unwrap_or(0);

				// BC: input = ab_amount_out of B, output at spot
				let bc_amount_in = ab_amount_out;
				let bc_amount_out = calc_amount_out(bc_amount_in, pb, pc).unwrap_or(0);

				// CA: input = bc_amount_out of C, output at spot.
				// Note: ca_amount_out may differ from ab_amount_in by ≤1 due to
				// accumulated rounding across the 3 spot conversions. The protocol
				// absorbs this dust difference.
				let ca_amount_in = bc_amount_out;
				let ca_amount_out = calc_amount_out(ca_amount_in, pc, pa).unwrap_or(0);

				if ab_amount_in == 0
					|| ab_amount_out == 0
					|| bc_amount_in == 0
					|| bc_amount_out == 0
					|| ca_amount_in == 0
					|| ca_amount_out == 0
				{
					continue;
				}

				// Final feasibility: verify each fill meets the intent's limit
				// (spot rate should satisfy, but check after rounding)
				let ab_entries = graph.get(&(a, b)).expect("edge (a,b) verified above");
				if !fills_meet_limits(ab_entries, ab_amount_in, ab_amount_out) {
					continue;
				}
				let bc_entries = graph.get(&(b, c)).expect("edge (b,c) verified above");
				if !fills_meet_limits(bc_entries, bc_amount_in, bc_amount_out) {
					continue;
				}
				let ca_entries = graph.get(&(c, a)).expect("edge (c,a) verified above");
				if !fills_meet_limits(ca_entries, ca_amount_in, ca_amount_out) {
					continue;
				}

				let ab_fill = fill_intent(
					graph.get_mut(&(a, b)).expect("edge (a,b) verified above"),
					ab_amount_in,
					ab_amount_out,
				);
				let bc_fill = fill_intent(
					graph.get_mut(&(b, c)).expect("edge (b,c) verified above"),
					bc_amount_in,
					bc_amount_out,
				);
				let ca_fill = fill_intent(
					graph.get_mut(&(c, a)).expect("edge (c,a) verified above"),
					ca_amount_in,
					ca_amount_out,
				);

				rings.push(RingTrade {
					edges: vec![((a, b), ab_fill), ((b, c), bc_fill), ((c, a), ca_fill)],
				});

				found = true;
				break;
			}

			if found {
				break;
			}
		}

		if !found {
			break;
		}
	}

	rings
}

fn first_with_remaining(entries: &[IntentEntry]) -> Option<&IntentEntry> {
	entries.iter().find(|e| e.remaining_in > 0)
}

/// Check that filling `amount_in` with `amount_out` across entries meets all limits.
///
/// Note: ring fills may partially consume a non-partial intent. This is safe because
/// the remaining volume goes through the normal AMM path, and the final resolution
/// always uses the full `amount_in` with a unified rate. Ring partial fills are
/// internal bookkeeping, not user-visible partial fills.
fn fills_meet_limits(entries: &[IntentEntry], total_in: Balance, total_out: Balance) -> bool {
	let mut remaining_in = total_in;
	for entry in entries {
		if remaining_in == 0 {
			break;
		}
		if entry.remaining_in == 0 {
			continue;
		}
		let fill_in = remaining_in.min(entry.remaining_in);
		let fill_out = mul_div(U256::from(fill_in), U256::from(total_out), U256::from(total_in))
			.and_then(|v| v.try_into().ok())
			.unwrap_or(0u128);

		if fill_in == entry.original_amount_in {
			// Full fill: must meet the intent's absolute minimum
			if fill_out < entry.min_amount_out {
				return false;
			}
		} else {
			// Partial fill: must meet pro-rata minimum
			let pro_rata_min = mul_div(
				U256::from(fill_in),
				U256::from(entry.min_amount_out),
				U256::from(entry.original_amount_in),
			)
			.and_then(|v| v.try_into().ok())
			.unwrap_or(0u128);
			if fill_out < pro_rata_min {
				return false;
			}
		}
		remaining_in = remaining_in.saturating_sub(fill_in);
	}
	true
}

fn fill_intent(entries: &mut [IntentEntry], amount_in: Balance, amount_out: Balance) -> Vec<MatchFill> {
	let mut fills = Vec::new();
	let mut remaining_in = amount_in;
	let mut remaining_out = amount_out;

	for entry in entries {
		if remaining_in == 0 {
			break;
		}
		if entry.remaining_in == 0 {
			continue;
		}

		let fill_in = remaining_in.min(entry.remaining_in);
		let fill_out = mul_div(U256::from(fill_in), U256::from(remaining_out), U256::from(remaining_in))
			.and_then(|v| v.try_into().ok())
			.unwrap_or(0);

		entry.remaining_in = entry.remaining_in.saturating_sub(fill_in);
		remaining_in = remaining_in.saturating_sub(fill_in);
		remaining_out = remaining_out.saturating_sub(fill_out);

		fills.push(MatchFill {
			intent_id: entry.intent_id,
			amount_in: fill_in,
			amount_out: fill_out,
		});
	}

	fills
}
