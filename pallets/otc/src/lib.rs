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
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_runtime::FixedU128;
use sp_runtime::traits::{BlockNumberProvider, ConstU32, One};
use sp_runtime::ArithmeticError;
use sp_runtime::{BoundedVec, DispatchError};
use sp_std::{result, vec::Vec};
use hydradx_traits::Registry;

#[cfg(test)]
mod tests;

pub mod types;
pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

use crate::types::*;

type BlockNumberFor<T> = <T as frame_system::Config>::BlockNumber;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{EncodeLike, HasCompact};

  use frame_system::pallet_prelude::OriginFor;
  use orml_traits::MultiReservableCurrency;

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

    /// Asset Registry mechanism - used to check if asset is correctly registered in asset registry
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError>;
    
    /// The block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

    type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

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
		/// Emitted after an Order has been placed
		OrderPlaced {
			order_id: OrderId,
		},
    /// An Order has been (partially) filled
    OrderFill {
      order_id: OrderId,
      who: T::AccountId,
      amount: Balance,
    },
	}

  #[pallet::error]
	pub enum Error<T> {
    /// Asset does not exist in registry
    AssetNotRegistered,
    /// Order is expired
    OrderExpired,
		/// Order cannot be found
		OrderNotFound,
    /// Size of order ID exceeds the bound
    OrderIdOutOfBound,
    /// Free balance is too low to place the order
    InsufficientBalance,
  }

  /// ID sequencer for Orders
	#[pallet::storage]
	#[pallet::getter(fn next_order_id)]
	pub type NextOrderId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

  #[pallet::storage]
	#[pallet::getter(fn orders)]
	pub type Orders<T: Config> =
		StorageMap<_, Blake2_128Concat, OrderId, Order<T::AccountId, T::AssetId, BlockNumberFor<T>>, OptionQuery>;

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
      expires: Option<T::BlockNumber>,
    ) -> DispatchResult {
      let who = ensure_signed(origin)?;

      let order = Order { who, asset_buy, asset_sell, amount_buy, amount_sell, expires };

      Self::validate_order(order.clone())?;

      let order_id = <NextOrderId<T>>::try_mutate(|next_id| -> result::Result<OrderId, DispatchError> {
        let current_id = *next_id;
        *next_id = next_id
          .checked_add(One::one())
          .ok_or(Error::<T>::OrderIdOutOfBound)?;
        Ok(current_id)
      })?;

      T::MultiReservableCurrency::reserve(order.asset_sell, &order.who, order.amount_sell)?;

      <Orders<T>>::insert(order_id, order);
      Self::deposit_event(Event::OrderPlaced { order_id: order_id });

      Ok(())
    }
  }
}


impl<T: Config> Pallet<T> {
  fn validate_order(order: Order<T::AccountId, T::AssetId, BlockNumberFor<T>>) -> DispatchResult {
    ensure!(
      T::AssetRegistry::exists(order.asset_sell),
      Error::<T>::AssetNotRegistered
    );

    ensure!(
      T::AssetRegistry::exists(order.asset_buy),
      Error::<T>::AssetNotRegistered
    );

    ensure!(
      T::MultiReservableCurrency::can_reserve(order.asset_sell.clone(), &order.who, order.amount_sell),
      Error::<T>::InsufficientBalance
    );

    if let Some(block_number) = order.expires {
      let current_block_number = T::BlockNumberProvider::current_block_number();
      ensure!(
        block_number > current_block_number,
        Error::<T>::OrderExpired
      );
    }

    Ok(())
  }
}
