// This file is part of hydration-node.

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

type AssetId = u32;
type Balance = u128;

pub use hydradx_traits::{
	router::{Filler, TradeOperation},
	IncrementalIdProvider,
};
pub use primitives::IncrementalId as IncrementalIdType;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::storage]
	/// Incremental ID
	#[pallet::getter(fn incremental_id)]
	pub(super) type IncrementalId<T: Config> = StorageValue<_, IncrementalIdType, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Trade executed.
		Swapped {
			swapper: T::AccountId,
			filler: T::AccountId,
			filler_type: Filler,
			operation: TradeOperation,
			asset_in: AssetId,
			asset_out: AssetId,
			amount_in: Balance,
			amount_out: Balance,
			fees: Vec<(AssetId, Balance)>,
			event_id: Option<u32>,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	pub fn next_incremental_id() -> IncrementalIdType {
		let next_incremental_id = Self::incremental_id();
		<IncrementalId<T>>::set(next_incremental_id + 1); // TODO: checked math
		next_incremental_id
	}

	#[allow(clippy::too_many_arguments)]
	pub fn deposit_trade_event(
		swapper: T::AccountId,
		filler: T::AccountId,
		filler_type: Filler,
		operation: TradeOperation,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
		fees: Vec<(AssetId, Balance)>,
		event_id: Option<IncrementalIdType>,
	) {
		Self::deposit_event(Event::<T>::Swapped {
			swapper,
			filler,
			filler_type,
			operation,
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			fees,
			event_id,
		});
	}
}

impl<T: Config> IncrementalIdProvider<IncrementalIdType> for Pallet<T> {
	fn next_id() -> IncrementalIdType {
		Self::next_incremental_id()
	}
}
