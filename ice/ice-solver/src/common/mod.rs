//! Common utilities shared between solver versions.

pub mod flow_graph;
pub mod ring_detection;

use hydra_dx_math::types::Ratio;
use ice_support::{AssetId, Balance, Intent, IntentData};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;

/// out = amount_in * (price_in / price_out)
///     = amount_in * price_in.n * price_out.d / (price_in.d * price_out.n)
///
/// Overflow-safe: handles large Ratio values (128-bit n/d) from real AMM spot prices.
/// Tries multiple computation orders to avoid U256 overflow while preserving precision.
pub fn calc_amount_out(amount_in: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
	let pi_n = U256::from(price_in.n);
	let pi_d = U256::from(price_in.d);
	let po_n = U256::from(price_out.n);
	let po_d = U256::from(price_out.d);
	let amt = U256::from(amount_in);

	// Strategy 1: direct — amount_in * (pi.n * po.d) / (pi.d * po.n)
	if let (Some(n), Some(d)) = (pi_n.checked_mul(po_d), pi_d.checked_mul(po_n)) {
		if let Some(result) = amt.checked_mul(n) {
			return result.checked_div(d)?.try_into().ok();
		}
		// amount_in * n overflows — split n/d (only useful when n >= d)
		if n >= d {
			let q = n.checked_div(d)?;
			let r = n.checked_rem(d)?;
			let base = amt.checked_mul(q)?;
			let correction = amt
				.checked_mul(r)
				.and_then(|v| v.checked_div(d))
				.unwrap_or(U256::zero());
			return base.checked_add(correction)?.try_into().ok();
		}
		// n < d: ratio < 1, split loses all precision — fall through to strategy 2/3
	}

	// Strategy 2: cross-cancel — (amount_in * pi.n / po.n) * (po.d / pi.d)
	// This works when pi.n and po.n are similar magnitude (both large) so their ratio is small.
	if let Some(ratio_n) = amt.checked_mul(pi_n) {
		let step1 = ratio_n.checked_div(po_n)?;
		if let Some(v) = step1.checked_mul(po_d) {
			return v.checked_div(pi_d)?.try_into().ok();
		}
	}

	// Strategy 3: (amount_in / pi.d) * pi.n then * po.d / po.n
	// Divide early to keep values small.
	let step1 = mul_div(amt, pi_n, pi_d)?;
	let result = mul_div(step1, po_d, po_n)?;
	result.try_into().ok()
}

/// Compute a * b / c with overflow protection.
pub fn mul_div(a: U256, b: U256, c: U256) -> Option<U256> {
	if c.is_zero() {
		return None;
	}
	if let Some(v) = a.checked_mul(b) {
		return v.checked_div(c);
	}
	// a * b overflows — use: (a / c) * b + (a % c) * b / c
	let q = a.checked_div(c)?;
	let r = a.checked_rem(c)?;
	let base = q.checked_mul(b)?;
	let correction = r.checked_mul(b).and_then(|v| v.checked_div(c)).unwrap_or(U256::zero());
	base.checked_add(correction)
}

pub fn collect_unique_assets(intents: &[Intent]) -> BTreeSet<AssetId> {
	intents
		.iter()
		.flat_map(|i| {
			let IntentData::Swap(swap) = &i.data;
			[swap.asset_in, swap.asset_out]
		})
		.collect()
}

pub fn is_satisfiable(intent: &Intent, spot_prices: &BTreeMap<AssetId, Ratio>) -> bool {
	let IntentData::Swap(swap) = &intent.data;

	let Some(price_in) = spot_prices.get(&swap.asset_in) else {
		log::trace!(target: "solver", "intent {}: not satisfiable — no spot price for asset_in {}", intent.id, swap.asset_in);
		return false;
	};
	let Some(price_out) = spot_prices.get(&swap.asset_out) else {
		log::trace!(target: "solver", "intent {}: not satisfiable — no spot price for asset_out {}", intent.id, swap.asset_out);
		return false;
	};

	let Some(calculated_out) = calc_amount_out(swap.amount_in, price_in, price_out) else {
		log::trace!(target: "solver", "intent {}: not satisfiable — calc_amount_out overflow for {} → {}", intent.id, swap.asset_in, swap.asset_out);
		return false;
	};
	if calculated_out < swap.amount_out {
		log::trace!(target: "solver", "intent {}: not satisfiable — spot output {} < min_out {} for {} → {}",
			intent.id, calculated_out, swap.amount_out, swap.asset_in, swap.asset_out);
		return false;
	}
	log::trace!(target: "solver", "intent {}: satisfiable — spot output {} >= min_out {} for {} → {}",
		intent.id, calculated_out, swap.amount_out, swap.asset_in, swap.asset_out);
	true
}

/// Analysis of net flow between two assets in opposing directions.
///
/// Determines how to split volume between direct matching and AMM:
/// - Scarce side (less total value) gets fully matched at spot rate
/// - Excess side gets direct match + AMM for remainder
#[derive(Debug, Clone, Copy)]
pub enum FlowDirection {
	/// Only forward (A→B) intents exist.
	SingleForward { amount: Balance },
	/// Only backward (B→A) intents exist.
	SingleBackward { amount: Balance },
	/// Both directions; A side has more value — excess A goes to AMM.
	ExcessForward {
		/// B→A rate output: amount of A given to B sellers via direct match
		scarce_out: Balance,
		/// Amount of B going to A sellers from direct match (= total_b_sold)
		direct_match: Balance,
		/// Net A to sell through AMM
		net_sell: Balance,
	},
	/// Both directions; B side has more value — excess B goes to AMM.
	ExcessBackward {
		/// A→B rate output: amount of B given to A sellers via direct match
		scarce_out: Balance,
		/// Amount of A going to B sellers from direct match (= total_a_sold)
		direct_match: Balance,
		/// Net B to sell through AMM
		net_sell: Balance,
	},
	/// Volumes cancel at spot — no AMM trade needed.
	PerfectCancel { a_as_b: Balance, b_as_a: Balance },
}

/// Analyze opposing flows to determine direct matching volumes and net AMM requirement.
///
/// Precondition: at least one of `total_a_sold`, `total_b_sold` must be > 0.
pub fn analyze_pair_flow(total_a_sold: Balance, total_b_sold: Balance, pa: &Ratio, pb: &Ratio) -> FlowDirection {
	debug_assert!(
		total_a_sold > 0 || total_b_sold > 0,
		"analyze_pair_flow called with both volumes zero"
	);
	if total_b_sold == 0 {
		return FlowDirection::SingleForward { amount: total_a_sold };
	}
	if total_a_sold == 0 {
		return FlowDirection::SingleBackward { amount: total_b_sold };
	}

	let a_as_b = calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);

	if a_as_b > total_b_sold {
		// Excess A: more A value than B value
		let matched_a_for_b = calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
		let net_a = total_a_sold.saturating_sub(matched_a_for_b);
		if net_a == 0 {
			return FlowDirection::PerfectCancel {
				a_as_b,
				b_as_a: matched_a_for_b,
			};
		}
		FlowDirection::ExcessForward {
			scarce_out: matched_a_for_b,
			direct_match: total_b_sold,
			net_sell: net_a,
		}
	} else if total_b_sold > a_as_b || a_as_b == 0 {
		// Excess B: more B value than A value
		let matched_b_for_a = a_as_b;
		let net_b = total_b_sold.saturating_sub(matched_b_for_a);
		if net_b == 0 {
			let b_as_a = calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
			return FlowDirection::PerfectCancel { a_as_b, b_as_a };
		}
		FlowDirection::ExcessBackward {
			scarce_out: matched_b_for_a,
			direct_match: total_a_sold,
			net_sell: net_b,
		}
	} else {
		// a_as_b == total_b_sold: perfect cancel
		let b_as_a = calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
		FlowDirection::PerfectCancel { a_as_b, b_as_a }
	}
}
