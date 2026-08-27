use crate::common::flow_graph::build_flow_graph;
use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Partial, SwapData};

fn make(id: IntentId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: Partial::No,
		}),
	}
}

fn make_partial_filled(
	id: IntentId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	already_filled: Balance,
) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: Partial::Yes(already_filled),
		}),
	}
}

fn entries_of<'a>(intents: &[&'a Intent]) -> Vec<(&'a Intent, Balance)> {
	intents
		.iter()
		.map(|i| {
			let cap = match &i.data {
				IntentData::Swap(s) => s.remaining(),
				_ => 0,
			};
			(*i, cap)
		})
		.collect()
}

#[test]
fn test_build_flow_graph_groups_by_directed_pair() {
	let i1 = make(1, 1, 2, 100, 90);
	let i2 = make(2, 1, 2, 50, 45);
	let i3 = make(3, 2, 1, 80, 70);

	let graph = build_flow_graph(&entries_of(&[&i1, &i2, &i3]));

	assert_eq!(graph.len(), 2, "expected two directed pairs");
	assert_eq!(graph.get(&(1, 2)).map(Vec::len).unwrap_or(0), 2);
	assert_eq!(graph.get(&(2, 1)).map(Vec::len).unwrap_or(0), 1);
}

#[test]
fn test_build_flow_graph_sorts_by_limit_price_ascending() {
	let i1 = make(1, 1, 2, 100, 90); // rate 0.9
	let i2 = make(2, 1, 2, 100, 50); // rate 0.5  ← cheapest
	let i3 = make(3, 1, 2, 100, 95); // rate 0.95 ← most expensive

	let graph = build_flow_graph(&entries_of(&[&i1, &i2, &i3]));
	let entries = graph.get(&(1, 2)).expect("pair (1,2) should exist");

	assert_eq!(entries.len(), 3);
	assert_eq!(entries[0].intent_id, 2, "cheapest first");
	assert_eq!(entries[2].intent_id, 3, "most expensive last");
}

#[test]
fn test_remaining_in_equals_amount_in_for_fresh_non_partial() {
	let i1 = make(1, 1, 2, 100, 90);
	let graph = build_flow_graph(&entries_of(&[&i1]));
	let entries = graph.get(&(1, 2)).unwrap();
	assert_eq!(entries[0].original_amount_in, 100);
	assert_eq!(entries[0].remaining_in, 100);
}

/// A partial intent that has already been partially filled must expose only
/// the unfilled portion via `remaining_in`, not the original amount.
#[test]
fn test_remaining_in_uses_remaining_for_partial() {
	let i1 = make_partial_filled(1, 1, 2, 100, 90, 60);
	let graph = build_flow_graph(&entries_of(&[&i1]));
	let entries = graph.get(&(1, 2)).unwrap();
	assert_eq!(
		entries[0].original_amount_in, 100,
		"original_amount_in should stay as the intent's amount_in",
	);
	assert_eq!(
		entries[0].remaining_in, 40,
		"remaining_in should be amount_in - filled = 40, got {}",
		entries[0].remaining_in,
	);
}

/// The caller's cap must not let remaining_in exceed `swap.remaining()` even
/// if the caller passes a larger value (e.g. stale fill plan).
#[test]
fn test_cap_bounded_by_remaining() {
	let i1 = make_partial_filled(1, 1, 2, 100, 90, 60); // remaining = 40
	let graph = build_flow_graph(&[(&i1, 1_000)]);
	let entries = graph.get(&(1, 2)).unwrap();
	assert_eq!(entries[0].remaining_in, 40);
}

/// A cap smaller than `swap.remaining()` must be honoured.
#[test]
fn test_cap_smaller_than_remaining() {
	let i1 = make(1, 1, 2, 100, 90); // remaining = 100
	let graph = build_flow_graph(&[(&i1, 30)]);
	let entries = graph.get(&(1, 2)).unwrap();
	assert_eq!(entries[0].remaining_in, 30);
}
