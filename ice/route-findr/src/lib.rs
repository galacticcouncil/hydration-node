//! # route-findr
//!
//! Route discovery for Hydration DEX — enumerates **all valid multi-hop trading
//! routes** for a given asset pair.
//!
//! Ported from the TypeScript SDK (`packages/sdk-next/src/sor/route/`).
//!
//! ## Types
//!
//! Uses canonical types from [`hydradx_traits::router`] and [`primitives`]:
//! - [`AssetId`] — concrete asset identifier from `primitives`
//! - [`PoolType`] — pool type discriminant
//! - [`PoolEdge`] — pool instance with its tradeable assets
//! - [`Trade`] — a single swap step (pool + asset pair)
//! - [`Route`] — bounded vector of trades (`BoundedVec<Trade, ConstU32<9>>`)
//!
//! ## Algorithm
//!
//! 1. Pools are partitioned into **trusted** (Omnipool, Stableswap, LBP, Aave,
//!    HSM) and **isolated** (XYK).
//! 2. Based on where the input/output assets live, one of three BFS strategies
//!    runs over the appropriate pool subset.
//! 3. BFS discovers acyclic paths up to [`MAX_NUMBER_OF_TRADES`] hops,
//!    preventing both asset revisits and same-pool reuse. The search is bounded
//!    by [`bfs::SearchLimits`] — shortest routes first, capped in both routes
//!    returned and paths expanded.
//!
//! ## Usage
//!
//! Pool edges come from `AMMInterface::pool_edges()` or `SimulatorSet::pool_edges()`.
//! Pass them to [`get_routes`] for a one-shot lookup, or build a
//! [`RouteFinder`] once and reuse it across many pairs on the same snapshot.
//!
//! [`AssetId`]: primitives::AssetId
//! [`PoolType`]: hydradx_traits::router::PoolType
//! [`PoolEdge`]: hydradx_traits::router::PoolEdge
//! [`Trade`]: hydradx_traits::router::Trade
//! [`Route`]: hydradx_traits::router::Route
//! [`MAX_NUMBER_OF_TRADES`]: hydradx_traits::router::MAX_NUMBER_OF_TRADES

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

#[allow(unused_macros)]
#[cfg(feature = "local-logs")]
macro_rules! dev_msg {
    ($($arg:tt)*) => { std::println!($($arg)*) };
}

#[allow(unused_macros)]
#[cfg(not(feature = "local-logs"))]
macro_rules! dev_msg {
	($($arg:tt)*) => {};
}

pub mod bfs;
pub mod graph;
pub mod strategy;
pub mod types;

#[cfg(test)]
pub mod testdata;

use alloc::vec::Vec;
use types::{AssetId, PoolEdge, Route};

pub use bfs::{SearchLimits, DEFAULT_MAX_EXPANSIONS, DEFAULT_MAX_ROUTES};
pub use strategy::RouteFinder;

/// Discover routes between two assets, under the default search limits.
pub fn get_routes(asset_in: AssetId, asset_out: AssetId, pools: Vec<PoolEdge>) -> Vec<Route<AssetId>> {
	strategy::suggest_routes(asset_in, asset_out, pools)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
	use crate::testdata;
	use types::PoolType;

	fn xyk(a: AssetId, b: AssetId) -> PoolEdge {
		PoolEdge {
			pool_type: PoolType::XYK,
			assets: alloc::vec![a, b],
		}
	}

	fn omnipool(assets: &[AssetId]) -> PoolEdge {
		PoolEdge {
			pool_type: PoolType::Omnipool,
			assets: assets.to_vec(),
		}
	}

	fn stableswap(id: AssetId, assets: &[AssetId]) -> PoolEdge {
		PoolEdge {
			pool_type: PoolType::Stableswap(id),
			assets: assets.to_vec(),
		}
	}

	fn trade(pool: PoolType<AssetId>, asset_in: AssetId, asset_out: AssetId) -> types::Trade<AssetId> {
		types::Trade {
			pool,
			asset_in,
			asset_out,
		}
	}

	// -- basic routing --

	#[test]
	fn get_routes_should_return_direct_trade_when_pair_shares_one_xyk_pool() {
		let routes = get_routes(1, 2, alloc::vec![xyk(1, 2)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
		assert_eq!(routes[0][0], trade(PoolType::XYK, 1, 2));
	}

	#[test]
	fn get_routes_should_orient_trade_by_direction_when_pair_is_reversed() {
		let routes = get_routes(2, 1, alloc::vec![xyk(1, 2)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0][0].asset_in, 2);
		assert_eq!(routes[0][0].asset_out, 1);
	}

	#[test]
	fn get_routes_should_chain_two_hops_when_no_direct_pool_exists() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), xyk(2, 3)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].asset_out, 2);
		assert_eq!(routes[0][1].asset_in, 2);
		assert_eq!(routes[0][1].asset_out, 3);
	}

	#[test]
	fn get_routes_should_return_several_routes_when_pair_is_reachable_multiple_ways() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), xyk(2, 3), xyk(1, 3)]);
		assert!(routes.len() >= 2);
	}

	#[test]
	fn get_routes_should_return_empty_when_pools_are_disconnected() {
		let routes = get_routes(1, 4, alloc::vec![xyk(1, 2), xyk(3, 4)]);
		assert!(routes.is_empty());
	}

	#[test]
	fn get_routes_should_return_empty_when_assets_are_identical() {
		let routes = get_routes(1, 1, alloc::vec![xyk(1, 2)]);
		assert!(routes.is_empty());
	}

	#[test]
	fn get_routes_should_return_empty_when_no_pools_are_supplied() {
		let routes = get_routes(1, 2, alloc::vec![]);
		assert!(routes.is_empty());
	}

	// -- omnipool specifics --

	#[test]
	fn get_routes_should_return_single_hop_when_both_assets_are_in_omnipool() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
		assert_eq!(routes[0][0].pool, PoolType::Omnipool);
	}

	#[test]
	fn get_routes_should_not_reuse_a_pool_when_building_a_route() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
	}

	// -- stableswap --

	#[test]
	fn get_routes_should_return_stableswap_hop_when_both_assets_share_a_pool() {
		let routes = get_routes(1, 3, alloc::vec![stableswap(100, &[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0][0].pool, PoolType::Stableswap(100));
	}

	// -- cross-pool routing --

	#[test]
	fn get_routes_should_bridge_xyk_to_omnipool_when_assets_live_in_different_pool_kinds() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), omnipool(&[2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].pool, PoolType::XYK);
		assert_eq!(routes[0][1].pool, PoolType::Omnipool);
	}

	#[test]
	fn get_routes_should_chain_stableswap_into_omnipool_when_bridging_assets() {
		let routes = get_routes(1, 3, alloc::vec![stableswap(100, &[1, 2]), omnipool(&[2, 3, 4])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].pool, PoolType::Stableswap(100));
		assert_eq!(routes[0][1].pool, PoolType::Omnipool);
	}

	// -- strategy selection --

	#[test]
	fn get_routes_should_exclude_xyk_when_both_assets_are_in_trusted_pools() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3]), xyk(1, 2)]);
		assert!(routes.iter().all(|r| r.iter().all(|t| t.pool != PoolType::XYK)));
	}

	#[test]
	fn get_routes_should_search_isolated_pools_only_when_neither_asset_is_trusted() {
		let routes = get_routes(10, 30, alloc::vec![xyk(10, 20), xyk(20, 30), omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert!(routes[0].iter().all(|t| t.pool == PoolType::XYK));
	}

	// -- cycle prevention --

	#[test]
	fn get_routes_should_return_acyclic_routes_when_graph_contains_a_cycle() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), xyk(2, 3), xyk(3, 1)]);
		for route in &routes {
			let assets: Vec<_> = core::iter::once(route[0].asset_in)
				.chain(route.iter().map(|t| t.asset_out))
				.collect();
			let unique: alloc::collections::BTreeSet<_> = assets.iter().collect();
			assert_eq!(assets.len(), unique.len(), "route revisits an asset");
		}
	}

	#[test]
	fn get_routes_should_traverse_distinct_pool_instances_when_each_adds_a_hop() {
		let routes = get_routes(
			1,
			4,
			alloc::vec![
				stableswap(10, &[1, 2]),
				stableswap(20, &[2, 3]),
				stableswap(30, &[3, 4]),
			],
		);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 3);
	}

	#[test]
	fn get_routes_should_return_empty_when_isolated_bridge_pool_is_filtered_out() {
		let routes = get_routes(1, 4, alloc::vec![xyk(1, 2), xyk(2, 3), xyk(3, 4)]);
		assert!(routes.is_empty());
	}

	// -- max trades limit --

	#[test]
	fn get_routes_should_return_route_when_hop_count_equals_the_maximum() {
		let pools: Vec<_> = (0u32..9).map(|i| stableswap(i + 100, &[i, i + 1])).collect();
		let routes = get_routes(0, 9, pools);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 9);
	}

	#[test]
	fn get_routes_should_return_empty_when_hop_count_exceeds_the_maximum() {
		let pools: Vec<_> = (0u32..10).map(|i| stableswap(i + 100, &[i, i + 1])).collect();
		let routes = get_routes(0, 10, pools);
		assert!(routes.is_empty());
	}

	// -- bounded search --

	/// A clique of `n` two-asset stableswap pools: every asset pair has its own
	/// pool, so the number of acyclic paths between two assets is factorial in
	/// `n`. This is the shape an unbounded enumeration cannot survive.
	fn clique(n: AssetId) -> Vec<PoolEdge> {
		let mut pools = Vec::new();
		let mut id = 100u32;
		for a in 0..n {
			for b in (a + 1)..n {
				pools.push(stableswap(id, &[a, b]));
				id += 1;
			}
		}
		pools
	}

	#[test]
	fn get_routes_should_cap_result_count_when_graph_is_densely_connected() {
		let routes = get_routes(0, 1, clique(9));
		assert_eq!(routes.len(), DEFAULT_MAX_ROUTES);
	}

	#[test]
	fn get_routes_should_return_shortest_routes_first_when_result_is_capped() {
		let routes = get_routes(0, 1, clique(9));
		let lengths: Vec<usize> = routes.iter().map(|r| r.len()).collect();
		assert_eq!(lengths[0], 1, "the direct pool must be the first route");
		assert!(
			lengths.windows(2).all(|w| w[0] <= w[1]),
			"routes must be returned shortest first, got {lengths:?}",
		);
	}

	#[test]
	fn get_routes_should_be_deterministic_when_called_repeatedly() {
		let a = get_routes(0, 1, clique(7));
		let b = get_routes(0, 1, clique(7));
		assert_eq!(a, b);
	}

	#[test]
	fn find_paths_should_stop_at_the_expansion_budget_when_graph_is_dense() {
		let limits = SearchLimits {
			max_hops: 9,
			max_routes: usize::MAX,
			max_expansions: 500,
		};
		let routes = RouteFinder::with_limits(clique(9), limits).routes(0, 1);
		// The budget, not the route cap, is what stops this search — so the
		// result is bounded well below the (factorial) true path count.
		assert!(!routes.is_empty());
		assert!(
			routes.len() < 500,
			"expected the expansion budget to bound the result, got {}",
			routes.len()
		);
	}

	#[test]
	fn find_paths_should_not_expand_paths_already_at_the_hop_limit() {
		// A 9-hop chain plus one extra pool that only a 10th hop could reach:
		// exploring depth 9 would enqueue paths that are then discarded.
		let mut pools: Vec<_> = (0u32..9).map(|i| stableswap(i + 100, &[i, i + 1])).collect();
		pools.push(stableswap(200, &[9, 10]));
		let limits = SearchLimits {
			max_hops: 9,
			max_routes: DEFAULT_MAX_ROUTES,
			// Enough for the 9-hop chain, not enough to also expand at depth 9.
			max_expansions: 11,
		};
		let finder = RouteFinder::with_limits(pools, limits);
		let routes = finder.routes(0, 9);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 9);
		assert!(finder.routes(0, 10).is_empty(), "a 10-hop route must not be returned");
	}

	#[test]
	fn route_finder_should_match_get_routes_when_reused_across_pairs() {
		let pools = testdata::mainnet_pools();
		let finder = RouteFinder::new(pools);
		for (a, b) in [(0u32, 222u32), (10, 22), (1002, 222), (27, 222), (0, 5), (5, 20)] {
			assert_eq!(
				finder.routes(a, b),
				get_routes(a, b, testdata::mainnet_pools()),
				"reused finder diverged from a one-shot lookup for {a} -> {b}",
			);
		}
	}

	#[test]
	fn route_finder_should_return_consistent_mixed_routes_when_isolated_asset_repeats() {
		// Isolated asset 4 only reaches the trusted pools through one XYK pool;
		// two mixed queries against different trusted counter-assets share the
		// cached mixed graph built for asset 4.
		let finder = RouteFinder::new(alloc::vec![omnipool(&[1, 2, 3]), xyk(4, 1)]);

		let to_one = finder.routes(4, 1);
		assert_eq!(to_one.len(), 1);
		assert_eq!(to_one[0].len(), 1);
		assert_eq!(to_one[0][0], trade(PoolType::XYK, 4, 1));

		let to_two = finder.routes(4, 2);
		assert_eq!(to_two.len(), 1);
		assert_eq!(to_two[0].len(), 2);
		assert_eq!(to_two[0][0], trade(PoolType::XYK, 4, 1));
		assert_eq!(to_two[0][1], trade(PoolType::Omnipool, 1, 2));
	}

	#[test]
	fn route_finder_should_cache_the_empty_mixed_graph_when_isolated_asset_is_unknown() {
		// Neither trusted counter-asset can reach isolated asset 999 — the
		// empty mixed graph built on the first query must not be mistaken for
		// "not yet cached" on the second.
		let finder = RouteFinder::new(alloc::vec![omnipool(&[1, 2, 3])]);
		assert!(finder.routes(1, 999).is_empty());
		assert!(finder.routes(2, 999).is_empty());
	}

	// -- mainnet snapshot tests --

	mod mainnet {
		use super::*;

		#[test]
		fn mainnet_snapshot_should_have_expected_pool_count() {
			let pools = testdata::mainnet_pools();
			assert_eq!(pools.len(), testdata::POOL_COUNT);
		}

		#[test]
		fn get_routes_should_find_direct_omnipool_route_when_pair_is_hdx_weth() {
			// HDX=0, WETH=222 — both in Omnipool → direct route expected
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			dev_msg!("get_routes 0->222: routes={:#?}", routes);
			assert!(!routes.is_empty(), "HDX→WETH should have at least one route");
			assert!(routes.iter().any(|r| r.len() == 1 && r[0].pool == PoolType::Omnipool));
		}

		#[test]
		fn get_routes_should_route_through_stableswap_when_pair_is_usdt_usdc() {
			// USDT=10, USDC=22 — both in Stableswap(102) [10, 22, 102]
			let routes = get_routes(10, 22, testdata::mainnet_pools());
			dev_msg!("get_routes 10->22: routes={:#?}", routes);
			assert!(!routes.is_empty());
			assert!(routes
				.iter()
				.any(|r| r.iter().any(|t| matches!(t.pool, PoolType::Stableswap(_)))));
		}

		#[test]
		fn get_routes_should_find_a_route_when_selling_an_aave_wrapped_asset() {
			// aUSDC=1002 in Aave [10, 1002], Stableswap [1002, ...], HSM [222, 1002]
			// WETH=222 in Omnipool — should find multi-hop route
			let routes = get_routes(1002, 222, testdata::mainnet_pools());
			dev_msg!("get_routes 1002->222: routes={:#?}", routes);
			assert!(!routes.is_empty(), "aUSDC→WETH should find a route");
		}

		#[test]
		fn get_routes_should_bridge_from_xyk_only_asset_when_target_is_in_omnipool() {
			// 27 only in XYK [0, 27], 0 (HDX) in Omnipool
			// 222 (WETH) in Omnipool → mixed strategy
			let routes = get_routes(27, 222, testdata::mainnet_pools());
			assert!(!routes.is_empty(), "XYK-only asset should bridge to Omnipool");
			assert!(routes.iter().any(|r| r[0].pool == PoolType::XYK));
		}

		#[test]
		fn get_routes_should_stay_within_xyk_when_neither_asset_is_trusted() {
			// 3370 only in XYK [5, 3370], 30 only in XYK [5, 30]
			// Neither in trusted pools → isolated-only strategy
			let routes = get_routes(3370, 30, testdata::mainnet_pools());
			assert!(routes.iter().all(|r| r.iter().all(|t| t.pool == PoolType::XYK)));
		}

		#[test]
		fn get_routes_should_return_empty_when_target_asset_is_unknown() {
			let routes = get_routes(0, 999999, testdata::mainnet_pools());
			assert!(routes.is_empty());
		}

		#[test]
		fn get_routes_should_return_only_acyclic_routes_on_the_mainnet_snapshot() {
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			for route in &routes {
				let assets: Vec<_> = core::iter::once(route[0].asset_in)
					.chain(route.iter().map(|t| t.asset_out))
					.collect();
				let unique: alloc::collections::BTreeSet<_> = assets.iter().collect();
				assert_eq!(assets.len(), unique.len(), "route has cycle: {route:?}");
			}
		}

		#[test]
		fn get_routes_should_respect_the_hop_limit_on_the_mainnet_snapshot() {
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			for route in &routes {
				assert!(route.len() <= 9, "route exceeds MAX_NUMBER_OF_TRADES: {}", route.len());
			}
		}

		#[test]
		fn get_routes_should_use_the_hsm_pool_when_it_connects_the_pair() {
			// HSM [222, 1002] — both in trusted
			let routes = get_routes(222, 1002, testdata::mainnet_pools());
			assert!(routes.iter().any(|r| r.iter().any(|t| t.pool == PoolType::HSM)));
		}
	}
}
