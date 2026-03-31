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

use crate::types::IncrementalIntentId;
use crate::types::Intent;
use crate::types::IntentInput;
use crate::types::Moment;
use core::cmp;
use frame_support::pallet_prelude::StorageValue;
use frame_support::pallet_prelude::*;
use frame_support::traits::Time;
use frame_support::Blake2_128Concat;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::offchain::SubmitTransaction;
use frame_system::pallet_prelude::*;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::lazy_executor::Mutate;
use hydradx_traits::lazy_executor::Source;
use hydradx_traits::registry::Inspect;
use hydradx_traits::router::{PoolType, Trade as OracleTrade};
use hydradx_traits::CreateBare;
use hydradx_traits::{OraclePeriod, PriceOracle};
use ice_support::AssetId;
use ice_support::Balance;
use ice_support::DcaData;
use ice_support::IntentData;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::ResolvedIntent;
use ice_support::SwapData;
use orml_traits::NamedMultiReservableCurrency;
pub use pallet::*;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::traits::Zero;
use sp_runtime::{FixedPointNumber, FixedU128};
use sp_std::prelude::*;
pub use weights::WeightInfo;

pub type NamedReserveIdentifier = [u8; 8];
pub const NAMED_RESERVE_ID: [u8; 8] = *b"ICE_int#";

pub const UNSIGNED_TXS_PRIORITY: u64 = 1000;
const OCW_LOG_TARGET: &str = "intent::offchain_worker";
const LOG_PREFIX: &str = "ICE#pallet_intent";
pub(crate) const OCW_TAG_PREFIX: &str = "intent-cleanup";

#[frame_support::pallet]
pub mod pallet {
	use crate::types::CallData;

	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + CreateBare<Call<Self>> {
		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// Multi currency mechanism
		type Currency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = AssetId,
			Balance = Balance,
		>;

		/// Intents' lazy callback execution handling
		type LazyExecutorHandler: Mutate<Self::AccountId, Error = DispatchError, BoundedCall = CallData>;

		/// Asset registry handler
		type RegistryHandler: Inspect<AssetId = AssetId>;

		/// Asset Id of hub asset
		#[pallet::constant]
		type HubAssetId: Get<AssetId>;

		/// Maximum deadline for intent in milliseconds.
		#[pallet::constant]
		type MaxAllowedIntentDuration: Get<Moment>;

		/// Oracle price provider for DCA dynamic slippage.
		type OraclePriceProvider: PriceOracle<AssetId, Price = EmaPrice>;

		/// Provider for the current block number (used for DCA scheduling).
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Minimum DCA period in blocks.
		#[pallet::constant]
		type MinDcaPeriod: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New intent was submitted.
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

		/// Portion of intent was resolved as part of ICE solution execution.
		IntentResovedPartially {
			id: IntentId,
			amount_in: Balance,
			amount_out: Balance,
		},

		/// Intent was canceled.
		IntentCanceled { id: IntentId },

		/// Intent expired.
		IntentExpired { id: IntentId },

		/// Failed to add intent's callback to queue for execution.
		FailedToQueueCallback { id: IntentId, error: DispatchError },

		/// A single DCA trade was executed; intent stays in storage for the next period.
		DcaTradeExecuted {
			id: IntentId,
			amount_in: Balance,
			amount_out: Balance,
			remaining_budget: Balance,
		},

		/// DCA intent completed (budget exhausted). Intent removed from storage.
		DcaCompleted { id: IntentId },
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
		/// Intent's resolution doesn't match intent's parameters.
		ResolveMismatch,
		///Resolution violates intent's limits.
		LimitViolation,
		/// Calculation overflow.
		ArithmeticOverflow,
		/// Referenced intent's owner doesn't exist.
		IntentOwnerNotFound,
		/// Account is not intent's owner.
		InvalidOwner,
		/// User doesn't have enough reserved funds.
		InsufficientReservedBalance,
		/// Partial intents are not supported at the moment.
		NotImplemented,
		/// Asset with specified id doesn't exists.
		AssetNotFound,
		/// DCA period is below minimum.
		InvalidDcaPeriod,
		/// DCA budget is less than a single trade amount.
		InvalidDcaBudget,
		/// DCA intent must not have a deadline.
		InvalidDcaDeadline,
	}

	#[pallet::storage]
	#[pallet::getter(fn get_intent)]
	pub type Intents<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, Intent>;

	#[pallet::storage]
	#[pallet::getter(fn intent_owner)]
	pub(super) type IntentOwner<T: Config> = StorageMap<_, Blake2_128Concat, IntentId, T::AccountId>;

	#[pallet::storage]
	/// Intent id sequencer
	#[pallet::getter(fn next_incremental_id)]
	pub(super) type NextIncrementalId<T: Config> = StorageValue<_, IncrementalIntentId, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Submit intent by user.
		///
		/// This extrinsics reserves fund for intents' execution.
		/// WARN: partial intents are not supported at the moment and its' creation is not allowed.
		///
		/// Parameters:
		///	- `intent`: intent's data
		///
		/// Emits:
		/// - `IntentSubmitted` when successful
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_intent())] //TODO: should probably include length of on_success/on_failure calls too
		pub fn submit_intent(origin: OriginFor<T>, intent: IntentInput) -> DispatchResult {
			let who = ensure_signed(origin)?;

			Self::add_intent(who, intent)?;
			Ok(())
		}

		/// Extrinsic unlocks reserved funds and removes intent.
		///
		/// Only intent's owner can cancel intent.
		///
		/// Parameters:
		/// - `id`: id of intent to be canceled.
		///
		/// Emits:
		/// - `IntentCanceled` when successful
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_intent())]
		pub fn remove_intent(origin: OriginFor<T>, id: IntentId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::cancel_intent(who, id)
		}

		/// Extrinsic removes expired intent, queue intent's on failure callback and unlocks funds.
		///
		/// Failure to queue callback for future execution doesn't fail clean up function.
		/// This is called automatically from OCW to remove expired intents but it can be called also
		/// called by any users.
		///
		/// Parameters:
		/// - `id`: id of intent to be cleaned up from storage.
		///
		/// Emits:
		/// - `FailedToQueueCallback` when callback's queuing fails
		/// - `IntentExpired` when successful
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::cleanup_intent())]
		pub fn cleanup_intent(origin: OriginFor<T>, id: IntentId) -> DispatchResultWithPostInfo {
			if ensure_none(origin.clone()).is_err() {
				ensure_signed(origin)?;
			}

			Intents::<T>::try_mutate_exists(id, |maybe_intent| {
				let intent = maybe_intent.as_mut().ok_or(Error::<T>::IntentNotFound)?;

				ensure!(
					intent.deadline.ok_or(Error::<T>::IntentActive)? <= T::TimestampProvider::now(),
					Error::<T>::IntentActive
				);

				IntentOwner::<T>::try_mutate_exists(id, |maybe_owner| -> Result<(), DispatchError> {
					let owner = maybe_owner.as_ref().ok_or(Error::<T>::IntentOwnerNotFound)?;

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
		//NOTE: this is tmp solution for testing.
		//TODO: create offchain bot that will do clean up instead of OCW.
		fn offchain_worker(_block_number: BlockNumberFor<T>) {
			let expired = Self::get_expired_intents();

			for (i, intent_id) in expired.iter().enumerate() {
				if i >= 10 {
					break;
				}

				let call = Call::cleanup_intent { id: *intent_id };
				let tx = T::create_bare(call.into());
				if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(tx) {
					debug_assert!(false, "laxy-executorn: failed to submit dispatch_top transaction");
					log::error!(target: OCW_LOG_TARGET, "{:?}: to submit cleanup_intent call, err: {:?}", LOG_PREFIX, e);
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
					TransactionSource::Local | TransactionSource::InBlock => { /* OCW or included in block are allowed */
					}
					_ => {
						return InvalidTransaction::Call.into();
					}
				};

				let Some(intent) = Intents::<T>::get(id) else {
					return InvalidTransaction::Call.into();
				};

				let Some(deadline) = intent.deadline else {
					return Err(TransactionValidityError::Invalid(InvalidTransaction::Call));
				};

				ensure!(deadline <= T::TimestampProvider::now(), InvalidTransaction::Call);

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
	/// Function unreserves funds and cancels intent.
	#[require_transactional]
	pub fn cancel_intent(who: T::AccountId, id: IntentId) -> DispatchResult {
		Intents::<T>::try_mutate_exists(id, |maybe_intent| {
			let intent = maybe_intent.as_ref().ok_or(Error::<T>::IntentNotFound)?;

			IntentOwner::<T>::try_mutate_exists(id, |maybe_owner| -> Result<(), DispatchError> {
				let owner = maybe_owner.clone().ok_or(Error::<T>::IntentOwnerNotFound)?;

				ensure!(owner == who, Error::<T>::InvalidOwner);

				let unlock_amount = match intent.data {
					IntentData::Swap(_) => intent.data.amount_in(),
					IntentData::Dca(ref dca) => dca.remaining_budget,
				};
				Self::unlock_funds(&who, intent.data.asset_in(), unlock_amount)?;

				Self::deposit_event(Event::<T>::IntentCanceled { id });

				*maybe_owner = None;
				Ok(())
			})?;

			*maybe_intent = None;
			Ok(())
		})
	}

	/// Function validates and reserves funds for intent's execution and adds intent to storage
	/// WARN: partial intents are not supported at the moment, look at `submit_intent()`
	#[require_transactional]
	pub fn add_intent(owner: T::AccountId, input: IntentInput) -> Result<IntentId, DispatchError> {
		let now = T::TimestampProvider::now();
		if let Some(deadline) = input.deadline {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: add_intent(), deadline: {:?}, now: {:?}, max_deadline: {:?}",
				LOG_PREFIX, deadline, now, now.saturating_add(T::MaxAllowedIntentDuration::get()));

			ensure!(deadline > now, Error::<T>::InvalidDeadline);
			ensure!(
				deadline < (now.saturating_add(T::MaxAllowedIntentDuration::get())),
				Error::<T>::InvalidDeadline
			);
		}

		let ed_in = T::RegistryHandler::existential_deposit(input.data.asset_in()).ok_or(Error::<T>::AssetNotFound)?;
		let ed_out =
			T::RegistryHandler::existential_deposit(input.data.asset_out()).ok_or(Error::<T>::AssetNotFound)?;

		let intent_data = match input.data {
			IntentDataInput::Swap(ref data) => {
				log::debug!(target: OCW_LOG_TARGET, "{:?}: add_intent(), asset_in: {:?}, ed_in: {:?}, amount_in: {:?}, aseet_out: {:?}, ed_out: {:?}, amount_out: {:?}",
					LOG_PREFIX, data.asset_in, ed_in, data.amount_in, data.asset_out, ed_out, data.amount_out);

				ensure!(data.amount_in >= ed_in, Error::<T>::InvalidIntent);
				ensure!(data.amount_out >= ed_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_in != data.asset_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);

				T::Currency::reserve_named(&NAMED_RESERVE_ID, data.asset_in, &owner, data.amount_in)?;

				IntentData::Swap(data.clone())
			}
			IntentDataInput::Dca(ref data) => {
				// DCA intents must not have a deadline
				ensure!(input.deadline.is_none(), Error::<T>::InvalidDcaDeadline);

				ensure!(data.period >= T::MinDcaPeriod::get(), Error::<T>::InvalidDcaPeriod);
				ensure!(data.amount_in >= ed_in, Error::<T>::InvalidIntent);
				ensure!(data.amount_out >= ed_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_in != data.asset_out, Error::<T>::InvalidIntent);
				ensure!(data.asset_out != T::HubAssetId::get(), Error::<T>::InvalidIntent);
				let reserve_amount = match data.budget {
					Some(budget) => {
						ensure!(budget >= data.amount_in, Error::<T>::InvalidDcaBudget);
						budget
					}
					None => data.amount_in.saturating_mul(2), // rolling: 2x buffer
				};

				T::Currency::reserve_named(&NAMED_RESERVE_ID, data.asset_in, &owner, reserve_amount)?;

				let current_block: u32 = T::BlockNumberProvider::current_block_number()
					.try_into()
					.unwrap_or(u32::MAX);

				IntentData::Dca(data.clone().into_data(reserve_amount, current_block))
			}
		};

		let intent = Intent {
			data: intent_data,
			deadline: input.deadline,
			on_resolved: input.on_resolved,
		};

		let id = Self::generate_new_intent_id(now);
		Intents::<T>::insert(id, &intent);
		IntentOwner::<T>::insert(id, &owner);
		Self::deposit_event(Event::IntentSubmitted { id, owner, intent });

		Ok(id)
	}

	/// Function returns expired intents
	pub fn get_expired_intents() -> Vec<IntentId> {
		let mut intents: Vec<(IntentId, Intent)> = Intents::<T>::iter().collect();
		intents.sort_by_key(|(_, intent)| intent.deadline);

		let now = T::TimestampProvider::now();
		intents.retain(|(_, intent)| intent.deadline.unwrap_or(Moment::MAX) <= now);

		intents.iter().map(|x| x.0).collect::<Vec<IntentId>>()
	}

	/// Function returns valid intents.
	///
	/// DCA intents are included only when their period has elapsed, budget is sufficient,
	/// and oracle price indicates the trade is feasible (pre-filter).
	/// They are transformed into `IntentData::Swap` with the hard limit as `amount_out`,
	/// so the solver treats them as regular one-shot swaps.
	/// The oracle effective limit is used only as a pre-filter gate — if the oracle-derived
	/// minimum exceeds what the solver could reasonably fill, the intent is skipped for this block.
	pub fn get_valid_intents() -> Vec<(IntentId, Intent)> {
		let current_block: u32 = T::BlockNumberProvider::current_block_number()
			.try_into()
			.unwrap_or(u32::MAX);

		let mut intents: Vec<(IntentId, Intent)> = Intents::<T>::iter()
			.filter_map(|(id, intent)| {
				match &intent.data {
					IntentData::Swap(_) => Some((id, intent)),
					IntentData::Dca(dca) => {
						// Period eligibility
						if current_block < dca.last_execution_block.saturating_add(dca.period) {
							return None;
						}
						// Budget sufficient for a trade
						if dca.remaining_budget < dca.amount_in {
							return None;
						}
						// Oracle pre-filter: skip if oracle indicates the trade is unlikely
						// to satisfy the user's slippage tolerance at current prices.
						// This prevents the solver from wasting time on intents that would
						// fail due to market conditions.
						if let Some(oracle_min) = Self::compute_dca_oracle_limit(dca) {
							if oracle_min > 0 && dca.amount_out > oracle_min {
								// Hard limit exceeds what oracle says market can provide
								// with the user's slippage tolerance — skip this block
								return None;
							}
						}
						// Transform to Swap with hard limit for solver
						let swap = dca.to_swap_data();
						let transformed = Intent {
							data: IntentData::Swap(swap),
							deadline: intent.deadline,
							on_resolved: intent.on_resolved.clone(),
						};
						Some((id, transformed))
					}
				}
			})
			.collect();
		intents.sort_by_key(|(id, _)| Reverse(*id));
		intents
	}

	/// Function validates if intent was resolved correctly
	pub fn validate_resolve(intent: &Intent, resolve: &IntentData) -> Result<(), DispatchError> {
		if let Some(deadline) = intent.deadline {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_resolve(), deadline: {:?}, now: {:?}", 
					LOG_PREFIX, deadline, T::TimestampProvider::now());

			ensure!(deadline > T::TimestampProvider::now(), Error::<T>::IntentExpired);
		}

		log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_resolve(), orig_asset_in: {:?}, resolve_asset_in: {:?}", 
					LOG_PREFIX, intent.data.asset_in(), resolve.asset_in());
		ensure!(
			intent.data.asset_in() == resolve.asset_in(),
			Error::<T>::ResolveMismatch
		);

		log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_resolve(), orig_asset_out: {:?}, resolve_asset_out: {:?}", 
					LOG_PREFIX, intent.data.asset_out(), resolve.asset_out());
		ensure!(
			intent.data.asset_out() == resolve.asset_out(),
			Error::<T>::ResolveMismatch
		);

		match intent.data {
			IntentData::Swap(_) => {
				Self::validate_swap_intent_resolve(intent, resolve)?;
			}
			IntentData::Dca(ref dca) => {
				Self::validate_dca_intent_resolve(dca, resolve)?;
			}
		}

		Ok(())
	}

	fn validate_swap_intent_resolve(intent: &Intent, resolve: &IntentData) -> Result<(), DispatchError> {
		let IntentData::Swap(ref swap) = intent.data else {
			return Err(Error::<T>::ResolveMismatch.into());
		};
		let IntentData::Swap(ref resolve_swap) = resolve else {
			return Err(Error::<T>::ResolveMismatch.into());
		};

		log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_swap_intent_resolve(), partial: {:?}, resolve.partial: {:?}", 
			LOG_PREFIX, swap.partial, resolve_swap.partial);

		ensure!(swap.partial == resolve_swap.partial, Error::<T>::ResolveMismatch);

		if swap.partial {
			if resolve_swap.amount_in == swap.amount_in {
				log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_swap_intent_resolve(), partial intent resolved fully, amount_in: {:?}, amount_out: {:?}, resolved.amount_out: {:?}", 
					LOG_PREFIX, swap.amount_in, swap.amount_out, resolve_swap.amount_out);

				ensure!(resolve_swap.amount_out >= swap.amount_out, Error::<T>::LimitViolation);

				return Ok(());
			}

			let limit = intent.data.pro_rata(resolve).ok_or(Error::<T>::ArithmeticOverflow)?;

			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_swap_intent_resolve(), partial intent resolved partially, amount_in: {:?}, resolve.amount_in: {:?}, limit: {:?}, resolve.amount_out: {:?}", 
				LOG_PREFIX, swap.amount_in, resolve_swap.amount_in, limit, resolve_swap.amount_out);

			ensure!(resolve_swap.amount_in < swap.amount_in, Error::<T>::LimitViolation);
			ensure!(resolve_swap.amount_out >= limit, Error::<T>::LimitViolation);
		} else {
			log::debug!(target: OCW_LOG_TARGET, "{:?}: validate_swap_intent_resolve(), ExactIn resolved, amount_in: {:?}, resolve.amount_in: {:?}, amount_out: {:?}, resolve.amount_out: {:?}", 
				LOG_PREFIX, swap.amount_in, resolve_swap.amount_in, swap.amount_out, resolve_swap.amount_out);

			ensure!(resolve_swap.amount_in == swap.amount_in, Error::<T>::LimitViolation);
			ensure!(resolve_swap.amount_out >= swap.amount_out, Error::<T>::LimitViolation);
		}

		Ok(())
	}

	/// Function resolves intent
	pub fn intent_resolved(who: &T::AccountId, resolve: &ResolvedIntent) -> DispatchResult {
		let ResolvedIntent { id, data: resolve } = resolve;
		Intents::<T>::try_mutate_exists(id, |maybe_intent| {
			let intent = maybe_intent.as_mut().ok_or(Error::<T>::IntentNotFound)?;
			let owner = Self::intent_owner(id).ok_or(Error::<T>::IntentOwnerNotFound)?;

			ensure!(owner == *who, Error::<T>::InvalidOwner);

			Self::validate_resolve(intent, resolve)?;

			let (fully_resolved, is_dca) = match intent.data {
				IntentData::Swap(ref mut s) => {
					let IntentData::Swap(ref r) = resolve else {
						return Err(Error::<T>::ResolveMismatch.into());
					};
					(Self::resolve_swap_intent(s, r)?, false)
				}
				IntentData::Dca(ref mut dca) => (Self::resolve_dca_intent(&owner, dca)?, true),
			};

			if fully_resolved {
				// Unreserve remaining funds
				let unreserve_amount = match intent.data {
					IntentData::Swap(_) => intent.data.amount_in(),
					IntentData::Dca(ref dca) => dca.remaining_budget,
				};
				if !unreserve_amount.is_zero() {
					Self::unlock_funds(&owner, intent.data.asset_in(), unreserve_amount)?;
				}

				//NOTE: it's ok to `take`, intent will be removed from storage.
				if let Some(cb) = intent.on_resolved.take() {
					if let Err(e) = T::LazyExecutorHandler::queue(Source::ICE(*id), who.clone(), cb) {
						Self::deposit_event(Event::FailedToQueueCallback { id: *id, error: e });
					};
				}

				*maybe_intent = None;
				IntentOwner::<T>::remove(id);

				if is_dca {
					Self::deposit_event(Event::DcaCompleted { id: *id });
				} else {
					Self::deposit_event(Event::IntentResolved {
						id: *id,
						amount_in: resolve.amount_in(),
						amount_out: resolve.amount_out(),
					});
				}
				return Ok(());
			}

			// Not fully resolved
			match intent.data {
				IntentData::Swap(_) => {
					ensure!(intent.data.is_partial(), Error::<T>::LimitViolation);
					Self::deposit_event(Event::IntentResovedPartially {
						id: *id,
						amount_in: resolve.amount_in(),
						amount_out: resolve.amount_out(),
					});
				}
				IntentData::Dca(ref dca) => {
					Self::deposit_event(Event::DcaTradeExecuted {
						id: *id,
						amount_in: resolve.amount_in(),
						amount_out: resolve.amount_out(),
						remaining_budget: dca.remaining_budget,
					});
				}
			}

			Ok(())
		})
	}

	// Function updates intent's `SwapData` and returns `true` if intent was fully resolved.
	fn resolve_swap_intent(intent: &mut SwapData, resolve: &SwapData) -> Result<bool, DispatchError> {
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

	/// Resolves a DCA intent after a single trade execution.
	/// Returns `true` if the DCA is complete (budget exhausted).
	fn resolve_dca_intent(owner: &T::AccountId, dca: &mut DcaData) -> Result<bool, DispatchError> {
		let current_block: u32 = T::BlockNumberProvider::current_block_number()
			.try_into()
			.unwrap_or(u32::MAX);

		// Deduct per-trade amount from remaining budget
		dca.remaining_budget = dca
			.remaining_budget
			.checked_sub(dca.amount_in)
			.ok_or(Error::<T>::ArithmeticOverflow)?;

		// Update last execution block
		dca.last_execution_block = current_block;

		// Rolling DCA: try to re-reserve one unit from free balance
		if dca.budget.is_none() {
			if T::Currency::reserve_named(&NAMED_RESERVE_ID, dca.asset_in, owner, dca.amount_in).is_ok() {
				dca.remaining_budget = dca.remaining_budget.saturating_add(dca.amount_in);
			}
			// If reserve fails, DCA may complete on next check
		}

		// DCA complete if insufficient budget for another trade
		Ok(dca.remaining_budget < dca.amount_in)
	}

	/// Validates a DCA intent's resolution against hard limits.
	/// Dynamic slippage is enforced in `get_valid_intents()` as a pre-filter, not here.
	fn validate_dca_intent_resolve(dca: &DcaData, resolve: &IntentData) -> Result<(), DispatchError> {
		// Resolve must spend exactly per-trade amount
		ensure!(resolve.amount_in() == dca.amount_in, Error::<T>::LimitViolation);
		// Hard limit check (always enforced regardless of oracle)
		ensure!(resolve.amount_out() >= dca.amount_out, Error::<T>::LimitViolation);
		Ok(())
	}

	/// Computes surplus for score matching.
	/// For swap intents: delegates to `IntentData::surplus()`.
	/// For DCA intents: uses the hard limit (`amount_out`) — same value used in
	/// `get_valid_intents()` transform, ensuring OCW and on-chain produce identical scores.
	pub fn compute_surplus(intent: &Intent, resolve: &IntentData) -> Option<Balance> {
		intent.data.surplus(resolve)
	}

	/// Returns the effective minimum output for a DCA trade:
	/// the tighter (higher) of the oracle-based limit and the user's hard limit.
	pub fn compute_dca_effective_limit(dca: &DcaData) -> Balance {
		match Self::compute_dca_oracle_limit(dca) {
			Some(oracle_min) => cmp::max(oracle_min, dca.amount_out),
			None => dca.amount_out, // oracle unavailable → hard limit only
		}
	}

	/// Computes the oracle-based minimum output for a DCA trade.
	/// Returns None if oracle data is unavailable.
	fn compute_dca_oracle_limit(dca: &DcaData) -> Option<Balance> {
		let route = sp_std::vec![OracleTrade {
			pool: PoolType::Omnipool,
			asset_in: dca.asset_in,
			asset_out: dca.asset_out,
		}];
		let oracle_price = T::OraclePriceProvider::price(&route, OraclePeriod::Short)?;
		// Oracle price for route A→B returns n/d representing asset_in per asset_out
		// (i.e., how much A costs per unit of B).
		// For sell: estimated_out = amount_in / price = amount_in * d / n
		let estimated_out =
			FixedU128::checked_from_rational(oracle_price.d, oracle_price.n)?.checked_mul_int(dca.amount_in)?;
		if estimated_out == 0 {
			return None;
		}
		let slippage_amount = dca.slippage.mul_floor(estimated_out);
		estimated_out.checked_sub(slippage_amount)
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
		// it is not an issue, we create new intent id together with created at timestamp, so it is not possible to create two intents with the same id
		let incremental_id = NextIncrementalId::<T>::mutate(|id| -> IncrementalIntentId {
			let current_id = *id;
			(*id, _) = id.overflowing_add(1);
			current_id
		});
		(deadline as u128) << 64 | incremental_id as u128
	}
}
