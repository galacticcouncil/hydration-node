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

use frame_support::dispatch::DispatchError;
use frame_support::sp_runtime::traits::{AtLeast32Bit, CheckedAdd, One};
use sp_std::vec::Vec;

use hydradx_traits::{Registry, ShareTokenRegistry};
use primitives::Balance;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Asset type
		type AssetId: Parameter + Member + Into<u32> + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset Id is not available. This only happens when it reaches the MAX value of given id type.
		NoIdAvailable,

		/// Asset does not exist
		AssetNotFound,
	}

	/// Core Asset Id
	#[pallet::storage]
	#[pallet::getter(fn core_asset_id)]
	pub type CoreAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

	/// Current asset id. Note: This must set so it does not clash with the CoreAssetId!
	#[pallet::storage]
	#[pallet::getter(fn next_asset_id)]
	pub type NextAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

	/// Created assets
	#[pallet::storage]
	#[pallet::getter(fn asset_ids)]
	pub type AssetIds<T: Config> = StorageMap<_, Twox64Concat, Vec<u8>, Option<T::AssetId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub core_asset_id: T::AssetId,
		pub next_asset_id: T::AssetId,
		pub asset_ids: Vec<(Vec<u8>, T::AssetId)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				core_asset_id: Default::default(),
				next_asset_id: Default::default(),
				asset_ids: vec![],
			}
		}
	}
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			CoreAssetId::<T>::put(self.core_asset_id);
			NextAssetId::<T>::put(self.next_asset_id);
			self.asset_ids.iter().for_each(|(name, asset_id)| {
				AssetIds::<T>::insert(name, Some(asset_id));
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Create asset for given name or return existing AssetId if such asset already exists.
	pub fn get_or_create_asset(name: Vec<u8>) -> Result<T::AssetId, DispatchError> {
		if <AssetIds<T>>::contains_key(&name) {
			Ok(<AssetIds<T>>::get(&name).unwrap())
		} else {
			let asset_id = Self::next_asset_id();
			let next_id = asset_id.checked_add(&One::one()).ok_or(Error::<T>::NoIdAvailable)?;
			<NextAssetId<T>>::put(next_id);
			<AssetIds<T>>::insert(name, Some(asset_id));
			Ok(asset_id)
		}
	}

	pub fn retrieve_asset(name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		if let Some(asset_id) = AssetIds::<T>::get(name) {
			Ok(asset_id)
		} else {
			Err(Error::<T>::AssetNotFound.into())
		}
	}
}

impl<T: Config> Registry<T::AssetId, Vec<u8>, Balance, DispatchError> for Pallet<T> {
	fn exists(asset_id: T::AssetId) -> bool {
		NextAssetId::<T>::get() > asset_id
	}

	fn retrieve_asset(name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		if let Some(asset_id) = AssetIds::<T>::get(&name) {
			Ok(asset_id)
		} else {
			Err(Error::<T>::AssetNotFound.into())
		}
	}

	fn create_asset(name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::AssetId, DispatchError> {
		Self::get_or_create_asset(name.clone())
	}
}

impl<T: Config> ShareTokenRegistry<T::AssetId, Vec<u8>, primitives::Balance, DispatchError> for Pallet<T> {
	fn retrieve_shared_asset(name: &Vec<u8>, _assets: &[T::AssetId]) -> Result<T::AssetId, DispatchError> {
		Self::retrieve_asset(name)
	}

	fn create_shared_asset(
		name: &Vec<u8>,
		_assets: &[T::AssetId],
		_existential_deposit: primitives::Balance,
	) -> Result<T::AssetId, DispatchError> {
		Self::get_or_create_asset(name.clone())
	}
}
