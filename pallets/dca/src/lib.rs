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

//! # DCA pallet
//!
//! ## Overview
//!
//! A dollar-cost averaging pallet that enables users to perform repeating orders.
//!
//! When an order is submitted, it will reserve the total amount (budget) specified by the user.
//!
//! A named reserve is allocated for the reserved amount of all DCA held by each user.
//!
//! The DCA plan is executed as long as there is remaining balance in the budget.
//!
//! If a trade fails due to specific errors whitelisted in the pallet config,
//! then retry happens up to the maximum number of retries specified also as config.
//! Once the max number of retries reached, the order is terminated permanently.
//!
//! If a trade fails due to other kind of errors, the order is terminated permanently without any retry logic.
//!
//! Orders are executed on block initialize and they are sorted based on randomness derived from relay chain block number.
//! Therefore they cannot be front-ran in the block they are executed.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, Len},
	transactional,
	weights::WeightToFee as FrameSupportWeight,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor, Origin};
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::{OraclePeriod, PriceOracle};
use orml_traits::arithmetic::CheckedAdd;
use orml_traits::MultiCurrency;
use orml_traits::NamedMultiReservableCurrency;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use scale_info::TypeInfo;
use sp_runtime::traits::CheckedMul;
use sp_runtime::traits::One;
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating},
	ArithmeticError, BoundedVec, DispatchError, FixedPointNumber, FixedU128, Permill,
};
use sp_std::vec::Vec;
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

pub const RETRY_TO_SEARCH_FOR_FREE_BLOCK: u32 = 5;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::traits::Contains;
	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use hydra_dx_math::ema::EmaPrice;
	use hydradx_traits::pools::SpotPriceProvider;
	use hydradx_traits::PriceOracle;
	use orml_traits::NamedMultiReservableCurrency;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_initialize(current_blocknumber: T::BlockNumber) -> Weight {
			let mut weight = T::WeightInfo::on_initialize_with_empty_block();

			let mut random_generator = T::RandomnessProvider::generator();

			let mut schedule_ids: Vec<ScheduleId> = ScheduleIdsPerBlock::<T>::get(current_blocknumber).to_vec();

			if !schedule_ids.is_empty() {
				Self::deposit_event(Event::ExecutionsStarted {
					block: current_blocknumber,
				});
			}

			schedule_ids.sort_by_cached_key(|_| random_generator.gen::<u32>());
			for schedule_id in schedule_ids {
				let Some(schedule) = Schedules::<T>::get(schedule_id) else {
					//We cant terminate here as there is no schedule information to do so
					continue;
				};

				let next_execution_block =
					match Self::prepare_schedule(current_blocknumber, &mut weight, schedule_id, &schedule) {
						Ok(block) => block,
						Err(err) => {
							if err == Error::<T>::PriceChangeIsBiggerThanMaxAllowed.into() {
								//The schedule is replanned instead of terminated
							} else {
								Self::terminate_schedule(schedule_id, &schedule, err);
							}
							continue;
						}
					};

				let trade_result = Self::execute_schedule(schedule_id, &schedule);

				match trade_result {
					Ok(_) => {
						if let Err(err) = Self::replan_or_complete(schedule_id, &schedule, next_execution_block) {
							Self::terminate_schedule(schedule_id, &schedule, err);
							continue;
						}
					}
					Err(err) => {
						Self::deposit_event(Event::TradeFailed {
							id: schedule_id,
							who: schedule.owner.clone(),
							error: err,
						});

						if T::ContinueOnErrors::contains(&err) {
							if let Err(err) = Self::retry_schedule(schedule_id, &schedule, next_execution_block) {
								Self::terminate_schedule(schedule_id, &schedule, err);
								continue;
							}
						} else {
							Self::terminate_schedule(schedule_id, &schedule, err)
						}
					}
				}
			}

			weight
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

		/// Origin able to terminate schedules
		type TechnicalOrigin: EnsureOrigin<Self::Origin>;

		///For named-reserving user's assets
		type Currency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::Asset,
			Balance = Balance,
		>;

		///AMMTrader for trade execution
		type AMMTrader: AMMTrader<Self::Origin, Self::Asset, Balance>;

		///Randomness provider to be used to sort the DCA schedules when they are executed in a block
		type RandomnessProvider: RandomnessProvider;

		///Oracle price provider to get the price between two assets
		type OraclePriceProvider: PriceOracle<Self::Asset, Price = EmaPrice>;

		///Spot price provider to get the current price between two asset
		type SpotPriceProvider: SpotPriceProvider<Self::Asset, Price = FixedU128>;

		///Errors on which we want to continue the schedule
		type ContinueOnErrors: Contains<DispatchError>;

		///Max price difference allowed between the last block and short oracle
		#[pallet::constant]
		type MaxPriceDifferenceBetweenBlocks: Get<Permill>;

		///The number of max schedules to be executed per block
		#[pallet::constant]
		type MaxSchedulePerBlock: Get<u32>;

		///The number of max retries on errors specified in `ContinueOnErrors`
		#[pallet::constant]
		type MaxNumberOfRetriesOnError: Get<u32>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::Asset>;

		///Storage bond in native currency
		#[pallet::constant]
		type StorageBondInNativeCurrency: Get<Balance>;

		///The fee receiver for transaction fees
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;

		///Max slippage limit treshold percentage to be used for contstraining limits between blocks
		#[pallet::constant]
		type MaxSlippageTresholdBetweenBlocks: Get<Permill>;

		/// Named reserve identifier to store named reserves for orders of each users
		#[pallet::constant]
		type NamedReserveId: Get<NamedReserveIdentifier>;

		/// Convert a weight value into a deductible fee
		type WeightToFee: WeightToFee<Balance = Balance>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///The DCA is scheduled
		ExecutionsStarted {
			block: BlockNumberFor<T>,
		},
		Scheduled {
			id: ScheduleId,
			who: T::AccountId,
		},
		///The DCA is planned for blocknumber
		ExecutionPlanned {
			id: ScheduleId,
			who: T::AccountId,
			block: BlockNumberFor<T>,
		},
		///The DCA trade has been successfully executed
		TradeExecuted {
			id: ScheduleId,
			who: T::AccountId,
		},
		///The DCA trade execution has been failed
		TradeFailed {
			id: ScheduleId,
			who: T::AccountId,
			error: DispatchError,
		},
		///The DCA is terminated and completely removed from the chain
		Terminated {
			id: ScheduleId,
			who: T::AccountId,
			error: DispatchError,
		},
		///The DCA is completed and completely removed from the chain
		Completed {
			id: ScheduleId,
			who: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		///Schedule not exist
		ScheduleNotFound,
		///Trade amount is less than fee
		TradeAmountIsLessThanFee,
		///Forbidden as the user is not the owner of the schedule
		Forbidden,
		///The next execution block number is not in the future
		BlockNumberIsNotInFuture,
		///Price change from oracle data is bigger than max allowed
		PriceChangeIsBiggerThanMaxAllowed,
		///Error occurred when calculating price
		CalculatingPriceError,
		///The total amount to be reserved should be larger than storage bond
		TotalAmountShouldBeLargerThanStorageBond,
		///The budget is too low for executing one DCA
		BudgetTooLow,
		///There is no free block found to plan DCA execution
		NoFreeBlockFound,
		///The DCA schedule has been manually terminated
		ManuallyTerminated,
		///Max number of retries reached for schedule
		MaxRetryReached,
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

	/// Keep tracking the remaining amounts to spend for DCA schedules
	#[pallet::storage]
	#[pallet::getter(fn remaining_amounts)]
	pub type RemainingAmounts<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, Balance, OptionQuery>;

	/// Keep tracking the retry on error flag for DCA schedules
	#[pallet::storage]
	#[pallet::getter(fn retries_on_error)]
	pub type RetriesOnError<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u32, OptionQuery>;

	/// Keep tracking of the schedule ids to be executed in the block
	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
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

			let storage_bond = Self::get_storage_bond(&schedule)?;
			ensure!(
				schedule.total_amount > storage_bond,
				Error::<T>::TotalAmountShouldBeLargerThanStorageBond
			);

			let weight_for_single_execution = Self::get_weight_for_single_execution()?;
			let transaction_fee =
				Self::convert_weight_to_fee(weight_for_single_execution, schedule.order.get_asset_in())?;

			let amount_in = match schedule.order {
				Order::Sell { amount_in, .. } => {
					//In sell the amount_in includes the transaction fee
					ensure!(amount_in > transaction_fee, Error::<T>::TradeAmountIsLessThanFee);
					Self::get_amount_to_sell(&schedule.order)?
				}
				Order::Buy { .. } => {
					let amount_to_unreserve = Self::get_amount_to_sell(&schedule.order)?;
					ensure!(
						amount_to_unreserve > transaction_fee,
						Error::<T>::TradeAmountIsLessThanFee
					);
					amount_to_unreserve
				}
			};

			ensure!(
				amount_in + transaction_fee <= schedule.total_amount,
				Error::<T>::BudgetTooLow
			);

			let next_schedule_id = Self::get_next_schedule_id()?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			ScheduleOwnership::<T>::insert(who.clone(), next_schedule_id, ());
			RemainingAmounts::<T>::insert(next_schedule_id, schedule.total_amount);
			RetriesOnError::<T>::insert(next_schedule_id, 0);

			Self::reserve_asset_in(&schedule, &who)?;

			let next_block_number = Self::get_next_block_number()?;
			let blocknumber_for_first_schedule_execution = start_execution_block.unwrap_or(next_block_number);
			Self::plan_schedule_for_block(who.clone(), blocknumber_for_first_schedule_execution, next_schedule_id)?;

			Self::deposit_event(Event::Scheduled {
				id: next_schedule_id,
				who,
			});

			Ok(())
		}

		/// Admin endpoint to terminate a DCA schedule and remove it completely from the chain.
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule_id`: schedule id
		/// - `next_execution_block`: block number where the schedule is planned.
		///
		/// Emits `Terminated` event when successful.
		///
		#[pallet::weight(<T as Config>::WeightInfo::terminate())]
		#[transactional]
		pub fn terminate(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: BlockNumberFor<T>,
		) -> DispatchResult {
			T::TechnicalOrigin::ensure_origin(origin)?;
			ensure!(Schedules::<T>::contains_key(schedule_id), Error::<T>::ScheduleNotFound);

			let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotFound)?;

			Self::unreserve_remaining_named_reserve(schedule_id, &schedule.owner)?;

			let schedule_ids_on_block = ScheduleIdsPerBlock::<T>::get(next_execution_block);
			ensure!(
				schedule_ids_on_block.contains(&schedule_id),
				Error::<T>::ScheduleNotFound,
			);

			Self::remove_schedule_id_from_next_execution_block(schedule_id, next_execution_block)?;
			Self::remove_schedule_from_storages(&schedule.owner, schedule_id);

			Self::deposit_event(Event::Terminated {
				id: schedule_id,
				who: schedule.owner,
				error: Error::<T>::ManuallyTerminated.into(),
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn prepare_schedule(
		current_blocknumber: T::BlockNumber,
		weight: &mut Weight,
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> Result<T::BlockNumber, DispatchError> {
		let weight_for_single_execution = Self::get_weight_for_single_execution()?;

		weight.saturating_accrue(weight_for_single_execution);

		Self::take_transaction_fee_from_user(
			schedule_id,
			&schedule.owner,
			&schedule.order,
			weight_for_single_execution,
		)?;

		let next_execution_block = current_blocknumber
			.checked_add(&schedule.period)
			.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		let is_price_change_bigger_than_max_allowed = Self::price_change_is_bigger_than_max_allowed(
			schedule.order.get_asset_in(),
			schedule.order.get_asset_out(),
		);

		if is_price_change_bigger_than_max_allowed {
			Self::plan_schedule_for_block(schedule.owner.clone(), next_execution_block, schedule_id)?;
			return Err(Error::<T>::PriceChangeIsBiggerThanMaxAllowed.into());
		}

		Ok(next_execution_block)
	}

	#[transactional]
	pub fn execute_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		let origin: OriginFor<T> = Origin::<T>::Signed(schedule.owner.clone()).into();

		let Ok(amount_to_sell)  = Self::get_amount_to_sell(&schedule.order) else {
			return Err(Error::<T>::InvalidState.into());
		};

		let sold_currency = schedule.order.get_asset_in();

		let remaining_amount_if_insufficient_balance = T::Currency::unreserve_named(
			&T::NamedReserveId::get(),
			sold_currency,
			&schedule.owner,
			amount_to_sell,
		);
		ensure!(remaining_amount_if_insufficient_balance == 0, Error::<T>::InvalidState);

		let Ok(()) = Self::decrease_remaining_amount(schedule_id, amount_to_sell) else {
			return Err(Error::<T>::InvalidState.into());
		};

		Self::execute_trade(origin, &schedule.order, amount_to_sell)
	}

	fn replan_or_complete(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		next_execution_block: T::BlockNumber,
	) -> DispatchResult {
		Self::deposit_event(Event::TradeExecuted {
			id: schedule_id,
			who: schedule.owner.clone(),
		});

		Self::reset_retries(schedule_id)?;

		let remaining_amount_to_use = RemainingAmounts::<T>::get(schedule_id).ok_or(Error::<T>::InvalidState)?;
		let amount_to_unreserve = Self::get_amount_to_sell(&schedule.order)?;

		if remaining_amount_to_use < amount_to_unreserve {
			Self::complete_schedule(schedule_id, schedule);
			return Ok(());
		}

		Self::plan_schedule_for_block(schedule.owner.clone(), next_execution_block, schedule_id)?;

		Ok(())
	}

	fn retry_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		next_execution_block: T::BlockNumber,
	) -> DispatchResult {
		let number_of_retries = Self::retries_on_error(schedule_id).ok_or(Error::<T>::InvalidState)?;

		if number_of_retries == T::MaxNumberOfRetriesOnError::get() {
			return Err(Error::<T>::MaxRetryReached.into());
		}

		Self::increment_retries(schedule_id)?;

		Self::plan_schedule_for_block(schedule.owner.clone(), next_execution_block, schedule_id)?;

		Ok(())
	}

	fn price_change_is_bigger_than_max_allowed(asset_a: T::Asset, asset_b: T::Asset) -> bool {
		let Some(current_price) = T::SpotPriceProvider::spot_price(asset_a, asset_b) else {
			return true;
		};

		let Ok(price_from_short_oracle) = Self::get_price_from_short_oracle(asset_a, asset_b) else {
   			return true;
		};

		let max_allowed = FixedU128::from(T::MaxPriceDifferenceBetweenBlocks::get());

		let Some(price_sum) = current_price
			.checked_add(&price_from_short_oracle) else {
			return true;
		};

		let Ok(max_allowed_difference) = max_allowed
			.checked_mul(
				&price_sum,
			)
			.ok_or(ArithmeticError::Overflow)
			else {
				return true;
		};

		let diff = if current_price > price_from_short_oracle {
			current_price.saturating_sub(price_from_short_oracle)
		} else {
			price_from_short_oracle.saturating_sub(current_price)
		};

		let Some(diff) = diff.checked_mul(&FixedU128::from(2)) else {
			return true;
		};

		diff > max_allowed_difference
	}

	fn get_price_from_last_block_oracle(asset_a: T::Asset, asset_b: T::Asset) -> Result<FixedU128, DispatchError> {
		let price = T::OraclePriceProvider::price(asset_a, asset_b, OraclePeriod::LastBlock)
			.ok_or(Error::<T>::CalculatingPriceError)?;

		let price_from_rational =
			FixedU128::checked_from_rational(price.n, price.d).ok_or(ArithmeticError::Overflow)?;

		Ok(price_from_rational)
	}

	fn get_price_from_short_oracle(asset_a: T::Asset, asset_b: T::Asset) -> Result<FixedU128, DispatchError> {
		let price = T::OraclePriceProvider::price(asset_a, asset_b, OraclePeriod::Short)
			.ok_or(Error::<T>::CalculatingPriceError)?;

		let price_from_rational =
			FixedU128::checked_from_rational(price.n, price.d).ok_or(ArithmeticError::Overflow)?;

		Ok(price_from_rational)
	}

	fn get_amount_to_sell(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		match order {
			Order::Sell {
				asset_in, amount_in, ..
			} => {
				let weight_for_single_execution = Self::get_weight_for_single_execution()?;
				let transaction_fee = Self::convert_weight_to_fee(weight_for_single_execution, *asset_in)?;

				let amount_to_sell = amount_in
					.checked_sub(transaction_fee)
					.ok_or(ArithmeticError::Underflow)?;
				Ok(amount_to_sell)
			}
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_limit,
				..
			} => {
				let (estimated_amount_in, slippage_amount) =
					Self::calculate_estimated_and_slippage_amounts(*asset_in, *asset_out, *amount_out)?;

				let max_limit_from_oracle_price = estimated_amount_in
					.checked_add(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;

				let estimated_amount_to_sell = min(*max_limit, max_limit_from_oracle_price);

				Ok(estimated_amount_to_sell)
			}
		}
	}

	fn decrease_remaining_amount(schedule_id: ScheduleId, amount_to_unreserve: Balance) -> DispatchResult {
		RemainingAmounts::<T>::try_mutate_exists(schedule_id, |maybe_remaining_amount| -> DispatchResult {
			let remaining_amount = maybe_remaining_amount.as_mut().ok_or(Error::<T>::ScheduleNotFound)?;

			if amount_to_unreserve > *remaining_amount {
				return Err(Error::<T>::InvalidState.into());
			};

			let new_amount = remaining_amount
				.checked_sub(amount_to_unreserve)
				.ok_or(ArithmeticError::Underflow)?;

			*remaining_amount = new_amount;

			Ok(())
		})?;

		Ok(())
	}

	fn increment_retries(schedule_id: ScheduleId) -> DispatchResult {
		RetriesOnError::<T>::try_mutate_exists(schedule_id, |maybe_retries| -> DispatchResult {
			let retries = maybe_retries.as_mut().ok_or(Error::<T>::ScheduleNotFound)?;

			retries.saturating_inc();

			Ok(())
		})?;

		Ok(())
	}

	fn reset_retries(schedule_id: ScheduleId) -> DispatchResult {
		RetriesOnError::<T>::try_mutate_exists(schedule_id, |maybe_retries| -> DispatchResult {
			let retries = maybe_retries.as_mut().ok_or(Error::<T>::ScheduleNotFound)?;

			*retries = 0;

			Ok(())
		})?;

		Ok(())
	}

	fn get_storage_bond(schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>) -> Result<Balance, DispatchError> {
		let storage_bond = if schedule.order.get_asset_in() == T::NativeAssetId::get() {
			T::StorageBondInNativeCurrency::get()
		} else {
			Self::get_storage_bond_in_sold_currency(&schedule.order)?
		};

		Ok(storage_bond)
	}

	fn get_storage_bond_in_sold_currency(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		let sold_currency = order.get_asset_in();
		let storage_bond_in_native_currency = T::StorageBondInNativeCurrency::get();

		let storage_bond_in_user_currency =
			Self::convert_to_currency_if_asset_is_not_native(sold_currency, storage_bond_in_native_currency)?;

		Ok(storage_bond_in_user_currency)
	}

	#[transactional]
	fn take_transaction_fee_from_user(
		schedule_id: ScheduleId,
		owner: &T::AccountId,
		order: &Order<<T as Config>::Asset>,
		weight_to_charge: Weight,
	) -> DispatchResult {
		let fee_currency = order.get_asset_in();

		let fee_amount_in_sold_asset = Self::convert_weight_to_fee(weight_to_charge, fee_currency)?;

		let remaining_amount_if_insufficient_balance = T::Currency::unreserve_named(
			&T::NamedReserveId::get(),
			order.get_asset_in(),
			owner,
			fee_amount_in_sold_asset,
		);
		ensure!(remaining_amount_if_insufficient_balance == 0, Error::<T>::InvalidState);

		Self::decrease_remaining_amount(schedule_id, fee_amount_in_sold_asset)?;

		T::Currency::transfer(fee_currency, owner, &T::FeeReceiver::get(), fee_amount_in_sold_asset)?;

		Ok(())
	}

	fn reserve_asset_in(
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		who: &T::AccountId,
	) -> DispatchResult {
		let currency_for_reserve = match schedule.order {
			Order::Buy { asset_in, .. } => asset_in,
			Order::Sell { asset_in, .. } => asset_in,
		};

		T::Currency::reserve_named(
			&T::NamedReserveId::get(),
			currency_for_reserve,
			who,
			schedule.total_amount,
		)?;

		Ok(())
	}

	fn terminate_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		error: DispatchError,
	) {
		let result = Self::unreserve_remaining_named_reserve(schedule_id, &schedule.owner);

		match result {
			Ok(()) => {
				Self::remove_schedule_from_storages(&schedule.owner, schedule_id);
				Self::deposit_event(Event::Terminated {
					id: schedule_id,
					who: schedule.owner.clone(),
					error,
				});
			}
			Err(error) => {
				Self::deposit_event(Event::Terminated {
					id: schedule_id,
					who: schedule.owner.clone(),
					error,
				});
			}
		}
	}

	fn complete_schedule(schedule_id: ScheduleId, schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>) {
		let result = Self::unreserve_remaining_named_reserve(schedule_id, &schedule.owner);

		match result {
			Ok(()) => {
				Self::remove_schedule_from_storages(&schedule.owner, schedule_id);
				Self::deposit_event(Event::Completed {
					id: schedule_id,
					who: schedule.owner.clone(),
				});
			}
			Err(error) => Self::deposit_event(Event::Terminated {
				id: schedule_id,
				who: schedule.owner.clone(),
				error,
			}),
		}
	}

	fn unreserve_remaining_named_reserve(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotFound)?;
		let sold_currency = schedule.order.get_asset_in();

		let remaining_amount = RemainingAmounts::<T>::get(schedule_id).ok_or(Error::<T>::InvalidState)?;

		T::Currency::unreserve_named(&T::NamedReserveId::get(), sold_currency, who, remaining_amount);

		Ok(())
	}

	fn weight_to_fee(weight: Weight) -> Balance {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight: Weight = weight.min(T::BlockWeights::get().max_block);
		<T as pallet::Config>::WeightToFee::weight_to_fee(&capped_weight)
	}

	fn plan_schedule_for_block(
		who: T::AccountId,
		blocknumber: T::BlockNumber,
		schedule_id: ScheduleId,
	) -> DispatchResult {
		let current_block_number = frame_system::Pallet::<T>::current_block_number();
		ensure!(blocknumber > current_block_number, Error::<T>::BlockNumberIsNotInFuture);

		let next_free_block = Self::find_next_free_block(blocknumber)?;

		if ScheduleIdsPerBlock::<T>::contains_key(next_free_block) {
			ScheduleIdsPerBlock::<T>::try_mutate_exists(next_free_block, |schedule_ids| -> DispatchResult {
				let schedule_ids = schedule_ids.as_mut().ok_or(Error::<T>::InvalidState)?;

				schedule_ids
					.try_push(schedule_id)
					.map_err(|_| Error::<T>::InvalidState)?;
				Ok(())
			})?;
			return Ok(());
		} else {
			let vec_with_first_schedule_id = Self::create_bounded_vec(schedule_id)?;
			ScheduleIdsPerBlock::<T>::insert(next_free_block, vec_with_first_schedule_id);
		}

		Self::deposit_event(Event::ExecutionPlanned {
			id: schedule_id,
			who,
			block: next_free_block,
		});
		Ok(())
	}

	fn find_next_free_block(blocknumber: T::BlockNumber) -> Result<T::BlockNumber, DispatchError> {
		let mut next_execution_block = blocknumber;

		// We bound it to MAX_NUMBER_OF_RETRY_FOR_PLANNING to find the block number.
		// We search for next free block with incrementing with the power of 2 (so 1 - 2 - 4 - 8 - 16)
		for retry_index in 0u32..RETRY_TO_SEARCH_FOR_FREE_BLOCK {
			if ScheduleIdsPerBlock::<T>::contains_key(next_execution_block) {
				let schedule_ids = ScheduleIdsPerBlock::<T>::get(next_execution_block);
				if schedule_ids.len() < T::MaxSchedulePerBlock::get() as usize {
					return Ok(next_execution_block);
				}
				let delay_with = 2u32.checked_pow(retry_index).ok_or(ArithmeticError::Overflow)?;
				next_execution_block = next_execution_block.saturating_add(delay_with.into());
			}

			if retry_index
				== RETRY_TO_SEARCH_FOR_FREE_BLOCK
					.checked_sub(1) //We substract 1 as we start the indexing from 0
					.ok_or(ArithmeticError::Underflow)?
				&& ScheduleIdsPerBlock::<T>::get(next_execution_block).len() == T::MaxSchedulePerBlock::get() as usize
			{
				return Err(Error::<T>::NoFreeBlockFound.into());
			}
		}
		Ok(next_execution_block)
	}

	fn get_next_schedule_id() -> Result<ScheduleId, ArithmeticError> {
		ScheduleIdSequencer::<T>::try_mutate(|current_id| {
			let schedule_id = *current_id;

			*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			Ok(schedule_id)
		})
	}

	fn get_next_block_number() -> Result<BlockNumberFor<T>, DispatchError> {
		let current_block_number = frame_system::Pallet::<T>::current_block_number();
		let next_block_number = current_block_number
			.checked_add(&T::BlockNumber::one())
			.ok_or(ArithmeticError::Overflow)?;

		Ok(next_block_number)
	}

	fn create_bounded_vec(
		next_schedule_id: ScheduleId,
	) -> Result<BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, DispatchError> {
		let schedule_id = vec![next_schedule_id];
		let bounded_vec: BoundedVec<ScheduleId, T::MaxSchedulePerBlock> =
			schedule_id.try_into().map_err(|_| Error::<T>::InvalidState)?;
		Ok(bounded_vec)
	}

	fn execute_trade(origin: T::Origin, order: &Order<T::Asset>, amount_to_sell: Balance) -> DispatchResult {
		match order {
			Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_limit,
				route: _,
			} => {
				let (estimated_amount_out, slippage_amount) =
					Self::calculate_estimated_and_slippage_amounts(*asset_out, *asset_in, *amount_in)?;

				let min_limit_with_slippage = estimated_amount_out
					.checked_sub(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;

				T::AMMTrader::sell(
					origin,
					*asset_in,
					*asset_out,
					amount_to_sell,
					max(*min_limit, min_limit_with_slippage),
				)
			}
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				..
			} => T::AMMTrader::buy(origin, *asset_in, *asset_out, *amount_out, amount_to_sell),
		}
	}

	fn calculate_estimated_and_slippage_amounts(
		asset_a: <T as Config>::Asset,
		asset_b: <T as Config>::Asset,
		amount: Balance,
	) -> Result<(Balance, Balance), DispatchError> {
		let price = Self::get_price_from_last_block_oracle(asset_a, asset_b)?;

		let estimated_amount = price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

		let slippage_amount = T::MaxSlippageTresholdBetweenBlocks::get().mul_floor(estimated_amount);

		Ok((estimated_amount, slippage_amount))
	}

	fn convert_weight_to_fee(weight: Weight, fee_currency: T::Asset) -> Result<u128, DispatchError> {
		let fee_amount_in_native = Self::weight_to_fee(weight);
		let fee_amount_in_sold_asset =
			Self::convert_to_currency_if_asset_is_not_native(fee_currency, fee_amount_in_native)?;

		Ok(fee_amount_in_sold_asset)
	}

	fn get_weight_for_single_execution() -> Result<Weight, DispatchError> {
		let max_schedule_per_block = max(T::MaxSchedulePerBlock::get().into(), 1);
		let weight = <T as Config>::WeightInfo::on_initialize()
			.checked_div(max_schedule_per_block)
			.ok_or(ArithmeticError::Underflow)?;

		Ok(weight)
	}

	fn convert_to_currency_if_asset_is_not_native(
		asset_id: T::Asset,
		asset_amount: u128,
	) -> Result<u128, DispatchError> {
		let amount = if asset_id == T::NativeAssetId::get() {
			asset_amount
		} else {
			let price = Self::get_price_from_last_block_oracle(T::NativeAssetId::get(), asset_id)?;

			price.checked_mul_int(asset_amount).ok_or(ArithmeticError::Overflow)?
		};

		Ok(amount)
	}

	fn remove_schedule_id_from_next_execution_block(
		schedule_id: ScheduleId,
		next_execution_block: T::BlockNumber,
	) -> DispatchResult {
		ScheduleIdsPerBlock::<T>::try_mutate_exists(next_execution_block, |maybe_schedule_ids| -> DispatchResult {
			let schedule_ids = maybe_schedule_ids.as_mut().ok_or(Error::<T>::ScheduleNotFound)?;

			let index = schedule_ids
				.iter()
				.position(|x| *x == schedule_id)
				.ok_or(Error::<T>::ScheduleNotFound)?;

			schedule_ids.remove(index);

			if schedule_ids.is_empty() {
				*maybe_schedule_ids = None;
			}
			Ok(())
		})?;

		Ok(())
	}

	fn remove_schedule_from_storages(owner: &T::AccountId, schedule_id: ScheduleId) {
		Schedules::<T>::remove(schedule_id);
		ScheduleOwnership::<T>::remove(owner, schedule_id);
		RemainingAmounts::<T>::remove(schedule_id);
		RetriesOnError::<T>::remove(schedule_id);
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
