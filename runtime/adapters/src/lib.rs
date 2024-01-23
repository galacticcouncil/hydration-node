// This file is part of hydradx-adapters.

// Copyright (C) 2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::FullCodec;
use cumulus_primitives_core::relay_chain::Hash;
use frame_support::{
	sp_runtime::{
		traits::{AtLeast32BitUnsigned, Convert, Get, MaybeSerializeDeserialize, Saturating, Zero},
		ArithmeticError, DispatchError, DispatchResult, FixedPointNumber, FixedPointOperand, FixedU128,
		SaturatedConversion,
	},
	traits::{Contains, LockIdentifier, OriginTrait},
	weights::{Weight, WeightToFee},
};
use hydra_dx_math::{
	ema::EmaPrice,
	ensure,
	omnipool::types::BalanceUpdate,
	support::rational::{round_to_rational, round_u512_to_rational, Rounding},
};
use hydradx_traits::router::{AssetPair, PoolType, RouteProvider, Trade};
use hydradx_traits::{
	liquidity_mining::PriceAdjustment, AggregatedOracle, AggregatedPriceOracle, LockedBalance, NativePriceOracle,
	OnLiquidityChangedHandler, OnTradeHandler, OraclePeriod, PriceOracle,
};
use orml_traits::GetByKey;
use orml_xcm_support::{OnDepositFail, UnknownAsset as UnknownAssetT};
use pallet_circuit_breaker::WeightInfo;
use pallet_ema_oracle::{OnActivityHandler, OracleError, Price};
use pallet_omnipool::traits::{AssetInfo, ExternalPriceProvider, OmnipoolHooks};
use pallet_stableswap::types::{PoolState, StableswapHooks};
use pallet_transaction_multi_payment::DepositFee;
use polkadot_xcm::latest::prelude::*;
use primitive_types::{U128, U512};
use primitives::constants::chain::{STABLESWAP_SOURCE, XYK_SOURCE};
use primitives::{constants::chain::OMNIPOOL_SOURCE, AccountId, AssetId, Balance, BlockNumber, CollectionId};
use sp_runtime::traits::BlockNumberProvider;
use sp_std::vec::Vec;
use sp_std::{collections::btree_map::BTreeMap, fmt::Debug, marker::PhantomData};
use warehouse_liquidity_mining::GlobalFarmData;
use xcm_builder::TakeRevenue;
use xcm_executor::{
	traits::{ConvertLocation, MatchesFungible, TransactAsset, WeightTrader},
	Assets,
};

pub mod inspect;
pub mod price;
pub mod xcm_exchange;
pub mod xcm_execute_filter;

#[cfg(test)]
mod tests;

/// Weight trader that accepts multiple assets as weight fee payment.
///
/// It uses `WeightToFee` in combination with a `NativePriceOracle` to set the right price for weight.
/// Keeps track of the assets used to pay for weight and can refund them one by one (interface only
/// allows returning one asset per refund). Will pass any remaining assets on `Drop` to
/// `TakeRevenue`.
pub struct MultiCurrencyTrader<
	AssetId,
	Balance: FixedPointOperand + TryInto<u128>,
	Price: FixedPointNumber,
	ConvertWeightToFee: WeightToFee<Balance = Balance>,
	AcceptedCurrencyPrices: NativePriceOracle<AssetId, Price>,
	ConvertCurrency: Convert<MultiAsset, Option<AssetId>>,
	Revenue: TakeRevenue,
> {
	weight: Weight,
	paid_assets: BTreeMap<(MultiLocation, Price), u128>,
	_phantom: PhantomData<(
		AssetId,
		Balance,
		Price,
		ConvertWeightToFee,
		AcceptedCurrencyPrices,
		ConvertCurrency,
		Revenue,
	)>,
}

impl<
		AssetId,
		Balance: FixedPointOperand + TryInto<u128>,
		Price: FixedPointNumber,
		ConvertWeightToFee: WeightToFee<Balance = Balance>,
		AcceptedCurrencyPrices: NativePriceOracle<AssetId, Price>,
		ConvertCurrency: Convert<MultiAsset, Option<AssetId>>,
		Revenue: TakeRevenue,
	> MultiCurrencyTrader<AssetId, Balance, Price, ConvertWeightToFee, AcceptedCurrencyPrices, ConvertCurrency, Revenue>
{
	/// Get the asset id of the first asset in `payment` and try to determine its price via the
	/// price oracle.
	fn get_asset_and_price(&mut self, payment: &Assets) -> Option<(MultiLocation, Price)> {
		if let Some(asset) = payment.fungible_assets_iter().next() {
			ConvertCurrency::convert(asset.clone())
				.and_then(|currency| AcceptedCurrencyPrices::price(currency))
				.and_then(|price| match asset.id {
					Concrete(location) => Some((location, price)),
					_ => None,
				})
		} else {
			None
		}
	}
}

impl<
		AssetId,
		Balance: FixedPointOperand + TryInto<u128>,
		Price: FixedPointNumber,
		ConvertWeightToFee: WeightToFee<Balance = Balance>,
		AcceptedCurrencyPrices: NativePriceOracle<AssetId, Price>,
		ConvertCurrency: Convert<MultiAsset, Option<AssetId>>,
		Revenue: TakeRevenue,
	> WeightTrader
	for MultiCurrencyTrader<AssetId, Balance, Price, ConvertWeightToFee, AcceptedCurrencyPrices, ConvertCurrency, Revenue>
{
	fn new() -> Self {
		Self {
			weight: Default::default(),
			paid_assets: Default::default(),
			_phantom: PhantomData,
		}
	}

	/// Will try to buy weight with the first asset in `payment`.
	///
	/// This is a reasonable strategy as the `BuyExecution` XCM instruction only passes one asset
	/// per buy.
	/// The fee is determined by `ConvertWeightToFee` in combination with the price determined by
	/// `AcceptedCurrencyPrices`.
	fn buy_weight(&mut self, weight: Weight, payment: Assets, _context: &XcmContext) -> Result<Assets, XcmError> {
		log::trace!(
			target: "xcm::weight", "MultiCurrencyTrader::buy_weight weight: {:?}, payment: {:?}",
			weight, payment
		);
		let (asset_loc, price) = self.get_asset_and_price(&payment).ok_or(XcmError::AssetNotFound)?;
		let fee = ConvertWeightToFee::weight_to_fee(&weight);
		let converted_fee = price.checked_mul_int(fee).ok_or(XcmError::Overflow)?;
		let amount: u128 = converted_fee.try_into().map_err(|_| XcmError::Overflow)?;
		let required = (Concrete(asset_loc), amount).into();
		let unused = payment.checked_sub(required).map_err(|_| XcmError::TooExpensive)?;
		self.weight = self.weight.saturating_add(weight);
		let key = (asset_loc, price);
		match self.paid_assets.get_mut(&key) {
			Some(v) => v.saturating_accrue(amount),
			None => {
				self.paid_assets.insert(key, amount);
			}
		}
		Ok(unused)
	}

	/// Will refund up to `weight` from the first asset tracked by the trader.
	fn refund_weight(&mut self, weight: Weight, _context: &XcmContext) -> Option<MultiAsset> {
		log::trace!(
			target: "xcm::weight", "MultiCurrencyTrader::refund_weight weight: {:?}, paid_assets: {:?}",
			weight, self.paid_assets
		);
		let weight = weight.min(self.weight);
		self.weight -= weight; // Will not underflow because of `min()` above.
		let fee = ConvertWeightToFee::weight_to_fee(&weight);
		if let Some(((asset_loc, price), amount)) = self.paid_assets.iter_mut().next() {
			let converted_fee: u128 = price.saturating_mul_int(fee).saturated_into();
			let refund = converted_fee.min(*amount);
			*amount -= refund; // Will not underflow because of `min()` above.

			let refund_asset = *asset_loc;
			if amount.is_zero() {
				let key = (*asset_loc, *price);
				self.paid_assets.remove(&key);
			}
			Some((Concrete(refund_asset), refund).into())
		} else {
			None
		}
	}
}

/// We implement `Drop` so that when the weight trader is dropped at the end of XCM execution, the
/// generated revenue is stored on-chain. This is configurable via the `Revenue` generic.
impl<
		AssetId,
		Balance: FixedPointOperand + TryInto<u128>,
		Price: FixedPointNumber,
		ConvertWeightToFee: WeightToFee<Balance = Balance>,
		AcceptedCurrencyPrices: NativePriceOracle<AssetId, Price>,
		ConvertCurrency: Convert<MultiAsset, Option<AssetId>>,
		Revenue: TakeRevenue,
	> Drop
	for MultiCurrencyTrader<AssetId, Balance, Price, ConvertWeightToFee, AcceptedCurrencyPrices, ConvertCurrency, Revenue>
{
	fn drop(&mut self) {
		for ((asset_loc, _), amount) in self.paid_assets.iter() {
			Revenue::take_revenue((*asset_loc, *amount).into());
		}
	}
}

/// Implements `TakeRevenue` by sending the assets to the fee receiver, using an implementor of
/// `DepositFee`.
///
/// Note: Only supports concrete fungible assets.
pub struct ToFeeReceiver<AccountId, AssetId, Balance, Price, C, D, F>(
	PhantomData<(AccountId, AssetId, Balance, Price, C, D, F)>,
);
impl<
		AccountId,
		AssetId,
		Balance: AtLeast32BitUnsigned,
		Price,
		C: Convert<MultiLocation, Option<AssetId>>,
		D: DepositFee<AccountId, AssetId, Balance>,
		F: Get<AccountId>,
	> TakeRevenue for ToFeeReceiver<AccountId, AssetId, Balance, Price, C, D, F>
{
	fn take_revenue(asset: MultiAsset) {
		match asset {
			MultiAsset {
				id: Concrete(loc),
				fun: Fungibility::Fungible(amount),
			} => {
				C::convert(loc).and_then(|id| {
					let receiver = F::get();
					D::deposit_fee(&receiver, id, amount.saturated_into::<Balance>())
						.map_err(|e| log::trace!(target: "xcm::take_revenue", "Could not deposit fee: {:?}", e))
						.ok()
				});
			}
			_ => {
				debug_assert!(false, "Can only accept concrete fungible tokens as revenue.");
				log::trace!(target: "xcm::take_revenue", "Can only accept concrete fungible tokens as revenue.");
			}
		}
	}
}

// Relay chain Block number provider.
// Reason why the implementation is different for benchmarks is that it is not possible
// to set or change the block number in a benchmark using parachain system pallet.
// That's why we revert to using the system pallet in the benchmark.
pub struct RelayChainBlockNumberProvider<T>(sp_std::marker::PhantomData<T>);

#[cfg(not(feature = "runtime-benchmarks"))]
impl<T: cumulus_pallet_parachain_system::Config> BlockNumberProvider for RelayChainBlockNumberProvider<T> {
	type BlockNumber = polkadot_parachain::primitives::RelayChainBlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		let maybe_data = cumulus_pallet_parachain_system::Pallet::<T>::validation_data();

		if let Some(data) = maybe_data {
			data.relay_parent_number
		} else {
			Self::BlockNumber::default()
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<T: frame_system::Config> BlockNumberProvider for RelayChainBlockNumberProvider<T> {
	type BlockNumber = frame_system::pallet_prelude::BlockNumberFor<T>;

	fn current_block_number() -> Self::BlockNumber {
		frame_system::Pallet::<T>::current_block_number()
	}
}

pub trait RelayChainBlockHashProvider {
	fn parent_hash() -> Option<Hash>;
}
// The reason why there is difference between PROD and benchmark is that it is not possible
// to set validation data in parachain system pallet in the benchmarks.
// So for benchmarking, we mock it out and return some hardcoded parent hash
pub struct RelayChainBlockHashProviderAdapter<Runtime>(sp_std::marker::PhantomData<Runtime>);

#[cfg(not(feature = "runtime-benchmarks"))]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		let validation_data = cumulus_pallet_parachain_system::Pallet::<Runtime>::validation_data();
		match validation_data {
			Some(data) => Some(data.parent_head.hash()),
			None => None,
		}
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl<Runtime> RelayChainBlockHashProvider for RelayChainBlockHashProviderAdapter<Runtime>
where
	Runtime: cumulus_pallet_parachain_system::Config,
{
	fn parent_hash() -> Option<cumulus_primitives_core::relay_chain::Hash> {
		// We use the same hash as for integration tests
		// so the integration tests don't fail when they are run with 'runtime-benchmark' feature
		let hash = [
			14, 87, 81, 192, 38, 229, 67, 178, 232, 171, 46, 176, 96, 153, 218, 161, 209, 229, 223, 71, 119, 143, 119,
			135, 250, 171, 69, 205, 241, 47, 227, 168,
		]
		.into();
		Some(hash)
	}
}

/// Passes on trade and liquidity data from the omnipool to the oracle.
pub struct OmnipoolHookAdapter<Origin, NativeAsset, Lrna, Runtime>(PhantomData<(Origin, NativeAsset, Lrna, Runtime)>);

impl<Origin, NativeAsset, Lrna, Runtime> OmnipoolHooks<Origin, AccountId, AssetId, Balance>
	for OmnipoolHookAdapter<Origin, NativeAsset, Lrna, Runtime>
where
	Lrna: Get<AssetId>,
	NativeAsset: Get<AssetId>,
	Runtime: pallet_ema_oracle::Config
		+ pallet_circuit_breaker::Config
		+ frame_system::Config<RuntimeOrigin = Origin>
		+ pallet_staking::Config
		+ pallet_referrals::Config,
	<Runtime as frame_system::Config>::AccountId: From<AccountId>,
	<Runtime as pallet_staking::Config>::AssetId: From<AssetId>,
	<Runtime as pallet_referrals::Config>::AssetId: From<AssetId>,
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
			Price::new(asset.after.reserve, asset.after.hub_reserve),
		)
		.map_err(|(_, e)| e)?;

		match asset.delta_changes.delta_reserve {
			BalanceUpdate::Increase(amount) => {
				pallet_circuit_breaker::Pallet::<Runtime>::ensure_add_liquidity_limit(
					origin,
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?;
			}
			BalanceUpdate::Decrease(amount) => {
				if !asset.safe_withdrawal {
					pallet_circuit_breaker::Pallet::<Runtime>::ensure_remove_liquidity_limit(
						origin,
						asset.asset_id.into(),
						asset.before.reserve.into(),
						amount.into(),
					)?;
				}
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
			Price::new(asset_in.after.reserve, asset_in.after.hub_reserve),
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
			Price::new(asset_out.after.hub_reserve, asset_out.after.reserve),
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
			Price::new(asset.after.hub_reserve, asset.after.reserve),
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
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single_liquidity_limit_entry();
		w1.saturating_add(w2).saturating_add(w3)
	}

	fn on_trade_weight() -> Weight {
		let w1 = OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(2);
		let w2 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::ensure_pool_state_change_limit();
		let w3 = <Runtime as pallet_circuit_breaker::Config>::WeightInfo::on_finalize_single_trade_limit_entry();
		w1.saturating_add(w2).saturating_add(w3)
	}

	fn on_trade_fee(
		fee_account: AccountId,
		trader: AccountId,
		asset: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error> {
		if asset == Lrna::get() {
			return Ok(Balance::zero());
		}
		let referrals_used = if asset == NativeAsset::get() {
			Balance::zero()
		} else {
			pallet_referrals::Pallet::<Runtime>::process_trade_fee(
				fee_account.clone().into(),
				trader.into(),
				asset.into(),
				amount,
			)?
		};

		let staking_used = pallet_staking::Pallet::<Runtime>::process_trade_fee(
			fee_account.into(),
			asset.into(),
			amount.saturating_sub(referrals_used),
		)?;
		Ok(staking_used.saturating_add(referrals_used))
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
				.map_err(|_| pallet_omnipool::Error::<Runtime>::InvalidOraclePrice)?;
		Ok(price)
	}

	fn get_price_weight() -> Weight {
		pallet_ema_oracle::Pallet::<Runtime>::get_price_weight()
	}
}

pub struct OraclePriceProvider<AssetId, AggregatedPriceGetter, Lrna>(
	PhantomData<(AssetId, AggregatedPriceGetter, Lrna)>,
);

impl<AssetId, AggregatedPriceGetter, Lrna> PriceOracle<AssetId>
	for OraclePriceProvider<AssetId, AggregatedPriceGetter, Lrna>
where
	u32: From<AssetId>,
	AggregatedPriceGetter: AggregatedPriceOracle<AssetId, BlockNumber, EmaPrice, Error = OracleError>,
	Lrna: Get<AssetId>,
	AssetId: Clone + Copy,
{
	type Price = EmaPrice;

	/// We calculate prices for trade (in a route) then making the product of them
	fn price(route: &[Trade<AssetId>], period: OraclePeriod) -> Option<EmaPrice> {
		let mut prices: Vec<EmaPrice> = Vec::with_capacity(route.len());
		for trade in route {
			let asset_a = trade.asset_in;
			let asset_b = trade.asset_out;
			let price = match trade.pool {
				PoolType::Omnipool => {
					let price_asset_a_lrna =
						AggregatedPriceGetter::get_price(asset_a, Lrna::get(), period, OMNIPOOL_SOURCE);

					let price_asset_a_lrna = match price_asset_a_lrna {
						Ok(price) => price.0,
						Err(OracleError::SameAsset) => EmaPrice::from(1),
						Err(_) => return None,
					};

					let price_lrna_asset_b =
						AggregatedPriceGetter::get_price(Lrna::get(), asset_b, period, OMNIPOOL_SOURCE);

					let price_lrna_asset_b = match price_lrna_asset_b {
						Ok(price) => price.0,
						Err(OracleError::SameAsset) => EmaPrice::from(1),
						Err(_) => return None,
					};

					let nominator = U128::full_mul(price_asset_a_lrna.n.into(), price_lrna_asset_b.n.into());
					let denominator = U128::full_mul(price_asset_a_lrna.d.into(), price_lrna_asset_b.d.into());

					let rational_as_u128 = round_to_rational((nominator, denominator), Rounding::Nearest);

					EmaPrice::new(rational_as_u128.0, rational_as_u128.1)
				}
				PoolType::Stableswap(pool_id) => {
					let price_asset_a_vs_share =
						AggregatedPriceGetter::get_price(asset_a, pool_id, period, STABLESWAP_SOURCE);

					let price_asset_a_vs_share = match price_asset_a_vs_share {
						Ok(price) => price.0,
						Err(OracleError::SameAsset) => EmaPrice::from(1),
						Err(_) => return None,
					};

					let price_share_vs_asset_b =
						AggregatedPriceGetter::get_price(pool_id, asset_b, period, STABLESWAP_SOURCE);

					let price_share_vs_asset_b = match price_share_vs_asset_b {
						Ok(price) => price.0,
						Err(OracleError::SameAsset) => EmaPrice::from(1),
						Err(_) => return None,
					};

					let nominator = U128::full_mul(price_asset_a_vs_share.n.into(), price_share_vs_asset_b.n.into());
					let denominator = U128::full_mul(price_asset_a_vs_share.d.into(), price_share_vs_asset_b.d.into());

					let rational_as_u128 = round_to_rational((nominator, denominator), Rounding::Nearest);

					EmaPrice::new(rational_as_u128.0, rational_as_u128.1)
				}
				PoolType::XYK => {
					let price_result = AggregatedPriceGetter::get_price(asset_a, asset_b, period, XYK_SOURCE);

					match price_result {
						Ok(price) => price.0,
						Err(OracleError::SameAsset) => EmaPrice::from(1),
						Err(_) => return None,
					}
				}
				_ => return None,
			};

			prices.push(price);
		}

		if prices.is_empty() {
			return None;
		}

		let nominator = prices
			.iter()
			.try_fold(U512::from(1u128), |acc, price| acc.checked_mul(U512::from(price.n)))?;

		let denominator = prices
			.iter()
			.try_fold(U512::from(1u128), |acc, price| acc.checked_mul(U512::from(price.d)))?;

		let rat_as_u128 = round_u512_to_rational((nominator, denominator), Rounding::Nearest);

		Some(EmaPrice::new(rat_as_u128.0, rat_as_u128.1))
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

		FixedU128::checked_from_rational(price.n, price.d).ok_or_else(|| ArithmeticError::Overflow.into())
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
/// to unsupported accounts to an alternative.
///
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
		AccountIdConvert: ConvertLocation<AccountId>,
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
	fn deposit_asset(asset: &MultiAsset, location: &MultiLocation, _context: &XcmContext) -> Result<(), XcmError> {
		match (
			AccountIdConvert::convert_location(location),
			CurrencyIdConvert::convert(asset.clone()),
			Match::matches_fungible(asset),
		) {
			// known asset
			(Some(who), Some(currency_id), Some(amount)) => {
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

	fn withdraw_asset(
		asset: &MultiAsset,
		location: &MultiLocation,
		_maybe_context: Option<&XcmContext>,
	) -> Result<Assets, XcmError> {
		UnknownAsset::withdraw(asset, location).or_else(|_| {
			let who = AccountIdConvert::convert_location(location)
				.ok_or_else(|| XcmError::from(Error::AccountIdConversionFailed))?;
			let currency_id = CurrencyIdConvert::convert(asset.clone())
				.ok_or_else(|| XcmError::from(Error::CurrencyIdConversionFailed))?;
			let amount: MultiCurrency::Balance = Match::matches_fungible(asset)
				.ok_or_else(|| XcmError::from(Error::FailedToMatchFungible))?
				.saturated_into();
			MultiCurrency::withdraw(currency_id, &who, amount).map_err(|e| XcmError::FailedToTransactAsset(e.into()))
		})?;

		Ok(asset.clone().into())
	}

	fn transfer_asset(
		asset: &MultiAsset,
		from: &MultiLocation,
		to: &MultiLocation,
		_context: &XcmContext,
	) -> Result<Assets, XcmError> {
		let from_account =
			AccountIdConvert::convert_location(from).ok_or_else(|| XcmError::from(Error::AccountIdConversionFailed))?;
		let to_account =
			AccountIdConvert::convert_location(to).ok_or_else(|| XcmError::from(Error::AccountIdConversionFailed))?;
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

// Dynamic fees volume adapter
pub struct OracleVolume(Balance, Balance);

impl pallet_dynamic_fees::traits::Volume<Balance> for OracleVolume {
	fn amount_in(&self) -> Balance {
		self.0
	}

	fn amount_out(&self) -> Balance {
		self.1
	}
}

pub struct OracleAssetVolumeProvider<Runtime, Lrna, Period>(PhantomData<(Runtime, Lrna, Period)>);

impl<Runtime, Lrna, Period> pallet_dynamic_fees::traits::VolumeProvider<AssetId, Balance>
	for OracleAssetVolumeProvider<Runtime, Lrna, Period>
where
	Runtime: pallet_ema_oracle::Config,
	Lrna: Get<AssetId>,
	Period: Get<OraclePeriod>,
{
	type Volume = OracleVolume;

	fn asset_volume(asset_id: AssetId) -> Option<Self::Volume> {
		let entry =
			pallet_ema_oracle::Pallet::<Runtime>::get_entry(asset_id, Lrna::get(), Period::get(), OMNIPOOL_SOURCE)
				.ok()?;
		Some(OracleVolume(entry.volume.a_in, entry.volume.a_out))
	}

	fn asset_liquidity(asset_id: AssetId) -> Option<Balance> {
		let entry =
			pallet_ema_oracle::Pallet::<Runtime>::get_entry(asset_id, Lrna::get(), Period::get(), OMNIPOOL_SOURCE)
				.ok()?;
		Some(entry.liquidity.a)
	}
}

pub struct VestingInfo<Runtime>(PhantomData<Runtime>);

impl<Runtime> pallet_staking::traits::VestingDetails<AccountId, Balance> for VestingInfo<Runtime>
where
	Runtime: pallet_balances::Config<Balance = Balance>,
	AccountId: codec::EncodeLike<<Runtime as frame_system::Config>::AccountId>,
{
	fn locked(who: AccountId) -> Balance {
		let lock_id = orml_vesting::VESTING_LOCK_ID;

		pallet_balances::Locks::<Runtime>::get(who)
			.iter()
			.find(|x| x.id == lock_id)
			.map(|p| p.amount)
			.unwrap_or_default()
	}
}

pub struct FreezableNFT<Runtime, Origin>(PhantomData<(Runtime, Origin)>);

impl<Runtime, Origin: OriginTrait<AccountId = AccountId>> pallet_staking::traits::Freeze<AccountId, CollectionId>
	for FreezableNFT<Runtime, Origin>
where
	Runtime: frame_system::Config<RuntimeOrigin = Origin> + pallet_uniques::Config<CollectionId = CollectionId>,
{
	fn freeze_collection(owner: AccountId, collection: CollectionId) -> DispatchResult {
		pallet_uniques::Pallet::<Runtime>::freeze_collection(Runtime::RuntimeOrigin::signed(owner), collection)
	}
}

pub struct MultiCurrencyLockedBalance<T, NativeAssetId: Get<AssetId>>(PhantomData<(T, NativeAssetId)>);

impl<T: orml_tokens::Config + pallet_balances::Config + frame_system::Config, NativeAssetId: Get<AssetId>>
	LockedBalance<AssetId, T::AccountId, Balance> for MultiCurrencyLockedBalance<T, NativeAssetId>
where
	AssetId: Into<<T as orml_tokens::Config>::CurrencyId>,
	Balance: From<<T as orml_tokens::Config>::Balance>,
	Balance: From<<T as pallet_balances::Config>::Balance>,
{
	fn get_by_lock(lock_id: LockIdentifier, currency_id: AssetId, who: T::AccountId) -> Balance {
		if currency_id == NativeAssetId::get() {
			match pallet_balances::Pallet::<T>::locks(who)
				.into_iter()
				.find(|lock| lock.id == lock_id)
			{
				Some(lock) => lock.amount.into(),
				None => Zero::zero(),
			}
		} else {
			match orml_tokens::Pallet::<T>::locks(who, currency_id.into())
				.into_iter()
				.find(|lock| lock.id == lock_id)
			{
				Some(lock) => lock.amount.into(),
				None => Zero::zero(),
			}
		}
	}
}

/// Passes on trade and liquidity changed data from the stableswap to the oracle.
pub struct StableswapHooksAdapter<Runtime>(PhantomData<Runtime>);

impl<Runtime> StableswapHooks<AssetId> for StableswapHooksAdapter<Runtime>
where
	Runtime: pallet_ema_oracle::Config + pallet_stableswap::Config,
{
	fn on_liquidity_changed(pool_id: AssetId, state: PoolState<AssetId>) -> DispatchResult {
		let pool_size = state.assets.len();

		// As we access by index, let's ensure correct vec lengths.
		ensure!(
			state.before.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.after.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.delta.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.share_prices.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);

		for idx in 0..pool_size {
			OnActivityHandler::<Runtime>::on_liquidity_changed(
				STABLESWAP_SOURCE,
				state.assets[idx],
				pool_id,
				state.delta[idx],
				state.issuance_before.abs_diff(state.issuance_after),
				state.after[idx],
				state.issuance_after,
				Price::new(state.share_prices[idx].0, state.share_prices[idx].1),
			)
			.map_err(|(_, e)| e)?;
		}

		Ok(())
	}

	fn on_trade(
		pool_id: AssetId,
		_asset_in: AssetId,
		_asset_out: AssetId,
		state: PoolState<AssetId>,
	) -> DispatchResult {
		let pool_size = state.assets.len();

		// As we access by index, let's ensure correct vec lengths.
		ensure!(
			state.before.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.after.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.delta.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);
		ensure!(
			state.share_prices.len() == pool_size,
			pallet_stableswap::Error::<Runtime>::IncorrectAssets.into()
		);

		for idx in 0..pool_size {
			OnActivityHandler::<Runtime>::on_trade(
				STABLESWAP_SOURCE,
				state.assets[idx],
				pool_id,
				state.delta[idx],
				0, // Correct
				state.after[idx],
				state.issuance_after,
				Price::new(state.share_prices[idx].0, state.share_prices[idx].1),
			)
			.map_err(|(_, e)| e)?;
		}

		Ok(())
	}

	fn on_liquidity_changed_weight(n: usize) -> Weight {
		OnActivityHandler::<Runtime>::on_liquidity_changed_weight().saturating_mul(n as u64)
	}

	fn on_trade_weight(n: usize) -> Weight {
		OnActivityHandler::<Runtime>::on_trade_weight().saturating_mul(n as u64)
	}
}

/// Price provider that returns a price of an asset that can be used to pay tx fee.
/// If an asset cannot be used as fee payment asset, None is returned.
pub struct AssetFeeOraclePriceProvider<A, AC, RP, Oracle, FallbackPrice, Period>(
	PhantomData<(A, AC, RP, Oracle, FallbackPrice, Period)>,
);

impl<AssetId, A, RP, AC, Oracle, FallbackPrice, Period> NativePriceOracle<AssetId, EmaPrice>
	for AssetFeeOraclePriceProvider<A, AC, RP, Oracle, FallbackPrice, Period>
where
	RP: RouteProvider<AssetId>,
	Oracle: PriceOracle<AssetId, Price = EmaPrice>,
	FallbackPrice: GetByKey<AssetId, Option<FixedU128>>,
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
			if let Some(price) = Oracle::price(&route, Period::get()) {
				Some(price)
			} else {
				let fp = FallbackPrice::get(&currency);
				fp.map(|price| EmaPrice::new(price.into_inner(), FixedU128::DIV))
			}
		} else {
			None
		}
	}
}
