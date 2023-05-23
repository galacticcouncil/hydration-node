use core::marker::PhantomData;

use codec::FullCodec;
use frame_support::{
	traits::{Contains, Get},
	weights::Weight,
};
use hydra_dx_math::ema::EmaPrice;
use hydra_dx_math::omnipool::types::BalanceUpdate;
use hydra_dx_math::support::rational::round_to_rational;
use hydra_dx_math::support::rational::Rounding;
use hydradx_traits::AggregatedPriceOracle;
use hydradx_traits::PriceOracle;
use hydradx_traits::{liquidity_mining::PriceAdjustment, OnLiquidityChangedHandler, OnTradeHandler, OraclePeriod};
use orml_xcm_support::OnDepositFail;
use orml_xcm_support::UnknownAsset as UnknownAssetT;
use pallet_circuit_breaker::WeightInfo;
use pallet_ema_oracle::Price;
use pallet_ema_oracle::{OnActivityHandler, OracleError};
use pallet_omnipool::traits::{AssetInfo, ExternalPriceProvider, OmnipoolHooks};
use primitive_types::U128;
use primitives::{AssetId, Balance, BlockNumber};
use sp_runtime::traits::MaybeSerializeDeserialize;
use sp_runtime::traits::{Convert, Zero};
use sp_runtime::SaturatedConversion;
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128};
use sp_std::fmt::Debug;
use warehouse_liquidity_mining::GlobalFarmData;
use xcm::latest::prelude::*;
use xcm_executor::{
	traits::{Convert as MoreConvert, MatchesFungible, TransactAsset},
	Assets,
};

/// Passes on trade and liquidity data from the omnipool to the oracle.
pub struct OmnipoolHookAdapter<Origin, Lrna, Runtime>(PhantomData<(Origin, Lrna, Runtime)>);

/// The source of the data for the oracle.
pub const OMNIPOOL_SOURCE: [u8; 8] = *b"omnipool";

impl<Origin, Lrna, Runtime> OmnipoolHooks<Origin, AssetId, Balance> for OmnipoolHookAdapter<Origin, Lrna, Runtime>
where
	Lrna: Get<AssetId>,
	Runtime: pallet_ema_oracle::Config + pallet_circuit_breaker::Config + frame_system::Config<Origin = Origin>,
{
	type Error = DispatchError;

	fn on_liquidity_changed(origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_liquidity_changed(
			OMNIPOOL_SOURCE,
			asset.asset_id,
			Lrna::get(),
			*asset.delta_changes.delta_reserve,
			*asset.delta_changes.delta_hub_reserve,
			asset.after.reserve,
			asset.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		match asset.delta_changes.delta_reserve {
			BalanceUpdate::Increase(amount) => pallet_circuit_breaker::Pallet::<Runtime>::ensure_add_liquidity_limit(
				origin,
				asset.asset_id.into(),
				asset.before.reserve.into(),
				amount.into(),
			)?,
			BalanceUpdate::Decrease(amount) => {
				pallet_circuit_breaker::Pallet::<Runtime>::ensure_remove_liquidity_limit(
					origin,
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?
			}
		};

		Ok(Self::on_liquidity_changed_weight())
	}

	fn on_trade(
		_origin: Origin,
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			asset_in.asset_id,
			Lrna::get(),
			*asset_in.delta_changes.delta_reserve,
			*asset_in.delta_changes.delta_hub_reserve,
			asset_in.after.reserve,
			asset_in.after.hub_reserve,
		)
		.map_err(|(_, e)| e)?;

		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			Lrna::get(),
			asset_out.asset_id,
			*asset_out.delta_changes.delta_hub_reserve,
			*asset_out.delta_changes.delta_reserve,
			asset_out.after.hub_reserve,
			asset_out.after.reserve,
		)
		.map_err(|(_, e)| e)?;

		let amount_in = *asset_in.delta_changes.delta_reserve;
		let amount_out = *asset_out.delta_changes.delta_reserve;

		pallet_circuit_breaker::Pallet::<Runtime>::ensure_pool_state_change_limit(
			asset_in.asset_id.into(),
			asset_in.before.reserve.into(),
			amount_in.into(),
			asset_out.asset_id.into(),
			asset_out.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(Self::on_trade_weight())
	}

	fn on_hub_asset_trade(_origin: Origin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		OnActivityHandler::<Runtime>::on_trade(
			OMNIPOOL_SOURCE,
			Lrna::get(),
			asset.asset_id,
			*asset.delta_changes.delta_hub_reserve,
			*asset.delta_changes.delta_reserve,
			asset.after.hub_reserve,
			asset.after.reserve,
		)
		.map_err(|(_, e)| e)?;

		let amount_out = *asset.delta_changes.delta_reserve;

		pallet_circuit_breaker::Pallet::<Runtime>::ensure_pool_state_change_limit(
			Lrna::get().into(),
			Balance::zero().into(),
			Balance::zero().into(),
			asset.asset_id.into(),
			asset.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(Self::on_trade_weight())
	}

	fn on_liquidity_changed_weight() -> Weight {
		let w1 = OnActivityHandler::<Runtime>::on_liquidity_changed_weight();
		let w2 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_add_liquidity_limit()
			.max(<Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_remove_liquidity_limit());
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single(); // TODO: implement and use on_finalize_single_liquidity_limit_entry benchmark
		w1.saturating_add(w2).saturating_add(w3)
	}

	fn on_trade_weight() -> Weight {
		let w1 = OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(2);
		let w2 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_pool_state_change_limit();
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single(); // TODO: implement and use on_finalize_single_trade_limit_entry benchmark
		w1.saturating_add(w2).saturating_add(w3)
	}
}

/// Passes ema oracle price to the omnipool.
pub struct EmaOraclePriceAdapter<Period, Runtime>(PhantomData<(Period, Runtime)>);

impl<Period, Runtime> ExternalPriceProvider<AssetId, Price> for EmaOraclePriceAdapter<Period, Runtime>
where
	Period: Get<OraclePeriod>,
	Runtime: pallet_ema_oracle::Config + pallet_omnipool::Config,
{
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<Price, Self::Error> {
		let (price, _) =
			pallet_ema_oracle::Pallet::<Runtime>::get_price(asset_a, asset_b, Period::get(), OMNIPOOL_SOURCE)
				.map_err(|_| pallet_omnipool::Error::<Runtime>::PriceDifferenceTooHigh)?;
		Ok(price)
	}

	fn get_price_weight() -> Weight {
		pallet_ema_oracle::Pallet::<Runtime>::get_price_weight()
	}
}

pub struct OmnipoolPriceProviderAdapter<AssetId, AggregatedPriceGetter, Lrna>(
	PhantomData<(AssetId, AggregatedPriceGetter, Lrna)>,
);

impl<AssetId, AggregatedPriceGetter, Lrna> PriceOracle<AssetId>
	for OmnipoolPriceProviderAdapter<AssetId, AggregatedPriceGetter, Lrna>
where
	u32: From<AssetId>,
	AggregatedPriceGetter: AggregatedPriceOracle<AssetId, BlockNumber, EmaPrice, Error = OracleError>,
	Lrna: Get<AssetId>,
{
	type Price = EmaPrice;

	fn price(asset_a: AssetId, asset_b: AssetId, period: OraclePeriod) -> Option<EmaPrice> {
		let price_asset_a_lrna = AggregatedPriceGetter::get_price(asset_a, Lrna::get(), period, OMNIPOOL_SOURCE);

		let price_asset_a_lrna = match price_asset_a_lrna {
			Ok(price) => price.0,
			Err(OracleError::SameAsset) => EmaPrice::from(1),
			Err(_) => return None,
		};

		let price_lrna_asset_b = AggregatedPriceGetter::get_price(Lrna::get(), asset_b, period, OMNIPOOL_SOURCE);

		let price_lrna_asset_b = match price_lrna_asset_b {
			Ok(price) => price.0,
			Err(OracleError::SameAsset) => EmaPrice::from(1),
			Err(_) => return None,
		};

		let nominator = U128::full_mul(price_asset_a_lrna.n.into(), price_lrna_asset_b.n.into());
		let denominator = U128::full_mul(price_asset_a_lrna.d.into(), price_lrna_asset_b.d.into());

		let rational_as_u128 = round_to_rational((nominator, denominator), Rounding::Nearest);
		let price_in_ema_price = EmaPrice::new(rational_as_u128.0, rational_as_u128.1);

		Some(price_in_ema_price)
	}
}

pub struct PriceAdjustmentAdapter<Runtime, LMInstance>(PhantomData<(Runtime, LMInstance)>);

impl<Runtime, LMInstance> PriceAdjustment<GlobalFarmData<Runtime, LMInstance>>
	for PriceAdjustmentAdapter<Runtime, LMInstance>
where
	Runtime: warehouse_liquidity_mining::Config<LMInstance>
		+ pallet_ema_oracle::Config
		+ pallet_omnipool_liquidity_mining::Config,
{
	type Error = DispatchError;
	type PriceAdjustment = FixedU128;

	fn get(global_farm: &GlobalFarmData<Runtime, LMInstance>) -> Result<Self::PriceAdjustment, Self::Error> {
		let (price, _) = pallet_ema_oracle::Pallet::<Runtime>::get_price(
			global_farm.reward_currency.into(),
			global_farm.incentivized_asset.into(), //LRNA
			OraclePeriod::TenMinutes,
			OMNIPOOL_SOURCE,
		)
		.map_err(|_| pallet_omnipool_liquidity_mining::Error::<Runtime>::PriceAdjustmentNotAvailable)?;

		FixedU128::checked_from_rational(price.n, price.d).ok_or(ArithmeticError::Overflow.into())
	}
}

/// Asset transaction errors.
enum Error {
	/// Failed to match fungible.
	FailedToMatchFungible,
	/// `MultiLocation` to `AccountId` Conversion failed.
	AccountIdConversionFailed,
	/// `CurrencyId` conversion failed.
	CurrencyIdConversionFailed,
}

impl From<Error> for XcmError {
	fn from(e: Error) -> Self {
		match e {
			Error::FailedToMatchFungible => XcmError::FailedToTransactAsset("FailedToMatchFungible"),
			Error::AccountIdConversionFailed => XcmError::FailedToTransactAsset("AccountIdConversionFailed"),
			Error::CurrencyIdConversionFailed => XcmError::FailedToTransactAsset("CurrencyIdConversionFailed"),
		}
	}
}

/// The `TransactAsset` implementation, to handle `MultiAsset` deposit/withdraw, but reroutes deposits and transfers
/// to unsupported accounts to an alternative
/// .
/// Note that teleport related functions are unimplemented.
///
/// Methods of `DepositFailureHandler` would be called on multi-currency deposit
/// errors.
///
/// If the asset is known, deposit/withdraw will be handled by `MultiCurrency`,
/// else by `UnknownAsset` if unknown.
///
/// Taken and modified from `orml_xcm_support`.
/// https://github.com/open-web3-stack/open-runtime-module-library/blob/4ae0372e2c624e6acc98305564b9d395f70814c0/xcm-support/src/currency_adapter.rs#L96-L202
#[allow(clippy::type_complexity)]
pub struct ReroutingMultiCurrencyAdapter<
	MultiCurrency,
	UnknownAsset,
	Match,
	AccountId,
	AccountIdConvert,
	CurrencyId,
	CurrencyIdConvert,
	DepositFailureHandler,
	RerouteFilter,
	RerouteDestination,
>(
	PhantomData<(
		MultiCurrency,
		UnknownAsset,
		Match,
		AccountId,
		AccountIdConvert,
		CurrencyId,
		CurrencyIdConvert,
		DepositFailureHandler,
		RerouteFilter,
		RerouteDestination,
	)>,
);

impl<
		MultiCurrency: orml_traits::MultiCurrency<AccountId, CurrencyId = CurrencyId>,
		UnknownAsset: UnknownAssetT,
		Match: MatchesFungible<MultiCurrency::Balance>,
		AccountId: sp_std::fmt::Debug + Clone,
		AccountIdConvert: MoreConvert<MultiLocation, AccountId>,
		CurrencyId: FullCodec + Eq + PartialEq + Copy + MaybeSerializeDeserialize + Debug,
		CurrencyIdConvert: Convert<MultiAsset, Option<CurrencyId>>,
		DepositFailureHandler: OnDepositFail<CurrencyId, AccountId, MultiCurrency::Balance>,
		RerouteFilter: Contains<(CurrencyId, AccountId)>,
		RerouteDestination: Get<AccountId>,
	> TransactAsset
	for ReroutingMultiCurrencyAdapter<
		MultiCurrency,
		UnknownAsset,
		Match,
		AccountId,
		AccountIdConvert,
		CurrencyId,
		CurrencyIdConvert,
		DepositFailureHandler,
		RerouteFilter,
		RerouteDestination,
	>
{
	fn deposit_asset(asset: &MultiAsset, location: &MultiLocation) -> Result<(), XcmError> {
		match (
			AccountIdConvert::convert_ref(location),
			CurrencyIdConvert::convert(asset.clone()),
			Match::matches_fungible(asset),
		) {
			// known asset
			(Ok(who), Some(currency_id), Some(amount)) => {
				if RerouteFilter::contains(&(currency_id, who.clone())) {
					MultiCurrency::deposit(currency_id, &RerouteDestination::get(), amount)
						.or_else(|err| DepositFailureHandler::on_deposit_currency_fail(err, currency_id, &who, amount))
				} else {
					MultiCurrency::deposit(currency_id, &who, amount)
						.or_else(|err| DepositFailureHandler::on_deposit_currency_fail(err, currency_id, &who, amount))
				}
			}
			// unknown asset
			_ => UnknownAsset::deposit(asset, location)
				.or_else(|err| DepositFailureHandler::on_deposit_unknown_asset_fail(err, asset, location)),
		}
	}

	fn withdraw_asset(asset: &MultiAsset, location: &MultiLocation) -> Result<Assets, XcmError> {
		UnknownAsset::withdraw(asset, location).or_else(|_| {
			let who = AccountIdConvert::convert_ref(location)
				.map_err(|_| XcmError::from(Error::AccountIdConversionFailed))?;
			let currency_id = CurrencyIdConvert::convert(asset.clone())
				.ok_or_else(|| XcmError::from(Error::CurrencyIdConversionFailed))?;
			let amount: MultiCurrency::Balance = Match::matches_fungible(asset)
				.ok_or_else(|| XcmError::from(Error::FailedToMatchFungible))?
				.saturated_into();
			MultiCurrency::withdraw(currency_id, &who, amount).map_err(|e| XcmError::FailedToTransactAsset(e.into()))
		})?;

		Ok(asset.clone().into())
	}

	fn transfer_asset(asset: &MultiAsset, from: &MultiLocation, to: &MultiLocation) -> Result<Assets, XcmError> {
		let from_account =
			AccountIdConvert::convert_ref(from).map_err(|_| XcmError::from(Error::AccountIdConversionFailed))?;
		let to_account =
			AccountIdConvert::convert_ref(to).map_err(|_| XcmError::from(Error::AccountIdConversionFailed))?;
		let currency_id = CurrencyIdConvert::convert(asset.clone())
			.ok_or_else(|| XcmError::from(Error::CurrencyIdConversionFailed))?;
		let to_account = if RerouteFilter::contains(&(currency_id, to_account.clone())) {
			RerouteDestination::get()
		} else {
			to_account
		};
		let amount: MultiCurrency::Balance = Match::matches_fungible(asset)
			.ok_or_else(|| XcmError::from(Error::FailedToMatchFungible))?
			.saturated_into();
		MultiCurrency::transfer(currency_id, &from_account, &to_account, amount)
			.map_err(|e| XcmError::FailedToTransactAsset(e.into()))?;

		Ok(asset.clone().into())
	}
}
