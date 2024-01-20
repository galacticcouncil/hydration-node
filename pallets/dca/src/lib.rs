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
//! The DCA pallet provides dollar-cost averaging functionality, allowing users to perform repeating orders.
//! This pallet enables the creation, execution and termination of schedules.
//!
//! ## Creating a Schedule
//!
//! Users can create a DCA schedule, which is planned to execute in a specific block.
//! If the block is not specified, the execution is planned for the next block.
//! In case the given block is full, the execution will be scheduled for the subsequent block.
//!
//! Upon creating a schedule, the user specifies a budget (`total_amount`) that will be reserved.
//! The currency of this reservation is the sold (`amount_in`) currency.
//!
//! ### Executing a Schedule
//!
//! Orders are executed during block initialization and are sorted based on randomness derived from the relay chain block hash.
//!
//! A trade is executed and replanned as long as there is remaining budget from the initial allocation.
//!
//! For both successful and failed trades, a fee is deducted from the schedule owner.
//! The fee is deducted in the sold (`amount_in`) currency.
//!
//! A trade can fail due to two main reasons:
//!
//! 1. Price Stability Error: If the price difference between the short oracle price and the current price
//! exceeds the specified threshold. The user can customize this threshold,
//! or the default value from the pallet configuration will be used.
//! 2. Slippage Error: If the minimum amount out (sell) or maximum amount in (buy) slippage limits are not reached.
//! These limits are calculated based on the last block's oracle price and the user-specified slippage.
//! If no slippage is specified, the default value from the pallet configuration will be used.
//!
//! If a trade fails due to these errors, the trade will be retried.
//! If the number of retries reaches the maximum number of retries, the schedule will be permanently terminated.
//! In the case of a successful trade, the retry counter is reset.
//!
//! If a trade fails due to other types of errors, the order is terminated without any retry logic.
//!
//! ## Terminating a Schedule
//!
//! Both users and technical origin can terminate a DCA schedule. However, users can only terminate schedules that they own.
//!
//! Once a schedule is terminated, it is completely and permanently removed from the blockchain.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::traits::DefensiveOption;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{Get, Len},
	transactional,
	weights::WeightToFee as FrameSupportWeight,
};
use frame_system::{
	ensure_signed,
	pallet_prelude::{BlockNumberFor, OriginFor},
	Origin,
};
use hydradx_adapters::RelayChainBlockHashProvider;
use hydradx_traits::router::{inverse_route, RouteProvider};
use hydradx_traits::router::{AmmTradeWeights, AmountInAndOut, RouterT, Trade};
use hydradx_traits::NativePriceOracle;
use hydradx_traits::OraclePeriod;
use hydradx_traits::PriceOracle;
use orml_traits::{arithmetic::CheckedAdd, MultiCurrency, NamedMultiReservableCurrency};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::traits::{CheckedMul, One};
use sp_runtime::{
	traits::{BlockNumberProvider, Saturating},
	ArithmeticError, BoundedVec, DispatchError, FixedPointNumber, FixedU128, Permill, Rounding,
};

use sp_std::vec::Vec;
use sp_std::{cmp::min, vec};

#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::*;

pub const SHORT_ORACLE_BLOCK_PERIOD: u32 = 10;
pub const MAX_NUMBER_OF_RETRY_FOR_RESCHEDULING: u32 = 10;
pub const FEE_MULTIPLIER_FOR_MIN_TRADE_LIMIT: Balance = 20;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use hydra_dx_math::ema::EmaPrice;
	use hydradx_traits::{NativePriceOracle, PriceOracle};
	use orml_traits::NamedMultiReservableCurrency;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_initialize(current_blocknumber: BlockNumberFor<T>) -> Weight {
			let mut weight = <T as pallet::Config>::WeightInfo::on_initialize_with_empty_block();

			let mut randomness_generator = Self::get_randomness_generator(current_blocknumber, None);

			let mut schedule_ids: Vec<ScheduleId> = ScheduleIdsPerBlock::<T>::take(current_blocknumber).to_vec();

			schedule_ids.sort_by_cached_key(|_| randomness_generator.gen::<u32>());
			for schedule_id in schedule_ids {
				Self::deposit_event(Event::ExecutionStarted {
					id: schedule_id,
					block: current_blocknumber,
				});

				let Some(schedule) = Schedules::<T>::get(schedule_id) else {
					//We cant terminate here as there is no schedule information to do so
					continue;
				};

				let weight_for_single_execution = Self::get_trade_weight(&schedule.order);
				weight.saturating_accrue(weight_for_single_execution);

				if let Err(e) = Self::prepare_schedule(
					current_blocknumber,
					weight_for_single_execution,
					schedule_id,
					&schedule,
					&mut randomness_generator,
				) {
					if e != Error::<T>::PriceUnstable.into() {
						Self::terminate_schedule(schedule_id, &schedule, e);
					};
					continue;
				};

				match Self::execute_trade(schedule_id, &schedule) {
					Ok(amounts) => {
						if let Err(err) = Self::replan_or_complete(
							schedule_id,
							&schedule,
							current_blocknumber,
							amounts,
							&mut randomness_generator,
						) {
							Self::terminate_schedule(schedule_id, &schedule, err);
						}
					}
					Err(error) => {
						Self::deposit_event(Event::TradeFailed {
							id: schedule_id,
							who: schedule.owner.clone(),
							error,
						});

						if error != Error::<T>::TradeLimitReached.into()
							&& error != Error::<T>::SlippageLimitReached.into()
						{
							Self::terminate_schedule(schedule_id, &schedule, error);
						} else if let Err(retry_error) =
							Self::retry_schedule(schedule_id, &schedule, current_blocknumber, &mut randomness_generator)
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
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset id type
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen;

		/// Origin able to terminate schedules
		type TechnicalOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		///For named-reserving user's assets
		type Currencies: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::AssetId,
			Balance = Balance,
		>;

		///Relay chain block hash provider for randomness
		type RelayChainBlockHashProvider: RelayChainBlockHashProvider;

		///Randomness provider to be used to sort the DCA schedules when they are executed in a block
		type RandomnessProvider: RandomnessProvider;

		///Oracle price provider to get the price between two assets
		type OraclePriceProvider: PriceOracle<Self::AssetId, Price = EmaPrice>;

		///Native price provider to get the price of assets that are accepted as fees
		type NativePriceOracle: NativePriceOracle<Self::AssetId, EmaPrice>;

		///Router implementation
		type RouteExecutor: RouterT<
			Self::RuntimeOrigin,
			Self::AssetId,
			Balance,
			Trade<Self::AssetId>,
			AmountInAndOut<Balance>,
		>;

		///Spot price provider to get the current price between two asset
		type RouteProvider: RouteProvider<Self::AssetId>;

		///Max price difference allowed between blocks
		#[pallet::constant]
		type MaxPriceDifferenceBetweenBlocks: Get<Permill>;

		///The number of max schedules to be executed per block
		#[pallet::constant]
		type MaxSchedulePerBlock: Get<u32>;

		///The number of max retries in case of trade limit error
		#[pallet::constant]
		type MaxNumberOfRetriesOnError: Get<u8>;

		/// Minimum trading limit for a single trade
		#[pallet::constant]
		type MinimumTradingLimit: Get<Balance>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

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

		/// AMMs trade weight information.
		type AmmTradeWeights: AmmTradeWeights<Trade<Self::AssetId>>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///The DCA execution is started
		ExecutionStarted { id: ScheduleId, block: BlockNumberFor<T> },
		///The DCA is scheduled for next execution
		Scheduled {
			id: ScheduleId,
			who: T::AccountId,
			period: BlockNumberFor<T>,
			total_amount: Balance,
			order: Order<T::AssetId>,
		},
		///The DCA is planned for blocknumber
		ExecutionPlanned {
			id: ScheduleId,
			who: T::AccountId,
			block: BlockNumberFor<T>,
		},
		///The DCA trade is successfully executed
		TradeExecuted {
			id: ScheduleId,
			who: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
		},
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
		RandomnessGenerationFailed {
			block: BlockNumberFor<T>,
			error: DispatchError,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		///Schedule not exist
		ScheduleNotFound,
		///The min trade amount is not reached
		MinTradeAmountNotReached,
		///Forbidden as the user is not the owner of the schedule
		Forbidden,
		///The next execution block number is not in the future
		BlockNumberIsNotInFuture,
		///Price is unstable as price change from oracle data is bigger than max allowed
		PriceUnstable,
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
		///Absolutely trade limit reached reached, leading to retry
		TradeLimitReached,
		///Slippage limit calculated from oracle is reached, leading to retry
		SlippageLimitReached,
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
		StorageMap<_, Blake2_128Concat, ScheduleId, Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>, OptionQuery>;

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
	pub type RetriesOnError<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u8, ValueQuery>;

	/// Keep tracking of the schedule ids to be executed in the block
	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates a new DCA (Dollar-Cost Averaging) schedule and plans the next execution
		/// for the specified block.
		///
		/// If the block is not specified, the execution is planned for the next block.
		/// If the given block is full, the execution will be planned in the subsequent block.
		///
		/// Once the schedule is created, the specified `total_amount` will be reserved for DCA.
		/// The reservation currency will be the `amount_in` currency of the order.
		///
		/// Trades are executed as long as there is budget remaining
		/// from the initial `total_amount` allocation.
		///
		/// If a trade fails due to slippage limit or price stability errors, it will be retried.
		/// If the number of retries reaches the maximum allowed,
		/// the schedule will be terminated permanently.
		/// In the case of a successful trade, the retry counter is reset.
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule`: schedule details
		/// - `start_execution_block`: start execution block for the schedule
		///
		/// Emits `Scheduled` and `ExecutionPlanned` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::schedule()
			+ <T as Config>::AmmTradeWeights::calculate_buy_trade_amounts_weight(&schedule.order.get_route_or_default::<T::RouteProvider>()))]
		#[transactional]
		pub fn schedule(
			origin: OriginFor<T>,
			schedule: Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			ensure!(who == schedule.owner, Error::<T>::Forbidden);

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
				Order::Buy { amount_out, .. } => {
					let route = schedule.order.get_route_or_default::<T::RouteProvider>();
					Self::get_amount_in_for_buy(&amount_out, &route)?
				}
			};
			let min_trade_amount_in_from_fee = transaction_fee.saturating_mul(FEE_MULTIPLIER_FOR_MIN_TRADE_LIMIT);
			ensure!(
				amount_in >= min_trade_amount_in_from_fee,
				Error::<T>::MinTradeAmountNotReached
			);
			ensure!(
				amount_in >= T::MinimumTradingLimit::get(),
				Error::<T>::MinTradeAmountNotReached
			);

			let amount_in_with_transaction_fee = amount_in
				.checked_add(transaction_fee)
				.ok_or(ArithmeticError::Overflow)?;
			ensure!(
				amount_in_with_transaction_fee <= schedule.total_amount,
				Error::<T>::BudgetTooLow
			);

			let next_schedule_id =
				ScheduleIdSequencer::<T>::try_mutate(|current_id| -> Result<ScheduleId, DispatchError> {
					let schedule_id = *current_id;
					*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;
					Ok(schedule_id)
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

			let blocknumber_for_first_schedule_execution = Self::get_next_execution_block(start_execution_block)?;

			let mut randomness_generator = Self::get_randomness_generator(
				frame_system::Pallet::<T>::current_block_number(),
				Some(next_schedule_id),
			);
			Self::plan_schedule_for_block(
				&who,
				blocknumber_for_first_schedule_execution,
				next_schedule_id,
				&mut randomness_generator,
			)?;

			Self::deposit_event(Event::Scheduled {
				id: next_schedule_id,
				who,
				period: schedule.period,
				total_amount: schedule.total_amount,
				order: schedule.order,
			});

			Ok(())
		}

		/// Terminates a DCA schedule and remove it completely from the chain.
		///
		/// This can be called by both schedule owner or the configured `T::TechnicalOrigin`
		///
		/// Parameters:
		/// - `origin`: schedule owner
		/// - `schedule_id`: schedule id
		/// - `next_execution_block`: block number where the schedule is planned.
		///
		/// Emits `Terminated` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::terminate())]
		#[transactional]
		pub fn terminate(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotFound)?;

			if T::TechnicalOrigin::ensure_origin(origin.clone()).is_err() {
				let who = ensure_signed(origin)?;
				ensure!(who == schedule.owner, Error::<T>::Forbidden);
			}

			Self::try_unreserve_all(schedule_id, &schedule);

			let next_execution_block = Self::get_next_execution_block(next_execution_block)?;

			//Remove schedule id from next execution block
			ScheduleIdsPerBlock::<T>::try_mutate_exists(
				next_execution_block,
				|maybe_schedule_ids| -> DispatchResult {
					let schedule_ids = maybe_schedule_ids.as_mut().ok_or(Error::<T>::ScheduleNotFound)?;

					let index = schedule_ids
						.binary_search(&schedule_id)
						.map_err(|_| Error::<T>::ScheduleNotFound)?;

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

impl<T: Config> Pallet<T> {
	fn get_randomness_generator(current_blocknumber: BlockNumberFor<T>, salt: Option<u32>) -> StdRng {
		match T::RandomnessProvider::generator(salt) {
			Ok(generator) => generator,
			Err(err) => {
				Self::deposit_event(Event::RandomnessGenerationFailed {
					block: current_blocknumber,
					error: err,
				});
				rand::rngs::StdRng::seed_from_u64(0)
			}
		}
	}

	fn get_next_execution_block(
		start_execution_block: Option<BlockNumberFor<T>>,
	) -> Result<BlockNumberFor<T>, DispatchError> {
		let blocknumber_for_first_schedule_execution = match start_execution_block {
			Some(blocknumber) => Ok(blocknumber),
			None => {
				let current_block_number = frame_system::Pallet::<T>::current_block_number();
				let next_block_number = current_block_number
					.checked_add(&BlockNumberFor::<T>::one())
					.ok_or(ArithmeticError::Overflow)?;

				Ok::<BlockNumberFor<T>, ArithmeticError>(next_block_number)
			}
		}?;

		Ok(blocknumber_for_first_schedule_execution)
	}

	fn prepare_schedule(
		current_blocknumber: BlockNumberFor<T>,
		weight_for_dca_execution: Weight,
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
		randomness_generator: &mut StdRng,
	) -> DispatchResult {
		Self::take_transaction_fee_from_user(schedule_id, schedule, weight_for_dca_execution)?;

		if Self::is_price_unstable(schedule) {
			Self::deposit_event(Event::TradeFailed {
				id: schedule_id,
				who: schedule.owner.clone(),
				error: Error::<T>::PriceUnstable.into(),
			});
			Self::retry_schedule(schedule_id, schedule, current_blocknumber, randomness_generator)?;

			return Err(Error::<T>::PriceUnstable.into());
		}

		Ok(())
	}

	#[transactional]
	pub fn execute_trade(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
	) -> Result<AmountInAndOut<Balance>, DispatchError> {
		let origin: OriginFor<T> = Origin::<T>::Signed(schedule.owner.clone()).into();

		match &schedule.order {
			Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_amount_out,
				..
			} => {
				let route = &schedule.order.get_route_or_default::<T::RouteProvider>();
				let remaining_amount =
					RemainingAmounts::<T>::get(schedule_id).defensive_ok_or(Error::<T>::InvalidState)?;
				let amount_to_sell = min(remaining_amount, *amount_in);

				Self::unallocate_amount(schedule_id, schedule, amount_to_sell)?;

				let route_for_slippage = inverse_route(route.to_vec());
				let (estimated_amount_out, slippage_amount) =
					Self::calculate_last_block_slippage(&route_for_slippage, amount_to_sell, schedule.slippage)?;
				let last_block_slippage_min_limit = estimated_amount_out
					.checked_sub(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;

				let trade_amounts = T::RouteExecutor::calculate_sell_trade_amounts(route, amount_to_sell)?;
				let last_trade = trade_amounts.last().defensive_ok_or(Error::<T>::InvalidState)?;
				let amount_out = last_trade.amount_out;

				if *min_amount_out > last_block_slippage_min_limit {
					ensure!(amount_out >= *min_amount_out, Error::<T>::TradeLimitReached);
				} else {
					ensure!(
						amount_out >= last_block_slippage_min_limit,
						Error::<T>::SlippageLimitReached
					);
				};

				T::RouteExecutor::sell(
					origin,
					*asset_in,
					*asset_out,
					amount_to_sell,
					amount_out,
					route.to_vec(),
				)?;

				Ok(AmountInAndOut {
					amount_in: amount_to_sell,
					amount_out,
				})
			}
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_amount_in,
				..
			} => {
				let route = schedule.order.get_route_or_default::<T::RouteProvider>();
				let amount_in = Self::get_amount_in_for_buy(amount_out, &route)?;

				Self::unallocate_amount(schedule_id, schedule, amount_in)?;

				let (estimated_amount_in, slippage_amount) =
					Self::calculate_last_block_slippage(&route, *amount_out, schedule.slippage)?;
				let last_block_slippage_max_limit = estimated_amount_in
					.checked_add(slippage_amount)
					.ok_or(ArithmeticError::Overflow)?;

				if *max_amount_in < last_block_slippage_max_limit {
					ensure!(amount_in <= *max_amount_in, Error::<T>::TradeLimitReached);
				} else {
					ensure!(
						amount_in <= last_block_slippage_max_limit,
						Error::<T>::SlippageLimitReached
					);
				};

				T::RouteExecutor::buy(origin, *asset_in, *asset_out, *amount_out, amount_in, route.to_vec())?;

				Ok(AmountInAndOut {
					amount_in,
					amount_out: *amount_out,
				})
			}
		}
	}

	fn replan_or_complete(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
		current_blocknumber: BlockNumberFor<T>,
		amounts: AmountInAndOut<Balance>,
		randomness_generator: &mut StdRng,
	) -> DispatchResult {
		Self::deposit_event(Event::TradeExecuted {
			id: schedule_id,
			who: schedule.owner.clone(),
			amount_in: amounts.amount_in,
			amount_out: amounts.amount_out,
		});

		RetriesOnError::<T>::remove(schedule_id);

		let remaining_amount: Balance =
			RemainingAmounts::<T>::get(schedule_id).defensive_ok_or(Error::<T>::InvalidState)?;
		let transaction_fee = Self::get_transaction_fee(&schedule.order)?;
		let min_amount_for_replanning = transaction_fee.saturating_mul(FEE_MULTIPLIER_FOR_MIN_TRADE_LIMIT);
		if remaining_amount < min_amount_for_replanning || remaining_amount < T::MinimumTradingLimit::get() {
			Self::complete_schedule(schedule_id, schedule);
			return Ok(());
		}

		//In buy we complete with returning leftover, in sell we sell the leftover in the next trade
		if let Order::Buy { amount_out, .. } = &schedule.order {
			let route = schedule.order.get_route_or_default::<T::RouteProvider>();
			let amount_to_unreserve: Balance = Self::get_amount_in_for_buy(amount_out, &route)?;

			let amount_for_next_trade: Balance = amount_to_unreserve
				.checked_add(transaction_fee)
				.ok_or(ArithmeticError::Overflow)?;

			if remaining_amount < amount_for_next_trade {
				Self::complete_schedule(schedule_id, schedule);
				return Ok(());
			}
		}

		let next_execution_block = current_blocknumber
			.checked_add(&schedule.period)
			.ok_or(ArithmeticError::Overflow)?;

		Self::plan_schedule_for_block(&schedule.owner, next_execution_block, schedule_id, randomness_generator)?;

		Ok(())
	}

	fn retry_schedule(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
		current_blocknumber: BlockNumberFor<T>,
		randomness_generator: &mut StdRng,
	) -> DispatchResult {
		let number_of_retries = Self::retries_on_error(schedule_id);

		let max_retries = schedule.max_retries.unwrap_or_else(T::MaxNumberOfRetriesOnError::get);
		ensure!(number_of_retries < max_retries, Error::<T>::MaxRetryReached);

		RetriesOnError::<T>::mutate(schedule_id, |retry| -> DispatchResult {
			retry.saturating_inc();
			Ok(())
		})?;

		let retry_multiplier = 2u32
			.checked_pow(number_of_retries.into())
			.ok_or(ArithmeticError::Overflow)?;
		let retry_delay = SHORT_ORACLE_BLOCK_PERIOD
			.checked_mul(retry_multiplier)
			.ok_or(ArithmeticError::Overflow)?;
		let next_execution_block = current_blocknumber
			.checked_add(&retry_delay.into())
			.ok_or(ArithmeticError::Overflow)?;

		Self::plan_schedule_for_block(&schedule.owner, next_execution_block, schedule_id, randomness_generator)?;

		Ok(())
	}

	fn is_price_unstable(schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>) -> bool {
		let route = &schedule.order.get_route_or_default::<T::RouteProvider>();

		let Ok(last_block_price) = Self::get_price_from_last_block_oracle(route) else {
			return true;
		};

		let Ok(price_from_short_oracle) = Self::get_price_from_short_oracle(route) else {
   			return true;
		};

		let max_allowed_diff = schedule
			.stability_threshold
			.unwrap_or_else(T::MaxPriceDifferenceBetweenBlocks::get);

		let max_allowed = FixedU128::from(max_allowed_diff);

		let Some(price_sum) = last_block_price
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

		let diff = if last_block_price > price_from_short_oracle {
			last_block_price.saturating_sub(price_from_short_oracle)
		} else {
			price_from_short_oracle.saturating_sub(last_block_price)
		};

		let Some(diff) = diff.checked_mul(&FixedU128::from(2)) else {
			return true;
		};

		diff > max_allowed_difference
	}

	fn get_amount_in_for_buy(amount_out: &Balance, route: &[Trade<T::AssetId>]) -> Result<Balance, DispatchError> {
		let trade_amounts = T::RouteExecutor::calculate_buy_trade_amounts(route, *amount_out)?;

		let first_trade = trade_amounts.last().defensive_ok_or(Error::<T>::InvalidState)?;

		Ok(first_trade.amount_in)
	}

	pub fn get_transaction_fee(order: &Order<T::AssetId>) -> Result<Balance, DispatchError> {
		Self::convert_weight_to_fee(Self::get_trade_weight(order), order.get_asset_in())
	}

	fn unallocate_amount(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
		amount_to_unreserve: Balance,
	) -> DispatchResult {
		RemainingAmounts::<T>::try_mutate_exists(schedule_id, |maybe_remaining_amount| -> DispatchResult {
			let remaining_amount = maybe_remaining_amount
				.as_mut()
				.defensive_ok_or(Error::<T>::InvalidState)?;

			ensure!(amount_to_unreserve <= *remaining_amount, Error::<T>::InvalidState);

			*remaining_amount = remaining_amount
				.checked_sub(amount_to_unreserve)
				.ok_or(ArithmeticError::Underflow)?;

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

	#[transactional]
	fn take_transaction_fee_from_user(
		schedule_id: ScheduleId,
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
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
		schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>,
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

	fn complete_schedule(schedule_id: ScheduleId, schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>) {
		Self::try_unreserve_all(schedule_id, schedule);

		Self::remove_schedule_from_storages(&schedule.owner, schedule_id);

		Self::deposit_event(Event::Completed {
			id: schedule_id,
			who: schedule.owner.clone(),
		});
	}

	fn try_unreserve_all(schedule_id: ScheduleId, schedule: &Schedule<T::AccountId, T::AssetId, BlockNumberFor<T>>) {
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
		who: &T::AccountId,
		blocknumber: BlockNumberFor<T>,
		schedule_id: ScheduleId,
		randomness_generator: &mut StdRng,
	) -> DispatchResult {
		let current_block_number = frame_system::Pallet::<T>::current_block_number();
		ensure!(blocknumber > current_block_number, Error::<T>::BlockNumberIsNotInFuture);

		let next_free_block = Self::find_next_free_block(blocknumber, randomness_generator)?;

		ScheduleIdsPerBlock::<T>::try_mutate(next_free_block, |schedule_ids| -> DispatchResult {
			schedule_ids
				.try_push(schedule_id)
				.map_err(|_| Error::<T>::InvalidState)?;
			Ok(())
		})?;

		Self::deposit_event(Event::ExecutionPlanned {
			id: schedule_id,
			who: who.clone(),
			block: next_free_block,
		});
		Ok(())
	}

	fn find_next_free_block(
		blocknumber: BlockNumberFor<T>,
		randomness_generator: &mut StdRng,
	) -> Result<BlockNumberFor<T>, DispatchError> {
		let mut next_execution_block = blocknumber;

		for i in 0..=MAX_NUMBER_OF_RETRY_FOR_RESCHEDULING {
			let schedule_ids = ScheduleIdsPerBlock::<T>::get(next_execution_block);
			if schedule_ids.len() < T::MaxSchedulePerBlock::get() as usize {
				return Ok(next_execution_block);
			}

			let lower_bound = 2u32.saturating_pow(i);
			let upper_bound = 2u32.saturating_pow(i.saturating_add(1)).saturating_sub(1);

			let delay_with = randomness_generator.gen_range(lower_bound..=upper_bound);
			next_execution_block = next_execution_block.saturating_add(delay_with.into());
		}

		Err(Error::<T>::NoFreeBlockFound.into())
	}

	fn calculate_last_block_slippage(
		route: &[Trade<T::AssetId>],
		amount: Balance,
		slippage: Option<Permill>,
	) -> Result<(Balance, Balance), DispatchError> {
		let price = Self::get_price_from_last_block_oracle(route)?;

		let estimated_amount = price.checked_mul_int(amount).ok_or(ArithmeticError::Overflow)?;

		let slippage_limit = slippage.unwrap_or_else(T::MaxPriceDifferenceBetweenBlocks::get);
		let slippage_amount = slippage_limit.mul_floor(estimated_amount);

		Ok((estimated_amount, slippage_amount))
	}

	fn convert_weight_to_fee(weight: Weight, fee_currency: T::AssetId) -> Result<Balance, DispatchError> {
		let fee_amount_in_native = Self::weight_to_fee(weight);
		let fee_amount_in_sold_asset = Self::convert_native_amount_to_currency(fee_currency, fee_amount_in_native)?;

		Ok(fee_amount_in_sold_asset)
	}

	// returns DCA overhead weight + router execution weight
	fn get_trade_weight(order: &Order<T::AssetId>) -> Weight {
		let route = &order.get_route_or_default::<T::RouteProvider>();
		match order {
			Order::Sell { .. } => <T as Config>::WeightInfo::on_initialize_with_sell_trade()
				.saturating_add(T::AmmTradeWeights::sell_and_calculate_sell_trade_amounts_weight(route)),
			Order::Buy { .. } => <T as Config>::WeightInfo::on_initialize_with_buy_trade()
				.saturating_add(T::AmmTradeWeights::buy_and_calculate_buy_trade_amounts_weight(route)),
		}
	}

	fn convert_native_amount_to_currency(
		asset_id: T::AssetId,
		asset_amount: Balance,
	) -> Result<Balance, DispatchError> {
		let amount = if asset_id == T::NativeAssetId::get() {
			asset_amount
		} else {
			let price = T::NativePriceOracle::price(asset_id).ok_or(Error::<T>::CalculatingPriceError)?;

			multiply_by_rational_with_rounding(asset_amount, price.n, price.d, Rounding::Up)
				.ok_or(ArithmeticError::Overflow)?
		};

		Ok(amount)
	}

	fn get_price_from_last_block_oracle(route: &[Trade<T::AssetId>]) -> Result<FixedU128, DispatchError> {
		let price =
			T::OraclePriceProvider::price(route, OraclePeriod::LastBlock).ok_or(Error::<T>::CalculatingPriceError)?;

		let price_from_rational =
			FixedU128::checked_from_rational(price.n, price.d).ok_or(ArithmeticError::Overflow)?;

		Ok(price_from_rational)
	}

	fn get_price_from_short_oracle(route: &[Trade<T::AssetId>]) -> Result<FixedU128, DispatchError> {
		let price =
			T::OraclePriceProvider::price(route, OraclePeriod::Short).ok_or(Error::<T>::CalculatingPriceError)?;

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
}

pub trait RandomnessProvider {
	fn generator(salt: Option<u32>) -> Result<StdRng, DispatchError>;
}

impl<T: Config> RandomnessProvider for Pallet<T> {
	fn generator(salt: Option<u32>) -> Result<StdRng, DispatchError> {
		let hash_value = T::RelayChainBlockHashProvider::parent_hash().ok_or(Error::<T>::NoParentHashFound)?;
		let hash_bytes = hash_value.as_fixed_bytes();
		let mut seed_arr = [0u8; 8];
		let max_len = hash_bytes.len().min(seed_arr.len()); //We ensure that we don't copy more bytes, preventing potential panics
		seed_arr[..max_len].copy_from_slice(&hash_bytes[..max_len]);

		let seed = match salt {
			Some(salt) => u64::from_le_bytes(seed_arr).wrapping_add(salt.into()),
			None => u64::from_le_bytes(seed_arr),
		};

		Ok(rand::rngs::StdRng::seed_from_u64(seed))
	}
}
