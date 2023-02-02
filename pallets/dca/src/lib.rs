#![allow(warnings)]
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

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::ensure;
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::{Get, Len};
use frame_support::transactional;
use frame_support::weights::WeightToFee as FrameSupportWeight;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use frame_system::Origin;
use hydradx_traits::pools::SpotPriceProvider;
use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_omnipool::WeightInfo as OmnipoolWeightInfo;
use pallet_transaction_multi_payment::TransactionMultiPaymentDataProvider;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_runtime::traits::Zero;
use sp_runtime::traits::{BlockNumberProvider, ConstU32};
use sp_runtime::ArithmeticError;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::vec;
use sp_std::vec::Vec;
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
use sp_runtime::traits::One;

//TODO:
//-ask config for bonds
//-calculate and generate weight
//-add doc comments for both API, and types.rs
//-add readme

type BlockNumberFor<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::types::Recurrence;
	use codec::{EncodeLike, HasCompact};
	use frame_support::weights::WeightToFee;

	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::pools::SpotPriceProvider;
	use hydradx_traits::router::ExecutorError;
	use orml_traits::MultiReservableCurrency;
	use pallet_transaction_multi_payment::TransactionMultiPaymentDataProvider;
	use sp_core::H256;
	use sp_runtime::traits::{MaybeDisplay, Saturating};
	use sp_runtime::FixedPointNumber;

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId: From<<T as pallet::Config>::Asset>,
	{
		fn on_initialize(current_blocknumber: T::BlockNumber) -> Weight {
			{
				//TODO: include all the logic - benchmark them
				let mut weight: u64 = Self::get_on_initialize_weight();

				let mut random_generator = T::RandomnessProvider::generator();

				let maybe_schedules: Option<BoundedVec<ScheduleId, T::MaxSchedulePerBlock>> =
					ScheduleIdsPerBlock::<T>::get(current_blocknumber);

				match maybe_schedules {
					Some(mut schedules) => {
						schedules.sort_by_key(|x| random_generator.gen::<u32>());
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
	pub trait Config: frame_system::Config + pallet_omnipool::Config + pallet_relaychain_info::Config {
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

		type AccountCurrencyAndPriceProvider: TransactionMultiPaymentDataProvider<
			Self::AccountId,
			Self::Asset,
			FixedU128,
		>;

		type MultiReservableCurrency: MultiReservableCurrency<
			Self::AccountId,
			CurrencyId = Self::Asset,
			Balance = Balance,
		>;

		type SpotPriceProvider: SpotPriceProvider<Self::Asset, Price = FixedU128>;

		type RandomnessProvider: RandomnessProvider;

		#[pallet::constant]
		type ExecutionBondInNativeCurrency: Get<Balance>;

		#[pallet::constant]
		type StorageBondInNativeCurrency: Get<Balance>;

		#[pallet::constant]
		type MaxSchedulePerBlock: Get<u32>;

		/// Native Asset
		#[pallet::constant]
		type NativeAssetId: Get<Self::Asset>;

		//TODO: rename to fee receiver
		#[pallet::constant]
		type SlashedBondReceiver: Get<Self::AccountId>;

		/// Convert a weight value into a deductible fee
		type WeightToFee: WeightToFee<Balance = Balance>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///First event
		Scheduled {
			id: ScheduleId,
			who: T::AccountId,
		},
		ExecutionPlanned {
			id: ScheduleId,
			who: T::AccountId,
			block: BlockNumberFor<T>,
		},
		Paused {
			id: ScheduleId,
			who: T::AccountId,
		},
		Resumed {
			id: ScheduleId,
			who: T::AccountId,
		},
		Terminated {
			id: ScheduleId,
			who: T::AccountId,
		},
		Suspended {
			id: ScheduleId,
			who: T::AccountId,
		},
		Completed {
			id: ScheduleId,
			who: T::AccountId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		///Error that should not really happen only in case of invalid state of the schedule storage entries.
		InvalidState,
		///Schedule not exist
		ScheduleNotExist,
		///Balance is too low to reserve for bond
		BalanceTooLowForReservingBond,
		///The user is not the owner of the schedule
		NotScheduleOwner,
		///The bond does not exist. It should not really happen, only in case of invalid state
		BondNotExist,
		///The next execution block number should be in the future
		BlockNumberIsNotInFuture,
		///There is not planned execution on the given block
		NoPlannedExecutionFoundOnBlock,
		///Schedule execution is not planned on block
		ScheduleExecutionNotPlannedOnBlock,
		///The schedule must be suspended when there is not execution block specified by the using during termination of a shcedule
		ScheduleMustBeSuspended,
		///Error that should not really happen only in case of invalid state of the schedule storage entries.
		CalculatingSpotPriceError,
	}

	/// Id sequencer for schedules
	#[pallet::storage]
	#[pallet::getter(fn next_schedule_id)]
	pub type ScheduleIdSequencer<T: Config> = StorageValue<_, ScheduleId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn schedules)]
	pub type Schedules<T: Config> =
		StorageMap<_, Blake2_128Concat, ScheduleId, Schedule<T::Asset, BlockNumberFor<T>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn owner_of)]
	pub type ScheduleOwnership<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn suspended)]
	pub type Suspended<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, (), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn remaining_recurrences)]
	pub type RemainingRecurrences<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u32, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, T::MaxSchedulePerBlock>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bond)]
	pub type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, Bond<T::Asset>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId: From<<T as pallet::Config>::Asset>,
	{
		///Schedule
		#[pallet::weight(<T as Config>::WeightInfo::schedule())]
		#[transactional]
		pub fn schedule(
			origin: OriginFor<T>,
			schedule: Schedule<T::Asset, BlockNumberFor<T>>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_that_next_blocknumber_bigger_than_current_block(start_execution_block)?;

			let next_schedule_id = Self::get_next_schedule_id()?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			Self::store_recurrence_in_case_of_fixed_schedule(next_schedule_id, &schedule.recurrence);
			ScheduleOwnership::<T>::insert(next_schedule_id, who.clone());

			let blocknumber_for_first_schedule_execution =
				start_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
			Self::plan_schedule_for_block(blocknumber_for_first_schedule_execution, next_schedule_id)?;

			Self::calculate_and_store_bond(who.clone(), next_schedule_id)?;

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

		///Pause
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

			Self::unreserve_excecution_bond(schedule_id, &who)?;

			Self::deposit_event(Event::Paused { id: schedule_id, who });

			Ok(())
		}

		///Resume
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

			let next_execution_block = next_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
			Self::plan_schedule_for_block(next_execution_block, schedule_id)?;

			Suspended::<T>::remove(schedule_id);

			Self::reserve_excecution_bond(schedule_id, &who)?;

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

		///Terminate
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

			Self::remove_planning_or_suspension(schedule_id, next_execution_block)?;
			Self::remove_schedule_from_storages(schedule_id);

			Self::discard_bond(schedule_id, &who)?;

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
	<T as pallet_omnipool::Config>::AssetId: From<<T as pallet::Config>::Asset>,
{
	fn execute_schedule(current_blocknumber: T::BlockNumber, weight: &mut u64, schedule_id: ScheduleId) {
		let schedule = exec_or_return_if_none!(Schedules::<T>::get(schedule_id));
		let owner = exec_or_return_if_none!(ScheduleOwnership::<T>::get(schedule_id));
		let origin: OriginFor<T> = Origin::<T>::Signed(owner.clone()).into();

		let trade_result = Self::execute_trade(origin, &schedule.order);
		*weight += Self::get_execute_schedule_weight();

		match trade_result {
			Ok(res) => {
				let take_transaction_fee_result = Self::take_transaction_fee_from_user(&owner, schedule.order);
				if let Err(error) = take_transaction_fee_result {
					exec_or_return_if_err!(Self::suspend_schedule(&owner, schedule_id));
				}

				let blocknumber_for_schedule =
					exec_or_return_if_none!(current_blocknumber.checked_add(&schedule.period.into()));

				match schedule.recurrence {
					Recurrence::Fixed(_) => {
						let remaining_reccurences = exec_or_return_if_err!(Self::decrement_recurrences(schedule_id));
						if !remaining_reccurences.is_zero() {
							exec_or_return_if_err!(Self::plan_schedule_for_block(
								blocknumber_for_schedule,
								schedule_id
							));
						} else {
							Self::remove_schedule_from_storages(schedule_id);
							exec_or_return_if_err!(Self::discard_bond(schedule_id, &owner));
							Self::deposit_event(Event::Completed {
								id: schedule_id,
								who: owner.clone(),
							});
						}
					}
					Recurrence::Perpetual => {
						exec_or_return_if_err!(Self::plan_schedule_for_block(blocknumber_for_schedule, schedule_id));
					}
				}
			}
			_ => {
				exec_or_return_if_err!(Self::suspend_schedule(&owner, schedule_id));
			}
		}
	}

	fn get_on_initialize_weight() -> u64 {
		crate::weights::HydraWeight::<T>::on_initialize().ref_time()
	}

	fn get_execute_schedule_weight() -> u64 {
		crate::weights::HydraWeight::<T>::execute_schedule().ref_time()
	}

	fn take_transaction_fee_from_user(owner: &T::AccountId, order: Order<<T as Config>::Asset>) -> DispatchResult {
		let fee_currency = match order {
			Order::Sell { asset_in, .. } => asset_in,
			Order::Buy { asset_in, .. } => asset_in,
		};

		let fee_amount_in_native = Self::weight_to_fee(<T as Config>::WeightInfo::on_initialize());
		let fee_amount_in_sold_asset =
			Self::convert_to_currency_if_asset_is_not_native(fee_currency, fee_amount_in_native)?;

		T::Currency::transfer(
			fee_currency.into(),
			&owner,
			&T::SlashedBondReceiver::get(),
			fee_amount_in_sold_asset,
		)?;

		Ok(())
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
		let schedule_ids = ScheduleIdsPerBlock::<T>::get(blocknumber_for_schedule).ok_or(Error::<T>::InvalidState)?;
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
			let mut schedule_ids = schedule_ids.as_mut().ok_or(Error::<T>::InvalidState)?;

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

	fn store_recurrence_in_case_of_fixed_schedule(next_schedule_id: ScheduleId, recurrence: &Recurrence) {
		if let Recurrence::Fixed(number_of_recurrence) = *recurrence {
			RemainingRecurrences::<T>::insert(next_schedule_id, number_of_recurrence);
		};
	}

	fn decrement_recurrences(schedule_id: ScheduleId) -> Result<u32, DispatchResult> {
		let remaining_recurrences =
			RemainingRecurrences::<T>::try_mutate_exists(schedule_id, |maybe_remaining_occurrances| {
				let mut remaining_ocurrences = maybe_remaining_occurrances.as_mut().ok_or(Error::<T>::InvalidState)?;

				*remaining_ocurrences = remaining_ocurrences.checked_sub(1).ok_or(ArithmeticError::Underflow)?;
				let remainings = remaining_ocurrences.clone();

				if *remaining_ocurrences == 0 {
					*maybe_remaining_occurrances = None;
				}

				Ok::<u32, DispatchError>(remainings)
			})?;

		Ok(remaining_recurrences)
	}

	fn execute_trade(origin: T::Origin, order: &Order<<T as Config>::Asset>) -> DispatchResult {
		match order {
			Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				route,
				min_limit,
			} => pallet_omnipool::Pallet::<T>::sell(
				origin,
				(*asset_in).into(),
				(*asset_out).into(),
				*amount_in,
				*min_limit,
			),
			Order::Buy {
				asset_in,
				asset_out,
				amount_out,
				max_limit,
				route,
			} => pallet_omnipool::Pallet::<T>::buy(
				origin,
				(*asset_out).into(),
				(*asset_in).into(),
				*amount_out,
				*max_limit,
			),
		}
	}

	fn remove_schedule_id_from_next_execution_block(
		schedule_id: ScheduleId,
		next_execution_block: T::BlockNumber,
	) -> DispatchResult {
		ScheduleIdsPerBlock::<T>::try_mutate_exists(next_execution_block, |maybe_schedule_ids| -> DispatchResult {
			let mut schedule_ids = maybe_schedule_ids.as_mut().ok_or(Error::<T>::ScheduleNotExist)?;

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

	fn calculate_and_store_bond(who: T::AccountId, next_schedule_id: ScheduleId) -> DispatchResult {
		let user_fee_currency = Self::get_user_fee_currency(&who)?;
		let total_bond_in_native_currency = Self::get_total_bond_from_config_in_native_currency()?;
		let total_bond_in_user_currency =
			Self::convert_to_currency_if_asset_is_not_native(user_fee_currency, total_bond_in_native_currency)?;

		let bond = Bond {
			asset: user_fee_currency,
			amount: total_bond_in_user_currency,
		};

		Self::reserve_bond(&who, &bond)?;

		Bonds::<T>::insert(next_schedule_id, bond);

		Ok(())
	}

	fn reserve_bond(who: &T::AccountId, bond: &Bond<T::Asset>) -> DispatchResult {
		ensure!(
			T::MultiReservableCurrency::can_reserve(bond.asset, &who, bond.amount),
			Error::<T>::BalanceTooLowForReservingBond
		);

		T::MultiReservableCurrency::reserve(bond.asset, &who, bond.amount)?;

		Ok(())
	}

	fn get_total_bond_from_config_in_native_currency() -> Result<u128, DispatchError> {
		let exb = T::ExecutionBondInNativeCurrency::get();
		let sb = T::StorageBondInNativeCurrency::get();
		let total_bond_in_native_currency = T::ExecutionBondInNativeCurrency::get()
			.checked_add(T::StorageBondInNativeCurrency::get())
			.ok_or(ArithmeticError::Overflow)?;

		Ok(total_bond_in_native_currency)
	}

	fn suspend_schedule(owner: &T::AccountId, schedule_id: ScheduleId) -> DispatchResult {
		Suspended::<T>::insert(schedule_id, ());
		Self::slash_execution_bond(schedule_id, &owner)?;
		Self::deposit_event(Event::Suspended {
			id: schedule_id,
			who: owner.clone(),
		});

		Ok(())
	}

	fn slash_execution_bond(schedule_id: ScheduleId, owner: &T::AccountId) -> DispatchResult {
		let execution_bond = Self::unreserve_excecution_bond(schedule_id, &owner)?;

		if let Some(execution_bond) = execution_bond {
			T::Currency::transfer(
				execution_bond.asset.into(),
				&owner,
				&T::SlashedBondReceiver::get(),
				execution_bond.amount,
			)?;
		};

		Ok(())
	}

	fn unreserve_excecution_bond(
		schedule_id: ScheduleId,
		who: &T::AccountId,
	) -> Result<Option<Bond<T::Asset>>, DispatchError> {
		let execution_bond = Bonds::<T>::try_mutate(schedule_id, |maybe_bond| {
			let bond = maybe_bond.as_mut().ok_or(Error::<T>::BondNotExist)?;

			let storage_bond_in_bond_currency = Self::get_storage_bond_in_currency(&who, bond.asset)?;

			return if bond.amount <= storage_bond_in_bond_currency {
				Ok::<Option<Bond<T::Asset>>, DispatchError>(None)
			} else {
				let to_be_extracted_amount = bond
					.amount
					.checked_sub(storage_bond_in_bond_currency)
					.ok_or(ArithmeticError::Underflow)?;

				bond.amount = bond
					.amount
					.checked_sub(to_be_extracted_amount)
					.ok_or(ArithmeticError::Underflow)?;

				T::MultiReservableCurrency::unreserve(bond.asset, &who, to_be_extracted_amount);

				Ok::<Option<Bond<T::Asset>>, DispatchError>(Some(Bond {
					amount: to_be_extracted_amount,
					asset: bond.asset,
				}))
			};
		})?;

		Ok(execution_bond)
	}

	fn reserve_excecution_bond(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		Bonds::<T>::try_mutate(schedule_id, |maybe_bond| -> DispatchResult {
			let bond = maybe_bond.as_mut().ok_or(Error::<T>::BondNotExist)?;

			let execution_bond_in_user_currency = Self::get_execution_bond_in_currency(&who, bond.asset)?;

			bond.amount = bond
				.amount
				.checked_add(execution_bond_in_user_currency)
				.ok_or(ArithmeticError::Underflow)?;

			T::MultiReservableCurrency::reserve(bond.asset, &who, execution_bond_in_user_currency)?;

			Ok(())
		})?;

		Ok(())
	}

	fn get_execution_bond_in_currency(who: &T::AccountId, bond_asset: T::Asset) -> Result<Balance, DispatchError> {
		let execution_bond_in_native_currency = T::ExecutionBondInNativeCurrency::get();

		let execution_bond_in_user_currency =
			Self::convert_to_currency_if_asset_is_not_native(bond_asset, execution_bond_in_native_currency)?;

		Ok(execution_bond_in_user_currency)
	}

	fn get_storage_bond_in_currency(who: &T::AccountId, bond_asset: T::Asset) -> Result<Balance, DispatchError> {
		let storage_bond_in_native_currency = T::StorageBondInNativeCurrency::get();

		let storage_bond_in_user_currency =
			Self::convert_to_currency_if_asset_is_not_native(bond_asset, storage_bond_in_native_currency)?;

		Ok(storage_bond_in_user_currency)
	}

	fn get_user_fee_currency(who: &T::AccountId) -> Result<T::Asset, DispatchError> {
		let user_currency_and_spot_price = T::AccountCurrencyAndPriceProvider::get_currency_and_price(&who)?;
		Ok(user_currency_and_spot_price.0)
	}

	fn convert_to_currency_if_asset_is_not_native(
		asset_id: T::Asset,
		asset_amount: u128,
	) -> Result<u128, DispatchError> {
		let total_bond_in_user_currency = if asset_id == T::NativeAssetId::get() {
			asset_amount
		} else {
			let price = T::SpotPriceProvider::spot_price(T::NativeAssetId::get(), asset_id)
				.ok_or(Error::<T>::CalculatingSpotPriceError)?;
			price.checked_mul_int(asset_amount).ok_or(ArithmeticError::Overflow)?
		};

		Ok(total_bond_in_user_currency)
	}

	fn remove_schedule_from_storages(schedule_id: ScheduleId) {
		Schedules::<T>::remove(schedule_id);
		Suspended::<T>::remove(schedule_id);
		ScheduleOwnership::<T>::remove(schedule_id);
		RemainingRecurrences::<T>::remove(schedule_id);
	}

	fn discard_bond(schedule_id: ScheduleId, owner: &T::AccountId) -> DispatchResult {
		let bond = Self::bond(schedule_id).ok_or(Error::<T>::InvalidState)?;
		T::MultiReservableCurrency::unreserve(bond.asset, &owner, bond.amount);
		Bonds::<T>::remove(schedule_id);

		Ok(())
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
				ensure!(
					Suspended::<T>::contains_key(&schedule_id),
					Error::<T>::ScheduleMustBeSuspended
				);
				Suspended::<T>::remove(schedule_id);
			}
		};

		Ok(())
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
		let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
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
