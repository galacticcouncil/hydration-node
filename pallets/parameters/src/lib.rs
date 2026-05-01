// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2025  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

use core::marker::PhantomData;
#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

pub use pallet::*;

use frame_support::pallet_prelude::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn is_testnet)]
	pub type IsTestnet<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn relay_parent_offset_override)]
	pub type RelayParentOffsetOverride<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub is_testnet: bool,
		pub relay_parent_offset_override: bool,
		pub _phantom: PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				is_testnet: false,
				relay_parent_offset_override: false,
				_phantom: PhantomData,
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			IsTestnet::<T>::put(self.is_testnet);
			RelayParentOffsetOverride::<T>::put(self.relay_parent_offset_override);
		}
	}

	impl<T: Config> Pallet<T> {
		/// Set the flag. Only used for tests.
		#[cfg(feature = "std")]
		pub fn set_testnet_flag(is_testnet: bool) {
			IsTestnet::<T>::put(is_testnet);
		}

		/// Set the relay parent offset override. Only used for tests.
		#[cfg(feature = "std")]
		pub fn set_relay_parent_offset_override(override_enabled: bool) {
			RelayParentOffsetOverride::<T>::put(override_enabled);
		}
	}
}
