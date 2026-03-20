//! Flow graph types and construction shared across solver versions.

use ice_support::{AssetId, Balance, Intent, IntentData, IntentId};
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
	/// Currently unused in ring detection (ring partial fills are internal bookkeeping),
	/// but stored for potential future use in fill prioritization.
	pub partial: bool,
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

/// Build flow graph from intents: group by directed pair, sort by limit price ascending.
pub fn build_flow_graph(intents: &[&Intent]) -> FlowGraph {
	let mut graph: FlowGraph = BTreeMap::new();

	for intent in intents {
		let IntentData::Swap(swap) = &intent.data;
		let pair = (swap.asset_in, swap.asset_out);

		let limit_price = (U256::from(swap.amount_out), U256::from(swap.amount_in));

		let entry = IntentEntry {
			intent_id: intent.id,
			asset_in: swap.asset_in,
			asset_out: swap.asset_out,
			original_amount_in: swap.amount_in,
			min_amount_out: swap.amount_out,
			limit_price,
			remaining_in: swap.amount_in,
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
