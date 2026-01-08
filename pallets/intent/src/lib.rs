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

use crate::types::{AssetId, Balance, IncrementalIntentId, Intent, IntentId, IntentKind, Moment};
use crate::types::{SwapData, SwapType};
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::Blake2_128Concat;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

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
		IntentSubmitted(T::AccountId, IntentId, Intent),
		/// Intent was resolved as part of ICE solution execution.
		IntentResolved {
			id: IntentId,
			owner: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
			fully: bool,
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
		#[pallet::weight(T::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: Intent) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::add_intent(who, intent)?;
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::cancel_intent())]
		pub fn cancel_intent(origin: OriginFor<T>, _intent: IntentId) -> DispatchResult {
			let _who = ensure_signed(origin)?;
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
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

		match intent.kind {
			IntentKind::Swap(ref data) => {
				ensure!(data.amount_in > Balance::zero(), Error::<T>::InvalidIntent);
				ensure!(data.amount_out > Balance::zero(), Error::<T>::InvalidIntent);
				ensure!(data.asset_in != data.asset_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);
			}
		}

		let intent_id = Self::generate_new_intent_id(intent.deadline);
		Intents::<T>::insert(intent_id, &intent);
		IntentOwner::<T>::insert(intent_id, &owner);
		Self::deposit_event(Event::IntentSubmitted(owner, intent_id, intent));
		Ok(intent_id)
	}

	pub fn get_valid_intents() -> Vec<(IntentId, Intent)> {
		let mut intents: Vec<(IntentId, Intent)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline > now);

		intents
	}

	/// Function validates if intent was resolved correctly.
	pub fn validate_resolve(intent: &Intent, resolve: &Intent) -> Result<(), DispatchError> {
		ensure!(intent.deadline > T::TimestampProvider::now(), Error::<T>::IntentExpired);

		ensure!(intent.asset_in() == resolve.asset_in(), Error::<T>::ResolveMismatch);
		ensure!(intent.asset_out() == resolve.asset_out(), Error::<T>::ResolveMismatch);
		ensure!(intent.on_success == resolve.on_success, Error::<T>::ResolveMismatch);
		ensure!(intent.on_failure == resolve.on_failure, Error::<T>::ResolveMismatch);

		match intent.kind {
			IntentKind::Swap(_) => {
				Self::validate_swap_intent_resolve(&intent, resolve)?;
			}
		}

		Ok(())
	}

	fn validate_swap_intent_resolve(intent: &Intent, resolve: &Intent) -> Result<(), DispatchError> {
		let IntentKind::Swap(ref swap) = intent.kind;
		let IntentKind::Swap(ref resolve_swap) = resolve.kind;

		ensure!(swap.swap_type == resolve_swap.swap_type, Error::<T>::ResolveMismatch);
		ensure!(swap.partial == resolve_swap.partial, Error::<T>::ResolveMismatch);

		match swap.swap_type {
			SwapType::ExactIn => {
				if swap.partial {
					if resolve_swap.amount_in == swap.amount_in {
						ensure!(resolve_swap.amount_out >= swap.amount_out, Error::<T>::LimitViolation);
						return Ok(());
					}

					let limit = intent.pro_rata(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
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

					let limit = intent.pro_rata(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;
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

	pub fn intent_resolved(id: IntentId, who: &T::AccountId, resolve: &Intent) -> DispatchResult {
		Intents::<T>::try_mutate_exists(id, |maybe_intent| {
			let intent = maybe_intent.as_mut().ok_or(Error::<T>::IntentNotFound)?;
			let owner = Self::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

			ensure!(owner == *who, Error::<T>::InvalidOwner);

			Self::validate_resolve(&intent, resolve)?;

			let fully_resolved;
			match intent.kind {
				IntentKind::Swap(ref mut s) => {
					let IntentKind::Swap(ref r) = resolve.kind;
					fully_resolved = Self::resolve_swap_intent(s, r)?;
				}
			};

			if fully_resolved {
				*maybe_intent = None;
				IntentOwner::<T>::remove(id);
			} else {
				ensure!(intent.is_partial(), Error::<T>::LimitViolation);
			}

			Self::deposit_event(Event::IntentResolved {
				id,
				owner,
				amount_in: resolve.amount_in(),
				amount_out: resolve.amount_out(),
				fully: fully_resolved,
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

	pub fn unlock_funds(_id: IntentId, _amount: Balance) -> DispatchResult {
		//WARN: implement real unclock with validation
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
