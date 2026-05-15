use crate::common::flow_graph::build_flow_graph;
use crate::common::ring_detection::detect_rings;
use hydra_dx_math::types::Ratio;
use ice_support::{AssetId, Intent, IntentData, Partial, SwapData};
use sp_std::collections::btree_map::BTreeMap;

fn make(id: u128, asset_in: u32, asset_out: u32, amount_in: u128, amount_out: u128) -> Intent {
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

fn unit_prices(assets: &[AssetId]) -> BTreeMap<AssetId, Ratio> {
	let mut m = BTreeMap::new();
	for &a in assets {
		m.insert(a, Ratio::new(1, 1));
	}
	m
}

#[test]
fn test_detect_ring_basic_3_cycle() {
	// 1 → 2 → 3 → 1 at 1:1, all intents 100.
	let i1 = make(1, 1, 2, 100, 90);
	let i2 = make(2, 2, 3, 100, 90);
	let i3 = make(3, 3, 1, 100, 90);
	let intents = [(&i1, 100u128), (&i2, 100), (&i3, 100)];

	let mut graph = build_flow_graph(&intents);
	let prices = unit_prices(&[1, 2, 3]);
	let rings = detect_rings(&mut graph, &prices);

	assert_eq!(rings.len(), 1, "one ring expected, got {}", rings.len());
	let ring = &rings[0];
	assert_eq!(ring.edges.len(), 3);
	// Each edge has exactly one fill of 100.
	let mut amounts: Vec<u128> = ring
		.edges
		.iter()
		.flat_map(|(_, fs)| fs.iter().map(|f| f.amount_in))
		.collect();
	amounts.sort();
	assert_eq!(amounts, vec![100, 100, 100]);
}

#[test]
fn test_ring_respects_entry_remaining_in() {
	// Same 3-cycle but (1,2) has only 40 remaining after we manually mutate the graph.
	// Bottleneck must be 40, not 100.
	let i1 = make(1, 1, 2, 100, 90);
	let i2 = make(2, 2, 3, 100, 90);
	let i3 = make(3, 3, 1, 100, 90);
	let intents = [(&i1, 100u128), (&i2, 100), (&i3, 100)];

	let mut graph = build_flow_graph(&intents);
	// Simulate the (1,2) intent already being 60% filled by a prior round.
	{
		let entry = graph.get_mut(&(1, 2)).unwrap();
		entry[0].remaining_in = 40;
	}

	let prices = unit_prices(&[1, 2, 3]);
	let rings = detect_rings(&mut graph, &prices);

	assert_eq!(rings.len(), 1);
	// All three legs must be bottlenecked at 40.
	for (_, fills) in &rings[0].edges {
		for f in fills {
			assert!(
				f.amount_in <= 40,
				"ring leg filled {} but bottleneck should be 40",
				f.amount_in,
			);
		}
	}
}

#[test]
fn test_ring_skips_when_one_edge_below_min() {
	// (1,2) has a tight limit that 1:1 spot can't satisfy: amount_in=100, amount_out=150.
	let i1 = make(1, 1, 2, 100, 150);
	let i2 = make(2, 2, 3, 100, 90);
	let i3 = make(3, 3, 1, 100, 90);
	let intents = [(&i1, 100u128), (&i2, 100), (&i3, 100)];

	let mut graph = build_flow_graph(&intents);
	let prices = unit_prices(&[1, 2, 3]);
	let rings = detect_rings(&mut graph, &prices);

	assert_eq!(
		rings.len(),
		0,
		"ring must be rejected when one leg fails min-out at spot"
	);
}

#[test]
fn test_no_4_cycle_detected() {
	// 1→2, 2→3, 3→4, 4→1 — only 4-cycle exists. Current impl does not find it.
	// This is a documentation test; if the algorithm is ever extended, update.
	let i1 = make(1, 1, 2, 100, 90);
	let i2 = make(2, 2, 3, 100, 90);
	let i3 = make(3, 3, 4, 100, 90);
	let i4 = make(4, 4, 1, 100, 90);
	let intents = [(&i1, 100u128), (&i2, 100), (&i3, 100), (&i4, 100)];

	let mut graph = build_flow_graph(&intents);
	let prices = unit_prices(&[1, 2, 3, 4]);
	let rings = detect_rings(&mut graph, &prices);

	assert_eq!(rings.len(), 0, "detect_rings currently only looks for 3-cycles");
}
