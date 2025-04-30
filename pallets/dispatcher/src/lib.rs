// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
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

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

use frame_support::dispatch::PostDispatchInfo;
use pallet_evm::GasWeightMapping;
use sp_runtime::{traits::Dispatchable, DispatchResultWithInfo};
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
use frame_support::pallet_prelude::Weight;
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::FullCodec;
	use frame_support::{
		dispatch::{GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::{Dispatchable, Hash};
	use sp_std::boxed::Box;

	pub type AccountId = u64;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The overarching call type.
		type RuntimeCall: IsType<<Self as frame_system::Config>::RuntimeCall>
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ FullCodec
			+ TypeInfo
			+ From<frame_system::Call<Self>>
			+ Parameter;

		type TreasuryManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type AaveManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type TreasuryAccount: Get<Self::AccountId>;
		type DefaultAaveManagerAccount: Get<Self::AccountId>;

		/// Gas to Weight conversion.
		type GasWeightMapping: GasWeightMapping;

		/// The weight information for this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn aave_manager_account)]
	pub type AaveManagerAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery, T::DefaultAaveManagerAccount>;

	#[pallet::storage]
	#[pallet::getter(fn account_gas_limits)]
	pub type AccountGasLimits<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		TreasuryManagerCallDispatched {
			call_hash: T::Hash,
			result: DispatchResultWithPostInfo,
		},
		AaveManagerCallDispatched {
			call_hash: T::Hash,
			result: DispatchResultWithPostInfo,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().weight;
			let call_len = call.encoded_size() as u32;

			T::WeightInfo::dispatch_as_treasury(call_len)
				.saturating_add(call_weight)
		})]
		pub fn dispatch_as_treasury(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			T::TreasuryManagerOrigin::ensure_origin(origin)?;

			let call_hash = T::Hashing::hash_of(&call);
			let call_len = call.encoded_size() as u32;

			let (result, actual_weight) = Self::do_dispatch(T::TreasuryAccount::get(), *call);
			actual_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_as_treasury(call_len)));

			Self::deposit_event(Event::<T>::TreasuryManagerCallDispatched { call_hash, result });

			Ok(actual_weight.into())
		}

		#[pallet::call_index(1)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().weight;
			let call_len = call.encoded_size() as u32;

			T::WeightInfo::dispatch_as_aave_manager(call_len)
				.saturating_add(call_weight)
		})]
		pub fn dispatch_as_aave_manager(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			T::AaveManagerOrigin::ensure_origin(origin)?;

			let call_hash = T::Hashing::hash_of(&call);
			let call_len = call.encoded_size() as u32;

			let (result, actual_weight) = Self::do_dispatch(AaveManagerAccount::<T>::get(), *call);
			actual_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_as_aave_manager(call_len)));

			Self::deposit_event(Event::<T>::AaveManagerCallDispatched { call_hash, result });

			Ok(actual_weight.into())
		}

		/// Sets the Aave manager account to be used as origin for dispatching calls.
		///
		/// This doesn't actually changes any ACL in the pool.
		///
		/// This is intented to be mainly used in testnet environments, where the manager account
		/// can be different.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::note_aave_manager())]
		pub fn note_aave_manager(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			ensure_root(origin)?;
			AaveManagerAccount::<T>::put(account);
			Ok(())
		}

		/// Dispatch a call with extra gas.
		///
		/// This allows executing calls with additional weight (gas) limit.
		/// The call will fail if it would consume more weight than the specified limit.
		#[pallet::call_index(3)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().weight;
			let call_len = call.encoded_size() as u32;
			let gas_weight = T::GasWeightMapping::gas_to_weight(*extra_gas, true);
			T::WeightInfo::dispatch_with_extra_gas(call_len)
				.saturating_add(call_weight)
				.saturating_add(gas_weight)
		})]
		pub fn dispatch_with_extra_gas(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
			extra_gas: u64,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			// Store the gas limit for this account
			AccountGasLimits::<T>::insert(&who, extra_gas);

			// Dispatch the call
			let (result, _) = Self::do_dispatch(who.clone(), *call);

			// Clean up storage
			AccountGasLimits::<T>::remove(&who);
			result
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Dispatch the call from the specified account as Signed Origin.
	///
	/// Return the result and the actual weight of the dispatched call if there is some.
	fn do_dispatch(
		account: T::AccountId,
		call: <T as Config>::RuntimeCall,
	) -> (DispatchResultWithInfo<PostDispatchInfo>, Option<Weight>) {
		let result = call.dispatch(frame_system::Origin::<T>::Signed(account).into());

		let call_actual_weight = match result {
			Ok(call_post_info) => call_post_info.actual_weight,
			Err(call_err) => call_err.post_info.actual_weight,
		};

		(result, call_actual_weight)
	}
}

// PUBLIC API
impl<T: Config> Pallet<T> {
	/// Get the gas limit for a specific account.
	pub fn get_gas_limit(account: &T::AccountId) -> u64 {
		AccountGasLimits::<T>::get(account).unwrap_or(0u64)
	}

	/// Set the gas limit for a specific account.
	pub fn decrease_gas_limit(account: &T::AccountId, amount: u64) {
		if amount == 0 {
			return;
		}
		let Some(current_limit) = AccountGasLimits::<T>::take(account) else {
			return;
		};
		let new_limit = current_limit.saturating_sub(amount);
		if new_limit > 0 {
			AccountGasLimits::<T>::insert(account, new_limit);
		}
	}
}
