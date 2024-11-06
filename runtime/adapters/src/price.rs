use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::weights::Weight;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::fee::SwappablePaymentAssetTrader;
use hydradx_traits::price::PriceProvider;
use hydradx_traits::router::{AssetPair, RouteProvider};
use hydradx_traits::{
	AccountFeeCurrency, AccountFeeCurrencyBalanceInCurrency, AggregatedPriceOracle, OraclePeriod, PriceOracle,
};
use primitives::{AccountId, AssetId, Balance};
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

		//2 reads as we are checking if from and to assets are transaction fee currencies
		price_weight.saturating_accrue(T::DbWeight::get().reads(2));

		let Some((converted, _)) = C::convert((from_currency, to_currency, account_balance)) else {
			return (0, price_weight);
		};
		(converted, price_weight)
	}
}

pub struct ConvertBalance<PriceProv, SwappablePaymentAssetSupport, DotAssetId>(
	sp_std::marker::PhantomData<(PriceProv, SwappablePaymentAssetSupport, DotAssetId)>,
);

// Converts `amount` of `from_currency` to `to_currency` using given oracle,
// or XYK math calculations in case of swappable (insufficient) assets.
// Input: (from_currency, to_currency, amount)
// Output: Option<(converted_amount, price)>
impl<PriceProv, SwappablePaymentAssetSupport, DotAssetId>
	Convert<(AssetId, AssetId, Balance), Option<(Balance, EmaPrice)>>
	for crate::price::ConvertBalance<PriceProv, SwappablePaymentAssetSupport, DotAssetId>
where
	PriceProv: PriceProvider<AssetId, Price = EmaPrice>,
	SwappablePaymentAssetSupport: SwappablePaymentAssetTrader<AccountId, AssetId, Balance>,
	DotAssetId: Get<AssetId>,
{
	fn convert((from_currency, to_currency, amount): (AssetId, AssetId, Balance)) -> Option<(Balance, EmaPrice)> {
		if from_currency == to_currency {
			return Some((amount, EmaPrice::one()));
		}

		let dot = DotAssetId::get();

		let from_currency_is_tx_fee_asset = SwappablePaymentAssetSupport::is_transaction_fee_currency(from_currency);
		let to_currency_is_tx_fee_asset = SwappablePaymentAssetSupport::is_transaction_fee_currency(to_currency);

		if from_currency_is_tx_fee_asset && to_currency_is_tx_fee_asset {
			let price = PriceProv::get_price(to_currency, from_currency)?;
			let converted = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Up)?;
			Some((converted, price))
		} else if !from_currency_is_tx_fee_asset && to_currency_is_tx_fee_asset {
			let amount_in_dot =
				SwappablePaymentAssetSupport::calculate_out_given_in(from_currency, dot, amount).ok()?;

			let price_between_to_currency_and_dot = PriceProv::get_price(to_currency, dot)?;
			let amount_in_to_currency = multiply_by_rational_with_rounding(
				amount_in_dot,
				price_between_to_currency_and_dot.n,
				price_between_to_currency_and_dot.d,
				Rounding::Up,
			)?;

			debug_assert!(amount_in_to_currency > 0, "amount in to-currency should be positive");
			debug_assert!(amount > 0, "amount in out-currency should be positive");
			let price = EmaPrice::new(amount_in_to_currency, amount);

			return Some((amount_in_to_currency, price));
		} else if from_currency_is_tx_fee_asset && !to_currency_is_tx_fee_asset {
			let price_dot_to_from_currency = PriceProv::get_price(dot, from_currency)?;
			let amount_in_dot = multiply_by_rational_with_rounding(
				amount,
				price_dot_to_from_currency.n,
				price_dot_to_from_currency.d,
				Rounding::Up,
			)?;
			let amount_in_to_currency =
				SwappablePaymentAssetSupport::calculate_in_given_out(to_currency, dot, amount_in_dot).ok()?;

			debug_assert!(amount_in_to_currency > 0, "amount in to-currency should be positive");
			debug_assert!(amount > 0, "amount in out-currency should be positive");
			let price = EmaPrice::new(amount_in_to_currency, amount);
			Some((amount_in_to_currency, price))
		} else {
			//Not supported when both asset is insufficient asset
			return None;
		}
	}
}
