use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{OraclePeriod, PriceOracle};
use sp_core::Get;
use sp_std::marker::PhantomData;

pub struct OraclePriceProviderUsingRoute<RP, OP, P>(PhantomData<(RP, OP, P)>);

impl<AssetId, RP, OP, P> PriceProvider<AssetId> for OraclePriceProviderUsingRoute<RP, OP, P>
where
	RP: RouteProvider<AssetId>,
	OP: PriceOracle<AssetId>,
	P: Get<OraclePeriod>,
{
	type Price = OP::Price;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		let route = RP::get_route(AssetPair::new(asset_a, asset_b));
		OP::price(&route, P::get())
	}
}
