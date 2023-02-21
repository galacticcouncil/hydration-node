// This file is part of pallet-dca.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

use codec::MaxEncodedLen;
use frame_support::ensure;
use frame_support::pallet_prelude::*;
use frame_support::traits::{Get, Len};
use frame_support::transactional;
use frame_support::weights::WeightToFee as FrameSupportWeight;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use frame_system::Origin;
use orml_traits::arithmetic::CheckedAdd;
use orml_traits::MultiCurrency;
use orml_traits::NamedMultiReservableCurrency;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use scale_info::TypeInfo;
use sp_runtime::traits::BlockNumberProvider;
use sp_runtime::traits::Saturating;
use sp_runtime::ArithmeticError;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::{traits::BlakeTwo256, traits::Hash};
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::cmp::max;
use sp_std::cmp::min;
use sp_std::vec;

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

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use orml_traits::{MultiReservableCurrency, NamedMultiReservableCurrency};
	use pallet_transaction_multi_payment::TransactionMultiPaymentDataProvider;

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

				match maybe_schedules {
					Some(mut schedules) => {
						schedules.sort_by_key(|_| random_generator.gen::<u32>());
						for schedule_id in schedules {
							Self::execute_schedule(current_blocknumber, &mut weight, schedule_id);
						}
					}
					None => (),
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

		///Account currency provider to get the set currency of the user
		type AccountCurrencyAndPriceProvider: TransactionMultiPaymentDataProvider<
			Self::AccountId,
			Self::Asset,
			FixedU128,
		>;

		///For named-reserving user's assets
		type Currency: NamedMultiReservableCurrency<Self::AccountId, ReserveIdentifier = NamedReserveIdentifier>;

		///Price provider to get the price of the native asset comparing to other assets
		//TODO: Replace it to price provider by Oracle once Oracle is ready
		type PriceProvider: PriceProvider<Self::Asset, Price = FixedU128>;

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
		///The DCA is suspended
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
		///The user is not the owner of the schedule
		NotScheduleOwner,
		///The next execution block number is not in the future
		BlockNumberIsNotInFuture,
		///There is not planned execution on the given block
		NoPlannedExecutionFoundOnBlock,
		///Schedule execution is not planned on block
		ScheduleExecutionNotPlannedOnBlock,
		///The schedule must be suspended when there is not execution block specified by the using during termination of a schedule
		ScheduleMustBeSuspended,
		///Error occurred when calculating spot price
		CalculatingSpotPriceError,
		///Invalid storage state: No schedule ids planned in block
		NoScheduleIdsPlannedInBlock,
		///No remaining occurrences found for schedule
		NoRemainingRecurrencesFound,
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
		StorageMap<_, Blake2_128Concat, ScheduleId, Schedule<T::Asset, BlockNumberFor<T>>, OptionQuery>;

	/// Storing schedule ownership
	#[pallet::storage]
	#[pallet::getter(fn owner_of)]
	pub type ScheduleOwnership<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, T::AccountId, OptionQuery>;

	/// Storing suspended schedules
	#[pallet::storage]
	#[pallet::getter(fn suspended)]
	pub type Suspended<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, (), OptionQuery>;

	/// Keep tracking the ramaining recurrences of fixed DCA schedules
	#[pallet::storage]
	#[pallet::getter(fn remaining_recurrences)]
	pub type RemainingRecurrences<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u32, OptionQuery>;

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
			schedule: Schedule<T::Asset, BlockNumberFor<T>>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_next_blocknumber_bigger_than_current_block(start_execution_block)?;
			Self::ensure_that_total_amount_is_bigger_than_storage_bond(&schedule)?;

			let next_schedule_id = Self::get_next_schedule_id()?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			ScheduleOwnership::<T>::insert(next_schedule_id, who.clone());

			let blocknumber_for_first_schedule_execution =
				start_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
			Self::plan_schedule_for_block(blocknumber_for_first_schedule_execution, next_schedule_id)?;

			let currency_for_reserve = match schedule.order {
				Order::Buy { asset_in, .. } => asset_in,
				Order::Sell { asset_in, .. } => asset_in,
			};

			ensure!(
				T::Currency::can_reserve(
					currency_for_reserve.into(),
					&who,
					schedule.total_amount.into()
				),
				Error::<T>::InsufficientBalanceForTotalAmount
			);

			T::Currency::reserve_named(
				&reserve_identifier(next_schedule_id),
				currency_for_reserve.into(),
				&who,
				schedule.total_amount.into(),
			)?;

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
			Self::ensure_that_next_blocknumber_bigger_than_current_block(next_execution_block)?;
			Self::ensure_that_schedule_is_suspended(schedule_id)?;

			let next_execution_block = next_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
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

		/// Terminate a DCA schedule with completely removing it from the chain.
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
			Self::remove_schedule_from_storages(schedule_id);

			Self::deposit_event(Event::Terminated {
				id: schedule_id,
				who: who.clone(),
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
	fn execute_schedule(current_blocknumber: T::BlockNumber, weight: &mut u64, schedule_id: ScheduleId) {
		let schedule = exec_or_return_if_none!(Schedules::<T>::get(schedule_id));
		let owner = exec_or_return_if_none!(ScheduleOwnership::<T>::get(schedule_id));
		let origin: OriginFor<T> = Origin::<T>::Signed(owner.clone()).into();

		let dca_reserve_identifier = &reserve_identifier(schedule_id);
		let sold_currency = Self::sold_currency(&schedule.order);
		let amount_to_unreserve = exec_or_return_if_err!(Self::amount_to_unreserve(&schedule.order));

		let remaining_named_reserve_balance =
			T::Currency::reserved_balance_named(&dca_reserve_identifier, sold_currency.into(), &owner);

		T::Currency::unreserve_named(
			&dca_reserve_identifier,
			sold_currency.into(),
			&owner,
			amount_to_unreserve.into(),
		);

		if remaining_named_reserve_balance < amount_to_unreserve.into() {
			Self::complete_dca(schedule_id, &owner);
			return;
		}

		exec_or_return_if_err!(Self::take_transaction_fee_from_user(&owner, &schedule.order));
		let trade_result = Self::execute_trade(origin, &schedule.order);
		*weight += Self::get_execute_schedule_weight();

		match trade_result {
			Ok(_) => {
				let blocknumber_for_schedule =
					exec_or_return_if_none!(current_blocknumber.checked_add(&schedule.period.into()));

				exec_or_return_if_err!(Self::plan_schedule_for_block(blocknumber_for_schedule, schedule_id));
			}
			_ => {
				exec_or_return_if_err!(Self::suspend_schedule(&owner, schedule_id));
			}
		}
	}

	fn amount_to_unreserve(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		let amount_to_sell = match order {
			Order::Sell { amount_in, .. } => Ok(*amount_in),
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_limit,
				..
			} => {
				let max_limit_from_spot_price = Self::get_max_limit_with_slippage(&asset_in, &asset_out, &amount_out)?;
				let max_limit = max(max_limit, &max_limit_from_spot_price);

				let fee_amount_in_sold_asset = Self::get_transaction_fee(*asset_in)?;
				let amount_to_sell_plus_fee = max_limit
					.checked_add(&fee_amount_in_sold_asset)
					.ok_or(ArithmeticError::Overflow)?;
				Ok(amount_to_sell_plus_fee)
			}
		};

		amount_to_sell
	}

	fn get_storage_bond_in_sold_currency(order: &Order<<T as Config>::Asset>) -> Result<Balance, DispatchError> {
		let sold_currency = Self::sold_currency(order);
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
		let fee_currency = Self::sold_currency(&order);

		let fee_amount_in_sold_asset = Self::get_transaction_fee(fee_currency)?;

		T::Currency::transfer(
			fee_currency.into(),
			&owner,
			&T::FeeReceiver::get(),
			fee_amount_in_sold_asset.into(),
		)?;

		Ok(())
	}

	fn unreserve_all_named_reserved_sold_currency(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		let schedule = Schedules::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
		let named_reserve_identitifer = reserve_identifier(schedule_id);
		let sold_currency = Self::sold_currency(&schedule.order);
		T::Currency::unreserve_all_named(&named_reserve_identitifer, sold_currency.into(), &who);

		Ok(())
	}

	fn sold_currency(order: &Order<T::Asset>) -> <T as Config>::Asset {
		let sold_currency = match order {
			Order::Sell { asset_in, .. } => asset_in,
			Order::Buy { asset_in, .. } => asset_in,
		};
		*sold_currency
	}

	fn get_transaction_fee(fee_currency: T::Asset) -> Result<u128, DispatchError> {
		let fee_amount_in_native = Self::weight_to_fee(<T as Config>::WeightInfo::on_initialize());
		let fee_amount_in_sold_asset =
			Self::convert_to_currency_if_asset_is_not_native(fee_currency, fee_amount_in_native)?;

		Ok(fee_amount_in_sold_asset)
	}

	fn weight_to_fee(weight: Weight) -> Balance {
		// cap the weight to the maximum defined in runtime, otherwise it will be the
		// `Bounded` maximum of its data type, which is not desired.
		let capped_weight: Weight = weight.min(T::BlockWeights::get().max_block);
		<T as pallet::Config>::WeightToFee::weight_to_fee(&capped_weight)
	}

	fn ensure_that_schedule_exists(schedule_id: &ScheduleId) -> DispatchResult {
		ensure!(Schedules::<T>::contains_key(schedule_id), Error::<T>::ScheduleNotExist);

		Ok(())
	}

	fn ensure_that_origin_is_schedule_owner(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		let schedule_owner = ScheduleOwnership::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
		ensure!(*who == schedule_owner, Error::<T>::NotScheduleOwner);

		Ok(())
	}

	fn add_schedule_id_to_existing_ids_per_block(
		next_schedule_id: ScheduleId,
		blocknumber_for_schedule: <T as frame_system::Config>::BlockNumber,
	) -> DispatchResult {
		let schedule_ids =
			ScheduleIdsPerBlock::<T>::get(blocknumber_for_schedule).ok_or(Error::<T>::NoScheduleIdsPlannedInBlock)?;
		if schedule_ids.len() == T::MaxSchedulePerBlock::get() as usize {
			let mut consequent_block = blocknumber_for_schedule.clone();
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

	fn get_next_block_mumber() -> BlockNumberFor<T> {
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

				T::AMMTrader::sell(
					origin,
					(*asset_in).into(),
					(*asset_out).into(),
					*amount_in - transaction_fee,
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
					(*asset_in).into(),
					(*asset_out).into(),
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
		let spot_price =
			T::PriceProvider::spot_price(*asset_in, *asset_out).ok_or(Error::<T>::CalculatingSpotPriceError)?;

		let estimated_amount_out = spot_price
			.checked_mul_int(*amount_in)
			.ok_or(ArithmeticError::Overflow)?;

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
		let spot_price =
			T::PriceProvider::spot_price(*asset_out, *asset_in).ok_or(Error::<T>::CalculatingSpotPriceError)?;

		let estimated_amount_in = spot_price
			.checked_mul_int(*amount_out)
			.ok_or(ArithmeticError::Overflow)?;

		let slippage_amount = T::SlippageLimitPercentage::get().mul_floor(estimated_amount_in);
		let max_limit_with_slippage = estimated_amount_in
			.checked_add(slippage_amount)
			.ok_or(ArithmeticError::Overflow)?;

		Ok(max_limit_with_slippage)
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
				.ok_or(Error::<T>::ScheduleExecutionNotPlannedOnBlock)?;

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
			let price = T::PriceProvider::spot_price(T::NativeAssetId::get(), asset_id)
				.ok_or(Error::<T>::CalculatingSpotPriceError)?;
			price.checked_mul_int(asset_amount).ok_or(ArithmeticError::Overflow)?
		};

		Ok(amount)
	}

	fn remove_schedule_from_storages(schedule_id: ScheduleId) {
		Schedules::<T>::remove(schedule_id);
		Suspended::<T>::remove(schedule_id);
		ScheduleOwnership::<T>::remove(schedule_id);
		RemainingRecurrences::<T>::remove(schedule_id);
	}

	fn ensure_that_next_blocknumber_bigger_than_current_block(
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
		schedule: &Schedule<T::Asset, T::BlockNumber>,
	) -> DispatchResult {
		let min_total_amount = if Self::sold_currency(&schedule.order) == T::NativeAssetId::get() {
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
					Error::<T>::ScheduleExecutionNotPlannedOnBlock,
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

	fn complete_dca(schedule_id: ScheduleId, owner: &T::AccountId) {
		Self::remove_schedule_from_storages(schedule_id);
		Self::deposit_event(Event::Completed {
			id: schedule_id,
			who: owner.clone(),
		});
	}
}

pub fn reserve_identifier(schedule: u32) -> [u8; 8] {
	let prefix = b"dca";
	let mut result = [0; 8];
	result[0..3].copy_from_slice(prefix);
	result[3..7].copy_from_slice(&schedule.to_be_bytes());
	hash_result(result)
}

fn hash_result(result: [u8; 8]) -> [u8; 8] {
	let hashed = BlakeTwo256::hash(&result);
	let mut hashed_array = [0; 8];
	hashed_array.copy_from_slice(&hashed.as_ref()[..8]);
	hashed_array
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
		let rng = rand::rngs::StdRng::seed_from_u64(seed);
		rng
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
