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

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

pub mod types;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::MultiplierProvider;
use codec::HasCompact;
use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::BlockNumberProvider;
use frame_system::pallet_prelude::BlockNumberFor;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::NativePriceOracle;
use primitives::BlockNumber;
use sp_core::U256;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedPointOperand;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::Saturating;

pub const ETH_HDX_REFERENCE_PRICE: FixedU128 = FixedU128::from_inner(16420844565569051996); //Current onchain ETH price on at block 4418935

#[frame_support::pallet]
pub mod pallet {
	use crate::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
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

		/// Default base fee per gas value. Used in genesis if no other value specified explicitly.
		type DefaultBaseFeePerGas: Get<u128>;

		/// Multiplier provider
		type Multiplier: MultiplierProvider;

		/// Native price oracle
		type NativePriceOracle: NativePriceOracle<Self::AssetId, EmaPrice>;

		/// Eth Asset
		#[pallet::constant]
		type WethAssetId: Get<Self::AssetId>;
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

	/// Base Evm Fee
	#[pallet::storage]
	#[pallet::getter(fn base_evm_fee)]
	pub type BaseFeePerGas<T> = StorageValue<_, U256, ValueQuery, DefaultBaseFeePerGas<T>>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_: BlockNumberFor<T>) -> Weight {
			//TODO: add a integration test
			BaseFeePerGas::<T>::mutate(|old_base_fee_per_gas| {
				let min_gas_price = T::DefaultBaseFeePerGas::get().saturating_div(10);
				let max_gas_price = 17304992000u128; //TODO: make it constant
				let mut multiplier = T::Multiplier::next();

				let mut new_base_fee_per_gas = T::DefaultBaseFeePerGas::get()
					+ multiplier
						.saturating_mul_int(T::DefaultBaseFeePerGas::get())
						.saturating_mul(3);

				let Some(eth_hdx_price) = T::NativePriceOracle::price(T::WethAssetId::get()) else {
					log::warn!(target: "runtime::dynamic-evm-fee", "Can not get HDX-ETH price from oracle");
					return;
				};
				let eth_hdx_price = FixedU128::from_rational(eth_hdx_price.n, eth_hdx_price.d);

				let (price_diff, hdx_pumps_against_hdx) = if eth_hdx_price > ETH_HDX_REFERENCE_PRICE {
					(eth_hdx_price.saturating_sub(ETH_HDX_REFERENCE_PRICE), false)
				} else {
					(ETH_HDX_REFERENCE_PRICE.saturating_sub(eth_hdx_price), true)
				};

				let Some(percentage_change) = price_diff
					.saturating_mul(FixedU128::saturating_from_integer(100))
					.const_checked_div(ETH_HDX_REFERENCE_PRICE) else {
					log::warn!(target: "runtime::dynamic-evm-fee", "Can not calculate percentage change");
					return;
				};

				let percentage_change_permill =
					Permill::from_rational(percentage_change.into_inner(), FixedU128::DIV * 100);

				let evm_fee_change = percentage_change_permill.mul_floor(new_base_fee_per_gas);

				if hdx_pumps_against_hdx {
					new_base_fee_per_gas = new_base_fee_per_gas.saturating_sub(evm_fee_change);
				} else {
					new_base_fee_per_gas = new_base_fee_per_gas.saturating_add(evm_fee_change);
				}

				new_base_fee_per_gas = new_base_fee_per_gas.clamp(min_gas_price, max_gas_price);

				*old_base_fee_per_gas = U256::from(new_base_fee_per_gas);
			});

			Weight::default() //TODO: benchmark
		}
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {}
}
