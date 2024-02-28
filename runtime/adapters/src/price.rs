use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::weights::Weight;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, PoolType, RouteProvider, Trade};
use hydradx_traits::{
	AccountFeeCurrency, AccountFeeCurrencyBalanceInCurrency, AggregatedPriceOracle, OraclePeriod, PriceOracle,
};
use primitives::{AssetId, Balance};
use sp_core::Get;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::Convert;
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

pub struct FeeAssetBalanceInCurrency<T, C, AC, I>(sp_std::marker::PhantomData<(T, C, AC, I)>);

impl<T, C, AC, I> AccountFeeCurrencyBalanceInCurrency<AssetId, T::AccountId> for FeeAssetBalanceInCurrency<T, C, AC, I>
where
	T: pallet_ema_oracle::Config + frame_system::Config,
	C: Convert<(AssetId, AssetId, Balance), Option<(Balance, EmaPrice)>>,
	AC: AccountFeeCurrency<T::AccountId, AssetId = AssetId>,
	I: frame_support::traits::fungibles::Inspect<T::AccountId, AssetId = AssetId, Balance = Balance>,
{
	type Output = (Balance, Weight);

	fn get_balance_in_currency(to_currency: AssetId, account: &T::AccountId) -> Self::Output {
		let from_currency = AC::get(account);
		let account_balance = I::reducible_balance(from_currency, account, Preservation::Preserve, Fortitude::Polite);
		let mut price_weight = T::DbWeight::get().reads(2); // 1 read to get currency and 1 read to get balance

		if from_currency == to_currency {
			return (account_balance, price_weight);
		}

		// This is a workaround, as there is no other way to get weight of price retrieval,
		// We get the weight from the ema-oracle weights to get price
		// Weight * 2 because we are reading from the storage twice ( from_currency/lrna and lrna/to_currency)
		// if this gets removed (eg. Convert returns weight), the constraint on T and ema-oracle is not necessary
		price_weight.saturating_accrue(pallet_ema_oracle::Pallet::<T>::get_price_weight().saturating_mul(2));

		let Some((converted, _ )) = C::convert((from_currency, to_currency, account_balance)) else{
			return (0,price_weight);
		};
		(converted, price_weight)
	}
}

pub struct ConvertAmount<P>(sp_std::marker::PhantomData<P>);

// Converts `amount` of `from_currency` to `to_currency` using given oracle
// Input: (from_currency, to_currency, amount)
// Output: Option<(converted_amount, price)>
impl<P> Convert<(AssetId, AssetId, Balance), Option<(Balance, EmaPrice)>> for crate::price::ConvertAmount<P>
where
	P: PriceProvider<AssetId, Price = EmaPrice>,
{
	fn convert((from_currency, to_currency, amount): (AssetId, AssetId, Balance)) -> Option<(Balance, EmaPrice)> {
		if from_currency == to_currency {
			return Some((amount, EmaPrice::one()));
		}
		let price = P::get_price(to_currency, from_currency)?;
		let converted = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Up)?;
		Some((converted, price))
	}
}
