// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// # OTC pallet
// ## General description
// This pallet provides basic over-the-counter (OTC) trading functionality.
// It allows anyone to `place_order` by specifying a pair of assets (in and out), their respective amounts, and
// whether the order is partially fillable. The order price is static and calculated as `amount_out / amount_in`.
//
// ## Notes
// The pallet implements a minimum order size as an alternative to storage fees. The amounts of an open order cannot
// be lower than the existential deposit for the respective asset, multiplied by `ExistentialDepositMultiplier`.
// This is validated at `place_order` but also at `partial_fill_order` - meaning that a user cannot leave dust amounts
// below the defined threshold after filling an order (instead they should fill the order completely).
//
// ## Dispatachable functions
// * `place_order` -  create a new OTC order.
// * `partial_fill_order` - fill an OTC order (partially).
// * `fill_order` - fill an OTC order (completely).
// * `cancel_order` - cancel an open OTC order.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::MaxEncodedLen;
use frame_support::{pallet_prelude::*, require_transactional};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use hydradx_traits::Registry;
use orml_traits::{GetByKey, MultiCurrency, NamedMultiReservableCurrency};
use sp_core::U256;
use sp_runtime::{
	traits::{One, Zero},
	DispatchError,
};
use sp_std::vec::Vec;
#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub type Balance = u128;
pub type OrderId = u32;
pub type NamedReserveIdentifier = [u8; 8];

pub const NAMED_RESERVE_ID: NamedReserveIdentifier = *b"otcorder";

#[derive(Encode, Decode, Debug, Eq, PartialEq, Clone, TypeInfo, MaxEncodedLen)]
pub struct Order<AccountId, AssetId> {
	pub owner: AccountId,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partially_fillable: bool,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Identifier for the class of asset.
		type AssetId: Member + Parameter + Copy + HasCompact + MaybeSerializeDeserialize + MaxEncodedLen;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;

		/// Named reservable multi currency
		type Currency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::AssetId,
			Balance = Balance,
		>;

		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type ExistentialDeposits: GetByKey<Self::AssetId, Balance>;

		#[pallet::constant]
		type ExistentialDepositMultiplier: Get<u8>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An Order has been cancelled
		Cancelled { order_id: OrderId },
		/// An Order has been completely filled
		Filled {
			order_id: OrderId,
			who: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
		},
		/// An Order has been partially filled
		PartiallyFilled {
			order_id: OrderId,
			who: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
		},
		/// An Order has been placed
		Placed {
			order_id: OrderId,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			partially_fillable: bool,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset does not exist in registry
		AssetNotRegistered,
		/// Order cannot be found
		OrderNotFound,
		/// Size of order ID exceeds the bound
		OrderIdOutOfBound,
		/// Cannot partially fill an order which is not partially fillable
		OrderNotPartiallyFillable,
		/// Order amount_in and amount_out must at all times be greater than the existential deposit
		/// for the asset multiplied by the ExistentialDepositMultiplier.
		/// A fill order may not leave behind amounts smaller than this.
		OrderAmountTooSmall,
		/// Error with math calculations
		MathError,
		/// The caller does not have permission to complete the action
		Forbidden,
		/// Reserved amount not sufficient.
		InsufficientReservedAmount,
	}

	/// ID sequencer for Orders
	#[pallet::storage]
	#[pallet::getter(fn next_order_id)]
	pub type NextOrderId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T: Config> = StorageMap<_, Blake2_128Concat, OrderId, Order<T::AccountId, T::AssetId>, OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new OTC order
		///  
		/// Parameters:
		/// - `asset_in`: Asset which is being bought
		/// - `asset_out`: Asset which is being sold
		/// - `amount_in`: Amount that the order is seeking to buy
		/// - `amount_out`: Amount that the order is selling
		/// - `partially_fillable`: Flag indicating whether users can fill the order partially
		///
		/// Validations:
		/// - asset_in must be registered
		/// - amount_in must be higher than the existential deposit of asset_in multiplied by
		///   ExistentialDepositMultiplier
		/// - amount_out must be higher than the existential deposit of asset_out multiplied by
		///   ExistentialDepositMultiplier
		///
		/// Events:
		/// - `Placed` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::place_order())]
		pub fn place_order(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: Balance,
			amount_out: Balance,
			partially_fillable: bool,
		) -> DispatchResult {
			let owner = ensure_signed(origin)?;
			let order = Order {
				owner,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				partially_fillable,
			};

			ensure!(T::AssetRegistry::exists(order.asset_in), Error::<T>::AssetNotRegistered);
			Self::ensure_min_order_amount(order.asset_in, order.amount_in)?;
			Self::ensure_min_order_amount(order.asset_out, amount_out)?;

			<NextOrderId<T>>::try_mutate(|next_id| -> DispatchResult {
				let order_id = *next_id;

				T::Currency::reserve_named(&NAMED_RESERVE_ID, order.asset_out, &order.owner, order.amount_out)?;
				<Orders<T>>::insert(order_id, &order);

				Self::deposit_event(Event::Placed {
					order_id,
					asset_in: order.asset_in,
					asset_out: order.asset_out,
					amount_in: order.amount_in,
					amount_out,
					partially_fillable: order.partially_fillable,
				});

				*next_id = next_id.checked_add(One::one()).ok_or(Error::<T>::OrderIdOutOfBound)?;
				Ok(())
			})
		}

		/// Fill an OTC order (partially)
		///  
		/// Parameters:
		/// - `order_id`: ID of the order
		/// - `amount_in`: Amount with which the order is being filled
		///
		/// Validations:
		/// - order must be partially_fillable
		/// - after the partial_fill, the remaining order.amount_in must be higher than the existential deposit
		///   of asset_in multiplied by ExistentialDepositMultiplier
		/// - after the partial_fill, the remaining order.amount_out must be higher than the existential deposit
		///   of asset_out multiplied by ExistentialDepositMultiplier
		///
		/// Events:
		/// `PartiallyFilled` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::partial_fill_order())]
		pub fn partial_fill_order(origin: OriginFor<T>, order_id: OrderId, amount_in: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			<Orders<T>>::try_mutate(order_id, |maybe_order| -> DispatchResult {
				let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				ensure!(order.partially_fillable, Error::<T>::OrderNotPartiallyFillable);

				let amount_out_calculation = U256::from(order.amount_out)
					.checked_mul(U256::from(amount_in))
					.and_then(|v| v.checked_div(U256::from(order.amount_in)))
					.ok_or(Error::<T>::MathError)?;
				let amount_out = Balance::try_from(amount_out_calculation).map_err(|_| Error::<T>::MathError)?;

				order.amount_in = order.amount_in.checked_sub(amount_in).ok_or(Error::<T>::MathError)?;
				order.amount_out = order.amount_out.checked_sub(amount_out).ok_or(Error::<T>::MathError)?;

				Self::ensure_min_order_amount(order.asset_out, order.amount_out)?;
				Self::ensure_min_order_amount(order.asset_in, order.amount_in)?;

				Self::execute_order(order, &who, amount_in, amount_out)?;

				Self::deposit_event(Event::PartiallyFilled {
					order_id,
					who,
					amount_in,
					amount_out,
				});
				Ok(())
			})
		}

		/// Fill an OTC order (completely)
		///  
		/// Parameters:
		/// - `order_id`: ID of the order
		///
		/// Events:
		/// `Filled` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::fill_order())]
		pub fn fill_order(origin: OriginFor<T>, order_id: OrderId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

			Self::execute_order(&order, &who, order.amount_in, order.amount_out)?;
			<Orders<T>>::remove(order_id);

			Self::deposit_event(Event::Filled {
				order_id,
				who,
				amount_in: order.amount_in,
				amount_out: order.amount_out,
			});
			Ok(())
		}

		/// Cancel an open OTC order
		///  
		/// Parameters:
		/// - `order_id`: ID of the order
		/// - `asset`: Asset which is being filled
		/// - `amount`: Amount which is being filled
		///
		/// Validations:
		/// - caller is order owner
		///
		/// Emits `Cancelled` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_order())]
		pub fn cancel_order(origin: OriginFor<T>, order_id: OrderId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			<Orders<T>>::try_mutate_exists(order_id, |maybe_order| -> DispatchResult {
				let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				ensure!(order.owner == who, Error::<T>::Forbidden);

				let remaining_to_unreserve =
					T::Currency::unreserve_named(&NAMED_RESERVE_ID, order.asset_out, &order.owner, order.amount_out);
				ensure!(remaining_to_unreserve.is_zero(), Error::<T>::InsufficientReservedAmount);
				*maybe_order = None;

				Self::deposit_event(Event::Cancelled { order_id });
				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	fn ensure_min_order_amount(asset: T::AssetId, amount: Balance) -> DispatchResult {
		let min_amount = T::ExistentialDeposits::get(&asset)
			.checked_mul(T::ExistentialDepositMultiplier::get().into())
			.ok_or(Error::<T>::MathError)?;

		ensure!(amount >= min_amount, Error::<T>::OrderAmountTooSmall);

		Ok(())
	}

	#[require_transactional]
	fn execute_order(
		order: &Order<T::AccountId, T::AssetId>,
		who: &T::AccountId,
		amount_in: Balance,
		amount_out: Balance,
	) -> DispatchResult {
		T::Currency::transfer(order.asset_in, who, &order.owner, amount_in)?;
		let remaining_to_unreserve =
			T::Currency::unreserve_named(&NAMED_RESERVE_ID, order.asset_out, &order.owner, amount_out);
		ensure!(remaining_to_unreserve.is_zero(), Error::<T>::InsufficientReservedAmount);
		T::Currency::transfer(order.asset_out, &order.owner, who, amount_out)?;

		Ok(())
	}
}
