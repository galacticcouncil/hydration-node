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

#[cfg(test)]
pub mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

use evm::ExitReason;
use frame_support::dispatch::{PostDispatchInfo, WithPostDispatchInfo};
use hydradx_traits::evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM};
use pallet_evm::GasWeightMapping;
use sp_core::{crypto::AccountId32, U256};
use sp_runtime::{traits::Dispatchable, DispatchResultWithInfo};
use sp_std::{vec, vec::Vec};
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
use frame_support::pallet_prelude::Weight;
pub use pallet::*;

pub type CallResult = (ExitReason, Vec<u8>);

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

		/// EVM handler.
		type Evm: EVM<CallResult>;

		/// EVM account mapping
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// Gas limit for EVM calls
		#[pallet::constant]
		type GasLimit: Get<u64>;

		/// Maximum number of EVM calls in a batch
		#[pallet::constant]
		type BatchLimit: Get<u32>;

		/// Convert gas to weight
		type GasWeightMapping: GasWeightMapping;

		type TreasuryManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type AaveManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type EvmBatchOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		type TreasuryAccount: Get<Self::AccountId>;
		type DefaultAaveManagerAccount: Get<Self::AccountId>;

		/// The weight information for this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn aave_manager_account)]
	pub type AaveManagerAccount<T: Config> = StorageValue<_, T::AccountId, ValueQuery, T::DefaultAaveManagerAccount>;

	#[pallet::error]
	pub enum Error<T> {
		TreasuryManagerCallFailed,
		AaveManagerCallFailed,
		TooManyCalls,
		EvmCallFailed,
		EmptyBatch,
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
		EvmBatchDispatched {
			caller: T::AccountId,
			num_calls: u32,
		},
		/// An individual EVM call in a batch completed successfully
		EvmCallCompleted {
			index: u32,
			target: EvmAddress,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
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

		/// Execute multiple EVM calls in a batch.
		/// If any call fails, the entire batch is reverted.
		///
		/// Parameters:
		/// - `origin`: Signed origin.
		/// - `calls`: A vector of tuples containing (target, input, value) for each EVM call.
		///
		/// Emits `EvmBatchDispatched` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight({
			let call_count = calls.len().min(T::BatchLimit::get() as usize) as u32;
			T::WeightInfo::dispatch_batch_all_evm(call_count)
		})]
		pub fn dispatch_batch_all_evm(
			origin: OriginFor<T>,
			calls: Vec<(EvmAddress, Vec<u8>, U256)>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			T::EvmBatchOrigin::ensure_origin(frame_system::Origin::<T>::Signed(who.clone()).into())?;

			ensure!(!calls.is_empty(), Error::<T>::EmptyBatch);

			let calls_len = calls.len();
			ensure!(calls_len <= T::BatchLimit::get() as usize, Error::<T>::TooManyCalls);

			let caller_evm_address = T::EvmAccounts::evm_address(&who);
			let mut total_gas_used = 0u64;

			// Execute each call in sequence
			for (index, (target, input, value)) in calls.iter().enumerate() {
				let context = CallContext::new_call(*target, caller_evm_address);

				let (exit_reason, output) = T::Evm::call(context, input.clone(), *value, T::GasLimit::get());

				// If any call fails, revert the entire batch
				match exit_reason {
					ExitReason::Succeed(_) => {
						// Add estimated gas used for this call
						total_gas_used = total_gas_used.saturating_add(T::GasLimit::get());
						Self::deposit_event(Event::<T>::EvmCallCompleted {
							index: index as u32,
							target: *target,
						});
					}
					_ => {
						log::debug!(
							target: "dispatcher",
							"EVM batch execution failed at index {}: {:?}, Output: {:?}",
							index,
							exit_reason,
							output
						);

						// Calculate the base weight plus the weight of the executed calls so far
						let base_weight = T::WeightInfo::dispatch_batch_all_evm(calls_len as u32);
						let call_weight = T::GasWeightMapping::gas_to_weight(total_gas_used, true);
						let total_weight = base_weight.saturating_add(call_weight);

						return Err(Error::<T>::EvmCallFailed.with_weight(total_weight));
					}
				}
			}

			Self::deposit_event(Event::<T>::EvmBatchDispatched {
				caller: who,
				num_calls: calls_len as u32,
			});

			// Calculate total weight: base plus per-call weight
			let base_weight = T::WeightInfo::dispatch_batch_all_evm(calls_len as u32);
			let call_weight = T::GasWeightMapping::gas_to_weight(total_gas_used, true);

			Ok(Some(base_weight.saturating_add(call_weight)).into())
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
