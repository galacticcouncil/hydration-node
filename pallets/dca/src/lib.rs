// This file is part of pallet-dca.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

//! # DCA pallet
//!
//! ## Overview
//!
//! A dollar-cost averaging pallet that enables users to perform repeating orders.
//!
//! When an order is submitted, it will reserve the total amount (budget) specified by the user, as a named reserve.
//!
//! The DCA plan is executed as long as there is balance in the budget.
//!
//! If a trade fails then the oder is suspended and has to be resumed or terminated by the user.
//!
//! Orders are executed on block initialize and they are sorted based on randomness derived from relay chain block number.
//! Therefore they cannot be front-ran in the block they are executed.
//!

use codec::MaxEncodedLen;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, Len},
	transactional,
	weights::WeightToFee as FrameSupportWeight,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor, Origin};
use orml_traits::arithmetic::CheckedAdd;
use orml_traits::MultiCurrency;
use orml_traits::NamedMultiReservableCurrency;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use scale_info::TypeInfo;
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating},
	ArithmeticError, BoundedVec, DispatchError, FixedPointNumber, FixedU128, Permill,
};
use sp_std::{
	cmp::{max, min},
	vec,
};
#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::*;

type BlockNumberFor<T> = <T as frame_system::Config>::BlockNumber;

type NamedReserveIdentifier = [u8; 8];

pub const NAMED_RESERVE_ID: NamedReserveIdentifier = *b"dcaorder";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use hydra_dx_math::ema::EmaPrice;
	use orml_traits::NamedMultiReservableCurrency;
	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T>
	where
		<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<
			<T as frame_system::Config>::AccountId,
		>>::CurrencyId: From<<T as pallet::Config>::Asset>,
		<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<
			<T as frame_system::Config>::AccountId,
		>>::Balance: From<u128>,
	{
		fn on_initialize(current_blocknumber: T::BlockNumber) -> Weight {
			{
				let mut weight: u64 = Self::get_on_initialize_weight();

				let mut random_generator = T::RandomnessProvider::generator();

				let maybe_schedules: Option<BoundedVec<ScheduleId, T::MaxSchedulePerBlock>> =
					ScheduleIdsPerBlock::<T>::get(current_blocknumber);

				if let Some(mut schedules) = maybe_schedules {
					schedules.sort_by_key(|_| random_generator.gen::<u32>());
					for schedule_id in schedules {
						Self::execute_schedule(current_blocknumber, &mut weight, schedule_id);
					}
				}

				Weight::from_ref_time(weight)
			}
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_relaychain_info::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		/// Identifier for the class of asset.
		type Asset: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		///For named-reserving user's assets
		type Currency: NamedMultiReservableCurrency<Self::AccountId, ReserveIdentifier = NamedReserveIdentifier>;

		///Price provider to get the price between two assets
		type PriceProvider: PriceProvider<Self::Asset, Price = EmaPrice>;

		///AMMTrader for trade execution
		type AMMTrader: AMMTrader<Self::Origin, Self::Asset, Balance>;

		///Randomness provider to be used to sort the DCA schedules when they are executed in a block
		type RandomnessProvider: RandomnessProvider;

		///The number of max schedules to be executed per block
		#[pallet::constant]
		type MaxSchedulePerBlock: Get<u32>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::Asset>;

		///Storage bond in native currency
		#[pallet::constant]
		type StorageBondInNativeCurrency: Get<Balance>;

		///The fee receiver for transaction fees
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;

		///Slippage limit percentage to be used for calculating min and max limits for trades
		#[pallet::constant]
		type SlippageLimitPercentage: Get<Permill>;

		/// Convert a weight value into a deductible fee
		type WeightToFee: WeightToFee<Balance = Balance>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///The DCA is scheduled
		Scheduled { id: ScheduleId, who: T::AccountId },
		///The DCA is planned for blocknumber
		ExecutionPlanned {
			id: ScheduleId,
			who: T::AccountId,
			block: BlockNumberFor<T>,
		},
		///The DCA is paused from execution
		Paused { id: ScheduleId, who: T::AccountId },
		///The DCA is resumed to be executed
		Resumed { id: ScheduleId, who: T::AccountId },
		///The DCA is terminated and completely removed from the chain
		Terminated { id: ScheduleId, who: T::AccountId },
		///The DCA is suspended because it is paused by user or the DCA execution failed
		Suspended { id: ScheduleId, who: T::AccountId },
		///The DCA is completed and completely removed from the chain
		Completed { id: ScheduleId, who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {
		///Schedule not exist
		ScheduleNotExist,
		///The user has not enough balance for the reserving the total amount to spend
		InsufficientBalanceForTotalAmount,
		///Trade amount is less than fee
		TradeAmountIsLessThanFee,
		///Forbidden as the user is not the owner of the schedule
		Forbidden,
		///The next execution block number is not in the future
		BlockNumberIsNotInFuture,
		///There is not planned execution on the given block
		NoPlannedExecutionFoundOnBlock,
		///Schedule execution is not planned on block
		ScheduleMustBeSuspended,
		///Error occurred when calculating price
		CalculatingPriceError,
		///Invalid storage state: No schedule ids planned in block
		NoScheduleIdsPlannedInBlock,
		///The total amount to be reserved should be larger than storage bond
		TotalAmountShouldBeLargerThanStorageBond,
		///Error that should not really happen only in case of invalid state of the schedule storage entries
		InvalidState,
	}

	/// Id sequencer for schedules
	#[pallet::storage]
	#[pallet::getter(fn next_schedule_id)]
	pub type ScheduleIdSequencer<T: Config> = StorageValue<_, ScheduleId, ValueQuery>;

	/// Storing schedule details
	#[pallet::storage]
	#[pallet::getter(fn schedules)]
	pub type Schedules<T: Config> =
		StorageMap<_, Blake2_128Concat, ScheduleId, Schedule<T::AccountId, T::Asset, BlockNumberFor<T>>, OptionQuery>;

	/// Storing schedule ownership
	#[pallet::storage]
	#[pallet::getter(fn owner_of)]
	pub type ScheduleOwnership<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Twox64Concat, ScheduleId, (), OptionQuery>;

	/// Storing suspended schedules
	#[pallet::storage]
	#[pallet::getter(fn suspended)]
	pub type Suspended<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, (), OptionQuery>;

	/// Keep tracking the remaining recurrences for DCA schedules
	#[pallet::storage]
	#[pallet::getter(fn remaining_recurrences)]
	pub type RemainingRecurrences<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u32, OptionQuery>;

	/// Keep tracking the remaining amounts to spend for DCA schedules
	#[pallet::storage]
	#[pallet::getter(fn remaining_amounts)]
	pub type RemainingAmounts<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, Balance, OptionQuery>;

	/// Keep tracking of the schedule ids to be executed in the block
	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<
			<T as frame_system::Config>::AccountId,
		>>::CurrencyId: From<<T as pallet::Config>::Asset>,
		<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<
			<T as frame_system::Config>::AccountId,
		>>::Balance: From<u128>,
	{
		/// Creates a new DCA schedule and plans the execution in the specified start execution block.
		/// If start execution block number is not specified, then the schedule is planned in the consequent block.
		///
		/// The order will be executed within the configured AMM trade pool
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule`: schedule details
		/// - `start_execution_block`: start execution block for the schedule
		///
		/// Emits `Scheduled` and `ExecutionPlanned` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::schedule())]
		#[transactional]
		pub fn schedule(
			origin: OriginFor<T>,
			schedule: Schedule<T::AccountId, T::Asset, BlockNumberFor<T>>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_next_blocknumber_is_bigger_than_current_block(start_execution_block)?;
			Self::ensure_that_total_amount_is_bigger_than_storage_bond(&schedule)?;
			Self::ensure_that_sell_amount_is_bigger_than_transaction_fee(&schedule)?;

			let next_schedule_id = Self::get_next_schedule_id()?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			ScheduleOwnership::<T>::insert(who.clone(),next_schedule_id,());
			RemainingAmounts::<T>::insert(next_schedule_id,schedule.total_amount);

			Self::reserve_named_reserve(&schedule, &who)?;

			let blocknumber_for_first_schedule_execution =
				start_execution_block.unwrap_or_else(|| Self::get_next_block_number());
			Self::plan_schedule_for_block(blocknumber_for_first_schedule_execution, next_schedule_id)?;

			Self::deposit_event(Event::Scheduled {
				id: next_schedule_id,
				who: who.clone(),
			});
			Self::deposit_event(Event::ExecutionPlanned {
				id: next_schedule_id,
				who,
				block: blocknumber_for_first_schedule_execution,
			});

			Ok(())
		}

		/// Pause the DCA schedule planned in the given block number
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule_id`: schedule id
		/// - `next_execution_block`: block number where the DCA is planned to be executed
		///
		/// Emits `Paused` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::pause())]
		#[transactional]
		pub fn pause(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_origin_is_schedule_owner(schedule_id, &who)?;

			Self::remove_schedule_id_from_next_execution_block(schedule_id, next_execution_block)?;
			Suspended::<T>::insert(schedule_id, ());

			Self::deposit_event(Event::Paused { id: schedule_id, who });

			Ok(())
		}

		/// Resume the suspended DCA schedule for the specified next execution block number
		/// If next execution block number is not specified, then the schedule is planned in the consequent block
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule_id`: schedule id
		/// - `next_execution_block`: block number to plan the next execution of the schedule.
		///
		/// Emits `Resumed`and `ExecutionPlanned` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::resume())]
		#[transactional]
		pub fn resume(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_origin_is_schedule_owner(schedule_id, &who)?;
			Self::ensure_that_next_blocknumber_is_bigger_than_current_block(next_execution_block)?;
			Self::ensure_that_schedule_is_suspended(schedule_id)?;

			let next_execution_block = next_execution_block.unwrap_or_else(|| Self::get_next_block_number());
			Self::plan_schedule_for_block(next_execution_block, schedule_id)?;

			Suspended::<T>::remove(schedule_id);

			Self::deposit_event(Event::Resumed {
				id: schedule_id,
				who: who.clone(),
			});
			Self::deposit_event(Event::ExecutionPlanned {
				id: schedule_id,
				who,
				block: next_execution_block,
			});

			Ok(())
		}

		/// Terminate a DCA schedule and remove it completely from the chain.
		/// The next execution block number should be specified in case of active schedule.
		/// To terminate a suspended schedule, the next execution block number should not be specified.
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule_id`: schedule id
		/// - `next_execution_block`: block number where the schedule is planned. None in case of suspended schedule
		///
		/// Emits `Terminated` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::terminate())]
		#[transactional]
		pub fn terminate(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_schedule_exists(&schedule_id)?;
			Self::ensure_that_origin_is_schedule_owner(schedule_id, &who)?;

			Self::unreserve_all_named_reserved_sold_currency(schedule_id, &who)?;

			Self::remove_planning_or_suspension(schedule_id, next_execution_block)?;
			Self::remove_schedule_from_storages(&who, schedule_id);

			Self::deposit_event(Event::Terminated {
				id: schedule_id,
				who,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T>
where
	<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId:
		From<<T as pallet::Config>::Asset>,

	<<T as pallet::Config>::Currency as orml_traits::MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance:
		From<u128>,
{
	fn ensure_that_next_blocknumber_is_bigger_than_current_block(
		next_execution_block: Option<T::BlockNumber>,
	) -> DispatchResult {
		if let Some(next_exection_block) = next_execution_block {
			let current_block_number = frame_system::Pallet::<T>::current_block_number();
			ensure!(
				next_exection_block > current_block_number,
				Error::<T>::BlockNumberIsNotInFuture
			);
		};

		Ok(())
	}

	fn ensure_that_total_amount_is_bigger_than_storage_bond(
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		let min_total_amount = if Self::get_sold_currency(&schedule.order) == T::NativeAssetId::get() {
			T::StorageBondInNativeCurrency::get()
		} else {
			Self::get_storage_bond_in_sold_currency(&schedule.order)?
		};

		ensure!(
			schedule.total_amount > min_total_amount,
			Error::<T>::TotalAmountShouldBeLargerThanStorageBond
		);

		Ok(())
	}

	fn ensure_that_schedule_is_suspended(schedule_id: ScheduleId) -> DispatchResult {
		ensure!(
			Suspended::<T>::contains_key(&schedule_id),
			Error::<T>::ScheduleMustBeSuspended
		);

		Ok(())
	}

	fn ensure_that_schedule_exists(schedule_id: &ScheduleId) -> DispatchResult {
		ensure!(Schedules::<T>::contains_key(schedule_id), Error::<T>::ScheduleNotExist);

		Ok(())
	}

	fn ensure_that_origin_is_schedule_owner(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
		ensure!(*who == schedule.owner, Error::<T>::Forbidden);

		Ok(())
	}

	fn ensure_that_sell_amount_is_bigger_than_transaction_fee(
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		match schedule.order {
			Order::Sell {
				asset_in, amount_in, ..
			} => {
				let transaction_fee = Self::get_transaction_fee(asset_in)?;
				ensure!(amount_in > transaction_fee, Error::<T>::TradeAmountIsLessThanFee);
			}
			Order::Buy { .. } => {
				//For buy we don't check as the calculated amount in will always include the fee
			}
		}

		Ok(())
	}

	pub fn execute_schedule(current_blocknumber: T::BlockNumber, weight: &mut u64, schedule_id: ScheduleId) {
		*weight += Self::get_execute_schedule_weight();

		let schedule = exec_or_return_if_none!(Schedules::<T>::get(schedule_id));
		let origin: OriginFor<T> = Origin::<T>::Signed(schedule.owner.clone()).into();

		let sold_currency = Self::get_sold_currency(&schedule.order);
		let amount_to_unreserve = exec_or_return_if_err!(Self::amount_to_unreserve(&schedule.order));

		let remaining_amount_to_use = exec_or_return_if_none!(RemainingAmounts::<T>::get(schedule_id));

		T::Currency::unreserve_named(
			&NAMED_RESERVE_ID,
			sold_currency.into(),
			&schedule.owner,
			amount_to_unreserve.into(),
		);

		exec_or_return_if_err!(Self::decrease_remaining_amount(schedule_id, amount_to_unreserve));

		if remaining_amount_to_use < amount_to_unreserve {
			Self::complete_dca(&schedule.owner, schedule_id);
			return;
		}

		exec_or_return_if_err!(Self::take_transaction_fee_from_user(&schedule.owner, &schedule.order));
		let trade_result = Self::execute_trade(origin, &schedule.order);

		match trade_result {
			Ok(_) => {
				let blocknumber_for_schedule =
					exec_or_return_if_none!(current_blocknumber.checked_add(&schedule.period));

				exec_or_return_if_err!(Self::plan_schedule_for_block(blocknumber_for_schedule, schedule_id));
			}
			_ => {
				exec_or_return_if_err!(Self::suspend_schedule(&schedule.owner, schedule_id));
			}
		}
	}

	fn amount_to_unreserve(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		match order {
			Order::Sell { amount_in, .. } => Ok(*amount_in),
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_limit,
				..
			} => {
				let amount_to_sell_for_buy =
					Self::calculate_sell_amount_for_buy(asset_in, asset_out, amount_out, max_limit)?;
				Ok(amount_to_sell_for_buy)
			}
		}
	}

	fn decrease_remaining_amount(schedule_id: ScheduleId, amount_to_unreserve: Balance) -> DispatchResult {
		RemainingAmounts::<T>::try_mutate_exists(schedule_id, |maybe_remaining_amount| -> DispatchResult {
			let remaining_amount = maybe_remaining_amount.as_mut().ok_or(Error::<T>::ScheduleNotExist)?;

			if amount_to_unreserve > *remaining_amount {
				*maybe_remaining_amount = None;
				return Ok(());
			}

			let new_amount = remaining_amount
				.checked_sub(amount_to_unreserve)
				.ok_or(ArithmeticError::Underflow)?;

			*remaining_amount = new_amount;

			Ok(())
		})?;

		Ok(())
	}

	fn calculate_sell_amount_for_buy(
		asset_in: &<T as Config>::Asset,
		asset_out: &<T as Config>::Asset,
		amount_out: &Balance,
		max_limit: &Balance,
	) -> Result<u128, DispatchError> {
		let max_limit_from_oracle_price = Self::get_max_limit_with_slippage(asset_in, asset_out, amount_out)?;
		let max_limit = max(max_limit, &max_limit_from_oracle_price);

		let fee_amount_in_sold_asset = Self::get_transaction_fee(*asset_in)?;
		let amount_to_sell_plus_fee = max_limit
			.checked_add(&fee_amount_in_sold_asset)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(amount_to_sell_plus_fee)
	}

	fn get_storage_bond_in_sold_currency(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		let sold_currency = Self::get_sold_currency(order);
		let storage_bond_in_native_currency = T::StorageBondInNativeCurrency::get();

		let storage_bond_in_user_currency =
			Self::convert_to_currency_if_asset_is_not_native(sold_currency, storage_bond_in_native_currency)?;

		Ok(storage_bond_in_user_currency)
	}

	fn get_on_initialize_weight() -> u64 {
		crate::weights::HydraWeight::<T>::on_initialize().ref_time()
	}

	fn get_execute_schedule_weight() -> u64 {
		crate::weights::HydraWeight::<T>::execute_schedule().ref_time()
	}

	fn take_transaction_fee_from_user(owner: &T::AccountId, order: &Order<<T as Config>::Asset>) -> DispatchResult {
		let fee_currency = Self::get_sold_currency(order);

		let fee_amount_in_sold_asset = Self::get_transaction_fee(fee_currency)?;

		T::Currency::transfer(
			fee_currency.into(),
			owner,
			&T::FeeReceiver::get(),
			fee_amount_in_sold_asset.into(),
		)?;

		Ok(())
	}

	fn reserve_named_reserve(
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		who: &T::AccountId,
	) -> DispatchResult {
		let currency_for_reserve = match schedule.order {
			Order::Buy { asset_in, .. } => asset_in,
			Order::Sell { asset_in, .. } => asset_in,
		};

		T::Currency::reserve_named(
			&NAMED_RESERVE_ID,
			currency_for_reserve.into(),
			who,
			schedule.total_amount.into(),
		)?;

		Ok(())
	}

	fn unreserve_all_named_reserved_sold_currency(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
		let sold_currency = Self::get_sold_currency(&schedule.order);
		T::Currency::unreserve_all_named(&NAMED_RESERVE_ID, sold_currency.into(), who);

		Ok(())
	}

	fn get_sold_currency(order: &Order<T::Asset>) -> <T as Config>::Asset {
		let sold_currency = match order {
			Order::Sell { asset_in, .. } => asset_in,
			Order::Buy { asset_in, .. } => asset_in,
		};
		*sold_currency
	}

	fn weight_to_fee(weight: Weight) -> Balance {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight: Weight = weight.min(T::BlockWeights::get().max_block);
		<T as pallet::Config>::WeightToFee::weight_to_fee(&capped_weight)
	}

	fn add_schedule_id_to_existing_ids_per_block(
		next_schedule_id: ScheduleId,
		blocknumber_for_schedule: <T as frame_system::Config>::BlockNumber,
	) -> DispatchResult {
		let schedule_ids =
			ScheduleIdsPerBlock::<T>::get(blocknumber_for_schedule).ok_or(Error::<T>::NoScheduleIdsPlannedInBlock)?;
		if schedule_ids.len() == T::MaxSchedulePerBlock::get() as usize {
			let mut consequent_block = blocknumber_for_schedule;
			consequent_block.saturating_inc();
			Self::plan_schedule_for_block(consequent_block, next_schedule_id)?;
			return Ok(());
		} else {
			Self::add_schedule_id_to_block(next_schedule_id, blocknumber_for_schedule)?;
		}

		Ok(())
	}

	fn add_schedule_id_to_block(
		next_schedule_id: ScheduleId,
		blocknumber_for_schedule: T::BlockNumber,
	) -> DispatchResult {
		ScheduleIdsPerBlock::<T>::try_mutate_exists(blocknumber_for_schedule, |schedule_ids| -> DispatchResult {
			let schedule_ids = schedule_ids.as_mut().ok_or(Error::<T>::NoScheduleIdsPlannedInBlock)?;

			schedule_ids
				.try_push(next_schedule_id)
				.map_err(|_| Error::<T>::InvalidState)?;
			Ok(())
		})?;

		Ok(())
	}

	fn plan_schedule_for_block(b: T::BlockNumber, schedule_id: ScheduleId) -> DispatchResult {
		if !ScheduleIdsPerBlock::<T>::contains_key(b) {
			let vec_with_first_schedule_id = Self::create_bounded_vec(schedule_id)?;
			ScheduleIdsPerBlock::<T>::insert(b, vec_with_first_schedule_id);
		} else {
			Self::add_schedule_id_to_existing_ids_per_block(schedule_id, b)?;
		}
		Ok(())
	}

	fn get_next_schedule_id() -> Result<ScheduleId, ArithmeticError> {
		ScheduleIdSequencer::<T>::try_mutate(|current_id| {
			*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			Ok(*current_id)
		})
	}

	fn get_next_block_number() -> BlockNumberFor<T> {
		let mut current_block_number = frame_system::Pallet::<T>::current_block_number();
		current_block_number.saturating_inc();

		current_block_number
	}

	fn create_bounded_vec(
		next_schedule_id: ScheduleId,
	) -> Result<BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, DispatchError> {
		let schedule_id = vec![next_schedule_id];
		let bounded_vec: BoundedVec<ScheduleId, T::MaxSchedulePerBlock> =
			schedule_id.try_into().map_err(|_| Error::<T>::InvalidState)?;
		Ok(bounded_vec)
	}

	fn execute_trade(origin: T::Origin, order: &Order<<T as Config>::Asset>) -> DispatchResult {
		match order {
			Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_limit,
				route: _,
			} => {
				let min_limit_with_slippage = Self::get_min_limit_with_slippage(asset_in, asset_out, amount_in)?;

				let transaction_fee = Self::get_transaction_fee(*asset_in)?;

				let amount_to_sell = amount_in
					.checked_sub(transaction_fee)
					.ok_or(ArithmeticError::Underflow)?;

				T::AMMTrader::sell(
					origin,
					*asset_in,
					*asset_out,
					amount_to_sell,
					min(*min_limit, min_limit_with_slippage),
				)
			}
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_limit,
				route: _,
			} => {
				let max_limit_with_slippage = Self::get_max_limit_with_slippage(asset_in, asset_out, amount_out)?;

				T::AMMTrader::buy(
					origin,
					*asset_in,
					*asset_out,
					*amount_out,
					max(*max_limit, max_limit_with_slippage),
				)
			}
		}
	}

	fn get_min_limit_with_slippage(
		asset_in: &<T as Config>::Asset,
		asset_out: &<T as Config>::Asset,
		amount_in: &Balance,
	) -> Result<u128, DispatchError> {
		let price = T::PriceProvider::price(*asset_in, *asset_out).ok_or(Error::<T>::CalculatingPriceError)?;
		let price = FixedU128::from_rational(price.n, price.d);

		let estimated_amount_out = price.checked_mul_int(*amount_in).ok_or(ArithmeticError::Overflow)?;

		let slippage_amount = T::SlippageLimitPercentage::get().mul_floor(estimated_amount_out);
		let min_limit_with_slippage = estimated_amount_out
			.checked_sub(slippage_amount)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(min_limit_with_slippage)
	}

	fn get_max_limit_with_slippage(
		asset_in: &<T as Config>::Asset,
		asset_out: &<T as Config>::Asset,
		amount_out: &Balance,
	) -> Result<u128, DispatchError> {
		let price = T::PriceProvider::price(*asset_out, *asset_in).ok_or(Error::<T>::CalculatingPriceError)?;

		let price = FixedU128::from_rational(price.n, price.d);

		let estimated_amount_in = price.checked_mul_int(*amount_out).ok_or(ArithmeticError::Overflow)?;

		let slippage_amount = T::SlippageLimitPercentage::get().mul_floor(estimated_amount_in);
		let max_limit_with_slippage = estimated_amount_in
			.checked_add(slippage_amount)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(max_limit_with_slippage)
	}

	fn get_transaction_fee(fee_currency: T::Asset) -> Result<u128, DispatchError> {
		let fee_amount_in_native = Self::weight_to_fee(<T as Config>::WeightInfo::on_initialize());
		let fee_amount_in_sold_asset =
			Self::convert_to_currency_if_asset_is_not_native(fee_currency, fee_amount_in_native)?;

		Ok(fee_amount_in_sold_asset)
	}

	fn remove_schedule_id_from_next_execution_block(
		schedule_id: ScheduleId,
		next_execution_block: T::BlockNumber,
	) -> DispatchResult {
		ScheduleIdsPerBlock::<T>::try_mutate_exists(next_execution_block, |maybe_schedule_ids| -> DispatchResult {
			let schedule_ids = maybe_schedule_ids.as_mut().ok_or(Error::<T>::ScheduleNotExist)?;

			let index = schedule_ids
				.iter()
				.position(|x| *x == schedule_id)
				.ok_or(Error::<T>::NoPlannedExecutionFoundOnBlock)?;

			schedule_ids.remove(index);

			if schedule_ids.is_empty() {
				*maybe_schedule_ids = None;
			}
			Ok(())
		})?;

		Ok(())
	}

	fn suspend_schedule(owner: &T::AccountId, schedule_id: ScheduleId) -> DispatchResult {
		Suspended::<T>::insert(schedule_id, ());
		Self::deposit_event(Event::Suspended {
			id: schedule_id,
			who: owner.clone(),
		});

		Ok(())
	}

	fn convert_to_currency_if_asset_is_not_native(
		asset_id: T::Asset,
		asset_amount: u128,
	) -> Result<u128, DispatchError> {
		let amount = if asset_id == T::NativeAssetId::get() {
			asset_amount
		} else {
			let price =
				T::PriceProvider::price(T::NativeAssetId::get(), asset_id).ok_or(Error::<T>::CalculatingPriceError)?;
			let price = FixedU128::from_rational(price.n, price.d);

			price.checked_mul_int(asset_amount).ok_or(ArithmeticError::Overflow)?
		};

		Ok(amount)
	}

	fn remove_schedule_from_storages(owner: &T::AccountId, schedule_id: ScheduleId) {
		Schedules::<T>::remove(schedule_id);
		Suspended::<T>::remove(schedule_id);
		ScheduleOwnership::<T>::remove(owner, schedule_id);
		RemainingRecurrences::<T>::remove(schedule_id);
		RemainingAmounts::<T>::remove(schedule_id);
	}

	fn remove_planning_or_suspension(
		schedule_id: ScheduleId,
		next_execution_block: Option<T::BlockNumber>,
	) -> DispatchResult {
		match next_execution_block {
			Some(block) => {
				let schedule_ids_on_block =
					ScheduleIdsPerBlock::<T>::get(block).ok_or(Error::<T>::NoPlannedExecutionFoundOnBlock)?;

				ensure!(
					schedule_ids_on_block.contains(&schedule_id),
					Error::<T>::NoPlannedExecutionFoundOnBlock,
				);

				Self::remove_schedule_id_from_next_execution_block(schedule_id, block)?;
			}
			None => {
				Self::ensure_that_schedule_is_suspended(schedule_id)?;
				Suspended::<T>::remove(schedule_id);
			}
		};

		Ok(())
	}

	fn complete_dca(owner: &T::AccountId, schedule_id: ScheduleId) {
		Self::remove_schedule_from_storages(owner, schedule_id);
		Self::deposit_event(Event::Completed {
			id: schedule_id,
			who: owner.clone(),
		});
	}
}

pub trait RandomnessProvider {
	fn generator() -> StdRng;
}

impl<T: Config> RandomnessProvider for Pallet<T> {
	fn generator() -> StdRng {
		let hash_value = pallet_relaychain_info::Pallet::<T>::parent_hash();
		let mut seed_arr = [0u8; 8];
		seed_arr.copy_from_slice(&hash_value.as_fixed_bytes()[0..8]);
		let seed = u64::from_le_bytes(seed_arr);
		rand::rngs::StdRng::seed_from_u64(seed)
	}
}

#[macro_export]
macro_rules! exec_or_return_if_none {
	($opt:expr) => {
		match $opt {
			Some(val) => val,
			None => {
				log::error!(target: "runtime::dca", "Unexpected error happened while executing schedule.");
				return;
			}
		}
	};
}

#[macro_export]
macro_rules! exec_or_return_if_err {
	($res:expr) => {
		match $res {
			Ok(val) => val,
			Err(e) => {
				log::error!(
					target: "runtime::dca",
					"Unexpected error happened while executing schedule, with message: {:?}.",
					e
				);
				return;
			}
		}
	};
}
