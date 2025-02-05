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
	ensure,
	pallet_prelude::{RuntimeDebug, TypeInfo},
	traits::ConstU32,
	weights::Weight,
};
use frame_system::{ensure_signed, pallet_prelude::*, Origin};
use pallet_transaction_payment::OnChargeTransaction;
use sp_core::Get;
use sp_runtime::{
	traits::{BlockNumberProvider, Dispatchable, One},
	BoundedVec, DispatchError,
};

pub use pallet::*;
pub mod weights;
pub use weights::WeightInfo;

// #[cfg(test)]
// mod tests;

//types
pub type CallId = u128;
pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type BoundedCall = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;

const NO_TIP: u32 = 0;

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
		dispatch::{DispatchInfo, GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::{ValueQuery, *},
	};
	use sp_runtime::DispatchResult;

	#[pallet::config]
	pub trait Config:
		frame_system::Config + pallet_transaction_payment::Config<RuntimeCall = <Self as pallet::Config>::RuntimeCall>
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The aggregated call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, Info = DispatchInfo, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;

		/// Block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		// type ChargeTransaction: SignedExtension<
		// 	AccountId = Self::AccountId,
		// 	Call = <Self as pallet::Config>::RuntimeCall,
		// >;

		/// Max. number of submits offchain worker will make in each run
		#[pallet::constant]
		type OcwMaxSubmits: Get<u8>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn next_call_id)]
	pub(super) type Sequencer<T: Config> = StorageValue<_, CallId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn dispatch_next_id)]
	pub(super) type DispatchNextId<T: Config> = StorageValue<_, CallId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn call_queue)]
	pub(super) type CallQueue<T: Config> =
		StorageMap<_, Blake2_128Concat, CallId, CallData<T::AccountId, BlockNumberFor<T>>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Executed { id: CallId, result: DispatchResult },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Provided data can't be decoded
		Corrupted,

		/// `id` reach max value
		IdOverflow,

		/// Arithmetic or type conversion overflow
		Overflow,

		NotImplemented,

		/// No call to dispatch was found at the top of the calls' queue
		EmptyQueue,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(1000, 1000))]
		//TODO: add BoundedCall as param for this extrinsic so encoded call length is calculated
		//correctly
		pub fn dispatch_top(origin: OriginFor<T>) -> DispatchResult {
			let _who = ensure_signed(origin)?;

			//TODO: figure out call weigth(it should include dispatched call's weightout call and
			//how to refund submitter and charge origin that dispatched call.

			DispatchNextId::<T>::try_mutate(|id| {
				let call_data = CallQueue::<T>::take(*id).ok_or(Error::<T>::EmptyQueue)?;

				let result = if let Ok(call) = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..]) {
					//TODO: charge/refund fee from correct user + make correct weight function
					let info = call.get_dispatch_info();
					let fee = pallet_transaction_payment::Pallet::<T>::compute_fee(
						call_data.call.len().try_into().map_err(|_| Error::<T>::Overflow)?,
						&info,
						NO_TIP.into(),
					);

					//TODO: handle this correclty, result contains imbalance necessary for
					//`correct_and_deposit_fee`
					let _ = <T as pallet_transaction_payment::Config>::OnChargeTransaction::withdraw_fee(
						&call_data.origin,
						&call,
						&info,
						fee,
						NO_TIP.into(),
					);

					let o: OriginFor<T> = Origin::<T>::Signed(call_data.origin).into();
					let result = call.dispatch(o);

					result
				} else {
					Err(Error::<T>::Corrupted.into())
				};

				//TODO: charge and refund fees

				Self::deposit_event(Event::Executed {
					id: *id,
					result: result.map(|_| ()).map_err(|e| e.error),
				});

				*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;
				return Ok(());
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(1000, 1000))]
		pub fn add(origin: OriginFor<T>, as_o: T::AccountId, call: BoundedCall) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			//TODO: remove whole extrinsic, this is only for testing

			Self::queue_call(as_o, call)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn queue_call(origin: T::AccountId, bounded_call: BoundedCall) -> Result<(), DispatchError> {
		ensure!(
			<T as Config>::RuntimeCall::decode(&mut &bounded_call[..]).is_ok(),
			Error::<T>::Corrupted
		);

		CallQueue::<T>::insert(
			Self::get_next_call_id()?,
			CallData {
				origin,
				call: bounded_call,
				created_at: T::BlockNumberProvider::current_block_number(),
			},
		);

		Ok(())
	}

	fn get_next_call_id() -> Result<CallId, DispatchError> {
		Sequencer::<T>::try_mutate(|current_val| {
			let ret = *current_val;
			*current_val = current_val.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

			Ok(ret)
		})
	}
}
