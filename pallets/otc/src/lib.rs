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
// whether the order is partially fillable. Fee is applied to all trades and is deducted from the `amount_out`.
// Because of the fee, the order price is static and calculated as `(amount_out - fee) / amount_in`.
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
#![allow(clippy::manual_inspect)]

use codec::MaxEncodedLen;
use frame_support::{pallet_prelude::*, require_transactional, traits::ExistenceRequirement};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use hydradx_traits::Inspect;
use orml_traits::{GetByKey, MultiCurrency, NamedMultiReservableCurrency};
use pallet_broadcast::types::Destination;
use pallet_broadcast::types::Fee;
use sp_core::U256;
use sp_runtime::traits::{One, Zero};
use sp_runtime::Permill;
use sp_std::vec;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod weights;

pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
use pallet_broadcast::types::Asset;
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
	pub trait Config: frame_system::Config + pallet_broadcast::Config {
		/// Identifier for the class of asset.
		type AssetId: Member + Parameter + Copy + HasCompact + MaybeSerializeDeserialize + MaxEncodedLen + Into<u32>;

		/// Asset Registry mechanism - used to check if asset is correctly registered in asset registry.
		type AssetRegistry: Inspect<AssetId = Self::AssetId>;

		/// Named reservable multi currency.
		type Currency: NamedMultiReservableCurrency<
			Self::AccountId,
			ReserveIdentifier = NamedReserveIdentifier,
			CurrencyId = Self::AssetId,
			Balance = Balance,
		>;

		/// Existential deposits provider.
		type ExistentialDeposits: GetByKey<Self::AssetId, Balance>;

		#[pallet::constant]
		/// Multiplier used to compute minimal amounts of asset_in and asset_out in an OTC.
		type ExistentialDepositMultiplier: Get<u8>;

		/// Fee deducted from amount_out.
		#[pallet::constant]
		type Fee: Get<Permill>;

		/// Fee receiver.
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// An Order has been cancelled
		Cancelled { order_id: OrderId },
		/// An Order has been completely filled
		/// Deprecated. Replaced by pallet_broadcast::Swapped
		Filled {
			order_id: OrderId,
			who: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
			fee: Balance,
		},
		/// An Order has been partially filled
		/// Deprecated. Replaced by pallet_broadcast::Swapped
		PartiallyFilled {
			order_id: OrderId,
			who: T::AccountId,
			amount_in: Balance,
			amount_out: Balance,
			fee: Balance,
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

			let fee = Self::calculate_fee(order.amount_out);

			Self::ensure_min_order_amount(order.asset_in, order.amount_in)?;
			// the fee is applied to amount_out
			Self::ensure_min_order_amount(
				order.asset_out,
				order.amount_out.checked_sub(fee).ok_or(Error::<T>::MathError)?,
			)?;

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
		/// `PartiallyFilled` event when successful. Deprecated.
		/// `pallet_broadcast::Swapped` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::partial_fill_order())]
		pub fn partial_fill_order(origin: OriginFor<T>, order_id: OrderId, amount_in: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			<Orders<T>>::try_mutate(order_id, |maybe_order| -> DispatchResult {
				let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;

				ensure!(order.partially_fillable, Error::<T>::OrderNotPartiallyFillable);

				let amount_out = Self::partial_amount_out(order.amount_out, amount_in, order.amount_in)?;
				let fee = Self::calculate_fee(amount_out);

				let new_amount_in = order.amount_in.checked_sub(amount_in).ok_or(Error::<T>::MathError)?;
				let new_amount_out = order.amount_out.checked_sub(amount_out).ok_or(Error::<T>::MathError)?;
				Self::ensure_remaining_order_valid(order.asset_in, order.asset_out, new_amount_in, new_amount_out)?;

				order.amount_in = new_amount_in;
				order.amount_out = new_amount_out;

				Self::execute_order(order, &who, amount_in, amount_out, fee)?;

				Self::deposit_fill_events(
					order_id,
					&order.owner,
					&who,
					order.asset_in,
					order.asset_out,
					amount_in,
					amount_out,
					fee,
					false,
				);

				Ok(())
			})
		}

		/// Fill an OTC order (completely)
		///  
		/// Parameters:
		/// - `order_id`: ID of the order
		///
		/// Events:
		/// `Filled` event when successful. Deprecated.
		/// `pallet_broadcast::Swapped` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::fill_order())]
		pub fn fill_order(origin: OriginFor<T>, order_id: OrderId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let order = <Orders<T>>::get(order_id).ok_or(Error::<T>::OrderNotFound)?;

			let fee = Self::calculate_fee(order.amount_out);

			Self::execute_order(&order, &who, order.amount_in, order.amount_out, fee)?;
			<Orders<T>>::remove(order_id);

			Self::deposit_fill_events(
				order_id,
				&order.owner,
				&who,
				order.asset_in,
				order.asset_out,
				order.amount_in,
				order.amount_out,
				fee,
				true,
			);

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
		fee: Balance,
	) -> DispatchResult {
		// Filler pays the maker first, so the maker's account keeps a live balance while its
		// reserved asset_out is released back.
		T::Currency::transfer(
			order.asset_in,
			who,
			&order.owner,
			amount_in,
			ExistenceRequirement::AllowDeath,
		)?;
		Self::release_reserved_asset_out(order.asset_out, &order.owner, who, amount_out, fee)
	}

	/// Pro-rata `amount_out` for filling `amount_in` of an order priced `order_amount_out : order_amount_in`.
	fn partial_amount_out(
		order_amount_out: Balance,
		amount_in: Balance,
		order_amount_in: Balance,
	) -> Result<Balance, DispatchError> {
		let amount_out = U256::from(order_amount_out)
			.checked_mul(U256::from(amount_in))
			.and_then(|v| v.checked_div(U256::from(order_amount_in)))
			.ok_or(Error::<T>::MathError)?;
		Ok(Balance::try_from(amount_out).map_err(|_| Error::<T>::MathError)?)
	}

	/// Ensure the residual of a partially filled order still clears the minimum order amount on both
	/// legs. The asset_out leg is checked net of the fee the *remaining* order would pay when filled,
	/// matching `place_order`, so an order is fillable through every entry point on the same terms.
	fn ensure_remaining_order_valid(
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		new_amount_in: Balance,
		new_amount_out: Balance,
	) -> DispatchResult {
		let remaining_fee = Self::calculate_fee(new_amount_out);
		Self::ensure_min_order_amount(asset_in, new_amount_in)?;
		Self::ensure_min_order_amount(
			asset_out,
			new_amount_out.checked_sub(remaining_fee).ok_or(Error::<T>::MathError)?,
		)
	}

	/// Release the maker's reserved `asset_out` for a fill: unreserve `amount_out`, pay the filler the
	/// amount net of fee, and send the fee to the fee receiver.
	fn release_reserved_asset_out(
		asset_out: T::AssetId,
		owner: &T::AccountId,
		filler: &T::AccountId,
		amount_out: Balance,
		fee: Balance,
	) -> DispatchResult {
		let remaining_to_unreserve =
			// returns any amount that was unable to be unreserved
			T::Currency::unreserve_named(&NAMED_RESERVE_ID, asset_out, owner, amount_out);
		ensure!(remaining_to_unreserve.is_zero(), Error::<T>::InsufficientReservedAmount);

		let amount_out_without_fee = amount_out.checked_sub(fee).ok_or(Error::<T>::MathError)?;
		T::Currency::transfer(
			asset_out,
			owner,
			filler,
			amount_out_without_fee,
			ExistenceRequirement::AllowDeath,
		)?;
		T::Currency::transfer(
			asset_out,
			owner,
			&T::FeeReceiver::get(),
			fee,
			ExistenceRequirement::AllowDeath,
		)?;
		Ok(())
	}

	/// Emit the (deprecated) `Filled`/`PartiallyFilled` event and the broadcast `Swapped` event for a
	/// fill. Full and partial fills report the swapper/filler in opposite orders; that is preserved.
	#[allow(clippy::too_many_arguments)]
	fn deposit_fill_events(
		order_id: OrderId,
		owner: &T::AccountId,
		filler: &T::AccountId,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: Balance,
		amount_out: Balance,
		fee: Balance,
		is_full_fill: bool,
	) {
		// TODO: Deprecated, remove when ready
		if is_full_fill {
			Self::deposit_event(Event::Filled {
				order_id,
				who: filler.clone(),
				amount_in,
				amount_out,
				fee,
			});
		} else {
			Self::deposit_event(Event::PartiallyFilled {
				order_id,
				who: filler.clone(),
				amount_in,
				amount_out,
				fee,
			});
		}

		let (swapper, broadcast_filler) = if is_full_fill {
			(filler.clone(), owner.clone())
		} else {
			(owner.clone(), filler.clone())
		};

		pallet_broadcast::Pallet::<T>::deposit_trade_event(
			swapper,
			broadcast_filler,
			pallet_broadcast::types::Filler::OTC(order_id),
			pallet_broadcast::types::TradeOperation::ExactIn,
			vec![Asset::new(asset_in.into(), amount_in)],
			vec![Asset::new(asset_out.into(), amount_out)],
			vec![Fee {
				asset: asset_out.into(),
				amount: fee,
				destination: Destination::Account(T::FeeReceiver::get()),
			}],
		);
	}

	/// Fill an order (fully or partially) where the filler provides `asset_in` *after* receiving
	/// `asset_out`, instead of before.
	///
	/// The maker's reserved `asset_out` for the filled portion (minus fee) is released to `filler`
	/// first, then `deliver` is invoked with that amount so the filler can source `asset_in` from it
	/// (e.g. by trading it through a pool), and finally `amount_in` of `asset_in` is pulled from
	/// `filler` to the maker to complete the fill. The final transfer fails if `deliver` did not
	/// leave `filler` holding at least `amount_in` of `asset_in`.
	///
	/// Must run inside a transaction: a failure in `deliver` or in any transfer rolls the whole fill
	/// back, so the order and the maker's reserve are never left half-settled.
	#[require_transactional]
	pub fn fill_order_with_deferred_delivery<F>(
		order_id: OrderId,
		filler: &T::AccountId,
		amount_in: Balance,
		deliver: F,
	) -> DispatchResult
	where
		F: FnOnce(Balance) -> DispatchResult,
	{
		<Orders<T>>::try_mutate_exists(order_id, |maybe_order| -> DispatchResult {
			let (owner, asset_in, asset_out, order_amount_in, order_amount_out, partially_fillable) = {
				let order = maybe_order.as_ref().ok_or(Error::<T>::OrderNotFound)?;
				(
					order.owner.clone(),
					order.asset_in,
					order.asset_out,
					order.amount_in,
					order.amount_out,
					order.partially_fillable,
				)
			};

			ensure!(amount_in <= order_amount_in, Error::<T>::MathError);
			let is_full_fill = amount_in == order_amount_in;

			let amount_out = if is_full_fill {
				order_amount_out
			} else {
				ensure!(partially_fillable, Error::<T>::OrderNotPartiallyFillable);
				Self::partial_amount_out(order_amount_out, amount_in, order_amount_in)?
			};

			let fee = Self::calculate_fee(amount_out);
			let amount_out_without_fee = amount_out.checked_sub(fee).ok_or(Error::<T>::MathError)?;

			// Validate the residual order before the router trade so a doomed fill is rejected cheaply.
			let remaining = if is_full_fill {
				None
			} else {
				let new_amount_in = order_amount_in.checked_sub(amount_in).ok_or(Error::<T>::MathError)?;
				let new_amount_out = order_amount_out.checked_sub(amount_out).ok_or(Error::<T>::MathError)?;
				Self::ensure_remaining_order_valid(asset_in, asset_out, new_amount_in, new_amount_out)?;
				Some((new_amount_in, new_amount_out))
			};

			// Collect-before-deliver drains the maker's `asset_out` before `asset_in` is handed back;
			// hold a provider reference so a maker holding only `asset_out` is not reaped (nonce reset)
			// in the gap. The whole call is transactional, so an early return rolls this back too.
			frame_system::Pallet::<T>::inc_providers(&owner);

			Self::release_reserved_asset_out(asset_out, &owner, filler, amount_out, fee)?;

			// Filler sources asset_in using the funds it just received.
			deliver(amount_out_without_fee)?;

			// Filler delivers asset_in to the maker, completing the fill.
			T::Currency::transfer(asset_in, filler, &owner, amount_in, ExistenceRequirement::AllowDeath)?;

			frame_system::Pallet::<T>::dec_providers(&owner)?;

			match remaining {
				None => {
					*maybe_order = None;
				}
				Some((new_amount_in, new_amount_out)) => {
					let order = maybe_order.as_mut().ok_or(Error::<T>::OrderNotFound)?;
					order.amount_in = new_amount_in;
					order.amount_out = new_amount_out;
				}
			}

			Self::deposit_fill_events(
				order_id,
				&owner,
				filler,
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				fee,
				is_full_fill,
			);

			Ok(())
		})
	}

	pub fn calculate_fee(amount: Balance) -> Balance {
		T::Fee::get().mul_ceil(amount)
	}
}
