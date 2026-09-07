//! Trusted / isolated pool routing strategy.
//!
//! Ported from `packages/sdk-next/src/sor/route/suggester.ts`.
//!
//! Pools are partitioned into:
//! - **Trusted**: Omnipool, Stableswap, LBP, Aave, HSM — deeper liquidity, preferred.
//! - **Isolated**: XYK — used when assets aren't reachable via trusted pools.
//!
//! The strategy minimises search scope:
//!
//! | `asset_in` in trusted? | `asset_out` in trusted? | Search over           |
//! |------------------------|-------------------------|-----------------------|
//! | no                     | no                      | relevant isolated     |
//! | yes                    | yes                     | trusted only          |
//! | mixed                  | mixed                   | trusted + relevant isolated |

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cell::RefCell;

use crate::bfs::{find_paths, SearchLimits};
use crate::graph::{build_graph, AdjacencyMap};
use crate::types::{AssetId, PoolEdge, PoolType, Route};

/// Returns `true` for pool types considered "trusted" (non-XYK).
fn is_trusted(pool_type: &PoolType<AssetId>) -> bool {
	!matches!(pool_type, PoolType::XYK)
}

/// Check if an asset appears in any of the given pools.
fn asset_in_pools(asset: AssetId, pools: &[PoolEdge]) -> bool {
	pools.iter().any(|p| p.assets.contains(&asset))
}

/// A pool set prepared for repeated route lookups.
///
/// The trusted-pool graph — the one every trusted-to-trusted pair searches, and
/// the overwhelmingly common case — is built once here instead of once per
/// pair. Callers that resolve many pairs against the same snapshot (a solver
/// batch) should build one `RouteFinder` and reuse it; [`crate::get_routes`]
/// remains available for one-shot lookups.
///
/// Not wired into the production ICE solve path today: `RouteDiscovery` (the
/// trait `HydrationSimulatorConfig` implements as `SmartRouteFinder` in
/// `runtime/hydradx/src/assets.rs`) is a stateless per-pair associated
/// function — it has no `&self` to hold a `RouteFinder` across the many
/// `discover_routes` calls one solve makes, so the runtime calls
/// [`crate::get_routes`] (one-shot) per pair and rebuilds the trusted graph
/// every time. Making that reuse safe would mean giving `RouteDiscovery` an
/// instance to carry across a solve — a wider trait change, not a cache
/// bolted on as a `static`/thread-local (which would leak stale routes across
/// blocks). Until that lands, `RouteFinder` is exercised by this crate's own
/// tests and is available for callers that already hold one instance per
/// snapshot.
pub struct RouteFinder {
	trusted: Vec<PoolEdge>,
	isolated: Vec<PoolEdge>,
	trusted_graph: AdjacencyMap,
	/// Mixed-strategy graph (trusted ∪ pools touching one isolated asset),
	/// keyed by that isolated asset. A `RouteFinder` is scoped to a single
	/// snapshot for its whole lifetime, so caching here carries the same
	/// safety as `trusted_graph` — it never needs invalidation before the
	/// instance itself is dropped.
	mixed_cache: RefCell<BTreeMap<AssetId, AdjacencyMap>>,
	limits: SearchLimits,
}

impl RouteFinder {
	pub fn new(pools: Vec<PoolEdge>) -> Self {
		Self::with_limits(pools, SearchLimits::default())
	}

	pub fn with_limits(pools: Vec<PoolEdge>, limits: SearchLimits) -> Self {
		let (trusted, isolated): (Vec<_>, Vec<_>) = pools.into_iter().partition(|p| is_trusted(&p.pool_type));
		let trusted_graph = build_graph(&trusted);
		Self {
			trusted,
			isolated,
			trusted_graph,
			mixed_cache: RefCell::new(BTreeMap::new()),
			limits,
		}
	}

	/// Discover routes between `asset_in` and `asset_out` using the
	/// trusted/isolated strategy.
	pub fn routes(&self, asset_in: AssetId, asset_out: AssetId) -> Vec<Route<AssetId>> {
		if asset_in == asset_out {
			return Vec::new();
		}
		let in_trusted = asset_in_pools(asset_in, &self.trusted);
		let out_trusted = asset_in_pools(asset_out, &self.trusted);

		match (in_trusted, out_trusted) {
			// Neither token in trusted pools → isolated only.
			(false, false) => {
				let relevant: Vec<_> = self
					.isolated
					.iter()
					.filter(|p| p.assets.contains(&asset_in) || p.assets.contains(&asset_out))
					.cloned()
					.collect();
				let graph = build_graph(&relevant);
				find_paths(&graph, asset_in, asset_out, self.limits)
			}

			// Both tokens in trusted pools → the prebuilt trusted graph.
			(true, true) => find_paths(&self.trusted_graph, asset_in, asset_out, self.limits),

			// Mixed → trusted + the isolated pools holding the isolated asset.
			// The combined graph only depends on `isolated_asset`, so it is
			// cached per asset: repeated mixed queries against the same
			// isolated token (common within one solver batch) skip the clone
			// + rebuild after the first.
			_ => {
				let isolated_asset = if !in_trusted { asset_in } else { asset_out };

				if let Some(graph) = self.mixed_cache.borrow().get(&isolated_asset) {
					return find_paths(graph, asset_in, asset_out, self.limits);
				}

				let relevant_isolated: Vec<_> = self
					.isolated
					.iter()
					.filter(|p| p.assets.contains(&isolated_asset))
					.cloned()
					.collect();

				let graph = if relevant_isolated.is_empty() {
					AdjacencyMap::new()
				} else {
					let mut combined = self.trusted.clone();
					combined.extend(relevant_isolated);
					build_graph(&combined)
				};

				let routes = find_paths(&graph, asset_in, asset_out, self.limits);
				self.mixed_cache.borrow_mut().insert(isolated_asset, graph);
				routes
			}
		}
	}
}

/// Discover all valid routes between `asset_in` and `asset_out` using the
/// trusted/isolated pool strategy.
pub fn suggest_routes(asset_in: AssetId, asset_out: AssetId, pools: Vec<PoolEdge>) -> Vec<Route<AssetId>> {
	RouteFinder::new(pools).routes(asset_in, asset_out)
}
