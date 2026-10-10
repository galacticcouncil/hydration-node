//! Graph construction from pool edges.
//!
//! Converts a list of [`PoolEdge`]s into a directed adjacency map where each
//! asset maps to all outgoing swap edges. For a pool with N assets, N×(N-1)
//! directed edges are created (every asset can be swapped for every other).
//!
//! Ported from `packages/sdk-next/src/sor/route/graph.ts`.

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;

use crate::types::{AssetId, PoolEdge, PoolType};

/// A directed edge in the pool graph.
#[derive(Debug, Clone)]
pub(crate) struct Edge {
	/// Index of the source pool in the original pool list.
	/// Used to prevent reusing the same pool within a single route,
	/// mirroring the SDK's pool-address cycle check.
	pub pool_index: usize,
	/// The pool type (needed to construct `Trade` output).
	pub pool_type: PoolType<AssetId>,
	/// Destination asset of this edge.
	pub asset_out: AssetId,
}

/// Adjacency list: maps each asset to its outgoing edges.
pub(crate) type AdjacencyMap = BTreeMap<AssetId, Vec<Edge>>;

/// Build a directed graph from pool edges.
pub(crate) fn build_graph(pools: &[PoolEdge]) -> AdjacencyMap {
	let mut graph = AdjacencyMap::new();

	for (pool_index, pool) in pools.iter().enumerate() {
		for &asset_in in &pool.assets {
			let edges = graph.entry(asset_in).or_default();
			for &asset_out in &pool.assets {
				if asset_in == asset_out {
					continue;
				}
				edges.push(Edge {
					pool_index,
					pool_type: pool.pool_type,
					asset_out,
				});
			}
		}
	}

	graph
}
