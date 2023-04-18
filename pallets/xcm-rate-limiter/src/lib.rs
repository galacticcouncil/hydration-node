// This file is part of HydraDX.

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

use codec::{Decode, Encode};
use cumulus_pallet_xcmp_queue::XcmDeferFilter;
use frame_support::dispatch::Weight;
use frame_support::traits::{Contains, EnsureOrigin};
use frame_support::{ensure, pallet_prelude::DispatchResult, traits::Get};
use frame_system::ensure_signed_or_root;
use frame_system::pallet_prelude::OriginFor;
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Zero};
use sp_runtime::{ArithmeticError, DispatchError, RuntimeDebug};
use xcm::lts::prelude::*;
use xcm::VersionedXcm;
use xcm::VersionedXcm::V3;

pub mod weights;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_support::traits::Contains;
	use polkadot_parachain::primitives::RelayChainBlockNumber;
	use xcm::lts::MultiLocation;

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo
			+ AtLeast32BitUnsigned;

		#[pallet::constant]
		type DeferDuration: Get<RelayChainBlockNumber>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	/// TODO:
	#[pallet::getter(fn liquidity_per_asset)]
	pub type LiquidityPerAsset<T: Config> = StorageMap<_, Blake2_128Concat, MultiLocation, u128, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Event1 {},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Invalid value for a limit. Limit must be non-zero.
		Error1,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set trade volume limit for an asset.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_trade_volume_limit())]
		pub fn asd(origin: OriginFor<T>, asset_id: T::AssetId, trade_volume_limit: (u32, u32)) -> DispatchResult {
			Ok(())
		}
	}
}

fn get_loc_and_amount(m: &MultiAsset) -> Option<(MultiLocation, u128)> {
	match m.id {
		AssetId::Concrete(location) => match m.fun {
			Fungibility::Fungible(amount) => Some((location, amount)),
			_ => None,
		},
		_ => None,
	}
}

impl<T: Config> Pallet<T> {
	fn get_locations_and_amounts(instruction: &Instruction<T::RuntimeCall>) -> Vec<(MultiLocation, u128)> {
		use Instruction::*;
		match instruction {
			ReserveAssetDeposited(multi_assets) | ReceiveTeleportedAsset(multi_assets) => multi_assets
				.inner()
				.iter()
				.flat_map(|asset| get_loc_and_amount(asset))
				.collect(),
			_ => Vec::new(),
		}
	}
}

impl<T: Config> XcmDeferFilter<T::RuntimeCall> for Pallet<T> {
	fn deferred_by(
		para: polkadot_parachain::primitives::Id,
		sent_at: polkadot_core_primitives::BlockNumber,
		xcm: &VersionedXcm<T::RuntimeCall>,
	) -> Option<polkadot_core_primitives::BlockNumber> {
		if let V3(xcm) = xcm {
			if let Some(instruction) = xcm.first() {
				for (location, amount) in Pallet::<T>::get_locations_and_amounts(instruction) {
					let mut liquidity_per_asset = LiquidityPerAsset::<T>::get(location);
					liquidity_per_asset += amount;

					LiquidityPerAsset::<T>::insert(location, liquidity_per_asset);

					if liquidity_per_asset >= 1000 * 1_000_000_000_000 {
						return Some(T::DeferDuration::get());
					}
				}
			}
		}

		None
	}
}
