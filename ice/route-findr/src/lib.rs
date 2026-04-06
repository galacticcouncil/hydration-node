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
//! 3. BFS discovers all acyclic paths up to [`MAX_NUMBER_OF_TRADES`] hops,
//!    preventing both asset revisits and same-pool reuse.
//!
//! ## Usage
//!
//! Pool edges come from `AMMInterface::pool_edges()` or `SimulatorSet::pool_edges()`.
//! Pass them to [`get_routes`] for route discovery.
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

/// Discover all valid routes between two assets.
pub fn get_routes(asset_in: AssetId, asset_out: AssetId, pools: Vec<PoolEdge>) -> Vec<Route<AssetId>> {
	strategy::suggest_routes(asset_in, asset_out, pools)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
	use super::*;
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
	fn direct_xyk_route() {
		let routes = get_routes(1, 2, alloc::vec![xyk(1, 2)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
		assert_eq!(routes[0][0], trade(PoolType::XYK, 1, 2));
	}

	#[test]
	fn reverse_direction() {
		let routes = get_routes(2, 1, alloc::vec![xyk(1, 2)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0][0].asset_in, 2);
		assert_eq!(routes[0][0].asset_out, 1);
	}

	#[test]
	fn multi_hop_xyk() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), xyk(2, 3)]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].asset_out, 2);
		assert_eq!(routes[0][1].asset_in, 2);
		assert_eq!(routes[0][1].asset_out, 3);
	}

	#[test]
	fn multiple_routes_between_same_pair() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), xyk(2, 3), xyk(1, 3)]);
		assert!(routes.len() >= 2);
	}

	#[test]
	fn no_route_exists() {
		let routes = get_routes(1, 4, alloc::vec![xyk(1, 2), xyk(3, 4)]);
		assert!(routes.is_empty());
	}

	#[test]
	fn same_asset_returns_empty() {
		let routes = get_routes(1, 1, alloc::vec![xyk(1, 2)]);
		assert!(routes.is_empty());
	}

	#[test]
	fn empty_pools_returns_empty() {
		let routes = get_routes(1, 2, alloc::vec![]);
		assert!(routes.is_empty());
	}

	// -- omnipool specifics --

	#[test]
	fn omnipool_direct_route() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
		assert_eq!(routes[0][0].pool, PoolType::Omnipool);
	}

	#[test]
	fn omnipool_no_multi_hop_through_same_pool() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 1);
	}

	// -- stableswap --

	#[test]
	fn stableswap_direct_route() {
		let routes = get_routes(1, 3, alloc::vec![stableswap(100, &[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0][0].pool, PoolType::Stableswap(100));
	}

	// -- cross-pool routing --

	#[test]
	fn xyk_bridge_to_omnipool() {
		let routes = get_routes(1, 3, alloc::vec![xyk(1, 2), omnipool(&[2, 3])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].pool, PoolType::XYK);
		assert_eq!(routes[0][1].pool, PoolType::Omnipool);
	}

	#[test]
	fn stableswap_then_omnipool() {
		let routes = get_routes(1, 3, alloc::vec![stableswap(100, &[1, 2]), omnipool(&[2, 3, 4])]);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 2);
		assert_eq!(routes[0][0].pool, PoolType::Stableswap(100));
		assert_eq!(routes[0][1].pool, PoolType::Omnipool);
	}

	// -- strategy selection --

	#[test]
	fn trusted_only_excludes_xyk() {
		let routes = get_routes(1, 3, alloc::vec![omnipool(&[1, 2, 3]), xyk(1, 2)]);
		assert!(routes.iter().all(|r| r.iter().all(|t| t.pool != PoolType::XYK)));
	}

	#[test]
	fn isolated_only_when_no_trusted_pools_have_assets() {
		let routes = get_routes(10, 30, alloc::vec![xyk(10, 20), xyk(20, 30), omnipool(&[1, 2, 3])]);
		assert_eq!(routes.len(), 1);
		assert!(routes[0].iter().all(|t| t.pool == PoolType::XYK));
	}

	// -- cycle prevention --

	#[test]
	fn no_asset_revisit_in_cycle_graph() {
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
	fn different_pool_instances_can_both_be_used() {
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
	fn isolated_only_filters_to_relevant_pools() {
		let routes = get_routes(1, 4, alloc::vec![xyk(1, 2), xyk(2, 3), xyk(3, 4)]);
		assert!(routes.is_empty());
	}

	// -- max trades limit --

	#[test]
	fn exactly_max_trades_succeeds() {
		let pools: Vec<_> = (0u32..9).map(|i| stableswap(i + 100, &[i, i + 1])).collect();
		let routes = get_routes(0, 9, pools);
		assert_eq!(routes.len(), 1);
		assert_eq!(routes[0].len(), 9);
	}

	#[test]
	fn exceeding_max_trades_returns_empty() {
		let pools: Vec<_> = (0u32..10).map(|i| stableswap(i + 100, &[i, i + 1])).collect();
		let routes = get_routes(0, 10, pools);
		assert!(routes.is_empty());
	}

	// -- mainnet snapshot tests --

	mod mainnet {
		use super::*;
		use crate::testdata;

		#[test]
		fn snapshot_has_expected_pool_count() {
			let pools = testdata::mainnet_pools();
			assert_eq!(pools.len(), testdata::POOL_COUNT);
		}

		#[test]
		fn hdx_to_weth_via_omnipool() {
			// HDX=0, WETH=222 — both in Omnipool → direct route expected
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			dev_msg!("get_routes 0->222: routes={:#?}", routes);
			assert!(!routes.is_empty(), "HDX→WETH should have at least one route");
			assert!(routes.iter().any(|r| r.len() == 1 && r[0].pool == PoolType::Omnipool));
		}

		#[test]
		fn usdt_to_usdc_via_stableswap() {
			// USDT=10, USDC=22 — both in Stableswap(102) [10, 22, 102]
			let routes = get_routes(10, 22, testdata::mainnet_pools());
			dev_msg!("get_routes 10->22: routes={:#?}", routes);
			assert!(!routes.is_empty());
			assert!(routes
				.iter()
				.any(|r| r.iter().any(|t| matches!(t.pool, PoolType::Stableswap(_)))));
		}

		#[test]
		fn aave_wrapped_to_omnipool_asset() {
			// aUSDC=1002 in Aave [10, 1002], Stableswap [1002, ...], HSM [222, 1002]
			// WETH=222 in Omnipool — should find multi-hop route
			let routes = get_routes(1002, 222, testdata::mainnet_pools());
			dev_msg!("get_routes 1002->222: routes={:#?}", routes);
			assert!(!routes.is_empty(), "aUSDC→WETH should find a route");
		}

		#[test]
		fn xyk_only_asset_to_omnipool() {
			// 27 only in XYK [0, 27], 0 (HDX) in Omnipool
			// 222 (WETH) in Omnipool → mixed strategy
			let routes = get_routes(27, 222, testdata::mainnet_pools());
			assert!(!routes.is_empty(), "XYK-only asset should bridge to Omnipool");
			assert!(routes.iter().any(|r| r[0].pool == PoolType::XYK));
		}

		#[test]
		fn isolated_xyk_pair() {
			// 3370 only in XYK [5, 3370], 30 only in XYK [5, 30]
			// Neither in trusted pools → isolated-only strategy
			let routes = get_routes(3370, 30, testdata::mainnet_pools());
			assert!(routes.iter().all(|r| r.iter().all(|t| t.pool == PoolType::XYK)));
		}

		#[test]
		fn no_route_to_nonexistent_asset() {
			let routes = get_routes(0, 999999, testdata::mainnet_pools());
			assert!(routes.is_empty());
		}

		#[test]
		fn all_routes_are_acyclic() {
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			for route in &routes {
				let assets: Vec<_> = core::iter::once(route[0].asset_in)
					.chain(route.iter().map(|t| t.asset_out))
					.collect();
				let unique: alloc::collections::BTreeSet<_> = assets.iter().collect();
				assert_eq!(assets.len(), unique.len(), "route has cycle: {:?}", route);
			}
		}

		#[test]
		fn all_routes_respect_max_trades() {
			let routes = get_routes(0, 222, testdata::mainnet_pools());
			for route in &routes {
				assert!(route.len() <= 9, "route exceeds MAX_NUMBER_OF_TRADES: {}", route.len());
			}
		}

		#[test]
		fn hsm_pool_routing() {
			// HSM [222, 1002] — both in trusted
			let routes = get_routes(222, 1002, testdata::mainnet_pools());
			assert!(routes.iter().any(|r| r.iter().any(|t| t.pool == PoolType::HSM)));
		}
	}
}
