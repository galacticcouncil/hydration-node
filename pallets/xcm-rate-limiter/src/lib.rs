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

//! ## XCM Rate Limiter Pallet
//!
//! ### Overview
//!
//! This pallet provides an implementation of `XcmDeferFilter` that tracks incoming tokens and defers iff
//! they exceed the rate limit configured in `RateLimitFor`.
//!
//! #### Integration
//!
//! The `RateLimitFor` associated type is supposed to be provided by the `AssetRegistry`,
//! but could work with any other implementation.
//!
//! This pallet does not provide any extrinsics of its own,
//! but it is meant to provide the implementation of `XcmDeferFilter` for the `XcmpQueue`.
//!
//! #### Implementation
//!
//! The duration for deferring an XCM is calculated based on:
//! - the incoming amount
//! - the rate limit of the asset
//! - the configured `DeferDuration`
//! - the amounts of tokens accumulated over time but decayed based on time and rate limit
//!
//! The tokens are deferred once the rate limit is exceeded, with 2 times the rate limit corresponding to deferred duration.
//! For example, if the rate limit is 1000 tokens per 10 blocks, then 1500 tokens will be deferred by 5 blocks.
//!
//! THe accumulated amounts decay linearly at the rate limit. For example: With rate limit 1000 tokens per 10 blocks,
//! the accumulated amount will be reduced by 100 tokens per block.
//!
//! The filter works with XCM v3 and so assumes that other versions can be converted to it.
//!
//! The filter processes only the first instruction of the XCM message, because that is how assets will arrive on chain.
//! This is guaranteed by `AllowTopLevelExecution` which is standard in the ecosystem.
//!

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use cumulus_pallet_xcmp_queue::XcmDeferFilter;

use frame_support::pallet_prelude::Weight;
use frame_support::traits::Get;
use hydra_dx_math::rate_limiter::{calculate_deferred_duration, calculate_new_accumulated_amount};

use orml_traits::GetByKey;
use polkadot_parachain::primitives::RelayChainBlockNumber;
use scale_info::TypeInfo;
use sp_core::MaxEncodedLen;
use sp_runtime::traits::AtLeast32BitUnsigned;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::traits::Convert;
use sp_runtime::RuntimeDebug;
use sp_runtime::SaturatedConversion;
use sp_std::vec::Vec;
use xcm::lts::prelude::*;
use xcm::VersionedXcm;
use xcm::VersionedXcm::V3;

pub mod weights;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
pub use weights::WeightInfo;

#[derive(Clone, Default, Encode, Decode, RuntimeDebug, MaxEncodedLen, TypeInfo, Eq, PartialEq)]
pub struct AccumulatedAmount {
	pub amount: u128,
	pub last_updated: RelayChainBlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;

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

		/// Defer duration base to be used for calculating the specific defer duration for any asset
		#[pallet::constant]
		type DeferDuration: Get<RelayChainBlockNumber>;

		/// The maximum number of blocks to defer XCMs by.
		#[pallet::constant]
		type MaxDeferDuration: Get<RelayChainBlockNumber>;

		/// Relay chain block number provider
		type RelayBlockNumberProvider: BlockNumberProvider<BlockNumber = RelayChainBlockNumber>;

		/// Convert from `MultiLocation` to local `AssetId`
		type CurrencyIdConvert: Convert<MultiLocation, Option<Self::AssetId>>;

		/// Xcm rate limit getter for each asset
		type RateLimitFor: GetByKey<Self::AssetId, Option<u128>>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	/// Accumulated amounts for each asset
	#[pallet::getter(fn accumulated_amount)]
	pub type AccumulatedAmounts<T: Config> =
		StorageMap<_, Blake2_128Concat, MultiLocation, AccumulatedAmount, ValueQuery>;

	#[pallet::event]
	pub enum Event<T: Config> {}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
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
			// NOTE: This does not address the native asset "coming back" from other chains.
			ReserveAssetDeposited(multi_assets) | ReceiveTeleportedAsset(multi_assets) => {
				multi_assets.inner().iter().flat_map(get_loc_and_amount).collect()
			}
			_ => Vec::new(),
		}
	}
}

impl<T: Config> XcmDeferFilter<T::RuntimeCall> for Pallet<T> {
	fn deferred_by(
		_para: polkadot_parachain::primitives::Id,
		_sent_at: RelayChainBlockNumber,
		versioned_xcm: &VersionedXcm<T::RuntimeCall>,
	) -> (Weight, Option<RelayChainBlockNumber>) {
		use xcm::IntoVersion;
		let maybe_xcm = versioned_xcm.clone().into_version(3);
		let Ok(V3(xcm)) = maybe_xcm else { return (Weight::default(), Some(T::MaxDeferDuration::get())) };
		// SAFETY NOTE: It is fine to only look at the first instruction because that is how assets will arrive on chain.
		//              This is guaranteed by `AllowTopLevelExecution` which is standard in the ecosystem.
		let Some(instruction) = xcm.first() else { return (Weight::default(), None) };
		for (location, amount) in Pallet::<T>::get_locations_and_amounts(instruction) {
			let accumulated_liquidity = AccumulatedAmounts::<T>::get(location);

			let Some(asset_id) = T::CurrencyIdConvert::convert(location) else { continue };
			let Some(limit_per_duration) = T::RateLimitFor::get(&asset_id) else { continue };
			let defer_duration = T::DeferDuration::get();

			let current_time = T::RelayBlockNumberProvider::current_block_number();
			// let's assume one read for `RateLimitFor` as well as a read and write for `AccumulatedAmounts` updates.
			let weight = if current_time == accumulated_liquidity.last_updated {
				T::DbWeight::get().reads(1)
			} else {
				T::DbWeight::get().reads_writes(2, 1)
			};
			let time_difference = current_time.saturating_sub(accumulated_liquidity.last_updated);

			let new_accumulated_amount = calculate_new_accumulated_amount(
				defer_duration.saturated_into(),
				limit_per_duration,
				amount,
				accumulated_liquidity.amount,
				time_difference.saturated_into(),
			);

			let deferred_by = calculate_deferred_duration(
				defer_duration.saturated_into(),
				limit_per_duration,
				new_accumulated_amount,
			);

			AccumulatedAmounts::<T>::insert(
				location,
				AccumulatedAmount {
					amount: new_accumulated_amount,
					last_updated: current_time,
				},
			);

			//TODO: we should not return because there might be other locations/assets in the xcm to be handled. When todo is processed, extend doc.
			if deferred_by > 0 {
				return (
					weight,
					Some(deferred_by.min(T::MaxDeferDuration::get().saturated_into())),
				);
			} else {
				return (weight, None);
			}
		}

		(Weight::default(), None)
	}
}
