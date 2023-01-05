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
use frame_support::traits::Get;
use frame_support::transactional;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use frame_system::Origin;
use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use orml_traits::MultiReservableCurrency;
use pallet_transaction_multi_payment::TransactionMultiPaymentDataProvider;
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_runtime::traits::Zero;
use sp_runtime::traits::{BlockNumberProvider, ConstU32};
use sp_runtime::ArithmeticError;
use sp_runtime::FixedPointNumber;
use sp_runtime::FixedU128;
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::vec::Vec;
#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
use crate::types::{AssetId, Balance, BlockNumber, ScheduleId};
pub use pallet::*;

const MAX_NUMBER_OF_TRADES: u32 = 5;
const MAX_NUMBER_OF_SCHEDULES_PER_BLOCK: u32 = 20; //TODO: use config for this

type BlockNumberFor<T> = <T as frame_system::Config>::BlockNumber;

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub enum Recurrence {
	Fixed(u128),
	Perpetual,
}

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub enum Order<AssetId> {
	Sell {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
		route: BoundedVec<Trade, ConstU32<MAX_NUMBER_OF_TRADES>>,
	},
	Buy {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
		route: BoundedVec<Trade, ConstU32<MAX_NUMBER_OF_TRADES>>,
	},
}

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Schedule<AssetId> {
	pub period: BlockNumber, //TODO: use proper block number
	pub recurrence: Recurrence,
	pub order: Order<AssetId>,
}

///A single trade for buy/sell, describing the asset pair and the pool type in which the trade is executed
#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Trade {
	pub pool: PoolType, //TODO: consider using the same type as in route executor
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

#[derive(Encode, Decode, Clone, Copy, Debug, Eq, PartialEq, TypeInfo, MaxEncodedLen)]
pub enum PoolType {
	XYK,
}

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Bond<AssetId> {
	pub asset: AssetId,
	pub amount: Balance,
}

pub struct UserAssetIdAndSpotPrice<AssetId> {
	pub asset_id: AssetId,
	pub spot_price: Option<FixedU128>,
}

impl<AssetId> UserAssetIdAndSpotPrice<AssetId> {
	pub fn new(asset_id: AssetId, spot_price: Option<FixedU128>) -> UserAssetIdAndSpotPrice<AssetId> {
		UserAssetIdAndSpotPrice { asset_id, spot_price }
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{EncodeLike, HasCompact};
	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::router::ExecutorError;
	use orml_traits::MultiReservableCurrency;
	use pallet_transaction_multi_payment::TransactionMultiPaymentDataProvider;
	use sp_runtime::traits::{MaybeDisplay, Saturating};
	use sp_runtime::FixedPointNumber;
	use std::fmt::Debug;

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
				let mut weight: u64 = 0;

				let maybe_schedules: Option<BoundedVec<ScheduleId, ConstU32<20>>> =
					ScheduleIdsPerBlock::<T>::get(current_blocknumber);

				match maybe_schedules {
					Some(schedules) => {
						//TODO: order schedules randomly
						for schedule_id in schedules {
							let schedule = Schedules::<T>::get(schedule_id).unwrap();
							let owner = ScheduleOwnership::<T>::get(schedule_id).unwrap();
							let origin: OriginFor<T> = Origin::<T>::Signed(owner.clone()).into();

							let trade_result = Self::execute_trade(origin, &schedule.order);

							match trade_result {
								Ok(res) => {
									let blocknumber_for_schedule =
										current_blocknumber.checked_add(&schedule.period.into()).unwrap();

									match schedule.recurrence {
										Recurrence::Fixed(_) => {
											let remaining_reccurences =
												Self::decrement_recurrences(schedule_id).unwrap();
											if !remaining_reccurences.is_zero() {
												Self::plan_schedule_for_block(blocknumber_for_schedule, schedule_id);
											} else {
												Self::discard_bond(schedule_id, &owner);
											}
										}
										Recurrence::Perpetual => {
											Self::plan_schedule_for_block(blocknumber_for_schedule, schedule_id);
										}
									}
								}
								_ => {
									Suspended::<T>::insert(schedule_id, ());
									//TODO: slash execution bond
									//TODO: emit suspended
								}
							}
						}
					}
					None => (),
				}

				//TODO: increment the weight once an action happens
				//weight += T::WeightInfo::get_spot_price().ref_time();

				Weight::from_ref_time(weight)
			}
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_omnipool::Config {
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

		#[pallet::constant]
		type ExecutionBondInNativeCurrency: Get<Balance>;

		#[pallet::constant]
		type StorageBondInNativeCurrency: Get<Balance>;

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
	}

	#[pallet::error]
	pub enum Error<T> {
		///Unexpected error
		UnexpectedError,
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
	}

	/// Id sequencer for schedules
	#[pallet::storage]
	#[pallet::getter(fn next_schedule_id)]
	pub type ScheduleIdSequencer<T: Config> = StorageValue<_, ScheduleId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn schedules)]
	pub type Schedules<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, Schedule<T::Asset>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn suspended)]
	pub type Suspended<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, (), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn schedule_ownership)]
	pub type ScheduleOwnership<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, T::AccountId, OptionQuery>;

	//TODO: the number of recurrences can not be 0, so remove item once there - https://www.notion.so/DCA-061a93f912fd43b3a8e3e413abb8afdf#24dc5396cce542f681862ec7b1e54c15
	#[pallet::storage]
	#[pallet::getter(fn remaining_recurrences)]
	pub type RemainingRecurrences<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, u128, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn schedule_ids_per_block)]
	pub type ScheduleIdsPerBlock<T: Config> =
		StorageMap<_, Blake2_128Concat, BlockNumberFor<T>, BoundedVec<ScheduleId, ConstU32<20>>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn bond)]
	pub type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, ScheduleId, Bond<T::Asset>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		<T as pallet_omnipool::Config>::AssetId: From<<T as pallet::Config>::Asset>,
	{
		///Schedule
		#[pallet::weight(<T as Config>::WeightInfo::sell(5))]
		#[transactional]
		pub fn schedule(
			origin: OriginFor<T>,
			schedule: Schedule<T::Asset>,
			start_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let next_schedule_id = Self::get_next_schedule_id()?;

			Schedules::<T>::insert(next_schedule_id, &schedule);
			Self::store_recurrence_in_case_of_fixed_schedule(next_schedule_id, &schedule.recurrence);
			ScheduleOwnership::<T>::insert(next_schedule_id, who.clone());

			let blocknumber_for_first_schedule_execution =
				start_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
			Self::plan_schedule_for_block(blocknumber_for_first_schedule_execution, next_schedule_id);

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
		#[pallet::weight(<T as Config>::WeightInfo::sell(5))]
		#[transactional]
		pub fn pause(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: BlockNumberFor<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let schedule_owner = ScheduleOwnership::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
			ensure!(who == schedule_owner, Error::<T>::NotScheduleOwner);

			Self::remove_schedule_id_from_next_execution_block(schedule_id, next_execution_block)?;
			Suspended::<T>::insert(schedule_id, ());

			Self::unreserve_excecution_bond(schedule_id, &who)?;

			Self::deposit_event(Event::Paused { id: schedule_id, who });

			Ok(())
		}

		///Resume
		#[pallet::weight(<T as Config>::WeightInfo::sell(5))]
		#[transactional]
		pub fn resume(
			origin: OriginFor<T>,
			schedule_id: ScheduleId,
			next_execution_block: Option<BlockNumberFor<T>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let schedule_owner = ScheduleOwnership::<T>::get(schedule_id).ok_or(Error::<T>::ScheduleNotExist)?;
			ensure!(who == schedule_owner, Error::<T>::NotScheduleOwner);

			Self::ensure_that_next_blocknumber_bigger_than_current_block(next_execution_block)?;

			let next_execution_block = next_execution_block.unwrap_or_else(|| Self::get_next_block_mumber());
			Self::plan_schedule_for_block(next_execution_block, schedule_id);

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
	}
}

impl<T: Config> Pallet<T>
where
	<T as pallet_omnipool::Config>::AssetId: From<<T as pallet::Config>::Asset>,
{
	fn plan_schedule_for_block(b: T::BlockNumber, schedule_id: ScheduleId) {
		if !ScheduleIdsPerBlock::<T>::contains_key(b) {
			let vec_with_first_schedule_id = Self::create_bounded_vec(schedule_id);
			ScheduleIdsPerBlock::<T>::insert(b, vec_with_first_schedule_id);
		} else {
			//TODO: if the block is full, then we should plan it the next one or so
			Self::add_schedule_id_to_existing_ids_per_block(schedule_id, b);
		}
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

	fn create_bounded_vec(next_schedule_id: ScheduleId) -> BoundedVec<ScheduleId, ConstU32<20>> {
		let schedule_id = vec![next_schedule_id];
		let bounded_vec: BoundedVec<ScheduleId, ConstU32<20>> = schedule_id.try_into().unwrap(); //TODO: here use constant instead of hardcoded value
		bounded_vec
	}

	fn store_recurrence_in_case_of_fixed_schedule(next_schedule_id: ScheduleId, recurrence: &Recurrence) {
		if let Recurrence::Fixed(number_of_recurrence) = *recurrence {
			RemainingRecurrences::<T>::insert(next_schedule_id, number_of_recurrence);
		};
	}

	fn add_schedule_id_to_existing_ids_per_block(
		next_schedule_id: ScheduleId,
		blocknumber_for_schedule: <T as frame_system::Config>::BlockNumber,
	) -> DispatchResult {
		ScheduleIdsPerBlock::<T>::try_mutate_exists(blocknumber_for_schedule, |schedule_ids| -> DispatchResult {
			let mut schedule_ids = schedule_ids.as_mut().ok_or(Error::<T>::UnexpectedError)?; //TODO: add different error handling

			schedule_ids
				.try_push(next_schedule_id)
				.map_err(|_| Error::<T>::UnexpectedError)?;
			Ok(())
		})?;

		Ok(())
	}

	fn decrement_recurrences(schedule_id: ScheduleId) -> Result<u128, DispatchResult> {
		let remaining_recurrences =
			RemainingRecurrences::<T>::try_mutate_exists(schedule_id, |maybe_remaining_occurrances| {
				let mut remaining_ocurrences = maybe_remaining_occurrances
					.as_mut()
					.ok_or(Error::<T>::UnexpectedError)?; //TODO: add RaminingReccurenceNotExist error

				*remaining_ocurrences = remaining_ocurrences.checked_sub(1).ok_or(Error::<T>::UnexpectedError)?; //TODO: add arithmetic error
				let remainings = remaining_ocurrences.clone(); //TODO: do this in a smarter way?

				if *remaining_ocurrences == 0 {
					*maybe_remaining_occurrances = None;
				}

				Ok::<u128, DispatchError>(remainings)
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

			let index = schedule_ids.iter().position(|x| *x == schedule_id).unwrap();
			schedule_ids.remove(index);

			if schedule_ids.is_empty() {
				*maybe_schedule_ids = None;
			}
			Ok(())
		})?;

		Ok(())
	}

	fn calculate_and_store_bond(who: T::AccountId, next_schedule_id: ScheduleId) -> DispatchResult {
		let user_asset_and_price = Self::get_user_currency_and_spot_price(&who)?;

		let total_bond_in_native_currency = Self::get_total_bond_from_config_in_native_currency()?;

		let total_bond_in_user_currency = match { user_asset_and_price.spot_price } {
			//TODO: rather check if the asset id is equal to native
			Some(spot_price) => {
				spot_price
					.checked_mul_int(total_bond_in_native_currency)
					.ok_or(ArithmeticError::Overflow)? //TODO: verify with Lumir or so if this is the right way to do the conversion
			}
			None => total_bond_in_native_currency,
		};

		let bond = Bond {
			asset: user_asset_and_price.asset_id,
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
		let total_bond_in_native_currency = T::ExecutionBondInNativeCurrency::get()
			.checked_add(T::StorageBondInNativeCurrency::get())
			.ok_or(Error::<T>::UnexpectedError)?;

		Ok(total_bond_in_native_currency)
	}

	fn unreserve_excecution_bond(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		Bonds::<T>::try_mutate(schedule_id, |maybe_bond| -> DispatchResult {
			let bond = maybe_bond.as_mut().ok_or(Error::<T>::BondNotExist)?;
			let user_asset_and_spot_price = Self::get_user_currency_and_spot_price(&who)?;

			let execution_bond_in_native_currency = T::ExecutionBondInNativeCurrency::get();

			//TODO: refactor - find some common logic to extract as similar things happen
			let execution_bond_in_user_currency = match user_asset_and_spot_price.spot_price {
				Some(spot_price) => spot_price
					.checked_mul_int(execution_bond_in_native_currency)
					.ok_or(ArithmeticError::Overflow)?,
				None => execution_bond_in_native_currency,
			};

			//TODO: handle the case for when the set currency is different than in the bond, so the user has changed in afterwards

			bond.amount = bond
				.amount
				.checked_sub(execution_bond_in_user_currency)
				.ok_or(ArithmeticError::Underflow)?;

			//TODO: the only case when the storage bond won't be reserved after this is if the total bond before this is less than the current storage bond

			T::MultiReservableCurrency::unreserve(bond.asset, &who, execution_bond_in_user_currency);

			Ok(())
		})?;

		Ok(())
	}

	fn reserve_excecution_bond(schedule_id: ScheduleId, who: &T::AccountId) -> DispatchResult {
		Bonds::<T>::try_mutate(schedule_id, |maybe_bond| -> DispatchResult {
			let bond = maybe_bond.as_mut().ok_or(Error::<T>::BondNotExist)?;
			let user_asset_and_spot_price = Self::get_user_currency_and_spot_price(&who)?;

			let execution_bond_in_native_currency = T::ExecutionBondInNativeCurrency::get();

			let execution_bond_in_user_currency = match user_asset_and_spot_price.spot_price {
				Some(spot_price) => spot_price
					.checked_mul_int(execution_bond_in_native_currency)
					.ok_or(ArithmeticError::Overflow)?,
				None => execution_bond_in_native_currency,
			};

			//TODO: handle the case for when the set currency is different than in the bond, so the user has changed in afterwards

			bond.amount = bond
				.amount
				.checked_add(execution_bond_in_user_currency)
				.ok_or(ArithmeticError::Underflow)?;

			T::MultiReservableCurrency::reserve(bond.asset, &who, execution_bond_in_user_currency)?;

			Ok(())
		})?;

		Ok(())
	}

	fn get_user_currency_and_spot_price(
		who: &T::AccountId,
	) -> Result<UserAssetIdAndSpotPrice<T::Asset>, DispatchError> {
		let user_currency_and_spot_price = T::AccountCurrencyAndPriceProvider::get_currency_and_price(&who)?;
		Ok(UserAssetIdAndSpotPrice::new(
			user_currency_and_spot_price.0,
			user_currency_and_spot_price.1,
		))
	}

	fn discard_bond(schedule_id: ScheduleId, owner: &T::AccountId) {
		let bond = Self::bond(schedule_id).unwrap();
		T::MultiReservableCurrency::unreserve(bond.asset, &owner, bond.amount);
		Bonds::<T>::remove(schedule_id);
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
}
