use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{OraclePeriod, PriceOracle};
use sp_core::Get;

pub struct OraclePriceProviderUsingRoute<RP, OP, P>;

impl<AssetId, RP, OP, P> SpotPriceProvider<AssetId> for OraclePriceProviderUsingRoute<RP, OP, P>
where
	RP: RouteProvider<AssetId>,
	OP: PriceOracle<AssetId>,
	P: Get<OraclePeriod>,
{
	type Price = EmaPrice;

	fn pair_exists(asset_a: AssetId, asset_b: AssetId) -> bool {
		!RP::get_route(AssetPair::new(asset_a, asset_b)).is_empty()
	}

	fn spot_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		let route = RP::get_route(AssetPair::new(asset_a, asset_b));
		OP::price(&route, P::get())
	}
}
