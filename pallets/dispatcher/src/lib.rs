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
use hydradx_traits::evm::MaybeEvmCall;
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
	use frame_support::dispatch::DispatchErrorWithPostInfo;
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

		/// The trait to check whether RuntimeCall is [pallet_evm::Call::call].
		type EvmCallIdentifier: MaybeEvmCall<<Self as Config>::RuntimeCall>;

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
	#[pallet::whitelist_storage]
	#[pallet::getter(fn extra_gas)]
	pub type ExtraGas<T: Config> = StorageValue<_, u64, ValueQuery>;

	#[pallet::storage]
	#[pallet::whitelist_storage]
	#[pallet::getter(fn last_evm_call_failed)]
	pub type LastEvmCallFailed<T: Config> = StorageValue<_, bool, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		EvmCallFailed,
		NotEvmCall,
	}

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

			let (result, actual_weight) = Self::do_dispatch(
				frame_system::Origin::<T>::Signed(T::TreasuryAccount::get()).into(),
				*call,
			);
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

			let (result, actual_weight) = Self::do_dispatch(
				frame_system::Origin::<T>::Signed(AaveManagerAccount::<T>::get()).into(),
				*call,
			);
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
		/// The extra gas is not refunded, even if not used.
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
			ExtraGas::<T>::set(extra_gas);
			let (result, actual_weight) = Self::do_dispatch(origin, *call);
			ExtraGas::<T>::kill();

			if extra_gas == 0u64 {
				return result;
			}

			// We need to add the extra gas to the actual weight - because evm execution does not account for it
			// If actual weight is None, we still account for extra gas
			let actual_weight = if let Some(weight) = actual_weight {
				weight
			} else {
				Weight::zero()
			};
			let extra_weight = T::GasWeightMapping::gas_to_weight(extra_gas, true);
			let actual_weight = Some(actual_weight.saturating_add(extra_weight));

			match result {
				Ok(_) => Ok(PostDispatchInfo {
					actual_weight,
					pays_fee: Pays::Yes,
				}),
				Err(err) => Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight,
						pays_fee: Pays::Yes,
					},
					error: err.error,
				}),
			}
		}

		/// Execute a single EVM call.
		/// This extrinsic will fail if the EVM call returns any other ExitReason than `ExitSucceed(Returned)`.
		/// Look the [hydradx_runtime::evm::runner::WrapRunner] implementation for details.
		///
		/// Parameters:
		/// - `origin`: Signed origin.
		/// - `call`: presumably `pallet_evm::Call::call` as boxed `RuntimeCall`.
		///
		/// Emits `EvmCallFailed` event when failed.
		#[pallet::call_index(4)]
		#[pallet::weight(call.get_dispatch_info().weight)]
		pub fn dispatch_evm_call(
			origin: OriginFor<T>,
			call: Box<<T as Config>::RuntimeCall>,
		) -> DispatchResultWithPostInfo {
			ensure!(T::EvmCallIdentifier::is_evm_call(&call), Error::<T>::NotEvmCall);

			let (result, actual_weight) = Self::do_dispatch(origin, *call);

			match result {
				Ok(_) if Self::last_evm_call_failed() => Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight,
						pays_fee: Pays::Yes,
					},
					error: Error::<T>::EvmCallFailed.into(),
				}),
				Ok(_) => Ok(PostDispatchInfo {
					actual_weight,
					pays_fee: Pays::Yes,
				}),
				Err(err) => Err(DispatchErrorWithPostInfo {
					post_info: PostDispatchInfo {
						actual_weight,
						pays_fee: Pays::Yes,
					},
					error: err.error,
				}),
			}
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Dispatch the call from the specified account as Signed Origin.
	///
	/// Return the result and the actual weight of the dispatched call if there is some.
	fn do_dispatch(
		origin: T::RuntimeOrigin,
		call: <T as Config>::RuntimeCall,
	) -> (DispatchResultWithInfo<PostDispatchInfo>, Option<Weight>) {
		let result = call.dispatch(origin);

		let call_actual_weight = match result {
			Ok(call_post_info) => call_post_info.actual_weight,
			Err(call_err) => call_err.post_info.actual_weight,
		};

		(result, call_actual_weight)
	}
}

// PUBLIC API
impl<T: Config> Pallet<T> {
	/// Decrease the gas for a specific account.
	pub fn decrease_extra_gas(amount: u64) {
		if amount == 0 {
			return;
		}
		let current_value = ExtraGas::<T>::take();
		let new_value = current_value.saturating_sub(amount);
		if new_value > 0 {
			ExtraGas::<T>::set(new_value);
		}
	}

	pub fn set_last_evm_call_failed(status: bool) {
		if !status {
			LastEvmCallFailed::<T>::kill();
		} else {
			LastEvmCallFailed::<T>::put(status);
		}
	}
}
