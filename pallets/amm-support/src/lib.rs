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

type AssetId = u32;
type Balance = u128;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::app_crypto::sp_core;
use frame_support::sp_runtime::{ArithmeticError, BoundedVec, DispatchError, DispatchResult};
pub use hydradx_traits::{
	router::{AssetType, ExecutionType, ExecutionTypeStack, Fee, Filler, OtcOrderId, TradeOperation},
	IncrementalIdProvider,
};
pub use primitives::IncrementalId as IncrementalIdType;
use primitives::ItemId as NftId;
use scale_info::TypeInfo;
use sp_core::{ConstU32, RuntimeDebug};
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const MAX_STACK_SIZE: u32 = 10;

#[derive(RuntimeDebug, Encode, Decode, Default, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct ExecutionIdStack(BoundedVec<ExecutionType<IncrementalIdType>, ConstU32<MAX_STACK_SIZE>>);
impl ExecutionIdStack {
	fn push(&mut self, execution_type: ExecutionType<IncrementalIdType>) -> Result<(), ()> {
		self.0.try_push(execution_type).map_err(|_| ())
	}

	fn pop(&mut self) -> Result<ExecutionType<IncrementalIdType>, ()> {
		self.0.pop().ok_or(())
	}

	fn get(self) -> Vec<ExecutionType<IncrementalIdType>> {
		self.0.into_inner()
	}

}

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

	// TODO:
	#[pallet::storage]
	/// Next available incremental ID
	#[pallet::getter(fn id_stack)]
	pub(super) type IdStack<T: Config> = StorageValue<_, ExecutionIdStack, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		MaxStackSizeReached,
		EmptyStack,
	}

	// on initialize - clear operation_id stack if not empty
	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Trade executed.
		Swapped {
			swapper: T::AccountId,
			filler: T::AccountId,
			filler_type: Filler<AssetId, OtcOrderId>,
			operation: TradeOperation,
			inputs: Vec<(AssetType<AssetId, NftId>, Balance)>,
			outputs: Vec<(AssetType<AssetId, NftId>, Balance)>,
			fees: Vec<Fee<AssetId, Balance, T::AccountId>>,
			operation_id: Vec<ExecutionType<IncrementalIdType>>,
		},
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

	#[allow(clippy::too_many_arguments)]
	pub fn deposit_trade_event(
		swapper: T::AccountId,
		filler: T::AccountId,
		filler_type: Filler<AssetId, OtcOrderId>,
		operation: TradeOperation,
		inputs: Vec<(AssetType<AssetId, NftId>, Balance)>,
		outputs: Vec<(AssetType<AssetId, NftId>, Balance)>,
		fees: Vec<Fee<AssetId, Balance, T::AccountId>>,
	) {
		Self::deposit_event(Event::<T>::Swapped {
			swapper,
			filler,
			filler_type,
			operation,
			inputs,
			outputs,
			fees,
			operation_id: <Self as ExecutionTypeStack<IncrementalIdType>>::get(),
		});
	}
}

impl<T: Config> ExecutionTypeStack<IncrementalIdType> for Pallet<T> {
	fn push(execution_type: ExecutionType<IncrementalIdType>) -> DispatchResult {
		IdStack::<T>::try_mutate(|stack| -> DispatchResult {
			stack
				.push(execution_type)
				.map_err(|_| Error::<T>::MaxStackSizeReached.into())
		})
	}

	fn pop() -> Result<ExecutionType<IncrementalIdType>, DispatchError> {
		IdStack::<T>::try_mutate(|stack| -> Result<ExecutionType<IncrementalIdType>, DispatchError> {
			stack.pop().map_err(|_| Error::<T>::EmptyStack.into())
		})
	}

	fn get() -> Vec<ExecutionType<IncrementalIdType>> {
		IdStack::<T>::get().get()
	}
}
impl<T: Config> IncrementalIdProvider<IncrementalIdType> for Pallet<T> {
	fn next_id() -> Result<IncrementalIdType, DispatchError> {
		Self::next_incremental_id()
	}
}
