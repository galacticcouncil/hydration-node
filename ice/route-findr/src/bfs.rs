//! Breadth-first search path finder.
//!
//! Ported from `packages/sdk-next/src/sor/route/bfs.ts`.
//!
//! Discovers acyclic paths (up to `MAX_NUMBER_OF_TRADES` hops), shortest first.
//!
//! Prevents cycles by checking that a candidate edge does not:
//! 1. Revisit an asset already in the path.
//! 2. Reuse a pool already traversed in the path (tracked by pool index).
//!
//! This mirrors the SDK's `Bfs.isNotVisited` which checks both asset ID
//! and pool address.
//!
//! The search is *bounded*: the number of paths pulled off the queue and the
//! number of routes returned are both capped (see [`SearchLimits`]). Path
//! enumeration in a dense pool graph is exponential in the hop limit, and every
//! returned route costs the caller a full AMM simulation per quote, so an
//! unbounded search is both a solver-latency and a block-time hazard. Because
//! BFS pops in strict insertion order the caps keep the *shortest* routes and
//! the result is deterministic for a given pool list.

extern crate alloc;
use alloc::collections::VecDeque;
use alloc::vec;
use alloc::vec::Vec;

use frame_support::BoundedVec;

use crate::graph::{AdjacencyMap, Edge};
use crate::types::{AssetId, PoolType, Route, Trade, MAX_NUMBER_OF_TRADES};

/// Bounds on a single path search.
#[derive(Debug, Clone, Copy)]
pub struct SearchLimits {
	/// Longest route, in hops. Never exceeds [`MAX_NUMBER_OF_TRADES`] — a longer
	/// route cannot be represented by [`Route`].
	pub max_hops: usize,
	/// Maximum number of routes returned. The shortest ones win.
	pub max_routes: usize,
	/// Maximum number of partial paths expanded before the search gives up.
	pub max_expansions: usize,
}

/// Default routes returned per pair. Mainnet pairs resolve to fewer than ten
/// routes today; the cap only bites on a pathologically connected pool set.
pub const DEFAULT_MAX_ROUTES: usize = 16;

/// Default expansion budget. Reached only by a graph dense enough that
/// enumerating it exhaustively would dominate the solve.
pub const DEFAULT_MAX_EXPANSIONS: usize = 20_000;

impl Default for SearchLimits {
	fn default() -> Self {
		Self {
			max_hops: MAX_NUMBER_OF_TRADES as usize,
			max_routes: DEFAULT_MAX_ROUTES,
			max_expansions: DEFAULT_MAX_EXPANSIONS,
		}
	}
}

impl SearchLimits {
	fn hops(&self) -> usize {
		self.max_hops.min(MAX_NUMBER_OF_TRADES as usize)
	}
}

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

/// Convert an internal path to a [`Route`], or `None` if it does not fit the
/// bound. Truncating instead would silently return a route that does not reach
/// `asset_out`.
fn path_to_route(path: &[PathNode]) -> Option<Route<AssetId>> {
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
	if trades.len() != path.len().saturating_sub(1) {
		return None;
	}
	BoundedVec::try_from(trades).ok()
}

/// Find acyclic paths from `start` to `end` under `limits`, shortest first.
pub(crate) fn find_paths(
	graph: &AdjacencyMap,
	start: AssetId,
	end: AssetId,
	limits: SearchLimits,
) -> Vec<Route<AssetId>> {
	let max_hops = limits.hops();
	let mut results = Vec::new();
	if start == end || limits.max_routes == 0 {
		return results;
	}

	let mut queue: VecDeque<Vec<PathNode>> = VecDeque::new();
	queue.push_back(vec![PathNode {
		asset: start,
		pool_index: None,
		pool_type: None,
	}]);

	let mut expansions = 0usize;
	while let Some(path) = queue.pop_front() {
		expansions += 1;
		if expansions > limits.max_expansions {
			dev_msg!("route-findr: expansion budget exhausted for {start} -> {end}");
			break;
		}

		// path is never empty: the seed has one node and every push extends one.
		let Some(last) = path.last() else {
			continue;
		};
		let trade_count = path.len() - 1;

		if last.asset == end && trade_count > 0 {
			if let Some(route) = path_to_route(&path) {
				results.push(route);
				if results.len() >= limits.max_routes {
					dev_msg!("route-findr: route cap reached for {start} -> {end}");
					break;
				}
			}
			// A path that already reached `end` is never extended: doing so
			// could only revisit `end`, which `is_valid_extension` rejects.
			continue;
		}

		// Expanding a path that is already at the hop limit can only produce
		// paths one hop too long, which are discarded on dequeue.
		if trade_count >= max_hops {
			continue;
		}

		if let Some(edges) = graph.get(&last.asset) {
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
