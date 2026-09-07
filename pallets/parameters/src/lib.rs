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
use frame_system::pallet_prelude::OriginFor;
use sp_core::H160;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::BlockNumberFor;

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// The Uniswap v3 contract addresses the router venue resolves through were set.
		UniswapV3AddressesSet {
			factory: H160,
			swap_router: H160,
			quoter: H160,
		},
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn is_testnet)]
	pub type IsTestnet<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn relay_parent_offset_override)]
	pub type RelayParentOffsetOverride<T> = StorageValue<_, bool, ValueQuery>;

	#[pallet::type_value]
	pub fn DefaultTwoSecBlocksSince<T: Config>() -> BlockNumberFor<T> {
		u32::MAX.into()
	}

	#[pallet::storage]
	#[pallet::getter(fn two_sec_blocks_since)]
	/// Block number at which the runtime switched from 6-second to 2-second blocks.
	pub type TwoSecBlocksSince<T: Config> = StorageValue<_, BlockNumberFor<T>, ValueQuery, DefaultTwoSecBlocksSince<T>>;

	#[pallet::storage]
	#[pallet::getter(fn uniswap_v3_factory)]
	pub type UniswapV3Factory<T> = StorageValue<_, H160, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn uniswap_v3_swap_router)]
	pub type UniswapV3SwapRouter<T> = StorageValue<_, H160, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn uniswap_v3_quoter)]
	pub type UniswapV3Quoter<T> = StorageValue<_, H160, OptionQuery>;

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

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Point the Uniswap v3 router venue at a deployment.
		///
		/// These three addresses are the only link between the runtime and the
		/// Uniswap contracts: `UniswapV3TradeExecutor` resolves every pool through
		/// `factory`, quotes through `quoter`, and swaps through `swap_router`.
		/// Until they are set the venue is inert — every trade returns
		/// "factory not configured" rather than reaching the EVM.
		///
		/// They cannot be constants. Each deployment lands its contracts at
		/// different addresses (plain CREATE, so one extra transaction shifts every
		/// one of them), and the chain id does not disambiguate — a fork reports the
		/// same id as the chain it forked. Off-chain consumers should read this
		/// storage rather than hardcode, for the same reason.
		///
		/// Setting a wrong address is not loud: `getPool` against an address holding
		/// no code simply finds no pool, so the venue goes quiet instead of failing.
		/// Verify against the deployment artefacts after enactment.
		///
		/// Weight: three unconditional storage writes, no reads, no iteration.
		#[pallet::call_index(0)]
		#[pallet::weight(T::DbWeight::get().writes(3))]
		pub fn set_uniswap_v3_addresses(
			origin: OriginFor<T>,
			factory: H160,
			swap_router: H160,
			quoter: H160,
		) -> DispatchResult {
			frame_system::ensure_root(origin)?;
			UniswapV3Factory::<T>::put(factory);
			UniswapV3SwapRouter::<T>::put(swap_router);
			UniswapV3Quoter::<T>::put(quoter);

			Self::deposit_event(Event::UniswapV3AddressesSet {
				factory,
				swap_router,
				quoter,
			});
			Ok(())
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
