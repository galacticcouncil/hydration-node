use frame_support::traits::Contains;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{NativePriceOracle, OraclePeriod, PriceOracle};
use orml_traits::MultiCurrency;
use sp_core::Get;
use sp_runtime::traits::{CheckedMul, One};
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::marker::PhantomData;
use hydra_dx_math::ema::EmaPrice;

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

pub struct OmnipoolSpotPriceProvider<O>(PhantomData<O>);

impl<O> PriceProvider<O::AssetId> for OmnipoolSpotPriceProvider<O>
where
	O: pallet_omnipool::Config,
{
	type Price = FixedU128;

	fn get_price(asset_a: O::AssetId, asset_b: O::AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(FixedU128::one());
		}
		if asset_a == O::HubAssetId::get() {
			let asset_b = pallet_omnipool::Pallet::<O>::load_asset_state(asset_b).ok()?;
			FixedU128::checked_from_rational(asset_b.hub_reserve, asset_b.reserve)
		} else if asset_b == O::HubAssetId::get() {
			let asset_a = pallet_omnipool::Pallet::<O>::load_asset_state(asset_a).ok()?;
			FixedU128::checked_from_rational(asset_a.reserve, asset_a.hub_reserve)
		} else {
			let asset_a = pallet_omnipool::Pallet::<O>::load_asset_state(asset_a).ok()?;
			let asset_b = pallet_omnipool::Pallet::<O>::load_asset_state(asset_b).ok()?;
			// (A / LRNA) * (LRNA / B) = A / B
			let price_a = FixedU128::checked_from_rational(asset_a.reserve, asset_a.hub_reserve)?;
			let price_b = FixedU128::checked_from_rational(asset_b.hub_reserve, asset_b.reserve)?;
			price_a.checked_mul(&price_b)
		}
	}
}

pub struct XYKSpotPriceProvider<P>(PhantomData<P>);

impl<AssetId, P> PriceProvider<AssetId> for XYKSpotPriceProvider<P>
where
	P: pallet_xyk::Config,
	AssetId: Copy + Into<u32>,
{
	type Price = FixedU128;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if pallet_xyk::Pallet::<P>::exists(pallet_xyk::types::AssetPair::new(asset_b.into(), asset_a.into())) {
			let pair_account = pallet_xyk::Pallet::<P>::pair_account_from_assets(asset_a.into(), asset_b.into());
			let asset_a_reserve = P::Currency::free_balance(asset_a.into(), &pair_account);
			let asset_b_reserve = P::Currency::free_balance(asset_b.into(), &pair_account);

			FixedU128::checked_from_rational(asset_a_reserve, asset_b_reserve)
		} else {
			None
		}
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
