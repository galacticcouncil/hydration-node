//! Arithmetic, flow-analysis and graph helpers shared by the solver stages.

pub mod flow_graph;
pub mod ring_detection;
pub mod route_cache;

pub use route_cache::RouteCache;

use hydra_dx_math::types::Ratio;
use ice_support::{AssetId, Balance, Intent, IntentData};
use sp_core::{U256, U512};
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;

/// out = amount_in * (price_in / price_out)
///     = amount_in * price_in.n * price_out.d / (price_in.d * price_out.n)
///
/// Exact: the product of three 128-bit factors is at most 384 bits and the
/// divisor at most 256, so the whole expression is evaluated in `U512` with a
/// single floor division — no precision-losing fallbacks. `None` means the
/// exact result does not fit a `Balance` (or the divisor is zero); callers must
/// treat that as "cannot value this flow", never as zero.
pub fn calc_amount_out(amount_in: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
	let n = U512::from(amount_in)
		.checked_mul(U512::from(price_in.n))?
		.checked_mul(U512::from(price_out.d))?;
	let d = U512::from(price_in.d).checked_mul(U512::from(price_out.n))?;
	if d.is_zero() {
		return None;
	}
	balance_from_u512(n / d)
}

/// Exact `a * b / c` (integer floor) evaluated in `U512`.
///
/// `None` on a zero divisor or when the exact quotient exceeds `U256`. Unlike a
/// split-and-recombine fallback this never silently drops the remainder term.
pub fn mul_div(a: U256, b: U256, c: U256) -> Option<U256> {
	if c.is_zero() {
		return None;
	}
	let n = U512::from(a).checked_mul(U512::from(b))?;
	U256::try_from(n / U512::from(c)).ok()
}

/// `a * b / c` as a `Balance`. `None` when the divisor is zero or the exact
/// quotient does not fit 128 bits.
pub fn mul_div_balance(a: Balance, b: Balance, c: Balance) -> Option<Balance> {
	if c == 0 {
		return None;
	}
	let n = U512::from(a).checked_mul(U512::from(b))?;
	balance_from_u512(n / U512::from(c))
}

fn balance_from_u512(v: U512) -> Option<Balance> {
	U256::try_from(v).ok().and_then(|v| Balance::try_from(v).ok())
}

pub fn collect_unique_assets(intents: &[Intent]) -> BTreeSet<AssetId> {
	intents
		.iter()
		.filter_map(|i| {
			let IntentData::Swap(swap) = &i.data else {
				return None;
			};
			Some([swap.asset_in, swap.asset_out])
		})
		.flatten()
		.collect()
}

pub fn is_satisfiable(intent: &Intent, spot_prices: &BTreeMap<AssetId, Ratio>) -> bool {
	let IntentData::Swap(swap) = &intent.data else {
		return false;
	};

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
/// `None` means the pair cannot be valued at these reference prices — a
/// conversion did not fit 128 bits. Callers must then price both directions
/// independently through the AMM; treating an unrepresentable conversion as a
/// zero-valued flow would silently classify the whole pair as one-sided excess
/// and hand the scarce side a zero rate.
///
/// Precondition: at least one of `total_a_sold`, `total_b_sold` must be > 0.
pub fn analyze_pair_flow(
	total_a_sold: Balance,
	total_b_sold: Balance,
	pa: &Ratio,
	pb: &Ratio,
) -> Option<FlowDirection> {
	debug_assert!(
		total_a_sold > 0 || total_b_sold > 0,
		"analyze_pair_flow called with both volumes zero"
	);
	if total_b_sold == 0 {
		return Some(FlowDirection::SingleForward { amount: total_a_sold });
	}
	if total_a_sold == 0 {
		return Some(FlowDirection::SingleBackward { amount: total_b_sold });
	}

	let a_as_b = calc_amount_out(total_a_sold, pa, pb)?;
	let b_as_a = calc_amount_out(total_b_sold, pb, pa)?;

	match a_as_b.cmp(&total_b_sold) {
		Ordering::Greater => {
			// More A value than B value: B is fully matched, net A goes to the AMM.
			let net_a = total_a_sold.saturating_sub(b_as_a);
			if net_a == 0 {
				return Some(FlowDirection::PerfectCancel { a_as_b, b_as_a });
			}
			Some(FlowDirection::ExcessForward {
				scarce_out: b_as_a,
				direct_match: total_b_sold,
				net_sell: net_a,
			})
		}
		Ordering::Less => {
			// More B value than A value: A is fully matched, net B goes to the AMM.
			let net_b = total_b_sold.saturating_sub(a_as_b);
			if net_b == 0 {
				return Some(FlowDirection::PerfectCancel { a_as_b, b_as_a });
			}
			Some(FlowDirection::ExcessBackward {
				scarce_out: a_as_b,
				direct_match: total_a_sold,
				net_sell: net_b,
			})
		}
		Ordering::Equal => Some(FlowDirection::PerfectCancel { a_as_b, b_as_a }),
	}
}
