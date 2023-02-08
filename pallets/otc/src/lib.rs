#![allow(warnings)]
//
// This file is part of https://github.com/galacticcouncil/HydraDX-node

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
use frame_support::pallet_prelude::*;
use frame_support::transactional;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use orml_traits::{ arithmetic::{CheckedAdd, CheckedSub}, MultiReservableCurrency};
use orml_traits::GetByKey;
use scale_info::TypeInfo;
use sp_core::U256;
use sp_runtime::traits::Saturating;
use sp_runtime::FixedU128;
use sp_runtime::traits::{ConstU32, One};
use sp_runtime::ArithmeticError;
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::{result, vec::Vec};
use hydradx_traits::Registry;
use hydra_dx_math::MathError::Overflow;
use hydra_dx_math::MathError;
use hydra_dx_math::to_u256;
use orml_traits::MultiCurrency;

#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{EncodeLike, HasCompact};
	

	#[pallet::pallet]
	#[pallet::generate_store(pub(crate) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {    
		/// Identifier for the class of asset.
		type AssetId: Member
		+ Parameter
		+ Ord
		+ Default
		+ Copy
		+ HasCompact
		+ MaybeSerializeDeserialize
		+ MaxEncodedLen
		+ TypeInfo;

		/// Multi currency mechanism
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		
		type ExistentialDeposits: GetByKey<Self::AssetId, Balance>;

		#[pallet::constant]
		type ExistentialDepositMultiplier: Get<u128>;

		type MultiReservableCurrency: MultiReservableCurrency<
			Self::AccountId,
			CurrencyId = Self::AssetId,
			Balance = Balance,
			>;

		/// Native Asset
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An Order has been placed
		OrderPlaced {
			order_id: OrderId,
			asset_buy: T::AssetId,
			asset_sell: T::AssetId,
			amount_buy: Balance,
			amount_sell: Balance,
			partially_fillable: bool,
		},
		/// An Order has been partially filled
		OrderPartiallyFilled {
			order_id: OrderId,
			who: T::AccountId,
			amount_fill: Balance,
			amount_receive: Balance,
		},
		/// An Order has been completely filled
		OrderFilled {
			order_id: OrderId,
			who: T::AccountId,
			amount_fill: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset does not exist in registry
		AssetNotRegistered,
		/// The asset used to fill the order is different than asset_buy of the order
		AssetNotInOrder,
		/// When filling and order, the fill amount cannot be greater than the remaining order amount
		CannotFillMoreThanOrdered,
		/// Free balance is too low to place the order
		InsufficientBalance,
		/// Order cannot be found
		OrderNotFound,
		/// Size of order ID exceeds the bound
		OrderIdOutOfBound,
		/// Cannot partially fill an order which is not partially fillable
		OrderNotPartiallyFillable,
		/// Order amount_buy and amount_sell must be greater than the existential deposit
		/// for the asset multiplied by the ExistentialDepositMultiplier
		OrderSizeTooSmall,
		/// A partial order fill cannot leave remaning amount_buy or amount_sell smaller
		/// than the existential deposit for the asset multiplied by ExistentialDepositMultiplier
		RemainingOrderSizeTooSmall,
		/// Error with math calculations
		MathError,
	}

	/// ID sequencer for Orders
	#[pallet::storage]
	#[pallet::getter(fn next_order_id)]
	pub type NextOrderId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T: Config> =
		StorageMap<_, Blake2_128Concat, OrderId, Order<T::AccountId, T::AssetId>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(<T as Config>::WeightInfo::place_order())]
		#[transactional]
		pub fn place_order(
			origin: OriginFor<T>,
			asset_buy: T::AssetId,
			asset_sell: T::AssetId,
			amount_buy: Balance,
			amount_sell: Balance,
			partially_fillable: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;

			/// TODO: amount sell -> named reserve
			let order = Order { owner, asset_buy, asset_sell, amount_buy, amount_sell, partially_fillable };

			Self::validate_place_order(order.clone())?;

			let order_id = <NextOrderId<T>>::try_mutate(|next_id| -> result::Result<OrderId, DispatchError> {
				let current_id = *next_id;
				*next_id = next_id
					.checked_add(One::one())
					.ok_or(Error::<T>::OrderIdOutOfBound)?;
				Ok(current_id)
			})?;

			T::MultiReservableCurrency::reserve(order.asset_sell, &order.owner, order.amount_sell)?;

			<Orders<T>>::insert(order_id, order.clone());
			Self::deposit_event(Event::OrderPlaced {
				order_id: order_id,
				asset_buy: order.asset_buy,
				asset_sell: order.asset_sell,
				amount_buy: order.amount_buy,
				amount_sell: order.amount_sell,
				partially_fillable: order.partially_fillable,
			});

			Ok(())
		}

		/// TODO: update weight fn
		#[pallet::weight(<T as Config>::WeightInfo::place_order())]
		#[transactional]
		pub fn fill_order(
			origin: OriginFor<T>,
			order_id: OrderId,
			asset_fill: T::AssetId,
			amount_fill: Balance
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Orders<T>>::try_mutate_exists(order_id, |maybe_order| -> DispatchResult {
				let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
				let amount_receive = Self::amount_receive(order, amount_fill)?;

				Self::validate_fill_order(order, who.clone(), asset_fill, amount_fill, amount_receive)?;

				Self::execute_deal(order, who.clone(), amount_fill, amount_receive)?;

				let remaining_amount_buy = Self::amount_remaining(order.amount_buy, amount_fill)?;

				if(remaining_amount_buy > 0_u128) {
					Self::update_storage(order, amount_fill, amount_receive)?;
					Self::deposit_event(Event::OrderPartiallyFilled { order_id, who, amount_fill, amount_receive });
				} else {
					// cleanup storage
					*maybe_order = None;
					Self::deposit_event(Event::OrderFilled { order_id, who, amount_fill });	
				}

				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	fn validate_place_order(order: Order<T::AccountId, T::AssetId>) -> DispatchResult {
		ensure!(
			T::AssetRegistry::exists(order.asset_sell),
			Error::<T>::AssetNotRegistered
		);

		ensure!(
			T::AssetRegistry::exists(order.asset_buy),
			Error::<T>::AssetNotRegistered
		);

		ensure!(
			T::MultiReservableCurrency::can_reserve(order.asset_sell.clone(), &order.owner, order.amount_sell),
			Error::<T>::InsufficientBalance
		);

		let min_amount_buy = Self::min_order_size(order.asset_buy)?;

		ensure!(
			order.amount_buy > min_amount_buy,
			Error::<T>::OrderSizeTooSmall
		);

		let min_amount_sell = Self::min_order_size(order.asset_sell)?;

		ensure!(
			order.amount_sell > min_amount_sell,
			Error::<T>::OrderSizeTooSmall
		);

		Ok(())
	}

	fn validate_fill_order(
		order: &mut Order<T::AccountId, T::AssetId>,
		who: T::AccountId,
		asset_fill: T::AssetId,
		amount_fill: Balance,
		amount_receive: Balance,
	) -> DispatchResult {
		ensure!(
			order.asset_buy == asset_fill,
			Error::<T>::AssetNotInOrder
		);

		ensure!(
			order.amount_buy >= amount_fill,
			Error::<T>::CannotFillMoreThanOrdered
		);

		ensure!(
			T::Currency::ensure_can_withdraw(asset_fill, &who, amount_fill).is_ok(),
			Error::<T>::InsufficientBalance
		);

		if(!order.partially_fillable) {
			ensure!(
				amount_fill == order.amount_buy,
				Error::<T>::OrderNotPartiallyFillable
			)
		} else {
			let remaining_amount_buy = Self::amount_remaining(order.amount_buy, amount_fill)?;

			if(remaining_amount_buy > 0_u128) {
				let min_amount_buy = Self::min_order_size(order.asset_buy)?;

				ensure!(
					remaining_amount_buy > min_amount_buy,
					Error::<T>::RemainingOrderSizeTooSmall
				);
			}

			let remaining_amount_sell = Self::amount_remaining(order.amount_sell, amount_receive)?;

			if(remaining_amount_sell > 0_u128) {
				let min_amount_sell = Self::min_order_size(order.asset_buy)?;

				ensure!(
					remaining_amount_sell > min_amount_sell,
					Error::<T>::RemainingOrderSizeTooSmall
				);
			}
		}

		Ok(())
	}

	fn min_order_size(asset: T::AssetId) -> Result<Balance, Error<T>> {
		T::ExistentialDeposits::get(&asset)
			.checked_mul(T::ExistentialDepositMultiplier::get())
			.ok_or(Error::<T>::MathError)
	}

	fn amount_receive(order: &mut Order<T::AccountId, T::AssetId>, amount_fill: Balance) -> Result<Balance, Error<T>> {
		order.amount_sell
			.checked_mul(amount_fill)
			.and_then(|v| v.checked_div(order.amount_buy))
			.ok_or(Error::<T>::MathError)
	}

	fn amount_remaining(amount_initial: Balance, amount_change: Balance) -> Result<Balance, Error<T>> {
		amount_initial
			.checked_sub(amount_change)
			.ok_or(Error::<T>::MathError)
	}

	fn execute_deal(
		order: &mut Order<T::AccountId, T::AssetId>,
		who: T::AccountId,
		amount_fill: Balance,
		amount_receive: Balance,
	) -> DispatchResult {
		T::MultiReservableCurrency::unreserve(order.asset_sell, &order.owner, amount_receive);

		T::Currency::transfer(
			order.asset_buy,
			&who,
			&order.owner,
			amount_fill,
		)?;

		T::Currency::transfer(
			order.asset_sell,
			&order.owner,
			&who,
			amount_receive,
		)?;

		Ok(())
	}

	fn update_storage(
		order: &mut Order<T::AccountId, T::AssetId>,
		amount_fill: Balance,
		amount_receive: Balance,
	) -> DispatchResult {
		let new_amount_buy = Self::amount_remaining(order.amount_buy, amount_fill)?;
		let new_amount_sell = Self::amount_remaining(order.amount_sell, amount_receive)?;

		let updated_order = Order {
			owner: order.owner.clone(),
			asset_buy: order.asset_buy,
			asset_sell: order.asset_sell,
			amount_buy: new_amount_buy,
			amount_sell: new_amount_sell,
			partially_fillable: order.partially_fillable,
		};

		*order = updated_order;

		Ok(())
	}
}
