//! Breadth-first search path finder.
//!
//! Ported from `packages/sdk-next/src/sor/route/bfs.ts`.
//!
//! Discovers every acyclic path (up to `MAX_NUMBER_OF_TRADES` hops).
//!
//! Prevents cycles by checking that a candidate edge does not:
//! 1. Revisit an asset already in the path.
//! 2. Reuse a pool already traversed in the path (tracked by pool index).
//!
//! This mirrors the SDK's `Bfs.isNotVisited` which checks both asset ID
//! and pool address.

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use frame_support::BoundedVec;

use crate::graph::{AdjacencyMap, Edge};
use crate::types::{AssetId, PoolType, Route, Trade, MAX_NUMBER_OF_TRADES};

/// A node in a BFS path under construction.
#[derive(Debug, Clone)]
struct PathNode {
	asset: AssetId,
	/// Index of the pool used to reach this node (`None` for the start node).
	pool_index: Option<usize>,
	/// Pool type used to reach this node (`None` for the start node).
	pool_type: Option<PoolType<AssetId>>,
}

/// Check whether extending the path with `edge` would create a cycle.
fn is_valid_extension(path: &[PathNode], edge: &Edge) -> bool {
	for node in path {
		if node.asset == edge.asset_out {
			return false;
		}
		if let Some(idx) = node.pool_index {
			if idx == edge.pool_index {
				return false;
			}
		}
	}
	true
}

/// Convert an internal path to a [`Route`].
fn path_to_route(path: &[PathNode]) -> Route<AssetId> {
	let trades: Vec<Trade<AssetId>> = path
		.windows(2)
		.filter_map(|pair| {
			pair[1].pool_type.map(|pool| Trade {
				pool,
				asset_in: pair[0].asset,
				asset_out: pair[1].asset,
			})
		})
		.collect();
	BoundedVec::truncate_from(trades)
}

/// Find all acyclic paths from `start` to `end`, up to [`MAX_NUMBER_OF_TRADES`] hops.
pub(crate) fn find_all_paths(graph: &AdjacencyMap, start: AssetId, end: AssetId) -> Vec<Route<AssetId>> {
	let max_trades = MAX_NUMBER_OF_TRADES as usize;
	let mut results = Vec::new();
	let mut queue: VecDeque<Vec<PathNode>> = VecDeque::new();

	queue.push_back(vec![PathNode {
		asset: start,
		pool_index: None,
		pool_type: None,
	}]);

	while let Some(path) = queue.pop_front() {
		let trade_count = path.len() - 1;

		if trade_count > max_trades {
			continue;
		}

		let current_asset = path.last().expect("path is never empty").asset;

		if current_asset == end && trade_count > 0 {
			results.push(path_to_route(&path));
			continue;
		}

		if let Some(edges) = graph.get(&current_asset) {
			for edge in edges {
				if is_valid_extension(&path, edge) {
					let mut new_path = path.clone();
					new_path.push(PathNode {
						asset: edge.asset_out,
						pool_index: Some(edge.pool_index),
						pool_type: Some(edge.pool_type),
					});
					queue.push_back(new_path);
				}
			}
		}
	}

	results
}
