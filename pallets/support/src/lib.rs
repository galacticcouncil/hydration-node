// This file is part of hydration-node.

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
#![allow(clippy::too_many_arguments)]

use crate::types::*;
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::sp_runtime::{BoundedVec, DispatchError, DispatchResult};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::ConstU32;
use sp_std::vec::Vec;
use sp_std::mem::discriminant;
#[cfg(test)]
mod tests;

pub mod types;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const MAX_STACK_SIZE: u32 = 16;

const LOG_TARGET: &str = "runtime::amm-support";

type ExecutionIdStack = BoundedVec<ExecutionType, ConstU32<MAX_STACK_SIZE>>;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::storage]
	/// Next available incremental ID
	#[pallet::getter(fn incremental_id)]
	pub(super) type IncrementalId<T: Config> = StorageValue<_, IncrementalIdType, ValueQuery>;

	#[pallet::storage]
	/// Execution context to figure out where the trade is originated from
	#[pallet::getter(fn execution_context)]
	pub(super) type ExecutionContext<T: Config> = StorageValue<_, ExecutionIdStack, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Trade executed.
		Swapped {
			swapper: T::AccountId,
			filler: T::AccountId,
			filler_type: Filler,
			operation: TradeOperation,
			inputs: Vec<Asset>,
			outputs: Vec<Asset>,
			fees: Vec<Fee<T::AccountId>>,
			operation_stack: Vec<ExecutionType>,
		},
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			ExecutionContext::<T>::kill();
			
			T::DbWeight::get().reads_writes(1, 1)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	pub fn deposit_trade_event(
		swapper: T::AccountId,
		filler: T::AccountId,
		filler_type: Filler,
		operation: TradeOperation,
		inputs: Vec<Asset>,
		outputs: Vec<Asset>,
		fees: Vec<Fee<T::AccountId>>,
	) {
		let operation_stack = Self::get_context();
		Self::deposit_event(Event::<T>::Swapped {
			swapper,
			filler,
			filler_type,
			operation,
			inputs,
			outputs,
			fees,
			operation_stack,
		});
	}

	pub fn add_to_context<F>(execution_type: F) -> Result<IncrementalIdType, DispatchError>
	where
		F: FnOnce(u32) -> ExecutionType,
	{
		let next_id = IncrementalId::<T>::try_mutate(|current_id| -> Result<IncrementalIdType, DispatchError> {
			let inc_id = *current_id;
			*current_id = current_id.overflowing_add(1).0;
			Ok(inc_id)
		})?;

		ExecutionContext::<T>::try_mutate(|stack| -> DispatchResult {
			//We make it fire and forget, and it should fail only in test and when if wrongly used
			debug_assert_ne!(stack.len(), MAX_STACK_SIZE as usize, "Stack should not be full");
			if let Err(err) = stack.try_push(execution_type(next_id)) {
				log::warn!(target: LOG_TARGET, "The max stack size of execution stack has been reached: {:?}", err);
			}

			Ok(())
		})?;

		Ok(next_id)
	}


	// The `expected_last_stack_entry` parameter ensures the stack behaves as intended.
	// It prevents issues where exceeding the stack size results in ignored actions,
	// which could lead to unexpected stack data when the stack is decreased.
	pub fn remove_from_context<F>(expected_last_stack_entry: F) -> DispatchResult
		where
		F: FnOnce(u32) -> ExecutionType
	{
		ExecutionContext::<T>::try_mutate(|stack| -> DispatchResult {
			//We make it fire and forget, and it should fail only in test and when if wrongly used
			debug_assert_ne!(stack.len(), 0, "The stack should not be empty when decreased");

			if let Some(last_stack_entry) = stack.last() {
				let expected_last_entry = expected_last_stack_entry(0);//We use a dummy 0 as id as we only compare type
				if discriminant(last_stack_entry) == discriminant(&expected_last_entry) {
					if stack.pop().is_none() {
						log::warn!(target: LOG_TARGET,"The execution stack should not be empty when decreased. The stack should be populated first, or should not be decreased more than its size");
					}
				}
			}

			Ok(())
		})?;

		Ok(())
	}

	pub fn get_context() -> Vec<ExecutionType> {
		ExecutionContext::<T>::get().to_vec()
	}
}
