use crate::common::split::{HopTolerance, Tolerance};
use hydradx_traits::router::{PoolType, Route, Trade};
use ice_support::AssetId;

fn route(pools: &[PoolType<AssetId>]) -> Route<AssetId> {
	let hops: Vec<Trade<AssetId>> = pools
		.iter()
		.enumerate()
		.map(|(i, pool)| Trade {
			pool: *pool,
			asset_in: i as AssetId,
			asset_out: i as AssetId + 1,
		})
		.collect();
	hops.try_into().unwrap()
}

#[test]
fn default_should_haircut_one_bps_per_route_when_the_route_has_several_hops() {
	let t = Tolerance::default();
	let r = route(&[PoolType::Aave, PoolType::Omnipool, PoolType::Stableswap(100)]);
	assert_eq!(t.adjust(&r, 1_234_567), 1_234_567 - 123);
	assert_eq!(t.adjust_strict(&r, 1_234_567), 1_234_567 - 124);
}

#[test]
fn adjust_should_sum_hop_tolerances_when_venues_differ() {
	let t = Tolerance::parse("route=0,omnipool=50,aave=0+2,stableswap=10+1").unwrap();
	let r = route(&[PoolType::Aave, PoolType::Omnipool, PoolType::Stableswap(100)]);
	// 60 ppm of 10_000_000 = 600, plus 3 units.
	assert_eq!(t.adjust(&r, 10_000_000), 10_000_000 - 603);
}

#[test]
fn adjust_should_not_haircut_when_every_hop_is_zero() {
	let t = Tolerance::parse("route=0").unwrap();
	let r = route(&[PoolType::UniswapV3(3000), PoolType::Aave]);
	assert_eq!(t.adjust(&r, 987_654_321), 987_654_321);
	assert_eq!(t.adjust_strict(&r, 987_654_321), 987_654_321);
}

#[test]
fn adjust_strict_should_round_the_share_up_when_it_is_fractional() {
	let t = Tolerance::parse("route=0,omnipool=30").unwrap();
	let r = route(&[PoolType::Omnipool]);
	assert_eq!(t.adjust(&r, 1_000), 1_000);
	assert_eq!(t.adjust_strict(&r, 1_000), 999);
}

#[test]
fn parse_should_keep_defaults_when_a_venue_is_not_named() {
	let t = Tolerance::parse("uniswapv3=0+5").unwrap();
	assert_eq!(t.route_ppm, 100);
	assert_eq!(t.uniswap_v3, HopTolerance { ppm: 0, units: 5 });
	assert_eq!(t.omnipool, HopTolerance::default());
}

#[test]
fn parse_should_fail_when_route_has_units() {
	assert_eq!(Tolerance::parse("route=0+1"), Err("route takes no units"));
}
