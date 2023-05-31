// This file is part of HydraDX-node

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
//! Orders are executed on block initialize and they are sorted based on randomness derived from relay chain block hash.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;

use cumulus_primitives_core::relay_chain::Hash;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, Len},
	transactional,
	weights::WeightToFee as FrameSupportWeight,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor, Origin};
use hydradx_traits::pools::SpotPriceProvider;
use hydradx_traits::router::PoolType;
use hydradx_traits::{OraclePeriod, PriceOracle};
use orml_traits::arithmetic::CheckedAdd;
use orml_traits::MultiCurrency;
use orml_traits::NamedMultiReservableCurrency;
use pallet_route_executor::Trade;
use pallet_route_executor::TradeAmountsCalculator;
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

pub const SHORT_ORACLE_BLOCK_PERIOD: u32 = 10;
pub const RETRY_TO_SEARCH_FOR_FREE_BLOCK: u32 = 5;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;

	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use hydra_dx_math::ema::EmaPrice;
	use hydradx_traits::pools::SpotPriceProvider;
	use hydradx_traits::PriceOracle;
	use orml_traits::NamedMultiReservableCurrency;
	use sp_runtime::DispatchError::BadOrigin;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T>
	where
		<T as pallet_route_executor::Config>::AssetId: From<<T as pallet::Config>::Asset>,
		<T as pallet_route_executor::Config>::Balance: From<u128>,
		u128: From<<T as pallet_route_executor::Config>::Balance>,
	{
		fn on_initialize(current_blocknumber: T::BlockNumber) -> Weight {
			let mut weight = <T as pallet::Config>::WeightInfo::on_initialize_with_empty_block();

			let Ok(mut random_generator) = T::RandomnessProvider::generator() else {
				Self::deposit_event(Event::RandomnessGenerationFailed {
					block: current_blocknumber,
				});
				return weight;
			};

			let mut schedule_ids: Vec<ScheduleId> = ScheduleIdsPerBlock::<T>::get(current_blocknumber).to_vec();

			schedule_ids.sort_by_cached_key(|_| random_generator.gen::<u32>());
			for schedule_id in schedule_ids {
				Self::deposit_event(Event::ExecutionStarted {
					id: schedule_id,
					block: current_blocknumber,
				});

				let Some(schedule) = Schedules::<T>::get(schedule_id) else {
					//We cant terminate here as there is no schedule information to do so
					continue;
				};

				match Self::prepare_schedule(current_blocknumber, &mut weight, schedule_id, &schedule) {
					Ok(block) => block,
					Err(err) => {
						if err != Error::<T>::PriceChangeIsBiggerThanMaxAllowed.into() {
							Self::terminate_schedule(schedule_id, &schedule, err);
						};
						continue;
					}
				}

				match Self::execute_trade(schedule_id, &schedule) {
					Ok(_) => {
						if let Err(err) = Self::replan_or_complete(schedule_id, &schedule, current_blocknumber) {
							Self::terminate_schedule(schedule_id, &schedule, err);
						}
					}
					Err(error) => {
						Self::deposit_event(Event::TradeFailed {
							id: schedule_id,
							who: schedule.owner.clone(),
							error,
						});

						if error != Error::<T>::TradeLimitReached.into() {
							Self::terminate_schedule(schedule_id, &schedule, error);
						} else if let Err(retry_error) =
							Self::retry_schedule(schedule_id, &schedule, current_blocknumber)
						{
							Self::terminate_schedule(schedule_id, &schedule, retry_error);
						}
					}
				}
			}

			weight
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_route_executor::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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
		type TechnicalOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		///For named-reserving user's assets
		type Currencies: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::Asset,
			Balance = Balance,
		>;

		///Relay chain block hash provider for randomness
		type RelayChainBlockHashProvider: RelayChainBlockHashProvider;

		///Randomness provider to be used to sort the DCA schedules when they are executed in a block
		type RandomnessProvider: RandomnessProvider;

		///Oracle price provider to get the price between two assets
		type OraclePriceProvider: PriceOracle<Self::Asset, Price = EmaPrice>;

		///Spot price provider to get the current price between two asset
		type SpotPriceProvider: SpotPriceProvider<Self::Asset, Price = FixedU128>;

		///Max price difference allowed between blocks
		#[pallet::constant]
		type MaxPriceDifferenceBetweenBlocks: Get<Permill>;

		///The number of max schedules to be executed per block
		#[pallet::constant]
		type MaxSchedulePerBlock: Get<u32>;

		///The number of max retries in case of trade limit error
		#[pallet::constant]
		type MaxNumberOfRetriesOnError: Get<u8>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::Asset>;

		///Minimum budget to be able to schedule a DCA, specified in native currency
		#[pallet::constant]
		type MinBudgetInNativeCurrency: Get<Balance>;

		///The fee receiver for transaction fees
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;

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
		///The DCA execution is started
		ExecutionStarted { id: ScheduleId, block: BlockNumberFor<T> },
		///The DCA is scheduled for next execution
		Scheduled { id: ScheduleId, who: T::AccountId },
		///The DCA is planned for blocknumber
		ExecutionPlanned {
			id: ScheduleId,
			who: T::AccountId,
			block: BlockNumberFor<T>,
		},
		///The DCA trade is successfully executed
		TradeExecuted { id: ScheduleId, who: T::AccountId },
		///The DCA trade execution is failed
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
		Completed { id: ScheduleId, who: T::AccountId },
		///Randomness generation failed possibly coming from missing data about relay chain
		RandomnessGenerationFailed { block: BlockNumberFor<T> },
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
		///The total amount to be reserved is smaller than min budget
		TotalAmountIsSmallerThanMinBudget,
		///The budget is too low for executing one DCA
		BudgetTooLow,
		///There is no free block found to plan DCA execution
		NoFreeBlockFound,
		///The DCA schedule has been manually terminated
		ManuallyTerminated,
		///Max number of retries reached for schedule
		MaxRetryReached,
		///The trade limit has been reached, leading to retry
		TradeLimitReached,
		///The route to execute the trade on is not specified
		RouteNotSpecified,
		///No parent hash has been found from relay chain
		NoParentHashFound,
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
	pub type RetriesOnError<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u8, OptionQuery>;

	/// Keep tracking of the schedule ids to be executed in the block
	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_route_executor::Config>::AssetId: From<<T as pallet::Config>::Asset>,
		<T as pallet_route_executor::Config>::Balance: From<u128>,
		u128: From<<T as pallet_route_executor::Config>::Balance>,
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
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::schedule())]
		#[transactional]
		pub fn schedule(
			origin: OriginFor<T>,
			schedule: Schedule<T::AccountId, T::Asset, BlockNumberFor<T>>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			ensure!(who == schedule.owner, Error::<T>::Forbidden);

			ensure!(schedule.order.get_route_length() > 0, Error::<T>::RouteNotSpecified);

			let min_budget = Self::convert_native_amount_to_currency(
				schedule.order.get_asset_in(),
				T::MinBudgetInNativeCurrency::get(),
			)?;
			ensure!(
				schedule.total_amount >= min_budget,
				Error::<T>::TotalAmountIsSmallerThanMinBudget
			);

			let transaction_fee = Self::get_transaction_fee(&schedule.order)?;

			let amount_in = match schedule.order {
				Order::Sell { amount_in, .. } => amount_in,
				Order::Buy {
					amount_out, ref route, ..
				} => {
					let amount_in = Self::get_amount_in_for_buy(&amount_out, route)?;
					amount_in.into()
				}
			};
			ensure!(amount_in > transaction_fee, Error::<T>::TradeAmountIsLessThanFee);

			let amount_in_with_transaction_fee = amount_in
				.checked_add(transaction_fee)
				.ok_or(ArithmeticError::Overflow)?;
			ensure!(
				amount_in_with_transaction_fee <= schedule.total_amount,
				Error::<T>::BudgetTooLow
			);

			let next_schedule_id = ScheduleIdSequencer::<T>::try_mutate(|current_id| {
				let schedule_id = *current_id;

				*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

				Ok::<u32, ArithmeticError>(schedule_id)
			})?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			ScheduleOwnership::<T>::insert(who.clone(), next_schedule_id, ());
			RemainingAmounts::<T>::insert(next_schedule_id, schedule.total_amount);
			RetriesOnError::<T>::insert(next_schedule_id, 0);

			T::Currencies::reserve_named(
				&T::NamedReserveId::get(),
				schedule.order.get_asset_in(),
				&who,
				schedule.total_amount,
			)?;

			let blocknumber_for_first_schedule_execution = match start_execution_block {
				Some(blocknumber) => Ok(blocknumber),
				None => {
					let current_block_number = frame_system::Pallet::<T>::current_block_number();
					let next_block_number = current_block_number
						.checked_add(&T::BlockNumber::one())
						.ok_or(ArithmeticError::Overflow)?;

					Ok::<T::BlockNumber, ArithmeticError>(next_block_number)
				}
			}?;

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
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::terminate())]
		#[transactional]
		pub fn terminate(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: BlockNumberFor<T>,
		) -> DispatchResult {
			let ensure_technical_origin = T::TechnicalOrigin::ensure_origin(origin.clone());
			let ensure_signed = ensure_signed(origin);
			if ensure_technical_origin.is_err() && ensure_signed.is_err() {
				return Err(BadOrigin);
			}

			let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotFound)?;

			if let Ok(who) = ensure_signed {
				ensure!(who == schedule.owner, Error::<T>::Forbidden);
			}

			Self::try_unreserve_all(schedule_id, &schedule);

			let schedule_ids_on_block = ScheduleIdsPerBlock::<T>::get(next_execution_block);
			ensure!(
				schedule_ids_on_block.contains(&schedule_id),
				Error::<T>::ScheduleNotFound,
			);

			//Remove schedule id from next execution block
			ScheduleIdsPerBlock::<T>::try_mutate_exists(
				next_execution_block,
				|maybe_schedule_ids| -> DispatchResult {
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
				},
			)?;

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

impl<T: Config> Pallet<T>
where
	<T as pallet_route_executor::Config>::AssetId: From<<T as pallet::Config>::Asset>,
	<T as pallet_route_executor::Config>::Balance: From<u128>,
	u128: From<<T as pallet_route_executor::Config>::Balance>,
{
	fn prepare_schedule(
		current_blocknumber: T::BlockNumber,
		weight: &mut Weight,
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		let weight_for_single_execution = Self::get_trade_weight(&schedule.order);

		weight.saturating_accrue(weight_for_single_execution);

		Self::take_transaction_fee_from_user(schedule_id, schedule, weight_for_single_execution)?;

		if Self::price_change_is_bigger_than_max_allowed(schedule) {
			Self::retry_schedule(schedule_id, schedule, current_blocknumber)?;

			return Err(Error::<T>::PriceChangeIsBiggerThanMaxAllowed.into());
		}

		Ok(())
	}

	#[transactional]
	pub fn execute_trade(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		let origin: OriginFor<T> = Origin::<T>::Signed(schedule.owner.clone()).into();

		match &schedule.order {
			Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_limit,
				slippage,
				route,
			} => {
				let remaining_amount_to_use =
					RemainingAmounts::<T>::get(schedule_id).ok_or(Error::<T>::InvalidState)?;
				let amount_to_sell = min(remaining_amount_to_use, *amount_in);

				Self::unallocate_amount(schedule_id, schedule, amount_to_sell)?;

				let (estimated_amount_out, slippage_amount) =
					Self::calculate_estimated_and_slippage_amounts(*asset_out, *asset_in, amount_to_sell, *slippage)?;
				let min_limit_with_slippage = estimated_amount_out
					.checked_sub(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;
				let min_limit = max(*min_limit, min_limit_with_slippage);

				let route = Self::convert_to_vec(route);
				let trade_amounts =
					pallet_route_executor::Pallet::<T>::calculate_sell_trade_amounts(&route, amount_to_sell.into())?;
				let last_trade = trade_amounts.last().ok_or(Error::<T>::InvalidState)?;
				let amount_out = last_trade.amount_out;

				if amount_out < min_limit.into() {
					return Err(Error::<T>::TradeLimitReached.into());
				}

				pallet_route_executor::Pallet::<T>::sell(
					origin,
					(*asset_in).into(),
					(*asset_out).into(),
					(amount_to_sell).into(),
					min_limit.into(),
					route,
				)
			}
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				slippage,
				max_limit,
				route,
			} => {
				let amount_in = Self::get_amount_in_for_buy(amount_out, route)?;

				Self::unallocate_amount(schedule_id, schedule, amount_in.into())?;

				let (estimated_amount_in, slippage_amount) =
					Self::calculate_estimated_and_slippage_amounts(*asset_in, *asset_out, *amount_out, *slippage)?;
				let max_limit_with_slippage = estimated_amount_in
					.checked_add(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;

				let max_limit = min(*max_limit, max_limit_with_slippage);
				if amount_in > max_limit.into() {
					return Err(Error::<T>::TradeLimitReached.into());
				}

				pallet_route_executor::Pallet::<T>::buy(
					origin,
					(*asset_in).into(),
					(*asset_out).into(),
					(*amount_out).into(),
					max_limit.into(),
					Self::convert_to_vec(route),
				)
			}
		}
	}

	fn replan_or_complete(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		current_blocknumber: T::BlockNumber,
	) -> DispatchResult {
		Self::deposit_event(Event::TradeExecuted {
			id: schedule_id,
			who: schedule.owner.clone(),
		});

		Self::reset_retries(schedule_id)?;

		let remaining_amount_to_use: T::Balance = RemainingAmounts::<T>::get(schedule_id)
			.ok_or(Error::<T>::InvalidState)?
			.into();
		let transaction_fee = Self::get_transaction_fee(&schedule.order)?;

		if remaining_amount_to_use < transaction_fee.into() {
			Self::complete_schedule(schedule_id, schedule);
			return Ok(());
		}

		//In buy we complete with returning leftover, in sell we sell the leftover in the next trade
		if let Order::Buy { amount_out, route, .. } = &schedule.order {
			let amount_to_unreserve: T::Balance = Self::get_amount_in_for_buy(amount_out, route)?;

			let amount_for_next_trade: T::Balance = amount_to_unreserve
				.checked_add(&(transaction_fee.into()))
				.ok_or(ArithmeticError::Overflow)?;

			if remaining_amount_to_use < amount_for_next_trade {
				Self::complete_schedule(schedule_id, schedule);
				return Ok(());
			}
		}

		let next_execution_block = current_blocknumber
			.checked_add(&schedule.period)
			.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		Self::plan_schedule_for_block(schedule.owner.clone(), next_execution_block, schedule_id)?;

		Ok(())
	}

	fn retry_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		current_blocknumber: T::BlockNumber,
	) -> DispatchResult {
		let number_of_retries = Self::retries_on_error(schedule_id).ok_or(Error::<T>::InvalidState)?;

		if number_of_retries == T::MaxNumberOfRetriesOnError::get() {
			return Err(Error::<T>::MaxRetryReached.into());
		}

		Self::increment_retries(schedule_id)?;

		let retry_multiplier = 2u32
			.checked_pow(number_of_retries.into())
			.ok_or(ArithmeticError::Overflow)?;
		let retry_delay = SHORT_ORACLE_BLOCK_PERIOD
			.checked_mul(retry_multiplier)
			.ok_or(ArithmeticError::Overflow)?;
		let next_execution_block = current_blocknumber
			.checked_add(&retry_delay.into())
			.ok_or(DispatchError::Arithmetic(ArithmeticError::Overflow))?;

		Self::plan_schedule_for_block(schedule.owner.clone(), next_execution_block, schedule_id)?;

		Ok(())
	}

	fn price_change_is_bigger_than_max_allowed(schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>) -> bool {
		let asset_a = schedule.order.get_asset_in();
		let asset_b = schedule.order.get_asset_out();
		let Some(current_price) = T::SpotPriceProvider::spot_price(asset_a, asset_b) else {
			return true;
		};

		let Ok(price_from_short_oracle) = Self::get_price_from_short_oracle(asset_a, asset_b) else {
   			return true;
		};

		let max_allowed_diff = schedule
			.order
			.get_slippage()
			.unwrap_or_else(T::MaxPriceDifferenceBetweenBlocks::get);

		let max_allowed = FixedU128::from(max_allowed_diff);

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

	fn get_amount_in_for_buy(
		amount_out: &Balance,
		route: &BoundedVec<Trade<<T as Config>::Asset>, ConstU32<5>>,
	) -> Result<T::Balance, DispatchError> {
		let route = Self::convert_to_vec(route);

		let trade_amounts =
			pallet_route_executor::Pallet::<T>::calculate_buy_trade_amounts(&route, (*amount_out).into())?;

		let first_trade = trade_amounts.last().ok_or(Error::<T>::InvalidState)?;

		Ok(first_trade.amount_in)
	}

	fn get_transaction_fee(order: &Order<<T as Config>::Asset>) -> Result<u128, DispatchError> {
		let transaction_fee = Self::convert_weight_to_fee(Self::get_trade_weight(order), order.get_asset_in())?;

		Ok(transaction_fee)
	}

	fn unallocate_amount(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		amount_to_unreserve: Balance,
	) -> DispatchResult {
		RemainingAmounts::<T>::try_mutate_exists(schedule_id, |maybe_remaining_amount| -> DispatchResult {
			let remaining_amount = maybe_remaining_amount.as_mut().ok_or(Error::<T>::InvalidState)?;

			if amount_to_unreserve > *remaining_amount {
				return Err(Error::<T>::InvalidState.into());
			};

			let new_amount = remaining_amount
				.checked_sub(amount_to_unreserve)
				.ok_or(ArithmeticError::Underflow)?;

			*remaining_amount = new_amount;

			Ok(())
		})?;

		let sold_currency = schedule.order.get_asset_in();

		let remaining_amount_if_insufficient_balance = T::Currencies::unreserve_named(
			&T::NamedReserveId::get(),
			sold_currency,
			&schedule.owner,
			amount_to_unreserve,
		);
		ensure!(remaining_amount_if_insufficient_balance == 0, Error::<T>::InvalidState);

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

	#[transactional]
	fn take_transaction_fee_from_user(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		weight_to_charge: Weight,
	) -> DispatchResult {
		let fee_currency = schedule.order.get_asset_in();
		let fee_amount_in_sold_asset = Self::convert_weight_to_fee(weight_to_charge, fee_currency)?;

		Self::unallocate_amount(schedule_id, schedule, fee_amount_in_sold_asset)?;

		T::Currencies::transfer(
			fee_currency,
			&schedule.owner,
			&T::FeeReceiver::get(),
			fee_amount_in_sold_asset,
		)?;

		Ok(())
	}

	fn terminate_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>,
		error: DispatchError,
	) {
		Self::try_unreserve_all(schedule_id, schedule);

		Self::remove_schedule_from_storages(&schedule.owner, schedule_id);

		Self::deposit_event(Event::Terminated {
			id: schedule_id,
			who: schedule.owner.clone(),
			error,
		});
	}

	fn complete_schedule(schedule_id: ScheduleId, schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>) {
		Self::try_unreserve_all(schedule_id, schedule);

		Self::remove_schedule_from_storages(&schedule.owner, schedule_id);

		Self::deposit_event(Event::Completed {
			id: schedule_id,
			who: schedule.owner.clone(),
		});
	}

	fn try_unreserve_all(schedule_id: ScheduleId, schedule: &Schedule<T::AccountId, T::Asset, T::BlockNumber>) {
		let sold_currency = schedule.order.get_asset_in();

		let Some(remaining_amount) = RemainingAmounts::<T>::get(schedule_id) else {
			//Invalid state, we ignore as we terminate the whole DCA anyway
			return;
		};

		T::Currencies::unreserve_named(
			&T::NamedReserveId::get(),
			sold_currency,
			&schedule.owner,
			remaining_amount,
		);
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

		// In a bound fashion, we search for next free block with the delays of 1 - 2 - 4 - 8 - 16.
		for retry_index in 0u32..=RETRY_TO_SEARCH_FOR_FREE_BLOCK {
			let schedule_ids = ScheduleIdsPerBlock::<T>::get(next_execution_block);
			if schedule_ids.len() < T::MaxSchedulePerBlock::get() as usize {
				return Ok(next_execution_block);
			}
			let delay_with = 2u32.checked_pow(retry_index).ok_or(ArithmeticError::Overflow)?;
			next_execution_block = next_execution_block.saturating_add(delay_with.into());
		}

		Err(Error::<T>::NoFreeBlockFound.into())
	}

	fn create_bounded_vec(
		next_schedule_id: ScheduleId,
	) -> Result<BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, DispatchError> {
		let schedule_id = vec![next_schedule_id];
		let bounded_vec: BoundedVec<ScheduleId, T::MaxSchedulePerBlock> =
			schedule_id.try_into().map_err(|_| Error::<T>::InvalidState)?;
		Ok(bounded_vec)
	}

	fn calculate_estimated_and_slippage_amounts(
		asset_a: <T as Config>::Asset,
		asset_b: <T as Config>::Asset,
		amount: Balance,
		slippage: Option<Permill>,
	) -> Result<(Balance, Balance), DispatchError> {
		let price = Self::get_price_from_last_block_oracle(asset_a, asset_b)?;

		let estimated_amount = price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

		let slippage_limit = slippage.unwrap_or_else(T::MaxPriceDifferenceBetweenBlocks::get);
		let slippage_amount = slippage_limit.mul_floor(estimated_amount);

		Ok((estimated_amount, slippage_amount))
	}

	fn convert_weight_to_fee(weight: Weight, fee_currency: T::Asset) -> Result<u128, DispatchError> {
		let fee_amount_in_native = Self::weight_to_fee(weight);
		let fee_amount_in_sold_asset = Self::convert_native_amount_to_currency(fee_currency, fee_amount_in_native)?;

		Ok(fee_amount_in_sold_asset)
	}

	fn get_trade_weight(order: &Order<<T as Config>::Asset>) -> Weight {
		match order {
			Order::Sell { .. } => <T as Config>::WeightInfo::on_initialize_with_sell_trade(),
			Order::Buy { .. } => <T as Config>::WeightInfo::on_initialize_with_buy_trade(),
		}
	}

	fn convert_native_amount_to_currency(asset_id: T::Asset, asset_amount: u128) -> Result<u128, DispatchError> {
		let amount = if asset_id == T::NativeAssetId::get() {
			asset_amount
		} else {
			let price = Self::get_price_from_last_block_oracle(asset_id, T::NativeAssetId::get())?;

			price.checked_mul_int(asset_amount).ok_or(ArithmeticError::Overflow)?
		};

		Ok(amount)
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

	fn remove_schedule_from_storages(owner: &T::AccountId, schedule_id: ScheduleId) {
		Schedules::<T>::remove(schedule_id);
		ScheduleOwnership::<T>::remove(owner, schedule_id);
		RemainingAmounts::<T>::remove(schedule_id);
		RetriesOnError::<T>::remove(schedule_id);
	}

	fn convert_to_vec(route: &BoundedVec<Trade<T::Asset>, ConstU32<5>>) -> Vec<Trade<T::AssetId>> {
		route
			.iter()
			.map(|t| {
				let trade: Trade<<T as pallet_route_executor::Config>::AssetId> = Trade {
					pool: match t.pool {
						PoolType::XYK => PoolType::XYK,
						PoolType::LBP => PoolType::LBP,
						PoolType::Stableswap(asset_id) => PoolType::Stableswap(asset_id.into()),
						PoolType::Omnipool => PoolType::Omnipool,
					},
					asset_in: t.asset_in.into(),
					asset_out: t.asset_out.into(),
				};
				trade
			})
			.collect()
	}
}

pub trait RelayChainBlockHashProvider {
	fn parent_hash() -> Option<Hash>;
}

pub trait RandomnessProvider {
	type Error;
	fn generator() -> Result<StdRng, Self::Error>;
}

impl<T: Config> RandomnessProvider for Pallet<T> {
	type Error = DispatchError;

	fn generator() -> Result<StdRng, Self::Error> {
		let hash_value = T::RelayChainBlockHashProvider::parent_hash().ok_or(Error::<T>::NoParentHashFound)?;
		let hash_bytes = hash_value.as_fixed_bytes();
		let mut seed_arr = [0u8; 8];
		let max_len = hash_bytes.len().min(seed_arr.len()); //We ensure that we don't copy more bytes, preventing potential panics
		seed_arr[..max_len].copy_from_slice(&hash_bytes[..max_len]);
		let seed = u64::from_le_bytes(seed_arr);
		Ok(rand::rngs::StdRng::seed_from_u64(seed))
	}
}
