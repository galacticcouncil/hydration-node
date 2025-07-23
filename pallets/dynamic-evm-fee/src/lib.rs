// This file is part of pallet-relaychain-info.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

//! # Dynamic EVM Fee
//!
//! ## Overview
//!
//! The goal of this pallet to have EVM transaction fees in tandem with Substrate fees.
//!
//! This pallet enables dynamic adjustment of the EVM transaction fee, leveraging two primary metrics:
//! - Current network congestion
//! - The oracle price difference between ETH and HDX
//!
//! Fees are calculated with the production of each new block, ensuring responsiveness to changing network conditions.
//!
//! ### Fee Adjustment Based on Network Congestion
//! The formula for adjusting fees in response to network congestion is as follows:
//!
//! BaseFeePerGas = DefaultBaseFeePerGas + (DefaultBaseFeePerGas * Multiplier * 3
//!
//! - `DefaultBaseFeePerGas`: This represents the minimum fee payable for a transaction, set in pallet configuration.
//! - `Multiplier`: Derived from current network congestion levels, this multiplier is computed within the `pallet-transaction-payment`.
//!
//! ### Fee Adjustment Based on ETH-HDX Price Fluctuations
//!
//! The transaction fee is also adjusted in accordance with in ETH-HDX oracle price change:
//! - When HDX increases in value against ETH, the evm fee is increased accordingly.
//! - When HDX decreases in value against ETH, the evm fee is decreased accordingly.
//!
//! This dual-criteria approach ensures that transaction fees remain fair and reflective of both market conditions and network demand.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

#[cfg(test)]
mod tests;

pub mod weights;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use weights::WeightInfo;

use codec::HasCompact;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::{
	Get, Hooks, MaxEncodedLen, MaybeSerializeDeserialize, Member, Parameter, StorageValue, StorageVersion, TypeInfo,
	ValueQuery,
};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;

use frame_support::weights::Weight;
use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::NativePriceOracle;
use orml_traits::GetByKey;
use sp_core::U256;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::Rounding;

#[frame_support::pallet]
pub mod pallet {
	use crate::*;
	use frame_support::pallet_prelude::{EnsureOrigin, IsType, OptionQuery};

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type for the runtime.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Minimum base fee per gas value. Used to bound  the base fee per gas in min direction.
		type MinBaseFeePerGas: Get<u128>;

		/// Maximum base fee per gas value. Used to bound the base fee per gas in max direction.
		type MaxBaseFeePerGas: Get<u128>;

		/// Default base fee per gas value. Used in genesis if no other value specified explicitly.
		type DefaultBaseFeePerGas: Get<u128>;

		/// Transaction fee multiplier provider
		type FeeMultiplier: Get<FixedU128>;

		/// Origin for setting EVM asset most frequently used for EVM transaction fees.
		type SetEvmPriceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// EVM asset prices for different periods to do comparison to scale evm fee
		///
		/// A price represents how much of `d` you need to get 1 of `n`.
		type EvmAssetPrices: GetByKey<Self::AssetId, Option<(EmaPrice, EmaPrice)>>;

		/// Default EVM asset ID used for EVM transaction fees, if EvmAsset is not explicitly set in storage
		#[pallet::constant]
		type WethAssetId: Get<Self::AssetId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub fn DefaultBaseFeePerGas<T: Config>() -> U256 {
		U256::from(T::DefaultBaseFeePerGas::get())
	}

	/// Base fee per gas
	#[pallet::storage]
	#[pallet::getter(fn base_evm_fee)]
	pub type BaseFeePerGas<T> = StorageValue<_, U256, ValueQuery, DefaultBaseFeePerGas<T>>;

	/// The EVM asset that is used for EVM transactions fee payment. If not set, the default evm asset is used
	#[pallet::storage]
	#[pallet::getter(fn evm_asset)]
	pub type EvmAsset<T: Config> = StorageValue<_, T::AssetId, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// EVM asset for EVM fee scaling has been set
		EvmAssetSet { asset_id: T::AssetId },
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			BaseFeePerGas::<T>::mutate(|old_base_fee_per_gas| {
				let multiplier = T::FeeMultiplier::get();

				let mut new_base_fee_per_gas = T::DefaultBaseFeePerGas::get().saturating_add(
					multiplier
						.saturating_mul_int(T::DefaultBaseFeePerGas::get())
						.saturating_mul(3),
				);

				let evm_asset = EvmAsset::<T>::get().unwrap_or(T::WethAssetId::get());

				let Some((eth_per_hdx, eth_per_hdx_reference)) = T::EvmAssetPrices::get(&evm_asset) else {
					log::warn!(target: "runtime::dynamic-evm-fee", "Could not get ETH-HDX price from oracle");
					return;
				};

				let price_diff = eth_per_hdx.saturating_div(&eth_per_hdx_reference);

				let Some(calculated_new_base_fee_per_gas) = multiply_by_rational_with_rounding(
					new_base_fee_per_gas,
					price_diff.n,
					price_diff.d,
					Rounding::Down,
				) else {
					return;
				};

				new_base_fee_per_gas =
					calculated_new_base_fee_per_gas.clamp(T::MinBaseFeePerGas::get(), T::MaxBaseFeePerGas::get());

				*old_base_fee_per_gas = U256::from(new_base_fee_per_gas);
			});

			T::WeightInfo::on_initialize()
		}

		fn integrity_test() {
			assert!(
				T::MinBaseFeePerGas::get() < T::MaxBaseFeePerGas::get(),
				"MinBaseFeePerGas should be less than MaxBaseFeePerGas, otherwise it fails when we clamp for bounding the base fee per gas."
			);
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Sets the EVM asset to scale up or down the EVM transaction fee.
		///
		/// The EVM asset is usually the most frequently used EVM asset on our chain, likely with the most liquidity
		///
		/// This needs to be called by the `SetEvmPriceOrigin` origin.
		///
		/// # Arguments
		/// * `origin`: The origin of the call, must be `SetEvmPriceOrigin`.
		/// * `asset_id`: The asset ID to set as the EVM asset.
		///
		/// Emits an event `EvmAssetSet` when the asset is successfully set.
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::zero())] //TODO: add bench and weight
		pub fn set_evm_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			T::SetEvmPriceOrigin::ensure_origin(origin)?;

			<EvmAsset<T>>::set(Some(asset_id));

			Self::deposit_event(Event::EvmAssetSet { asset_id });

			Ok(())
		}
	}
}

impl<T: Config> pallet_evm::FeeCalculator for Pallet<T> {
	fn min_gas_price() -> (U256, Weight) {
		let base_fee_per_gas = Self::base_evm_fee();

		(base_fee_per_gas, T::WeightInfo::on_initialize())
	}
}
