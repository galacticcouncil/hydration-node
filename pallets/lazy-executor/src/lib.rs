// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! # Lazy-Executor Pallet

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{
	dispatch::GetDispatchInfo,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	traits::ConstU32,
	weights::Weight,
};
use frame_system::{ensure_signed, pallet_prelude::*, Origin};
use sp_core::Get;
use sp_runtime::{
	traits::{BlockNumberProvider, Dispatchable, One},
	BoundedVec, DispatchError,
};

pub use pallet::*;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod tests;

pub type ItemId = u128;

pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type BoundedCall = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct CallData<AccountId, BlockNumber> {
	origin: AccountId,
	call: BoundedCall,
	created_at: BlockNumber,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::{ValueQuery, *},
	};
	use sp_runtime::DispatchResult;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The aggregated call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		// /// Max. amount of calls dispatched on idle per block no matter remaining weight in block.
		#[pallet::constant]
		type MaxDispatchedPerBlock: Get<u8>;

		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub(super) fn DefaultMaxAllowedWeight() -> Weight {
		//TODO: set proper default values
		Weight::from_parts(u64::max_value(), u64::max_value())
	}
	#[pallet::storage]
	#[pallet::getter(fn max_allowed_weight)]
	pub(super) type MaxAllowedWeight<T: Config> = StorageValue<_, Weight, ValueQuery, DefaultMaxAllowedWeight>;

	#[pallet::storage]
	#[pallet::getter(fn next_queue_id)]
	pub(super) type QueueSequencer<T: Config> = StorageValue<_, ItemId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn process_next_id)]
	pub(super) type ProcessNextId<T: Config> = StorageValue<_, ItemId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn call_queue)]
	pub(super) type CallQueue<T: Config> =
		StorageMap<_, Blake2_128Concat, ItemId, CallData<T::AccountId, BlockNumberFor<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Executed { id: ItemId, result: DispatchResult },
		ValidationFailed { id: ItemId, error: DispatchError },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Call's weight is bigger than max. allowed weight
		Overweight,

		/// Provided data can't be decoded
		Corrupted,

		/// `id` reach max value
		IdOverflow,

		/// No calls to process
		NothingToProcess,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(n: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
			Self::process_queue(n, remaining_weight)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(1000, 1000))]
		pub fn add(origin: OriginFor<T>, call: BoundedCall) -> DispatchResult {
			let who = ensure_signed(origin)?;
			//TODO: remove whole extrinsic, this is only for testing

			Self::execute(who, call)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn execute(origin: T::AccountId, bounded_call: BoundedCall) -> Result<(), DispatchError> {
		let call = <T as Config>::RuntimeCall::decode(&mut &bounded_call[..]).map_err(|_err| Error::<T>::Corrupted)?;

		Self::validate_call(&call)?;

		CallQueue::<T>::insert(
			Self::get_next_queue_id()?,
			CallData {
				origin,
				call: bounded_call,
				created_at: T::BlockNumberProvider::current_block_number(),
			},
		);

		Ok(())
	}

	pub fn validate_call(call: &<T as Config>::RuntimeCall) -> Result<(), DispatchError> {
		let max_weight = Self::max_allowed_weight();
		let call_weight = call.get_dispatch_info().weight;

		if call_weight.any_gt(max_weight) {
			return Err(Error::<T>::Overweight.into());
		}

		Ok(())
	}

	fn get_next_queue_id() -> Result<ItemId, DispatchError> {
		QueueSequencer::<T>::try_mutate(|current_val| {
			let ret = *current_val;
			*current_val = current_val.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

			Ok(ret)
		})
	}

	fn process_queue(now: BlockNumberFor<T>, remaining_weight: Weight) -> Weight {
		//TODO: replace with WeightInfo::base_weight after benchmarking
		let mut total_weight = T::WeightInfo::process_queue_base_weight();

		for i in 1..=T::MaxDispatchedPerBlock::get() {
			println!("{:?}/{:?}", i, T::MaxDispatchedPerBlock::get());
			let r = ProcessNextId::<T>::try_mutate(|id| -> Result<(), Error<T>> {
				CallQueue::<T>::try_mutate_exists(id.clone(), |maybe_call| -> Result<(), Error<T>> {
					let call_data = maybe_call.as_mut().ok_or(Error::<T>::NothingToProcess)?;

					println!("{:?}, {:?}", call_data.created_at, now);
					//NOTE: skip execution in same block so storage record exist for the call
					if call_data.created_at == now {
						return Err(Error::<T>::NothingToProcess);
					}

					let call = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..])
						.map_err(|_err| Error::<T>::Corrupted)?;

					if let Err(err) = Self::validate_call(&call) {
						*maybe_call = None;
						*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

						Self::deposit_event(Event::ValidationFailed { id: *id, error: err });
						return Ok(());
					}

					let call_weight = call.get_dispatch_info().weight;
					let iter_weight = call_weight.saturating_add(T::WeightInfo::process_queue_base_weight());
					if (total_weight.saturating_add(iter_weight)).any_gt(remaining_weight) {
						//NOTE: this is not failing case, just not enough space for the call in this block
						return Err(Error::<T>::Overweight);
					}

					let o: OriginFor<T> = Origin::<T>::Signed(call_data.origin.clone()).into();
					let res = call.dispatch(o);

					total_weight = total_weight.saturating_add(iter_weight);

					Self::deposit_event(Event::Executed {
						id: *id,
						result: res.map(|_| ()).map_err(|e| e.error),
					});
					*maybe_call = None;
					*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

					Ok(())
				})
			});

			println!("{:?}", r);
			match r {
				Err(Error::<T>::NothingToProcess) => break,
				Err(Error::<T>::Overweight) => break,
				Err(_err) => {
					//TODO: log error
				}
				_ => {}
			}

			println!("weight [total/remaining]: {:?}/{:?}", total_weight, remaining_weight);
			if total_weight.any_gt(remaining_weight) {
				//TODO: remove panic
				panic!("overweight block")
			}
		}

		total_weight
	}
}
