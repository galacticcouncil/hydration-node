use frame_support::traits::Contains;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{NativePriceOracle, OraclePeriod, PriceOracle};
use orml_traits::MultiCurrency;
use sp_core::Get;
use sp_runtime::traits::{CheckedMul, One};
use sp_runtime::{FixedPointNumber, FixedU128};
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

/// Price provider that returns a price of an asset that can be used to pay tx fee.
/// If an asset cannot be used as fee payment asset, None is returned.
pub struct AssetFeeOraclePriceProvider<A, AC, RP, Oracle, Period>(PhantomData<(A, AC, RP, Oracle, Period)>);

impl<AssetId, A, RP, AC, Oracle, Period> NativePriceOracle<AssetId, EmaPrice>
	for AssetFeeOraclePriceProvider<A, AC, RP, Oracle, Period>
where
	RP: RouteProvider<AssetId>,
	Oracle: PriceOracle<AssetId, Price = EmaPrice>,
	Period: Get<OraclePeriod>,
	A: Get<AssetId>,
	AssetId: Copy + PartialEq,
	AC: Contains<AssetId>,
{
	fn price(currency: AssetId) -> Option<EmaPrice> {
		if currency == A::get() {
			return Some(EmaPrice::one());
		}

		if AC::contains(&currency) {
			let route = RP::get_route(AssetPair::new(currency, A::get()));
			Oracle::price(&route, Period::get())
		} else {
			None
		}
	}
}

impl<AssetId, A, RP, AC, Oracle, Period> PriceProvider<AssetId>
	for AssetFeeOraclePriceProvider<A, AC, RP, Oracle, Period>
where
	RP: RouteProvider<AssetId>,
	Oracle: PriceOracle<AssetId, Price = EmaPrice>,
	Period: Get<OraclePeriod>,
	A: Get<AssetId>,
	AssetId: Copy + PartialEq,
	AC: Contains<AssetId>,
{
	type Price = EmaPrice;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(EmaPrice::one());
		}

		if AC::contains(&asset_a) {
			let route = RP::get_route(AssetPair::new(asset_a, asset_b));
			Oracle::price(&route, Period::get())
		} else {
			None
		}
	}
}
