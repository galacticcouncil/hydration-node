// Copyright (C) 2020-2025  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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
use frame_system::{
	ensure_signed,
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::*,
	Origin,
};
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

pub type Identificator = u128;
#[derive(Clone, Encode, Decode, PartialEq, Eq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum Source {
	ICE(Identificator),
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct CallData<AccountId> {
	origin: AccountId,
	call: BoundedCall,
}

const NO_TIP: u32 = 0;
//Encoded call's length offset for additional extrinsic's data in bytes.
//4(lenght) + 1(version&type) + 32(signer) + 65(signauture) + 16(tip) + 40(signedExtras) + 16(tip)
//NOTE: this is approximate number
const CALL_LEN_OFFSET: u32 = 158;
const LOG_TARGET: &str = "runtime::pallet-lazy-executor";
pub(crate) const OCW_TAG_PREFIX: &str = "lazy-executor-dispatch-top";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{DispatchInfo, DispatchResult},
		pallet_prelude::{TransactionSource, TransactionValidity, ValueQuery, *},
	};

	#[pallet::config]
	pub trait Config:
		SendTransactionTypes<Call<Self>>
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
		Queued {
			id: CallId,
			src: Source,
			who: T::AccountId,
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

		/// Call's weight is bigger than max allowed weight
		Overweight,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			if !sp_io::offchain::is_validator() {
				return;
			}
			log::info!(target: LOG_TARGET, "run offchain worker on block: {:?}", block_number);

			let mut next_id = Self::dispatch_next_id();
			for i in 0..Self::max_txs_per_block() {
				next_id = if let Some(n) = next_id.checked_add(i as u128) {
					n
				} else {
					log::error!(target: LOG_TARGET, "DispatchNextId max. limit reached");
					break;
				};

				if CallQueue::<T>::get(next_id).is_some() {
					let call = Call::dispatch_top { call_id: next_id };
					let r = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
					log::debug!(target: LOG_TARGET, "sutmitted transaction result: {:?}, call_id: {:?}", r, next_id);
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
			if let Call::dispatch_top { call_id } = unsigned_call {
				// discard call not coming from the local node
				match source {
					TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ }
					_ => {
						log::warn!(target: LOG_TARGET, "dispatch_top transaction is not local/in-block.");

						return InvalidTransaction::Call.into();
					}
				}

				ensure!(CallQueue::<T>::contains_key(call_id), InvalidTransaction::Call);

				let max_tx_per_block = Self::max_txs_per_block();
				let next_id = Self::dispatch_next_id();

				//NOTE: id starts from 0, so e.g. 0 + (5 - 1) == 4 => {0,1,2,3,4} transactions
				let max_valid_call_id = next_id
					.checked_add((max_tx_per_block - 1) as u128)
					.ok_or_else(|| InvalidTransaction::Call)?;

				let (provides, requires): (Option<u128>, Option<u128>) = match call_id {
					_ if *call_id == next_id => (Some(*call_id), None),
					_ if *call_id == max_valid_call_id => (None, Some(*call_id - 1)),
					_ if ((next_id + 1)..max_valid_call_id).contains(call_id) => (Some(*call_id), Some(*call_id - 1)),
					_ => {
						//NOTE: call_id > max_valid_call_id || call_id < next_id
						return InvalidTransaction::Call.into();
					}
				};

				let mut tx = ValidTransaction::with_tag_prefix(OCW_TAG_PREFIX)
					.priority(T::UnsignedPriority::get())
					.longevity(T::UnsignedLongevity::get())
					.propagate(false);

				tx = if let Some(p) = provides { tx.and_provides(p) } else { tx };
				tx = if let Some(r) = requires { tx.and_requires(r) } else { tx };

				return tx.build();
			}

			InvalidTransaction::Call.into()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(1)]
		#[pallet::weight({
			let info = if let Some(call_data) = CallQueue::<T>::get(call_id) {

			if let Ok(c) = <T as Config>::RuntimeCall::decode(&mut &call_data.call[..]) {
				c.get_dispatch_info()
			} else {
				DispatchInfo {
					weight: Default::default(),
					class: DispatchClass::Normal,
					pays_fee: Pays::No,
				}
			}

			} else {
				DispatchInfo {
					weight: Default::default(),
					class: DispatchClass::Normal,
					pays_fee: Pays::No,
				}
			};


			//TODO: add weight for storage read
			Weight::from_parts(1000, 1000).saturating_add(info.weight).saturating_add(T::DbWeight::get().reads(1_u64))
		})]
		pub fn dispatch_top(origin: OriginFor<T>, call_id: CallId) -> DispatchResult {
			ensure_none(origin)?;

			DispatchNextId::<T>::try_mutate(|id| {
				ensure!(*id == call_id, Error::<T>::CallMismatch);

				let call_data = CallQueue::<T>::take(*id).ok_or(Error::<T>::EmptyQueue)?;

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

				Ok(())
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight((Weight::from_parts(1000, 1000), Pays::No))]
		pub fn add(origin: OriginFor<T>, as_origin: T::AccountId, src: Source, call: BoundedCall) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			//TODO: remove whole extrinsic, this is only for testing

			Self::add_to_queue(src, as_origin, call)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn add_to_queue(src: Source, origin: T::AccountId, bounded_call: BoundedCall) -> Result<(), DispatchError> {
		let call = <T as Config>::RuntimeCall::decode(&mut &bounded_call[..]).map_err(|_| Error::<T>::Corrupted)?;

		let mut info = call.get_dispatch_info();
		info.weight = info.weight.saturating_add(T::WeightInfo::dispatch_top_base_weight());

		let call_id = Self::get_next_call_id()?;
		let dispatch_top_call: pallet::Call<T> = Call::dispatch_top { call_id };
		info.weight = info.weight.saturating_add(dispatch_top_call.get_dispatch_info().weight);

		if info.weight.any_gt(Self::max_weight_per_call()) {
			return Err(Error::<T>::Overweight.into());
		}

		let len = Call::<T>::dispatch_top { call_id }
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
