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

// #[cfg(test)]
// pub mod mock;
// #[cfg(test)]
// mod tests;

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

		/// Convert gas to weight
		type GasWeightMapping: GasWeightMapping;

		type TreasuryManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
		type AaveManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

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
			caller: T::AccountId,
			target: EvmAddress,
			input: Vec<u8>,
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

		/// Execute a single EVM call.
		/// This extrinsic will fail if the EVM call reverts.
		///
		/// This can be used with pallet_utility::batch_all to achieve the same
		/// functionality as batch_all itself with automatic revert on EVM call failure.
		///
		/// Parameters:
		/// - `origin`: Signed origin.
		/// - `target`: The EVM address of the contract to call.
		/// - `input`: The input data for the call.
		///
		/// Emits `EvmCallSuccess` event when successful.
		#[pallet::call_index(4)]
		#[pallet::weight({
			let gas_weight = T::GasWeightMapping::gas_to_weight(T::GasLimit::get(), true);
			T::WeightInfo::dispatch_evm_call().saturating_add(gas_weight)
		})]
		pub fn dispatch_evm_call(
			origin: OriginFor<T>,
			target: EvmAddress,
			input: Vec<u8>,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let caller_evm_address = T::EvmAccounts::evm_address(&who);
			let context = CallContext::new_call(target, caller_evm_address);

			let (exit_reason, output) = T::Evm::call(context, input.clone(), U256::zero(), T::GasLimit::get());

			// If the call fails, return an error
			match exit_reason {
				ExitReason::Succeed(_) => {
					let gas_used = T::GasLimit::get(); // TODO: We could potentially get actual gas used from EVM
					let call_weight = T::GasWeightMapping::gas_to_weight(gas_used, true);
					let total_weight = T::WeightInfo::dispatch_evm_call().saturating_add(call_weight);

					Self::deposit_event(Event::<T>::EvmCallCompleted { caller: who, target, input });

					Ok(Some(total_weight).into())
				}
				_ => {
					log::debug!(
						target: "dispatcher",
						"EVM call execution failed: {:?}, Output: {:?}",
						exit_reason,
						output
					);

					let gas_used = T::GasLimit::get(); // Use maximum gas as an approximation
					let call_weight = T::GasWeightMapping::gas_to_weight(gas_used, true);
					let total_weight = T::WeightInfo::dispatch_evm_call().saturating_add(call_weight);

					Err(Error::<T>::EvmCallFailed.with_weight(total_weight))
				}
			}
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
