use crate::common::split::{HopTolerance, Leg, Tolerance};
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, PoolType, Route, Trade};
use ice_support::{AssetId, Balance};

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

fn leg(t: &Tolerance, route: Route<AssetId>, hop_outs: &[Balance]) -> Leg {
	Leg {
		units: t.units_out(&route, hop_outs),
		amount_out: *hop_outs.last().unwrap(),
		amount_in: 0,
		route,
	}
}

/// Asset `i` has `DECIMALS[i]` decimals and every asset is worth the same.
const DECIMALS: [u32; 3] = [12, 6, 18];

struct DecimalsAmm;

impl AMMInterface for DecimalsAmm {
	type Error = ();
	type State = ();

	fn discover_routes(_: AssetId, _: AssetId, _: &()) -> Result<Vec<Route<AssetId>>, ()> {
		Err(())
	}

	fn sell(
		_: AssetId,
		_: AssetId,
		amount_in: Balance,
		route: Route<AssetId>,
		_: &(),
	) -> Result<((), TradeExecution), ()> {
		let amount_out = route.iter().fold(amount_in, |amount, t| {
			let (d_in, d_out) = (DECIMALS[t.asset_in as usize], DECIMALS[t.asset_out as usize]);
			amount * 10u128.pow(d_out) / 10u128.pow(d_in)
		});
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out,
				route,
			},
		))
	}

	fn buy(_: AssetId, _: AssetId, _: Balance, _: Route<AssetId>, _: &()) -> Result<((), TradeExecution), ()> {
		Err(())
	}

	fn get_spot_price(_: AssetId, _: AssetId, _: Route<AssetId>, _: &()) -> Result<Ratio, ()> {
		Err(())
	}

	fn price_denominator() -> AssetId {
		0
	}

	fn pool_edges(_: &()) -> Vec<PoolEdge<AssetId>> {
		Vec::new()
	}

	fn existential_deposit(_: AssetId) -> Balance {
		1
	}
}

#[test]
fn default_should_haircut_one_bps_per_route_when_the_route_has_several_hops() {
	let t = Tolerance::default();
	let l = leg(
		&t,
		route(&[PoolType::Aave, PoolType::Omnipool, PoolType::Stableswap(100)]),
		&[1, 1, 1_234_567],
	);
	assert_eq!(l.claimed_out(&t), 1_234_567 - 123);
	assert_eq!(l.claimed_out_strict(&t), 1_234_567 - 124);
}

#[test]
fn claimed_out_should_sum_hop_tolerances_when_venues_differ() {
	let t = Tolerance::parse("route=0,omnipool=50,aave=0+2,stableswap=10+1").unwrap();
	let l = leg(
		&t,
		route(&[PoolType::Aave, PoolType::Omnipool, PoolType::Stableswap(100)]),
		&[5_000_000, 10_000_000, 10_000_000],
	);
	// 60 ppm of 10_000_000 = 600, plus 2 units at twice the rate and 1 unit.
	assert_eq!(l.claimed_out(&t), 10_000_000 - 605);
}

#[test]
fn units_out_should_scale_an_intermediate_hop_when_its_asset_has_fewer_decimals() {
	let t = Tolerance::parse("route=0,stableswap=0+2").unwrap();
	let r = route(&[PoolType::Stableswap(1), PoolType::Stableswap(2)]);
	// 1 token of a 6-decimal asset into 1 token of an 18-decimal one: 2 units
	// short on the first hop are 2·10^12 short at the end.
	assert_eq!(t.units_out(&r, &[1_000_000, 10u128.pow(18)]), 2_000_000_000_002);
}

#[test]
fn leg_sell_should_carry_intermediate_units_into_the_output_asset_when_an_intermediate_hop_has_units() {
	let t = Tolerance::parse("route=0,stableswap=0+2").unwrap();
	let r = route(&[PoolType::Stableswap(1), PoolType::Stableswap(2)]);
	let (_, l) = Leg::sell::<DecimalsAmm>(&t, 0, 2, 10u128.pow(12), r, &()).unwrap();
	assert_eq!(l.amount_out, 10u128.pow(18));
	assert_eq!(l.units, 2_000_000_000_002);
	assert_eq!(l.claimed_out(&t), 10u128.pow(18) - 2_000_000_000_002);
}

#[test]
fn leg_sell_should_take_only_the_last_hop_units_when_intermediate_hops_have_none() {
	let t = Tolerance::parse("route=0,aave=0+3").unwrap();
	let r = route(&[PoolType::Omnipool, PoolType::Aave]);
	let (_, l) = Leg::sell::<DecimalsAmm>(&t, 0, 2, 10u128.pow(12), r, &()).unwrap();
	assert_eq!(l.amount_out, 10u128.pow(18));
	assert_eq!(l.units, 3);
}

#[test]
fn claimed_out_should_not_haircut_when_every_hop_is_zero() {
	let t = Tolerance::parse("route=0").unwrap();
	let l = leg(
		&t,
		route(&[PoolType::UniswapV3(3000), PoolType::Aave]),
		&[1, 987_654_321],
	);
	assert_eq!(l.claimed_out(&t), 987_654_321);
	assert_eq!(l.claimed_out_strict(&t), 987_654_321);
}

#[test]
fn claimed_out_strict_should_round_the_share_up_when_it_is_fractional() {
	let t = Tolerance::parse("route=0,omnipool=30").unwrap();
	let l = leg(&t, route(&[PoolType::Omnipool]), &[1_000]);
	assert_eq!(l.claimed_out(&t), 1_000);
	assert_eq!(l.claimed_out_strict(&t), 999);
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
