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
use alloc::vec::Vec;

use crate::bfs::find_all_paths;
use crate::graph::build_graph;
use crate::types::{AssetId, PoolEdge, PoolType, Route};

/// Returns `true` for pool types considered "trusted" (non-XYK).
fn is_trusted(pool_type: &PoolType<AssetId>) -> bool {
	!matches!(pool_type, PoolType::XYK)
}

/// Check if an asset appears in any of the given pools.
fn asset_in_pools(asset: AssetId, pools: &[PoolEdge]) -> bool {
	pools.iter().any(|p| p.assets.contains(&asset))
}

/// Discover all valid routes between `asset_in` and `asset_out` using the
/// trusted/isolated pool strategy.
pub fn suggest_routes(asset_in: AssetId, asset_out: AssetId, pools: Vec<PoolEdge>) -> Vec<Route<AssetId>> {
	let (trusted, isolated): (Vec<_>, Vec<_>) = pools.into_iter().partition(|p| is_trusted(&p.pool_type));

	let in_trusted = asset_in_pools(asset_in, &trusted);
	let out_trusted = asset_in_pools(asset_out, &trusted);

	match (in_trusted, out_trusted) {
		// Case 1: Neither token in trusted pools → isolated only
		(false, false) => {
			let relevant: Vec<_> = isolated
				.into_iter()
				.filter(|p| p.assets.contains(&asset_in) || p.assets.contains(&asset_out))
				.collect();
			let graph = build_graph(&relevant);
			find_all_paths(&graph, asset_in, asset_out)
		}

		// Case 2: Both tokens in trusted pools → trusted only
		(true, true) => {
			let graph = build_graph(&trusted);
			find_all_paths(&graph, asset_in, asset_out)
		}

		// Case 3: Mixed → trusted + relevant isolated
		_ => {
			let isolated_asset = if !in_trusted { asset_in } else { asset_out };
			let relevant_isolated: Vec<_> = isolated
				.into_iter()
				.filter(|p| p.assets.contains(&isolated_asset))
				.collect();

			if relevant_isolated.is_empty() {
				return Vec::new();
			}

			let mut combined = trusted;
			combined.extend(relevant_isolated);
			let graph = build_graph(&combined);
			find_all_paths(&graph, asset_in, asset_out)
		}
	}
}
