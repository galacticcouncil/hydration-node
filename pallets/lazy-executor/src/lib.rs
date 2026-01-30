// Copyright (C) 2020-2026  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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
	transactional,
	weights::Weight,
};
use frame_system::{offchain::SubmitTransaction, pallet_prelude::*, Origin};
use hydradx_traits::lazy_executor::Source;
use pallet_transaction_payment::OnChargeTransaction;
use sp_runtime::{
	traits::{Dispatchable, One},
	BoundedVec, DispatchError,
};

pub use pallet::*;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod tests;

pub type CallId = u128;
pub const MAX_DATA_SIZE: u32 = 4 * 1024 * 1024;
pub type BoundedCall = BoundedVec<u8, ConstU32<MAX_DATA_SIZE>>;
type BalanceOf<T> = <<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<T>>::Balance;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct CallData<AccountId> {
	origin: AccountId,
	call: BoundedCall,
}

const NO_TIP: u32 = 0;
//Encoded call's length offset for additional extrinsic's data in bytes.
//4(length) + 1(version&type) + 32(signer) + 65(signature) + 16(tip) + 40(signedExtras) + 16(tip)
//NOTE: this is approximate number
const CALL_LEN_OFFSET: u32 = 158;
const LOG_TARGET: &str = "runtime::pallet-lazy-executor";
pub(crate) const OCW_TAG_PREFIX: &str = "lazy-executor-dispatch-top";
pub(crate) const OCW_PROVIDES: &[u8; 12] = b"dispatch-top";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{DispatchInfo, DispatchResult},
		pallet_prelude::{TransactionSource, TransactionValidity, ValueQuery, *},
	};
	use hydradx_traits::CreateBare;

	#[pallet::config]
	pub trait Config:
		CreateBare<Call<Self>>
		+ frame_system::Config
		+ pallet_transaction_payment::Config<RuntimeCall = <Self as pallet::Config>::RuntimeCall>
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The aggregated call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, Info = DispatchInfo, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;

		/// Configuration for unsigned transaction priority
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		/// Configuration for unsigned transaction longevity
		#[pallet::constant]
		type UnsignedLongevity: Get<TransactionLongevity>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub(super) fn DefaultMaxTxPerBlock() -> u16 {
		10_u16
	}

	#[pallet::type_value]
	pub(super) fn DefaultMaxCallWeight() -> Weight {
		Weight::from_parts(10_000_000_000_u64, 26_000)
	}

	#[pallet::storage]
	#[pallet::getter(fn max_txs_per_block)]
	pub(super) type MaxTxPerBlock<T: Config> = StorageValue<_, u16, ValueQuery, DefaultMaxTxPerBlock>;

	#[pallet::storage]
	#[pallet::getter(fn max_weight_per_call)]
	//max weight of the `dispatch_top`. (Inner call's weight should be included)
	pub(super) type MaxCallWeight<T: Config> = StorageValue<_, Weight, ValueQuery, DefaultMaxCallWeight>;

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
		/// Call was queued for execution.
		Queued {
			id: CallId,
			src: Source,
			who: T::AccountId,
			fees: BalanceOf<T>,
		},

		/// Call was executed.
		Executed { id: CallId, result: DispatchResult },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Failed to decode provided call data.
		Corrupted,

		/// `id` reached max. value.
		IdOverflow,

		/// Arithmetic or type conversion overflow
		Overflow,

		/// User failed to pay fees for future execution.
		FailedToPayFees,

		/// Failed to deposit collected fees.
		FailedToDepositFees,

		/// Queue is empty.
		EmptyQueue,

		/// Call's weight is bigger than max allowed weight.
		Overweight,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			log::debug!(target: LOG_TARGET, "run offchain worker on block: {:?}", block_number);

			let mut next_id = Self::dispatch_next_id();
			for i in 0..Self::max_txs_per_block() {
				next_id = if let Some(n) = next_id.checked_add(i as u128) {
					n
				} else {
					log::debug!(target: LOG_TARGET, "queue is empty");
					break;
				};

				if CallQueue::<T>::get(next_id).is_some() {
					let call = Call::dispatch_top {};
					let tx = T::create_bare(call.into());
					let r = SubmitTransaction::<T, Call<T>>::submit_transaction(tx);
					log::debug!(target: LOG_TARGET, "sutmitted dispatch_top transaction, result: {:?}", r,);
				} else {
					break;
				}
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, unsigned_call: &self::Call<T>) -> TransactionValidity {
			if let Call::dispatch_top {} = unsigned_call {
				// discard call not coming from the local node
				match source {
					TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ }
					_ => {
						return InvalidTransaction::Call.into();
					}
				}

				ensure!(
					CallQueue::<T>::contains_key(Self::dispatch_next_id()),
					InvalidTransaction::Call
				);

				return ValidTransaction::with_tag_prefix(OCW_TAG_PREFIX)
					.priority(T::UnsignedPriority::get())
					.and_provides(OCW_PROVIDES.to_vec())
					.longevity(T::UnsignedLongevity::get())
					.propagate(false)
					.build();
			}

			InvalidTransaction::Call.into()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Extrinsics dispatches top call from the queue.
		///
		/// This is called from OWC.
		///
		/// Emits:
		/// - `Executed` when successful
		#[pallet::call_index(1)]
		#[pallet::weight({
			let info = if let Some(call_data) = CallQueue::<T>::get(DispatchNextId::<T>::get()) {
				if let Ok(c) = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..]) {
					c.get_dispatch_info()
				} else {
					DispatchInfo {
						call_weight: Default::default(),
						extension_weight: Default::default(),
						class: DispatchClass::Normal,
						pays_fee: Pays::No,
					}
				}
			} else {
				DispatchInfo {
					call_weight: Default::default(),
					extension_weight: Default::default(),
					class: DispatchClass::Normal,
					pays_fee: Pays::No,
				}
			};

			Weight::from_parts(1000, 1000).saturating_add(info.call_weight).saturating_add(T::DbWeight::get().reads(1_u64))
		})]
		pub fn dispatch_top(origin: OriginFor<T>) -> DispatchResult {
			ensure_none(origin)?;

			DispatchNextId::<T>::try_mutate(|id| {
				let call_data = CallQueue::<T>::take(*id).ok_or(Error::<T>::EmptyQueue)?;

				let result = if let Ok(call) = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..]) {
					let o: OriginFor<T> = Origin::<T>::Signed(call_data.origin).into();

					call.dispatch(o)
				} else {
					Err(Error::<T>::Corrupted.into())
				};

				Self::deposit_event(Event::Executed {
					id: *id,
					result: result.map(|_| ()).map_err(|e| e.error),
				});

				*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Function adds call to queue for future execution.
	///
	/// This function also charges fees for future call execution and fails if `origin` can't pay
	/// fees.
	#[transactional]
	pub fn add_to_queue(src: Source, origin: T::AccountId, bounded_call: BoundedCall) -> Result<(), DispatchError> {
		let call = <T as Config>::RuntimeCall::decode(&mut &bounded_call[..]).map_err(|_| Error::<T>::Corrupted)?;

		let mut info = call.get_dispatch_info();
		info.call_weight = info
			.call_weight
			.saturating_add(<T as Config>::WeightInfo::dispatch_top_base_weight());

		let call_id = Self::get_next_call_id()?;
		let dispatch_top_call: pallet::Call<T> = Call::dispatch_top {};
		info.call_weight = info
			.call_weight
			.saturating_add(dispatch_top_call.get_dispatch_info().call_weight);

		if info.call_weight.any_gt(Self::max_weight_per_call()) {
			return Err(Error::<T>::Overweight.into());
		}

		let len = Call::<T>::dispatch_top {}
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
		.map_err(|_| Error::<T>::FailedToPayFees)?;

		<T as pallet_transaction_payment::Config>::OnChargeTransaction::correct_and_deposit_fee(
			&origin,
			&info,
			&PostDispatchInfo {
				actual_weight: Some(info.call_weight),
				pays_fee: Pays::Yes,
			},
			fees,
			NO_TIP.into(),
			already_withdrawn,
		)
		.map_err(|_| Error::<T>::FailedToDepositFees)?;

		CallQueue::<T>::insert(
			call_id,
			CallData {
				origin: origin.clone(),
				call: bounded_call,
			},
		);

		Self::deposit_event(Event::Queued {
			id: call_id,
			src,
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

impl<T: Config> hydradx_traits::lazy_executor::Mutate<T::AccountId> for Pallet<T> {
	type Error = DispatchError;
	type BoundedCall = BoundedCall;

	fn queue(src: Source, origin: T::AccountId, call: Self::BoundedCall) -> Result<(), Self::Error> {
		Self::add_to_queue(src, origin, call)
	}
}
