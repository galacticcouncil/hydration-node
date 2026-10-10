//! Reference pricing for the DCA oracle floor, over a route derived from current
//! venue membership rather than a registered one.
//!
//! Both admission (`solver_intents`) and enforcement (`validate_resolve`) price
//! through this, so the derivation is consensus-critical: canonical ordering
//! throughout, and membership-only edges — nothing that moves within a block.

use crate::{EmaOracle, Runtime, LRNA};
use amm_simulator::uniswap_v3::Simulator as UniswapV3Simulator;
use frame_support::BoundedVec;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{PoolEdge, PoolType, Route, Trade};
use hydradx_traits::{OraclePeriod, PriceOracle};
use ice_support::{RoutingState, RoutingTarget};
use primitives::AssetId;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::vec;
use sp_std::vec::Vec;

type Oracle = hydradx_adapters::OraclePriceProvider<AssetId, EmaOracle, LRNA>;
type UniswapSimulator = UniswapV3Simulator<crate::ice_simulator_provider::UniswapV3<Runtime>>;

/// Every hop multiplies in another EMA, so a long reference path says less about
/// the pair than a short one. Coverage saturates just past this.
const MAX_HOPS: usize = 6;

/// Routes enumerated, and so the most that can be priced before giving up.
///
/// Membership does not imply an oracle entry, so a short route can come back
/// unpriceable while a longer one prices; this bound is how many such misses are
/// tolerated before the floor is abandoned. It deliberately bounds routes
/// *examined* rather than routes that price: this runs in `validate_unsigned`,
/// which is unweighted, so the work has to be bounded by construction and not by
/// how long finding an answer takes.
const MAX_CANDIDATES: usize = 16;

/// Guard only — measured never to bind on the live graph. It is here because this
/// runs in `validate_unsigned`, which is not weighted.
const MAX_EXPANSIONS: usize = 20_000;

/// Short-period EMA price over a derived route.
pub struct DerivedRouteShortPrice;

impl PriceProvider<AssetId> for DerivedRouteShortPrice {
	type Price = EmaPrice;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		price_over_derived_route(asset_a, asset_b, OraclePeriod::Short)
	}
}

fn price_over_derived_route(asset_a: AssetId, asset_b: AssetId, period: OraclePeriod) -> Option<EmaPrice> {
	// Fast path: the single Omnipool hop, which `price` resolves as a → LRNA → b.
	//
	// Membership is checked first and is not optional. An EMA entry outlives the
	// venue that fed it, and `get_updated_entry` fast-forwards that dead entry to
	// the current block, so pricing the hop blind would return a quote frozen at
	// delisting in preference to a route that still trades.
	if is_omnipool_leg(asset_a) && is_omnipool_leg(asset_b) {
		let direct: Route<AssetId> = BoundedVec::truncate_from(vec![Trade {
			pool: PoolType::Omnipool,
			asset_in: asset_a,
			asset_out: asset_b,
		}]);
		if let Some(price) = Oracle::price(&direct, period) {
			return Some(price);
		}
	}

	let edges = pricing_edges();

	// Searching for an endpoint no venue lists means exhausting the graph to prove a
	// negative.
	if !edges.iter().any(|edge| edge.assets.contains(&asset_a))
		|| !edges.iter().any(|edge| edge.assets.contains(&asset_b))
	{
		return None;
	}

	route_findr::routes_over(asset_a, asset_b, &edges, search_limits())
		.iter()
		.find_map(|route| Oracle::price(route, period))
}

/// Candidate reference paths for a pair, in the order they would be priced.
pub fn candidate_routes(asset_a: AssetId, asset_b: AssetId) -> Vec<Route<AssetId>> {
	route_findr::routes_over(asset_a, asset_b, &pricing_edges(), search_limits())
}

fn search_limits() -> route_findr::SearchLimits {
	route_findr::SearchLimits {
		max_hops: MAX_HOPS,
		max_routes: MAX_CANDIDATES,
		max_expansions: MAX_EXPANSIONS,
	}
}

fn is_omnipool_leg(asset: AssetId) -> bool {
	if asset == LRNA::get() {
		return true;
	}
	// Both halves are required, for different reasons. Membership, because an EMA
	// entry outlives the venue that fed it. Exclusion, because governance removing an
	// asset from the solver's routing has to remove it as a reference price too — the
	// graph already drops it, and a fast path that did not would let an excluded venue
	// set the floor for a trade it is not allowed to execute.
	pallet_omnipool::Assets::<Runtime>::contains_key(asset)
		&& pallet_ice::SolverRouting::<Runtime>::get(RoutingTarget::OmnipoolAsset(asset))
			!= Some(RoutingState::Excluded)
}

/// The venues allowed to set a reference price, in canonical order.
///
/// Order is load-bearing: the search breaks ties on insertion order, so two nodes
/// that built this list differently would enforce different floors.
pub fn pricing_edges() -> Vec<PoolEdge<AssetId>> {
	// One pass over the routing rules. The opt-in venues — Aave wraps and XYK pools
	// — live here rather than being discovered, so this is also where the pricing
	// graph learns about them. Exclusion wins over inclusion, matching
	// `ice_simulator_provider::registered`, so the graph prices over exactly the
	// venues the solver is allowed to trade.
	let mut excluded = Vec::new();
	let (mut wraps, mut excluded_wraps) = (BTreeSet::new(), BTreeSet::new());
	let (mut xyk_pools, mut excluded_xyk) = (BTreeSet::new(), BTreeSet::new());
	let (mut uniswap_pools, mut excluded_uniswap) = (BTreeSet::new(), BTreeSet::new());
	for (target, state) in pallet_ice::SolverRouting::<Runtime>::iter() {
		let (pairs, bucket, excluded_bucket) = match (&target, state) {
			(RoutingTarget::AaveWrap(reserve, atoken), _) => {
				(vec![(*reserve, *atoken)], &mut wraps, &mut excluded_wraps)
			}
			(RoutingTarget::AaveWraps(batch), _) => (batch.to_vec(), &mut wraps, &mut excluded_wraps),
			(RoutingTarget::XykPool(a, b), _) => (vec![(*a, *b)], &mut xyk_pools, &mut excluded_xyk),
			(RoutingTarget::XykPools(batch), _) => (batch.to_vec(), &mut xyk_pools, &mut excluded_xyk),
			(RoutingTarget::UniswapV3Pool(address), RoutingState::Included) => {
				uniswap_pools.insert(*address);
				continue;
			}
			(RoutingTarget::UniswapV3Pool(address), RoutingState::Excluded) => {
				excluded_uniswap.insert(*address);
				continue;
			}
			(RoutingTarget::UniswapV3Pools(batch), RoutingState::Included) => {
				uniswap_pools.extend(batch.iter().copied());
				continue;
			}
			(RoutingTarget::UniswapV3Pools(batch), RoutingState::Excluded) => {
				excluded_uniswap.extend(batch.iter().copied());
				continue;
			}
			_ => {
				if state == RoutingState::Excluded {
					excluded.push(target);
				}
				continue;
			}
		};
		match state {
			RoutingState::Included => bucket.extend(pairs),
			RoutingState::Excluded => excluded_bucket.extend(pairs),
		}
	}
	wraps.retain(|pair| !excluded_wraps.contains(pair));
	xyk_pools.retain(|pair| !excluded_xyk.contains(pair));
	uniswap_pools.retain(|pool| !excluded_uniswap.contains(pool));

	let mut edges = Vec::new();

	// LRNA is not a key in `Assets` but is tradable as a leg.
	let mut omnipool: Vec<AssetId> = pallet_omnipool::Assets::<Runtime>::iter_keys()
		.filter(|asset| !excluded.contains(&RoutingTarget::OmnipoolAsset(*asset)))
		.collect();
	omnipool.push(LRNA::get());
	omnipool.sort_unstable();
	omnipool.dedup();
	if omnipool.len() > 1 {
		edges.push(PoolEdge {
			pool_type: PoolType::Omnipool,
			assets: omnipool,
		});
	}

	// The oracle prices A→B inside a pool as A→share→B, so the share asset belongs
	// in the membership.
	let mut pools: Vec<_> = pallet_stableswap::Pools::<Runtime>::iter()
		.filter(|(id, _)| !excluded.contains(&RoutingTarget::StableswapPool(*id)))
		.collect();
	pools.sort_unstable_by_key(|(id, _)| *id);
	for (pool_id, pool) in pools {
		let mut assets: Vec<AssetId> = pool.assets.into_inner();
		assets.push(pool_id);
		assets.sort_unstable();
		assets.dedup();
		edges.push(PoolEdge {
			pool_type: PoolType::Stableswap(pool_id),
			assets,
		});
	}

	// Wraps are priced 1:1 with no oracle read, so they carry reachability rather
	// than price — which is the whole point: an asset whose liquidity sits in its
	// aToken (DOT, which moved to aDOT) has no reference price without one, and a
	// wrap mid-chain is what joins a stableswap pool to the Omnipool.
	for (reserve, atoken) in wraps {
		edges.push(PoolEdge {
			pool_type: PoolType::Aave,
			assets: vec![reserve, atoken],
		});
	}

	// A registration outlives the pool it names: pallet-xyk destroys a pool once its
	// last liquidity leaves, but the `XYK_SOURCE` EMA entry it wrote stays readable
	// forever. Emitting the edge on the registration alone would price off that dead
	// entry — the same failure as the `(DOT, LRNA)` Omnipool entry in
	// `price_over_derived_route`. `pool_assets` is the existence check the execution
	// snapshot already makes.
	//
	// Existence only, never reserves: the snapshot also drops empty-sided pools, but
	// liquidity moves within a block and admission (N-1) and enforcement (N) have to
	// agree on the graph.
	for (asset_a, asset_b) in xyk_pools {
		let pair_account = pallet_xyk::Pallet::<Runtime>::pair_account_from_assets(asset_a, asset_b);
		if pallet_xyk::Pallet::<Runtime>::pool_assets(&pair_account).is_none() {
			continue;
		}
		edges.push(PoolEdge {
			pool_type: PoolType::XYK,
			assets: vec![asset_a, asset_b],
		});
	}

	// Uniswap is the one venue whose pair the registry does not carry — a target is
	// just a pool address — so the tokens and fee come from the contract, which is
	// their source of truth. Three view calls per registered pool, on the graph path
	// only, and `pool_metadata` is the same read the solver's snapshot makes, so the
	// two cannot disagree about which assets a pool connects.
	// One curve per pair, matching the simulator: it keys on the pair alone, so a
	// second fee tier on the same pair is skipped there. Emitting both here would put
	// an edge in the graph for a pool the solver will not trade, and burn a candidate
	// slot on a duplicate that prices identically — the oracle keys on the pair too.
	let mut priced_pairs = BTreeSet::new();
	for pool in uniswap_pools {
		let Some(meta) = UniswapSimulator::pool_metadata(pool) else {
			continue;
		};
		let pair = (meta.asset_a.min(meta.asset_b), meta.asset_a.max(meta.asset_b));
		if !priced_pairs.insert(pair) {
			continue;
		}
		edges.push(PoolEdge {
			pool_type: PoolType::UniswapV3(meta.fee),
			assets: vec![meta.asset_a, meta.asset_b],
		});
	}

	edges
}
