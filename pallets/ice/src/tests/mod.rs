use crate::*;
use ice_support::AssetId;
use pretty_assertions::assert_eq;

mod mock;
mod ocw;
mod submit_solution;

fn prices_to_map(prices: Vec<(AssetId, Ratio)>) -> sp_std::collections::btree_map::BTreeMap<AssetId, Ratio> {
	let mut cp: BTreeMap<AssetId, Ratio> = BTreeMap::new();
	for (a_id, p) in prices {
		assert_eq!(cp.insert(a_id, p), None);
	}

	cp
}
