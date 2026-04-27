//! Flow graph types and construction shared across solver versions.

use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Partial};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::vec::Vec;

/// Directed pair (asset_in, asset_out)
pub type Pair = (AssetId, AssetId);

/// An intent entry in the flow graph, tracking its limit price and remaining fill volume.
#[derive(Debug, Clone)]
pub struct IntentEntry {
	pub intent_id: IntentId,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub original_amount_in: Balance,
	pub min_amount_out: Balance,
	/// Limit price as (numerator, denominator) = min_amount_out / amount_in
	pub limit_price: (U256, U256),
	/// Remaining amount_in not yet matched
	pub remaining_in: Balance,
	/// Whether this intent supports partial fills.
	/// Partial fill state. Used by v2 solver for variable fill amounts.
	pub partial: Partial,
}

/// The flow graph: intents grouped by directed pair.
pub type FlowGraph = BTreeMap<Pair, Vec<IntentEntry>>;

/// A fill record for one intent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchFill {
	pub intent_id: IntentId,
	pub amount_in: Balance,
	pub amount_out: Balance,
}

/// Build flow graph from intents with per-intent volume caps.
///
/// Each entry's `remaining_in` is bounded by the caller-provided `cap` — typically
/// the solver's decided `fill_amount` (fitted via clearing-price binary search) or
/// the intent's `swap.remaining()` when no narrower fill is known. This keeps
/// ring detection honest: it cannot match more volume than the user has reserved
/// and the solver has allocated.
///
/// `original_amount_in` and `limit_price` remain derived from the intent's original
/// `amount_in`/`amount_out` — so `fills_meet_limits` pro-rata formula keeps the
/// intent's real limit price. A partial whose cap is below `amount_in` will never
/// hit the "full fill" branch (cap ≤ amount_in and fill ≤ cap), so pro-rata is the
/// only enforced bound, as intended.
pub fn build_flow_graph(intents: &[(&Intent, Balance)]) -> FlowGraph {
	let mut graph: FlowGraph = BTreeMap::new();

	for &(intent, cap) in intents {
		let IntentData::Swap(swap) = &intent.data else {
			continue;
		};
		let pair = (swap.asset_in, swap.asset_out);

		let limit_price = (U256::from(swap.amount_out), U256::from(swap.amount_in));
		let remaining_in = cap.min(swap.remaining());

		let entry = IntentEntry {
			intent_id: intent.id,
			asset_in: swap.asset_in,
			asset_out: swap.asset_out,
			original_amount_in: swap.amount_in,
			min_amount_out: swap.amount_out,
			limit_price,
			remaining_in,
			partial: swap.partial,
		};

		graph.entry(pair).or_default().push(entry);
	}

	// Sort each group by limit price ascending (cheapest sellers first)
	for entries in graph.values_mut() {
		entries.sort_by(|a, b| {
			let lhs = a.limit_price.0.saturating_mul(b.limit_price.1);
			let rhs = b.limit_price.0.saturating_mul(a.limit_price.1);
			lhs.cmp(&rhs)
		});
	}

	graph
}
