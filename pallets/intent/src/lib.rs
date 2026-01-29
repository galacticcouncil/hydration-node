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

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod tests;

pub mod types;
mod weights;

use crate::types::CallbackType;
use crate::types::IncrementalIntentId;
use crate::types::Intent;
use crate::types::Moment;
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::Blake2_128Concat;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::offchain::SubmitTransaction;
use frame_system::pallet_prelude::*;
use hydradx_traits::lazy_executor::Mutate;
use hydradx_traits::lazy_executor::Source;
use hydradx_traits::CreateBare;
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::IntentData;
use ice_support::IntentId;
use ice_support::ResolvedIntent;
use ice_support::SwapData;
use ice_support::SwapType;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub type NamedReserveIdentifier = [u8; 8];
pub const NAMED_RESERVE_ID: [u8; 8] = *b"ICE_int#";

pub const UNSIGNED_TXS_PRIORITY: u64 = 1000;
const OCW_LOG_TARGET: &str = "intent::offchain_worker";
pub(crate) const OCW_TAG_PREFIX: &str = "intnt-cleanup";

#[frame_support::pallet]
pub mod pallet {
	use crate::types::CallData;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateBare<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Multi currency mechanism
		type Currency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = AssetId,
			Balance = Balance,
		>;

		type LazyExecutorHandler: Mutate<Self::AccountId, Error = DispatchError, BoundedCall = CallData>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<AssetId>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted
		IntentSubmitted {
			id: IntentId,
			owner: T::AccountId,
			intent: Intent,
		},
		/// Intent was resolved as part of ICE solution execution.
		IntentResolved {
			id: IntentId,
			amount_in: Balance,
			amount_out: Balance,
		},

		/// Portion of intent was resolved as parf of ICE solution execution.
		IntentResovedPartially {
			id: IntentId,
			amount_in: Balance,
			amount_out: Balance,
		},

		IntentCanceled {
			id: IntentId,
		},

		IntentExpired {
			id: IntentId,
		},

		FailedToQueueCallback {
			id: IntentId,
			callback: CallbackType,
			error: DispatchError,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Invalid deadline
		InvalidDeadline,
		/// Invalid intent parameters
		InvalidIntent,
		/// Referenced intent doesn't exist.
		IntentNotFound,
		/// Referenced intent has expired.
		IntentExpired,
		/// Referenced intent is still active.
		IntentActive,
		/// Intent's resolution doesn't match intent's params.
		ResolveMismatch,
		///Resolution violates intent's limits.
		LimitViolation,
		/// Caluclation overflow.
		ArithmeticOverflow,
		/// Referenced intent's owner doesn't exist.
		IntentOwnerNotFound,
		/// Account is not intent's owner.
		InvalidOwner,
		/// User doesn't have enough reserved funds.
		InsufficientReservedBalance,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub(super) type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent>;

	#[pallet::storage]
	#[pallet::getter(fn intent_owner)]
	pub(super) type IntentOwner<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, T::AccountId>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::add_intent(who, intent)?;
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_intent())]
		pub fn cancel_intent(origin: OriginFor<T>, id: IntentId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Intents::<T>::try_mutate_exists(id, |maybe_intent| {
				let intent = maybe_intent.as_ref().ok_or(Error::<T>::IntentNotFound)?;

				IntentOwner::<T>::try_mutate_exists(id, |maybe_owner| -> Result<(), DispatchError> {
					let owner = maybe_owner.clone().ok_or(Error::<T>::IntentOwnerNotFound)?;

					ensure!(owner == who, Error::<T>::InvalidOwner);

					Self::unlock_funds(&who, intent.data.asset_in(), intent.data.amount_in())?;

					Self::deposit_event(Event::<T>::IntentCanceled { id });

					*maybe_owner = None;
					Ok(())
				})?;

				*maybe_intent = None;
				Ok(())
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::cleanup_intent())]
		pub fn cleanup_intent(origin: OriginFor<T>, id: IntentId) -> DispatchResultWithPostInfo {
			if ensure_none(origin.clone()).is_err() {
				ensure_signed(origin)?;
			}

			Intents::<T>::try_mutate_exists(id, |maybe_intent| {
				let intent = maybe_intent.as_mut().ok_or(Error::<T>::IntentNotFound)?;

				ensure!(intent.deadline <= T::TimestampProvider::now(), Error::<T>::IntentActive);

				IntentOwner::<T>::try_mutate_exists(id, |maybe_owner| -> Result<(), DispatchError> {
					let owner = maybe_owner.as_ref().ok_or(Error::<T>::IntentOwnerNotFound)?;

					//NOTE: it's safe to take, intent will be removed.
					if let Some(cb) = intent.on_failure.take() {
						if let Err(e) = T::LazyExecutorHandler::queue(Source::ICE(id), owner.clone(), cb) {
							Self::deposit_event(Event::FailedToQueueCallback {
								id,
								callback: CallbackType::OnSuccess,
								error: e,
							});
						}
					}

					Self::unlock_funds(owner, intent.data.asset_in(), intent.data.amount_in())?;

					Self::deposit_event(Event::<T>::IntentExpired { id });

					*maybe_owner = None;
					Ok(())
				})?;

				*maybe_intent = None;
				Ok(Pays::No.into())
			})
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		//NOTE: this is tmp. solution for testing
		fn offchain_worker(_block_number: BlockNumberFor<T>) {
			let expired = Self::get_expired_intents();

			for (i, intent_id) in expired.iter().enumerate() {
				if i >= 10 {
					break;
				}

				let call = Call::cleanup_intent { id: *intent_id };
				let tx = T::create_bare(call.into());
				if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(tx) {
					log::error!(target: OCW_LOG_TARGET, "fialed to sumbmit cleanup_intent call, err: {:?}", e);
				};
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			if let Call::cleanup_intent { id } = call {
				match source {
					TransactionSource::Local | TransactionSource::InBlock => { /*OCW or included in block are allowed */
					}
					_ => {
						return InvalidTransaction::Call.into();
					}
				};

				let Some(intent) = Intents::<T>::get(id) else {
					return InvalidTransaction::Call.into();
				};

				ensure!(intent.deadline <= T::TimestampProvider::now(), InvalidTransaction::Call);

				return ValidTransaction::with_tag_prefix(OCW_TAG_PREFIX)
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides(Encode::encode(id))
					.longevity(1)
					.propagate(false)
					.build();
			}
			InvalidTransaction::Call.into()
		}
	}
}

impl<T: Config> Pallet<T> {
	#[require_transactional]
	pub fn add_intent(owner: T::AccountId, intent: Intent) -> Result<IntentId, DispatchError> {
		let now = T::TimestampProvider::now();
		ensure!(intent.deadline > now, Error::<T>::InvalidDeadline);
		ensure!(
			intent.deadline < (now.saturating_add(T::MaxAllowedIntentDuration::get())),
			Error::<T>::InvalidDeadline
		);

		match intent.data {
			IntentData::Swap(ref data) => {
				ensure!(data.amount_in > Balance::zero(), Error::<T>::InvalidIntent);
				ensure!(data.amount_out > Balance::zero(), Error::<T>::InvalidIntent);
				ensure!(data.asset_in != data.asset_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);

				T::Currency::reserve_named(&NAMED_RESERVE_ID, data.asset_in, &owner, data.amount_in)?;
			}
		}

		let id = Self::generate_new_intent_id(intent.deadline);
		Intents::<T>::insert(id, &intent);
		IntentOwner::<T>::insert(id, &owner);
		Self::deposit_event(Event::IntentSubmitted { id, owner, intent });

		Ok(id)
	}

	/// Function returns expired intents.
	pub fn get_expired_intents() -> Vec<IntentId> {
		let mut intents: Vec<(IntentId, Intent)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline <= now);

		intents.iter().map(|x| x.0).collect::<Vec<IntentId>>()
	}

	pub fn get_valid_intents() -> Vec<(IntentId, Intent)> {
		let mut intents: Vec<(IntentId, Intent)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline > now);

		intents
	}

	/// Function validates if intent was resolved correctly.
	pub fn validate_resolve(intent: &Intent, resolve: &IntentData) -> Result<(), DispatchError> {
		ensure!(intent.deadline > T::TimestampProvider::now(), Error::<T>::IntentExpired);

		ensure!(
			intent.data.asset_in() == resolve.asset_in(),
			Error::<T>::ResolveMismatch
		);
		ensure!(
			intent.data.asset_out() == resolve.asset_out(),
			Error::<T>::ResolveMismatch
		);

		match intent.data {
			IntentData::Swap(_) => {
				Self::validate_swap_intent_resolve(intent, resolve)?;
			}
		}

		Ok(())
	}

	fn validate_swap_intent_resolve(intent: &Intent, resolve: &IntentData) -> Result<(), DispatchError> {
		let IntentData::Swap(ref swap) = intent.data;
		let IntentData::Swap(ref resolve_swap) = resolve;

		ensure!(swap.swap_type == resolve_swap.swap_type, Error::<T>::ResolveMismatch);
		ensure!(swap.partial == resolve_swap.partial, Error::<T>::ResolveMismatch);

		match swap.swap_type {
			SwapType::ExactIn => {
				if swap.partial {
					if resolve_swap.amount_in == swap.amount_in {
						ensure!(resolve_swap.amount_out >= swap.amount_out, Error::<T>::LimitViolation);
						return Ok(());
					}

					let limit = intent.data.pro_rata(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
					ensure!(resolve_swap.amount_in < swap.amount_in, Error::<T>::LimitViolation);
					ensure!(resolve_swap.amount_out >= limit, Error::<T>::LimitViolation);
				} else {
					ensure!(resolve_swap.amount_in == swap.amount_in, Error::<T>::LimitViolation);
					ensure!(resolve_swap.amount_out >= swap.amount_out, Error::<T>::LimitViolation);
				};
			}
			SwapType::ExactOut => {
				if swap.partial {
					if resolve_swap.amount_out == swap.amount_out {
						ensure!(resolve_swap.amount_in <= swap.amount_in, Error::<T>::LimitViolation);
						return Ok(());
					}

					let limit = intent.data.pro_rata(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
					ensure!(resolve_swap.amount_in <= limit, Error::<T>::LimitViolation);
					ensure!(resolve_swap.amount_out < swap.amount_out, Error::<T>::LimitViolation);
				} else {
					ensure!(resolve_swap.amount_in <= swap.amount_in, Error::<T>::LimitViolation);
					ensure!(resolve_swap.amount_out == swap.amount_out, Error::<T>::LimitViolation);
				}
			}
		}

		Ok(())
	}

	pub fn intent_resolved(who: &T::AccountId, resolve: &ResolvedIntent) -> DispatchResult {
		let ResolvedIntent { id, data: resolve } = resolve;
		Intents::<T>::try_mutate_exists(id, |maybe_intent| {
			let intent = maybe_intent.as_mut().ok_or(Error::<T>::IntentNotFound)?;
			let owner = Self::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

			ensure!(owner == *who, Error::<T>::InvalidOwner);

			Self::validate_resolve(intent, resolve)?;

			let fully_resolved;
			match intent.data {
				IntentData::Swap(ref mut s) => {
					let IntentData::Swap(ref r) = resolve;
					fully_resolved = Self::resolve_swap_intent(s, r)?;
				}
			};

			if fully_resolved {
				if !intent.data.amount_in().is_zero() {
					Self::unlock_funds(&owner, intent.data.asset_in(), intent.data.amount_in())?;
				}

				//NOTE: it's ok to `take`, intent will be removed from storage.
				if let Some(cb) = intent.on_success.take() {
					if let Err(e) = T::LazyExecutorHandler::queue(Source::ICE(*id), who.clone(), cb) {
						Self::deposit_event(Event::FailedToQueueCallback {
							id: *id,
							callback: CallbackType::OnSuccess,
							error: e,
						});
					};
				}

				*maybe_intent = None;
				IntentOwner::<T>::remove(id);

				Self::deposit_event(Event::IntentResolved {
					id: *id,
					amount_in: resolve.amount_in(),
					amount_out: resolve.amount_out(),
				});
				return Ok(());
			}

			ensure!(intent.data.is_partial(), Error::<T>::LimitViolation);
			Self::deposit_event(Event::IntentResovedPartially {
				id: *id,
				amount_in: resolve.amount_in(),
				amount_out: resolve.amount_out(),
			});

			Ok(())
		})
	}

	// Function updates intent's `SwapData` and returns `true` if intent was fully resolved.
	fn resolve_swap_intent(intent: &mut SwapData, resolve: &SwapData) -> Result<bool, DispatchError> {
		match intent.swap_type {
			SwapType::ExactIn => {
				intent.amount_in = intent
					.amount_in
					.checked_sub(resolve.amount_in)
					.ok_or(Error::<T>::ArithmeticOverflow)?;

				intent.amount_out = intent.amount_out.saturating_sub(resolve.amount_out);

				if intent.amount_in.is_zero() {
					ensure!(intent.amount_out.is_zero(), Error::<T>::LimitViolation);
					return Ok(true);
				}

				Ok(false)
			}
			SwapType::ExactOut => {
				intent.amount_in = intent
					.amount_in
					.checked_sub(resolve.amount_in)
					.ok_or(Error::<T>::ArithmeticOverflow)?;

				intent.amount_out = intent
					.amount_out
					.checked_sub(resolve.amount_out)
					.ok_or(Error::<T>::ArithmeticOverflow)?;

				Ok(intent.amount_out.is_zero())
			}
		}
	}

	/// Function unlocks reserved `amount` of `asset_id` for `who`.
	#[inline(always)]
	pub fn unlock_funds(who: &T::AccountId, asset_id: AssetId, amount: Balance) -> DispatchResult {
		if !T::Currency::unreserve_named(&NAMED_RESERVE_ID, asset_id, who, amount).is_zero() {
			return Err(Error::<T>::InsufficientReservedBalance.into());
		}

		Ok(())
	}
}

impl<T: Config> Pallet<T> {
	fn generate_new_intent_id(deadline: Moment) -> IntentId {
		// We deliberately overflow here, so if we , for some reason, hit to max value, we will start from 0 again
		// it is not an issue, we create new intent id together with deadline, so it is not possible to create two intents with the same id
		let incremental_id = NextIncrementalId::<T>::mutate(|id| -> IncrementalIntentId {
			let current_id = *id;
			(*id, _) = id.overflowing_add(1);
			current_id
		});
		(deadline as u128) << 64 | incremental_id as u128
	}
}
