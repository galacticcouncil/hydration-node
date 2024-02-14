use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::weights::Weight;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, PoolType, RouteProvider, Trade};
use hydradx_traits::{
	AggregatedPriceOracle, FeePaymentCurrency, FeePaymentCurrencyBalanceInCurrency, OraclePeriod, PriceOracle,
};
use primitives::{AssetId, Balance};
use sp_core::Get;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::Rounding;
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

pub struct FeeAssetBalanceInCurrencyProvider<T, P, Period, AC, I>(sp_std::marker::PhantomData<(T, P, Period, AC, I)>);

impl<T, P, Period, AC, I> FeePaymentCurrencyBalanceInCurrency<AssetId, T::AccountId>
	for FeeAssetBalanceInCurrencyProvider<T, P, Period, AC, I>
where
	T: pallet_ema_oracle::Config + frame_system::Config,
	P: PriceOracle<AssetId, Price = EmaPrice>,
	Period: Get<OraclePeriod>,
	AC: FeePaymentCurrency<T::AccountId, AssetId = AssetId>,
	I: frame_support::traits::fungibles::Inspect<T::AccountId, AssetId = AssetId, Balance = Balance>,
{
	type Output = (Balance, Weight);

	fn get_balance_in_currency(to_currency: AssetId, account: &T::AccountId) -> Self::Output {
		let Some(from_currency) = AC::get(account) else {
			return (0,Weight::zero());
		};
		let account_balance = I::reducible_balance(from_currency, account, Preservation::Preserve, Fortitude::Polite);

		if from_currency == to_currency {
			return (account_balance, T::DbWeight::get().reads(2));
		}
		// We get the weight from the ema-oracle weights to get price
		// Weight * 2 because we are reading from the storage twice ( from_currency/lrna and lrna/to_currency)
		// TODO: it could really be a part of the PriceOracle trait?!
		let price_weight =
			pallet_ema_oracle::Pallet::<T>::get_price_weight().saturating_mul(2).saturating_add(T::DbWeight::get().reads(2));
		let Some(price) = P::price(&[Trade {
			pool: PoolType::Omnipool,
			asset_in: from_currency,
			asset_out: to_currency,
		}], Period::get()) else{
			return (0,price_weight);
		};
		let Some(converted) = multiply_by_rational_with_rounding(account_balance, price.n, price.d, Rounding::Down) else{
			return (0,price_weight);
		};
		(converted, price_weight)
	}
}
