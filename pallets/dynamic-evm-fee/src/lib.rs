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
//! - When HDX increases in value against ETH, the fee is reduced accordingly.
//! - When HDX decreases in value against ETH, the fee is increased accordingly.
//!
//! This dual-criteria approach ensures that transaction fees remain fair and reflective of both market conditions and network demand.

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use weights::WeightInfo;

use crate::types::MultiplierProvider;
use codec::HasCompact;
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::BlockNumberFor;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::NativePriceOracle;
use sp_core::U256;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::Saturating;

//TODO: consider moving to a config
pub const ETH_HDX_REFERENCE_PRICE: FixedU128 = FixedU128::from_inner(16420844565569051996); //Current onchain ETH price on at block 4418935
pub const MAX_BASE_FEE_PER_GAS: u128 = 17304992000u128;

#[frame_support::pallet]
pub mod pallet {
	use crate::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Default base fee per gas value. Used in genesis if no other value specified explicitly.
		type DefaultBaseFeePerGas: Get<u128>;

		//TODO: jus use Get<>
		/// Transaction fee multiplier provider
		type Multiplier: MultiplierProvider;

		/// Native price oracle
		type NativePriceOracle: NativePriceOracle<Self::AssetId, EmaPrice>;

		/// WETH Asset Id
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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			//TODO: add a integration test with price change from trades so oracle price changes
			BaseFeePerGas::<T>::mutate(|old_base_fee_per_gas| {
				let min_base_fee_per_gas = T::DefaultBaseFeePerGas::get().saturating_div(10);
				let multiplier = T::Multiplier::next();

				let mut new_base_fee_per_gas = T::DefaultBaseFeePerGas::get()
					.saturating_add(multiplier
						.saturating_mul_int(T::DefaultBaseFeePerGas::get())
						.saturating_mul(3));

				let Some(eth_hdx_price) = T::NativePriceOracle::price(T::WethAssetId::get()) else {
					log::warn!(target: "runtime::dynamic-evm-fee", "Could not get ETH-HDX price from oracle");
					return;
				};
				let eth_hdx_price = FixedU128::from_rational(eth_hdx_price.n, eth_hdx_price.d);

				// | P1 - P2 | (P1 + P2) / 2  * 100

				let sum = eth_hdx_price.saturating_add(ETH_HDX_REFERENCE_PRICE);

				let diff  = if eth_hdx_price > ETH_HDX_REFERENCE_PRICE {
					eth_hdx_price.saturating_sub(ETH_HDX_REFERENCE_PRICE)
				}else{
					ETH_HDX_REFERENCE_PRICE.saturating_sub(eth_hdx_price)
				};

				// TODO: ensure that sum > 1, just to be safe
				// or use checked div
				let pdiff = diff / (sum / FixedU128::from(2));

				let evm_fee_change = pdiff.saturating_mul_int(new_base_fee_per_gas);

				if eth_hdx_price < crate::ETH_HDX_REFERENCE_PRICE {
					new_base_fee_per_gas = new_base_fee_per_gas.saturating_sub(evm_fee_change);
				} else {
					new_base_fee_per_gas = new_base_fee_per_gas.saturating_add(evm_fee_change);
				}

				// Note that clamp panics!! if min > max
				if min_base_fee_per_gas > MAX_BASE_FEE_PER_GAS {
					// safe safe safe
					//TODO: add integrity test for this
					return;
				}
				new_base_fee_per_gas = new_base_fee_per_gas.clamp(min_base_fee_per_gas, MAX_BASE_FEE_PER_GAS);

				*old_base_fee_per_gas = U256::from(new_base_fee_per_gas);
			});

			T::WeightInfo::on_initialize()
		}


	}
}
impl<T: Config> pallet_evm::FeeCalculator for Pallet<T> {
	fn min_gas_price() -> (U256, Weight) {
		let base_fee_per_gas = Self::base_evm_fee();

		(base_fee_per_gas, T::WeightInfo::on_initialize())
	}
}