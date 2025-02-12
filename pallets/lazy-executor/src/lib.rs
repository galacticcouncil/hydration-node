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
	dispatch::{GetDispatchInfo, Pays, PostDispatchInfo},
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
pub type IntentId = u128;
pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type BoundedCall = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;
type BalanceOf<T> = <<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<T>>::Balance;

const NO_TIP: u32 = 0;
//Encoded call's length offset for additional extrinsic's data in bytes.
//4(lenght) + 1(version&type) + 32(signer) + 65(signauture) + 16(tip) + 40(signedExtras) + 16(tip)
//NOTE: this is approximate number
const CALL_LEN_OFFSET: u32 = 158;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct CallData<AccountId> {
	origin: AccountId,
	call: BoundedCall,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{DispatchInfo, DispatchResultWithPostInfo},
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

		/// Max. number of submits offchain worker will make in each run
		#[pallet::constant]
		type OcwMaxSubmits: Get<u8>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
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
	pub(super) type CallQueue<T: Config> = StorageMap<_, Blake2_128Concat, CallId, CallData<T::AccountId>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Queued {
			id: CallId,
			who: T::AccountId,
			intent_id: IntentId,
			fees: BalanceOf<T>,
		},

		Executed {
			id: CallId,
			result: DispatchResult,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Provided data can't be decoded
		Corrupted,

		/// `id` reached max. value
		IdOverflow,

		/// Arithmetic or type conversion overflow
		Overflow,

		/// User failed to pay fees for future execution
		FailedToPayFees,

		/// Failed to deposit collected fees
		FailedToDepositFees,

		/// Calls' queue is empty
		EmptyQueue,

		/// Provided call is not not call at the top of the queue
		CallMismatch,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(1)]
		#[pallet::weight({
			let info = if let Ok(c) = <T as Config>::RuntimeCall::decode(&mut &call[..]) {
				c.get_dispatch_info()
			} else {
				DispatchInfo {
					weight: Default::default(),
					class: DispatchClass::Normal,
					pays_fee: Pays::No,
				}
			};
			Weight::from_parts(1000, 1000).saturating_add(info.weight)
		})]
		pub fn dispatch_top(origin: OriginFor<T>, call: BoundedCall) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin)?;

			DispatchNextId::<T>::try_mutate(|id| {
				let call_data = CallQueue::<T>::take(*id).ok_or(Error::<T>::EmptyQueue)?;

				ensure!(call == call_data.call, Error::<T>::CallMismatch);

				let result = if let Ok(call) = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..]) {
					let o: OriginFor<T> = Origin::<T>::Signed(call_data.origin).into();
					let result = call.dispatch(o);

					result
				} else {
					Err(Error::<T>::Corrupted.into())
				};

				Self::deposit_event(Event::Executed {
					id: *id,
					result: result.map(|_| ()).map_err(|e| e.error),
				});

				*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

				Ok(PostDispatchInfo {
					actual_weight: None,
					pays_fee: Pays::No,
				})
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight((Weight::from_parts(1000, 1000), Pays::No))]
		pub fn add(
			origin: OriginFor<T>,
			as_origin: T::AccountId,
			intent_id: IntentId,
			call: BoundedCall,
		) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			//TODO: remove whole extrinsic, this is only for testing

			Self::add_to_queue(intent_id, as_origin, call)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn add_to_queue(
		intent_id: IntentId,
		origin: T::AccountId,
		bounded_call: BoundedCall,
	) -> Result<(), DispatchError> {
		let call = <T as Config>::RuntimeCall::decode(&mut &bounded_call[..]).map_err(|_| Error::<T>::Corrupted)?;

		let mut info = call.get_dispatch_info();
		info.weight = info.weight.saturating_add(T::WeightInfo::dispatch_top_base_weight());

		let len = Call::<T>::dispatch_top {
			call: bounded_call.clone(),
		}
		.encoded_size()
		.saturating_add(CALL_LEN_OFFSET.try_into().map_err(|_| Error::<T>::Overflow)?);

		let fees = pallet_transaction_payment::Pallet::<T>::compute_fee(
			len.try_into().map_err(|_| Error::<T>::Overflow)?,
			&info,
			NO_TIP.into(),
		);

		let already_withdrawn = <T as pallet_transaction_payment::Config>::OnChargeTransaction::withdraw_fee(
			&origin,
			&call,
			&info,
			fees,
			NO_TIP.into(),
		)
		//TODO: log error
		.map_err(|_| Error::<T>::FailedToPayFees)?;

		<T as pallet_transaction_payment::Config>::OnChargeTransaction::correct_and_deposit_fee(
			&origin,
			&info,
			&PostDispatchInfo {
				actual_weight: Some(info.weight),
				pays_fee: Pays::Yes,
			},
			fees,
			NO_TIP.into(),
			already_withdrawn,
		)
		//TODO: log error
		.map_err(|_| Error::<T>::FailedToDepositFees)?;

		let call_id = Self::get_next_call_id()?;
		CallQueue::<T>::insert(
			call_id,
			CallData {
				origin: origin.clone(),
				call: bounded_call,
			},
		);

		Self::deposit_event(Event::Queued {
			id: call_id,
			intent_id,
			who: origin,
			fees,
		});
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
