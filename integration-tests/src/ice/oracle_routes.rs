//! The DCA oracle floor priced over a derived route instead of a registered one.
//!
//! The snapshot is a gitignored scrape, so these are `#[ignore]`d:
//!
//! ```sh
//! cargo test -p runtime-integration-tests --locked ice::oracle_routes -- --ignored --nocapture
//! ```

use crate::polkadot_test_net::hydradx_run_to_next_block;
use frame_support::assert_ok;
use hydradx_runtime::ice_oracle_routes::DerivedRouteShortPrice;
use hydradx_runtime::{Runtime, System};
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{OraclePeriod, PriceOracle};
use ice_support::{AssetId, IntentData};
use sp_core::U512;

/// 13 Omnipool assets, 17 stableswap pools, 22 Aave pairs.
const SNAPSHOT: &str = "snapshots/ice/mainnet_sep";

const DOT: AssetId = 5;
const ADOT: AssetId = 1001;
const HDX: AssetId = 0;
const HOLLAR: AssetId = 222;

type Oracle = hydradx_adapters::OraclePriceProvider<AssetId, hydradx_runtime::EmaOracle, hydradx_runtime::LRNA>;

fn driver() -> crate::driver::HydrationTestDriver {
	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT)
}

/// The floor as priced before: whatever `Routes` had, or a single Omnipool hop.
fn registered_route_price(asset_in: AssetId, asset_out: AssetId) -> Option<hydra_dx_math::ema::EmaPrice> {
	let route = hydradx_runtime::Router::get_route(AssetPair::new(asset_in, asset_out));
	Oracle::price(&route, OraclePeriod::Short)
}

/// `Routes` is governance-fed and goes stale, so the floor must not depend on it.
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn derived_route_should_price_without_any_registered_route() {
	driver().execute(|| {
		let pairs = [(DOT, HDX), (HOLLAR, HDX), (ADOT, HOLLAR), (DOT, HOLLAR)];
		let before: Vec<_> = pairs
			.iter()
			.map(|(a, b)| DerivedRouteShortPrice::get_price(*a, *b).expect("priced with routes registered"))
			.collect();

		let registered = pallet_route_executor::Routes::<Runtime>::iter().count();
		let _ = pallet_route_executor::Routes::<Runtime>::clear(u32::MAX, None);
		println!("cleared {registered} registered routes");

		for ((asset_in, asset_out), expected) in pairs.iter().zip(before) {
			assert_eq!(
				DerivedRouteShortPrice::get_price(*asset_in, *asset_out),
				Some(expected),
				"{asset_in} -> {asset_out} must price identically with no registered routes"
			);
		}

		// With `Routes` empty the old path still prices DOT: the `(DOT, LRNA)` entry
		// outlived DOT's listing. That frozen quote is why membership is checked
		// before the fast path is taken.
		let stale = registered_route_price(DOT, HDX).expect("the dead (DOT, LRNA) entry still reads");
		let derived = DerivedRouteShortPrice::get_price(DOT, HDX).expect("derived route must price DOT -> HDX");
		assert_ne!(
			stale, derived,
			"the dead Omnipool entry and the live aDOT route must not agree — if they do, this test proves nothing"
		);
		println!("DOT -> HDX: dead-entry {stale:?} vs derived {derived:?}");

		assert_eq!(
			Some(derived),
			DerivedRouteShortPrice::get_price(ADOT, HDX),
			"an Aave wrap is 1:1, so DOT must price as aDOT"
		);
	});
}

#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn derived_route_should_keep_pricing_omnipool_pairs_unchanged() {
	driver().execute(|| {
		let before = registered_route_price(HOLLAR, HDX).expect("HOLLAR/HDX priced today");
		let after = DerivedRouteShortPrice::get_price(HOLLAR, HDX).expect("HOLLAR/HDX must still price");

		assert_eq!(before, after);
	});
}

#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn derived_route_should_be_stable_across_a_block_boundary() {
	driver().execute(|| {
		let pairs = [(DOT, HDX), (HOLLAR, HDX), (ADOT, HOLLAR), (DOT, HOLLAR)];
		let before: Vec<_> = pairs
			.iter()
			.map(|(a, b)| DerivedRouteShortPrice::get_price(*a, *b))
			.collect();

		hydradx_run_to_next_block();

		let after: Vec<_> = pairs
			.iter()
			.map(|(a, b)| DerivedRouteShortPrice::get_price(*a, *b))
			.collect();

		// Admission runs on block N-1 and enforcement on block N. The route must not
		// flip between them; the EMA behind it may still move.
		for ((pair, b), a) in pairs.iter().zip(before).zip(after) {
			assert_eq!(b.is_some(), a.is_some(), "pricing flipped for {pair:?}");
		}
	});
}

/// What every stored DCA intent's floor prices over, registered against derived.
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn probe_dca_floor_coverage() {
	driver().execute(|| {
		println!("=== DCA floor coverage, block {:?} ===", System::block_number());
		let (mut total, mut gained, mut lost) = (0u32, 0u32, 0u32);

		for (id, intent) in pallet_intent::Intents::<Runtime>::iter() {
			let IntentData::Dca(ref dca) = intent.data else {
				continue;
			};
			total += 1;
			let old = registered_route_price(dca.asset_in, dca.asset_out);
			let new = DerivedRouteShortPrice::get_price(dca.asset_in, dca.asset_out);
			match (old.is_some(), new.is_some()) {
				(false, true) => gained += 1,
				(true, false) => lost += 1,
				_ => {}
			}
			println!(
				"  #{id} {} -> {}: registered {} / derived {}",
				dca.asset_in,
				dca.asset_out,
				if old.is_some() { "priced" } else { "FAIL-OPEN" },
				if new.is_some() { "priced" } else { "FAIL-OPEN" },
			);
		}

		println!("{total} DCA intents: {gained} newly protected, {lost} lost coverage");
		assert_eq!(lost, 0, "the derived graph must not lose a pair that prices today");
	});
}

/// For every pair with a registered route, does the derived route reach the same
/// price? Disagreement is either a stale registration or a gap in the graph.
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn probe_derived_vs_registered_coverage() {
	driver().execute(|| {
		let registered: Vec<_> = pallet_route_executor::Routes::<Runtime>::iter_keys().collect();
		let mut worst_live_bps = 0u128;
		let (mut agree, mut differ, mut only_derived, mut only_registered, mut neither) =
			(0u32, 0u32, 0u32, 0u32, 0u32);
		let mut worst_bps = 0u128;

		for pair in &registered {
			let old = registered_route_price(pair.asset_in, pair.asset_out);
			let new = DerivedRouteShortPrice::get_price(pair.asset_in, pair.asset_out);
			match (old, new) {
				(Some(o), Some(n)) if o == n => agree += 1,
				(Some(o), Some(n)) => {
					differ += 1;
					// Relative gap in bps, on the rationals.
					let (on, od) = (U512::from(o.n), U512::from(o.d));
					let (nn, nd) = (U512::from(n.n), U512::from(n.d));
					let (lhs, rhs) = (on * nd, nn * od);
					let (hi, lo) = if lhs > rhs { (lhs, rhs) } else { (rhs, lhs) };
					let bps = if lo.is_zero() {
						u128::MAX
					} else {
						((hi - lo) * U512::from(10_000u32) / lo).low_u128()
					};
					worst_bps = worst_bps.max(bps);
					let reg_route = hydradx_runtime::Router::get_route(AssetPair::new(pair.asset_in, pair.asset_out));
					if route_is_live(&reg_route) {
						worst_live_bps = worst_live_bps.max(bps);
					}
					if bps > 100 {
						let reg = hydradx_runtime::Router::get_route(AssetPair::new(pair.asset_in, pair.asset_out));
						let fmt = |r: &[hydradx_traits::router::Trade<AssetId>]| {
							r.iter()
								.map(|t| format!("{:?}:{}->{}", t.pool, t.asset_in, t.asset_out))
								.collect::<Vec<_>>()
								.join(" ")
						};
						let der = hydradx_runtime::ice_oracle_routes::candidate_routes(pair.asset_in, pair.asset_out);
						println!(
							"  BIG {} -> {}: {bps} bps\n      registered: {}\n      derived:    {}",
							pair.asset_in,
							pair.asset_out,
							fmt(&reg),
							der.first().map(|r| fmt(r)).unwrap_or_default()
						);
					}
				}
				(None, Some(_)) => {
					only_derived += 1;
					println!("  derived only {} -> {}", pair.asset_in, pair.asset_out);
				}
				(Some(_), None) => {
					only_registered += 1;
					let candidates =
						hydradx_runtime::ice_oracle_routes::candidate_routes(pair.asset_in, pair.asset_out);
					let why = if candidates.is_empty() {
						"NO ROUTE".to_string()
					} else {
						let hops: Vec<String> = candidates[0]
							.iter()
							.map(|t| format!("{:?}:{}->{}", t.pool, t.asset_in, t.asset_out))
							.collect();
						format!("{} candidates, first: {}", candidates.len(), hops.join(" "))
					};
					println!("  REGISTERED ONLY {} -> {} [{why}]", pair.asset_in, pair.asset_out);
				}
				(None, None) => neither += 1,
			}
		}

		println!(
			"{} registered pairs: {agree} agree, {differ} differ, {only_derived} derived-only, \
			 {only_registered} registered-only, {neither} unpriced by both",
			registered.len()
		);
		println!("worst disagreement: {worst_bps} bps overall");
		println!("worst disagreement where the registered route is still live: {worst_live_bps} bps");
		assert!(
			worst_live_bps < 100,
			"a live registered route and the derived route must agree to within 1%; got {worst_live_bps} bps"
		);
	});
}

#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn probe_pricing_edges() {
	driver().execute(|| {
		for edge in hydradx_runtime::ice_oracle_routes::pricing_edges() {
			println!("{:?} {:?}", edge.pool_type, edge.assets);
		}
	});
}

/// For pairs only the registered route prices: is that route still over live
/// venues, or quoting a dead oracle entry?
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn probe_registered_only_liveness() {
	driver().execute(|| {
		let live_omnipool = |asset: AssetId| asset == 1 || pallet_omnipool::Assets::<Runtime>::contains_key(asset);
		let live_stableswap = |pool: AssetId| pallet_stableswap::Pools::<Runtime>::contains_key(pool);

		let (mut dead, mut live) = (0u32, 0u32);
		let mut dead_assets = std::collections::BTreeMap::<AssetId, u32>::new();

		for pair in pallet_route_executor::Routes::<Runtime>::iter_keys() {
			let old = registered_route_price(pair.asset_in, pair.asset_out);
			let new = DerivedRouteShortPrice::get_price(pair.asset_in, pair.asset_out);
			if !(old.is_some() && new.is_none()) {
				continue;
			}
			let route = hydradx_runtime::Router::get_route(AssetPair::new(pair.asset_in, pair.asset_out));
			let mut stale_leg = None;
			for hop in route.iter() {
				match hop.pool {
					hydradx_traits::router::PoolType::Omnipool => {
						for leg in [hop.asset_in, hop.asset_out] {
							if !live_omnipool(leg) {
								stale_leg = Some(leg);
							}
						}
					}
					hydradx_traits::router::PoolType::Stableswap(id) if !live_stableswap(id) => {
						stale_leg = Some(id);
					}
					_ => {}
				}
			}
			match stale_leg {
				Some(asset) => {
					dead += 1;
					*dead_assets.entry(asset).or_default() += 1;
				}
				None => {
					live += 1;
					let hops: Vec<String> = route
						.iter()
						.map(|t| format!("{:?}:{}->{}", t.pool, t.asset_in, t.asset_out))
						.collect();
					println!(
						"  LIVE-BUT-UNREACHED {} -> {} via {}",
						pair.asset_in,
						pair.asset_out,
						hops.join(" ")
					);
				}
			}
		}

		println!("registered-only pairs: {dead} quote a DEAD venue, {live} are live routes we miss");
		println!("dead legs by asset: {dead_assets:?}");
	});
}

/// Cost of the derived floor against the registered lookup it replaces.
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn probe_pricing_cost() {
	driver().execute(|| {
		let cases = [
			(HOLLAR, HDX, "fast: omnipool"),
			(DOT, HDX, "slow: aave + omnipool"),
			(20, 21, "slow: deep chain"),
			(16, 222, "slow: no route at all"),
		];
		const N: u32 = 200;

		for (asset_in, asset_out, label) in cases {
			let start = std::time::Instant::now();
			for _ in 0..N {
				let _ = DerivedRouteShortPrice::get_price(asset_in, asset_out);
			}
			let derived = start.elapsed() / N;

			let start = std::time::Instant::now();
			for _ in 0..N {
				let _ = registered_route_price(asset_in, asset_out);
			}
			let registered = start.elapsed() / N;

			let hops = hydradx_runtime::ice_oracle_routes::candidate_routes(asset_in, asset_out)
				.first()
				.map(|r| r.len())
				.unwrap_or(0);
			println!(
				"{label} ({asset_in}->{asset_out}, {hops} hops): derived {derived:?} vs registered {registered:?}"
			);
		}
	});
}

/// Whether every hop is over a venue that still exists — an EMA entry outlives the
/// venue that fed it.
fn route_is_live(route: &[hydradx_traits::router::Trade<AssetId>]) -> bool {
	route.iter().all(|hop| match hop.pool {
		hydradx_traits::router::PoolType::Omnipool => [hop.asset_in, hop.asset_out]
			.iter()
			.all(|leg| *leg == 1 || pallet_omnipool::Assets::<Runtime>::contains_key(*leg)),
		hydradx_traits::router::PoolType::Stableswap(id) => pallet_stableswap::Pools::<Runtime>::contains_key(id),
		_ => true,
	})
}

/// The live aDOT/HOLLAR pool, registered exactly as governance registers it, must
/// appear in the graph with the pair and fee read from the contract.
#[test]
#[ignore = "needs the gitignored snapshots/ice/mainnet_sep scrape"]
fn registered_uniswap_pool_should_enter_the_pricing_graph() {
	use hydradx_traits::router::PoolType;
	use ice_support::{RoutingState, RoutingTarget};

	driver().execute(|| {
		let pool = sp_core::H160(hex_literal::hex!("5C6208A3c316A801f8996750aA7b6f45Fc988548"));

		let uniswap_edges = || {
			hydradx_runtime::ice_oracle_routes::pricing_edges()
				.into_iter()
				.filter_map(|e| match e.pool_type {
					PoolType::UniswapV3(fee) => Some((e.assets, fee)),
					_ => None,
				})
				.collect::<Vec<_>>()
		};
		assert!(uniswap_edges().is_empty(), "nothing registered on the scrape");

		assert_ok!(hydradx_runtime::ICE::update_routing(
			hydradx_runtime::RuntimeOrigin::root(),
			RoutingTarget::UniswapV3Pool(pool),
			Some(RoutingState::Included),
		));

		// token0/token1/fee come from the contract, through the same read the
		// solver's snapshot makes.
		assert_eq!(uniswap_edges(), vec![(vec![ADOT, HOLLAR], 3000u32)]);

		// ...and excluding it takes the edge back out.
		assert_ok!(hydradx_runtime::ICE::update_routing(
			hydradx_runtime::RuntimeOrigin::root(),
			RoutingTarget::UniswapV3Pool(pool),
			Some(RoutingState::Excluded),
		));
		assert!(uniswap_edges().is_empty());
	});
}

// ---------------------------------------------------------------------------
// CI cover. The tests above read a mainnet scrape and are `#[ignore]`d with it;
// these build their state programmatically so the consensus-critical parts of the
// derivation — which venues enter the graph, which are kept out, and what happens
// when a registration outlives its pool — are checked on every run.
// ---------------------------------------------------------------------------

mod derivation {
	use super::*;
	use crate::driver::HydrationTestDriver;
	use frame_support::assert_ok;
	use hydradx_runtime::{RuntimeOrigin, ICE};
	use hydradx_traits::router::PoolType;
	use ice_support::{RoutingState, RoutingTarget};

	fn driver() -> HydrationTestDriver {
		HydrationTestDriver::default().setup_hydration()
	}

	fn edges_of(pool: PoolType<AssetId>) -> Vec<Vec<AssetId>> {
		hydradx_runtime::ice_oracle_routes::pricing_edges()
			.into_iter()
			.filter(|edge| edge.pool_type == pool)
			.map(|edge| edge.assets)
			.collect()
	}

	fn register(target: RoutingTarget, state: RoutingState) {
		assert_ok!(ICE::update_routing(RuntimeOrigin::root(), target, Some(state)));
	}

	fn an_omnipool_asset() -> AssetId {
		let mut assets: Vec<AssetId> = pallet_omnipool::Assets::<Runtime>::iter_keys().collect();
		assets.sort_unstable();
		assets[0]
	}

	fn a_stableswap_pool() -> (AssetId, Vec<AssetId>) {
		let (id, pool) = pallet_stableswap::Pools::<Runtime>::iter()
			.min_by_key(|(id, _)| *id)
			.expect("setup_hydration creates stablepools");
		(id, pool.assets.into_inner())
	}

	#[test]
	fn pricing_graph_should_carry_every_omnipool_asset_and_lrna_in_one_edge() {
		driver().execute(|| {
			let omnipool = edges_of(PoolType::Omnipool);
			assert_eq!(omnipool.len(), 1, "the Omnipool is a single star");

			let mut expected: Vec<AssetId> = pallet_omnipool::Assets::<Runtime>::iter_keys().collect();
			expected.push(hydradx_runtime::LRNA::get());
			expected.sort_unstable();
			expected.dedup();

			// LRNA is not a key in `Assets` but is tradable as a leg, and the oracle
			// prices every Omnipool hop through it.
			assert_eq!(omnipool[0], expected);
			assert!(omnipool[0].contains(&hydradx_runtime::LRNA::get()));
		});
	}

	#[test]
	fn pricing_graph_should_carry_the_share_asset_with_each_stableswap_pool() {
		driver().execute(|| {
			let (pool_id, assets) = a_stableswap_pool();
			let edge = edges_of(PoolType::Stableswap(pool_id));
			assert_eq!(edge.len(), 1);

			// The oracle prices A->B inside a pool as A->share->B, so the share asset
			// has to be a member even though nobody trades it.
			assert!(edge[0].contains(&pool_id), "share asset missing from {:?}", edge[0]);
			for asset in &assets {
				assert!(edge[0].contains(asset));
			}
		});
	}

	#[test]
	fn pricing_graph_should_drop_an_omnipool_asset_when_it_is_excluded() {
		driver().execute(|| {
			let excluded = an_omnipool_asset();
			assert!(edges_of(PoolType::Omnipool)[0].contains(&excluded));

			register(RoutingTarget::OmnipoolAsset(excluded), RoutingState::Excluded);

			assert!(
				!edges_of(PoolType::Omnipool)[0].contains(&excluded),
				"an excluded asset must not set a reference price"
			);
		});
	}

	#[test]
	fn pricing_graph_should_drop_a_stableswap_pool_when_it_is_excluded() {
		driver().execute(|| {
			let (pool_id, _) = a_stableswap_pool();
			assert_eq!(edges_of(PoolType::Stableswap(pool_id)).len(), 1);

			register(RoutingTarget::StableswapPool(pool_id), RoutingState::Excluded);

			assert!(edges_of(PoolType::Stableswap(pool_id)).is_empty());
		});
	}

	#[test]
	fn pricing_graph_should_ignore_an_xyk_registration_when_the_pool_does_not_exist() {
		driver().execute(|| {
			let (a, b) = (an_omnipool_asset(), HDX);
			assert!(edges_of(PoolType::XYK).is_empty());

			// Registering a pair that was never created on chain must not produce an
			// edge: pallet-xyk destroys a pool with its last liquidity, but the EMA
			// entry it wrote stays readable, so a trusted registration would price
			// off a dead venue.
			register(RoutingTarget::XykPool(a, b), RoutingState::Included);

			assert!(
				edges_of(PoolType::XYK).is_empty(),
				"a registration must not outlive the pool it names"
			);
		});
	}

	#[test]
	fn pricing_graph_should_carry_an_aave_wrap_when_it_is_registered() {
		driver().execute(|| {
			let (reserve, atoken) = (an_omnipool_asset(), HDX);
			assert!(edges_of(PoolType::Aave).is_empty());

			register(RoutingTarget::AaveWrap(reserve, atoken), RoutingState::Included);

			assert_eq!(edges_of(PoolType::Aave), vec![vec![reserve, atoken]]);

			// Exclusion wins over inclusion, matching `ice_simulator_provider::registered`.
			register(RoutingTarget::AaveWrap(reserve, atoken), RoutingState::Excluded);
			assert!(edges_of(PoolType::Aave).is_empty());
		});
	}

	#[test]
	fn derived_price_should_refuse_an_excluded_omnipool_asset_on_the_fast_path() {
		driver()
			.execute(|| {
				assert_ok!(hydradx_runtime::Omnipool::sell(
					RuntimeOrigin::signed(crate::polkadot_test_net::ALICE.into()),
					HDX,
					crate::polkadot_test_net::DOT,
					1_000_000_000_000,
					0,
				));
			})
			.new_block()
			.execute(|| {
				let dot = crate::polkadot_test_net::DOT;
				assert!(DerivedRouteShortPrice::get_price(HDX, dot).is_some());

				// The graph drops an excluded asset; the direct-hop fast path has to
				// as well, or the excluded venue still sets the floor for a trade the
				// solver is forbidden to execute.
				register(RoutingTarget::OmnipoolAsset(dot), RoutingState::Excluded);

				assert_eq!(
					DerivedRouteShortPrice::get_price(HDX, dot),
					None,
					"an excluded asset must not price through the Omnipool fast path"
				);
			});
	}

	#[test]
	fn pricing_graph_should_ignore_a_uniswap_registration_when_the_pool_has_no_contract() {
		driver().execute(|| {
			assert!(edges_of(PoolType::UniswapV3(0)).is_empty());

			// A registered address with no contract behind it answers nothing, so there
			// is no pair to connect — the edge must not be invented.
			register(
				RoutingTarget::UniswapV3Pool(sp_core::H160::repeat_byte(0xab)),
				RoutingState::Included,
			);

			assert!(hydradx_runtime::ice_oracle_routes::pricing_edges()
				.iter()
				.all(|e| !matches!(e.pool_type, PoolType::UniswapV3(_))));
		});
	}

	#[test]
	fn pricing_graph_should_be_identical_across_repeated_builds() {
		driver().execute(|| {
			// Both computation sites derive the route independently; if the edge list
			// is not canonical they can pick different routes and enforce different
			// floors on the same state.
			let first = hydradx_runtime::ice_oracle_routes::pricing_edges();
			let second = hydradx_runtime::ice_oracle_routes::pricing_edges();

			let shape = |edges: Vec<hydradx_traits::router::PoolEdge<AssetId>>| {
				edges
					.into_iter()
					.map(|e| (format!("{:?}", e.pool_type), e.assets))
					.collect::<Vec<_>>()
			};
			assert_eq!(shape(first), shape(second));
		});
	}

	#[test]
	fn derived_price_should_match_the_omnipool_hop_for_a_listed_pair() {
		driver()
			.execute(|| {
				assert_ok!(hydradx_runtime::Omnipool::sell(
					RuntimeOrigin::signed(crate::polkadot_test_net::ALICE.into()),
					HDX,
					crate::polkadot_test_net::DOT,
					1_000_000_000_000,
					0,
				));
			})
			.new_block()
			.execute(|| {
				let dot = crate::polkadot_test_net::DOT;
				let derived = DerivedRouteShortPrice::get_price(HDX, dot);
				let direct = Oracle::price(
					&[hydradx_traits::router::Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: dot,
					}],
					OraclePeriod::Short,
				);

				// Both legs are Omnipool members, so the fast path must return exactly
				// what the single hop returns — no graph, no route search.
				assert_eq!(derived, direct);
				assert!(derived.is_some());
			});
	}
}
