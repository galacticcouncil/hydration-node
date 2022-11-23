// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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
use codec::{Decode, Encode};
use frame_support::traits::Currency;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use sp_core::bytes;
use sp_core::RuntimeDebug;
use sp_std::vec::Vec;
use orml_traits::MultiCurrency;
pub use primitives::Balance;

use scale_info::TypeInfo;
use sp_runtime::DispatchResult;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;


#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use codec::HasCompact;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;


		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/*/// Balance type
        type Balance: Parameter
            + Member
            + Copy
            + PartialOrd
            + MaybeSerializeDeserialize
            + Default;*/
	}

	#[pallet::pallet]
	#[pallet::without_storage_info]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn initial_liquidity)]
	pub type InitialLiquidity<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, BalanceOf<T>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {

	pub fn something() {

	}
}

/// Handler used by AMM pools to perform some tasks when a trade is executed.
pub trait OnTradeHandler<AssetId, Balance> {
    fn on_trade(asset_id: AssetId, initial_liquidity: Balance);
}

impl<T: Config> OnTradeHandler<T::AssetId, BalanceOf<T>> for Pallet<T> {
	fn on_trade(asset_id: T::AssetId, initial_liquidity: BalanceOf<T>) -> DispatchResult {
		<InitialLiquidity<T>>::insert(asset_id, initial_liquidity);
		Ok(())
	}
}