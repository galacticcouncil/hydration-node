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
use polkadot_core_primitives::BlockNumber;
use polkadot_parachain::primitives::RelayChainBlockNumber;
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::traits::BlockNumberProvider;
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
	use sp_runtime::traits::BlockNumberProvider;
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

		type TechnicalOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type DeferDuration: Get<BlockNumber>;

		#[pallet::constant]
		type MaxDeferDuration: Get<BlockNumber>;

		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	/// TODO: document
	#[pallet::getter(fn liquidity_per_asset)]
	pub type LiquidityPerAsset<T: Config> =
		StorageMap<_, Blake2_128Concat, MultiLocation, (u128, RelayChainBlockNumber), ValueQuery>;

	#[pallet::storage]
	/// TODO: document
	#[pallet::getter(fn rate_limit)]
	pub type RateLimits<T: Config> = StorageMap<_, Blake2_128Concat, MultiLocation, u128, OptionQuery>;

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
		/// TODO: document
		// TODO: benchmark
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_trade_volume_limit())]
		pub fn set_limit(origin: OriginFor<T>, multi_location: MultiLocation, limit: u128) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;

			RateLimits::<T>::insert(multi_location, limit);
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
					// let mut liquidity_per_asset = LiquidityPerAsset::<T>::get(location);
					// liquidity_per_asset += amount;

					//LiquidityPerAsset::<T>::insert(location, liquidity_per_asset);

					// TODO: use config for the limit
					// We need to defer the messages that are above the set limit
					// by the ratio of the size of the transaction to the defer duration i.e.
					// If the transaction is 10x the limit, we defer it for 10x the defer duration
					// If the transaction is 0.5x the limit, we defer it for 0.5x the defer duration
					// As such we need to store last transaction size and the last update time
					// to calculate the ratio. i.e.
					// defer_duration = 10

					// limit_per_asset = 1000

					// last_update_time = 0
					// last_filled_volume = ((defer_duration - (current_time - last_update_time)) / defer_duration) * last_filled_volume

					// current_time = 5
					// last_transaction_size = 1000
					// current_transaction_size = 1000
					// volume_left = limit_per_asset - last_filled_volume
					// defer_ratio =  volume_left / current_transaction_size
					// defer_duration = defer_duration * defer_ratio
					// last_update_time = current_time
					// last_transaction_size = current_transaction_size
					//
					// last_filled_volume = ((10 - (5 - 0)) / 10) * 1000 = 500
					// volume_left = 1000 - 500 = 500
					// defer_ratio = 500 / 1000 = 0.5
					// defer_duration = 10 * 0.5 = 5

					let mut liquidity_per_asset = LiquidityPerAsset::<T>::get(location);

					let limit_per_duration: u128 = 1000 * 1_000_000_000_000;
					let defer_duration: u128 = T::DeferDuration::get().into();
					let deferred_by: u128 = (amount - limit_per_duration) / limit_per_duration * defer_duration;

					let current_time = T::BlockNumberProvider::current_block_number();
					let last_update_time = liquidity_per_asset.1;

					let time_difference: u128 =
						TryInto::<u128>::try_into(current_time - last_update_time.into()).ok()?;
					//let b: u128 = defer_duration - a.into();

					//TODO: CONTINUE FROM HERE - we need to use last_filled_volume instead of amount, maybe
					let last_filled_volume: u128 =
						(defer_duration - time_difference) * liquidity_per_asset.0 / defer_duration;

					liquidity_per_asset.0 += amount;
					liquidity_per_asset.1 = TryInto::<BlockNumber>::try_into(current_time).ok()?;

					LiquidityPerAsset::<T>::insert(location, liquidity_per_asset);

					if amount >= limit_per_duration {
						// convert deferred u128 to u32 safely
						return Some(deferred_by.try_into().unwrap_or(T::MaxDeferDuration::get()));
					}
				}
			}
		}

		None
	}
}
