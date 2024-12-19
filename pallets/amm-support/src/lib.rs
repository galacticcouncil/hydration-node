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
use frame_support::sp_runtime::{ArithmeticError, BoundedVec, DispatchError, DispatchResult};
use frame_system::pallet_prelude::BlockNumberFor;
pub use primitives::IncrementalId as IncrementalIdType;
use sp_core::{ConstU32};
use sp_std::vec::Vec;
#[cfg(test)]
mod tests;

pub mod types;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const MAX_STACK_SIZE: u32 = 10;

type ExecutionIdStack = BoundedVec<ExecutionType<IncrementalIdType>, ConstU32<MAX_STACK_SIZE>>;

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
	/// Execution context stack
	#[pallet::getter(fn id_stack)]
	pub(super) type ExecutionContext<T: Config> = StorageValue<_, ExecutionIdStack, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		MaxStackSizeReached,
		EmptyStack,
	}

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
			operation_id: Vec<ExecutionType<IncrementalIdType>>,
		},
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(_n: BlockNumberFor<T>) -> Weight {
			let mut weight: Weight = Weight::zero();
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 1));

			ExecutionContext::<T>::kill();

			Weight::from_parts(weight.ref_time(), 0)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	/// Returns next incremental ID and updates the storage.
	pub fn next_incremental_id() -> Result<IncrementalIdType, DispatchError> {
		IncrementalId::<T>::try_mutate(|current_id| -> Result<IncrementalIdType, DispatchError> {
			let inc_id = *current_id;
			*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;
			Ok(inc_id)
		})
	}

	pub fn deposit_trade_event(
		swapper: T::AccountId,
		filler: T::AccountId,
		filler_type: Filler,
		operation: TradeOperation,
		inputs: Vec<Asset>,
		outputs: Vec<Asset>,
		fees: Vec<Fee<T::AccountId>>,
	) {
		let operation_id = ExecutionContext::<T>::get().to_vec();
		Self::deposit_event(Event::<T>::Swapped {
			swapper,
			filler,
			filler_type,
			operation,
			inputs,
			outputs,
			fees,
			operation_id,
		});
	}

	pub fn add_to_context(execution_type: fn(u32) -> ExecutionType<u32>) -> Result<IncrementalIdType, DispatchError> {
		//TODO: double check what to do when these can fail, we dont really want failing due to this
		let next_id = IncrementalId::<T>::try_mutate(|current_id| -> Result<IncrementalIdType, DispatchError> {
			let inc_id = *current_id;
			*current_id = current_id.overflowing_add(1).0.into();
			Ok(inc_id)
		})?;

		ExecutionContext::<T>::try_mutate(|stack| -> DispatchResult {
			stack
				.try_push(execution_type(next_id))
				.map_err(|_| Error::<T>::MaxStackSizeReached)?;

			Ok(())
		})?;

		Ok(next_id)
	}

	pub fn remove_from_context() -> Result<ExecutionType<IncrementalIdType>, DispatchError> {
		//TODO: check what to do when it fails, we might dont want to bloc ktrades becase of it
		ExecutionContext::<T>::try_mutate(|stack| -> Result<ExecutionType<IncrementalIdType>, DispatchError> {
			stack.pop().ok_or(Error::<T>::EmptyStack.into())
		})
	}

	fn get() -> Vec<ExecutionType<IncrementalIdType>> {
		ExecutionContext::<T>::get().to_vec()
	}

}
