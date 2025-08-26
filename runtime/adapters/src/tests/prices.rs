use crate::OraclePriceProvider;
use frame_support::parameter_types;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::router::{PoolType, Trade};
use hydradx_traits::{AggregatedPriceOracle, OraclePeriod, PriceOracle, Source};
use pallet_ema_oracle::OracleError;
use primitives::constants::chain::Weight;

type AssetId = u32;

parameter_types! {
    pub const LRNAAssetId: AssetId = 1;
}

struct MockOracle;

impl AggregatedPriceOracle<AssetId, u32, EmaPrice> for MockOracle {
	type Error = OracleError;
	fn get_price(
		_asset_a: AssetId,
		_asset_b: AssetId,
		_period: OraclePeriod,
		_source: Source,
	) -> Result<(EmaPrice, u32), Self::Error> {
		Ok((EmaPrice::new(u128::MAX, u128::MAX), 0))
	}
	fn get_price_weight() -> Weight {
		Weight::zero()
	}
}

type PriceProviderForRoute = OraclePriceProvider<AssetId, MockOracle, LRNAAssetId>;

#[test]
fn price_provider_should_not_overflow_when_route_contains_more_than_4_trades() {
	let generate_route = |c| -> Vec<Trade<AssetId>> {
		let mut r = vec![];
		for _ in 0..c {
			r.push(Trade {
				pool: PoolType::Omnipool,
				asset_in: 1,
				asset_out: 0,
			})
		}
		r
	};

	let route = generate_route(2);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211452,
			340282366920938463463374607431768211452
		))
	);

	let route = generate_route(3);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211450,
			340282366920938463463374607431768211450
		))
	);

	let route = generate_route(4);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211448,
			340282366920938463463374607431768211448
		))
	);

	let route = generate_route(5);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211446,
			340282366920938463463374607431768211446
		))
	);

	let route = generate_route(6);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211444,
			340282366920938463463374607431768211444
		))
	);

	let route = generate_route(7);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211442,
			340282366920938463463374607431768211442
		))
	);

	let route = generate_route(8);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211440,
			340282366920938463463374607431768211440
		))
	);

	let route = generate_route(9);
	let price = PriceProviderForRoute::price(&route, OraclePeriod::LastBlock);
	assert_eq!(
		price,
		Some(EmaPrice::new(
			340282366920938463463374607431768211438,
			340282366920938463463374607431768211438
		))
	);
}
