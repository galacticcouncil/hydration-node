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

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{pallet_prelude::*, PalletId};
use frame_system::{
	offchain::{SendTransactionTypes, SubmitTransaction},
	pallet_prelude::{BlockNumberFor, OriginFor},
	RawOrigin,
};
use hydradx_traits::router::{
	AmmTradeWeights, AmountInAndOut, AssetPair, RouteProvider, RouteSpotPriceProvider, RouterT, Trade,
};
use orml_traits::{GetByKey, MultiCurrency};
use pallet_otc::weights::WeightInfo as OtcWeightInfo;
pub use pallet_otc::OrderId;
use sp_arithmetic::traits::{CheckedMul, Saturating};
use sp_arithmetic::{ArithmeticError, FixedPointNumber, FixedU128};
use sp_runtime::offchain::storage::StorageValueRef;
use sp_runtime::offchain::storage_lock::{StorageLock, Time};
use sp_runtime::offchain::Duration;
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec;
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const PALLET_ID: PalletId = PalletId(*b"otcsettl");

// value taken from https://github.com/substrate-developer-hub/recipes/blob/master/pallets/ocw-demo/src/lib.rs
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

/// Vector of `SortedOtcsStorageType`
pub const OFFCHAIN_WORKER_DATA: &[u8] = b"hydradx/otc-settlements/data/";
/// Last block number when we updated the `OFFCHAIN_WORKER_DATA`
pub const OFFCHAIN_WORKER_DATA_LAST_UPDATE: &[u8] = b"hydradx/otc-settlements/data-last-update/";
pub const SORTED_ORDERS_LOCK: &[u8] = b"hydradx/otc-settlements/lock/";
pub const LOCK_TIMEOUT_EXPIRATION: u64 = 5_000; // 5 seconds
/// The number of iterations in the binary search algorithm
pub const FILL_SEARCH_ITERATIONS: u32 = 40;

pub type AssetIdOf<T> = <T as pallet_otc::Config>::AssetId;
type SortedOtcsStorageType = (OrderId, FixedU128, FixedU128, FixedU128);

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_otc::Config + SendTransactionTypes<Call<Self>> {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Named reservable multi currency
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetIdOf<Self>, Balance = Balance>;

		/// Router implementation
		type Router: RouteProvider<AssetIdOf<Self>>
			+ RouterT<Self::RuntimeOrigin, AssetIdOf<Self>, Balance, Trade<AssetIdOf<Self>>, AmountInAndOut<Balance>>
			+ RouteSpotPriceProvider<AssetIdOf<Self>>;

		/// Provider of existential deposits.
		type ExistentialDeposits: GetByKey<AssetIdOf<Self>, Balance>;

		/// Determines the minimum profit
		#[pallet::constant]
		type ExistentialDepositMultiplier: Get<u8>;

		/// Account who receives the profit.
		#[pallet::constant]
		type ProfitReceiver: Get<Self::AccountId>;

		/// Determines when we consider an arbitrage as closed.
		#[pallet::constant]
		type PricePrecision: Get<FixedU128>;

		/// Router weight information.
		type RouterWeightInfo: AmmTradeWeights<Trade<AssetIdOf<Self>>>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			// limit the cases when the offchain worker run
			if sp_io::offchain::is_validator() {
				Self::sort_otcs(block_number);
				Self::settle_otcs();
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
					// receiving unsigned transaction from network - disallow
					return InvalidTransaction::Call.into();
				}
				TransactionSource::Local => {}   // produced by off-chain worker
				TransactionSource::InBlock => {} // some other node included it in a block
			};

			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("settle-otc-with-router")
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides([&provide])
					.longevity(3)
					.propagate(false)
					.build()
			};

			match call {
				Call::settle_otc_order { .. } => valid_tx(b"settle_otc_order".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A trade has been executed
		Executed { asset_id: AssetIdOf<T>, profit: Balance },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Otc order not found
		OrderNotFound,
		/// OTC order is not partially fillable
		NotPartiallyFillable,
		/// Provided route doesn't match the existing route
		InvalidRoute,
		/// Initial and final balance are different
		BalanceInconsistency,
		/// Trade amount higher than necessary
		TradeAmountTooHigh,
		/// Trade amount lower than necessary
		TradeAmountTooLow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Close an existing OTC arbitrage opportunity.
		///
		/// Executes a trade between an OTC order and some route.
		/// If the OTC order is partially fillable, the extrinsic fails if the existing arbitrage
		/// opportunity is not closed after the trade.
		/// If the OTC order is not partially fillable, fails if there is no profit after the trade.
		///
		/// `Origin` calling this extrinsic is not paying or receiving anything.
		///
		/// The profit made by closing the arbitrage is transferred to `FeeReceiver`.
		///
		/// Parameters:
		/// - `origin`: Signed or unsigned origin. Unsigned origin doesn't pay the TX fee,
		/// 			but can be submitted only by a collator.
		/// - `otc_id`: ID of the OTC order with existing arbitrage opportunity.
		/// - `amount`: Amount necessary to clone the arb.
		/// - `route`: The route we trade against. Required for the fee calculation.
		///
		/// Emits `Executed` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::settle_otc_order()
			.saturating_add(<T as Config>::RouterWeightInfo::sell_weight(route))
		.saturating_add(<T as Config>::RouterWeightInfo::get_route_weight())
		.saturating_add(<T as Config>::RouterWeightInfo::calculate_spot_price_with_fee_weight(route))
		.saturating_add(<T as pallet_otc::Config>::WeightInfo::fill_order().max(<T as pallet_otc::Config>::WeightInfo::partial_fill_order()))
		)]
		pub fn settle_otc_order(
			_origin: OriginFor<T>,
			otc_id: OrderId,
			amount: Balance,
			route: Vec<Trade<AssetIdOf<T>>>,
		) -> DispatchResult {
			Self::settle_otc(otc_id, amount, route)
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		PALLET_ID.into_account_truncating()
	}

	/// Ensure that the profit is more than some minimum amount.
	fn ensure_min_profit(asset: T::AssetId, _profit: Balance) -> DispatchResult {
		let _min_amount = <T as Config>::ExistentialDeposits::get(&asset)
			.checked_mul(<T as Config>::ExistentialDepositMultiplier::get().into())
			.ok_or(ArithmeticError::Overflow)?;

		// In the benchmark we doesn't make any trade, so this check would fail.
		#[cfg(not(feature = "runtime-benchmarks"))]
		// tell the binary search algorithm to find higher values
		ensure!(_profit >= _min_amount, Error::<T>::TradeAmountTooLow);

		Ok(())
	}

	/// Executes two trades: asset_a -> OTC -> asset_b, and asset_b -> Router -> asset_a.
	///
	/// If the OTC order is partially fillable, the extrinsic fails if the existing arbitrage
	/// opportunity is not closed after the trade.
	/// If the OTC order is not partially fillable, fails if there is no profit after the trade.
	pub fn settle_otc(otc_id: OrderId, amount: Balance, route: Vec<Trade<AssetIdOf<T>>>) -> DispatchResult {
		let pallet_acc = Self::account_id();

		let otc = <pallet_otc::Orders<T>>::get(otc_id).ok_or(Error::<T>::OrderNotFound)?;
		let (asset_a, asset_b) = (otc.asset_in, otc.asset_out);

		// get initial account balances
		let asset_a_balance_before = <T as Config>::Currency::free_balance(asset_a, &pallet_acc);
		let asset_b_balance_before = <T as Config>::Currency::free_balance(asset_b, &pallet_acc);

		<T as Config>::Currency::deposit(asset_a, &pallet_acc, amount)?;

		if !otc.partially_fillable {
			ensure!(otc.amount_out == amount, Error::<T>::NotPartiallyFillable);
		}

		ensure!(
			route
				== T::Router::get_route(AssetPair {
					asset_in: asset_b,
					asset_out: asset_a,
				}),
			Error::<T>::InvalidRoute
		);

		// get initial otc and router price
		let otc_price =
			FixedU128::checked_from_rational(otc.amount_out, otc.amount_in).ok_or(ArithmeticError::Overflow)?;

		// Router trade is disabled in the benchmarks, so disable this one as well.
		// Without disabling it, the requirements for the extrinsic cannot be met (e.g. profit).
		#[cfg(not(feature = "runtime-benchmarks"))]
		if otc.partially_fillable && amount != otc.amount_in {
			log::debug!(
			target: "offchain_worker::settle_otc",
				"calling partial fill order: amount {:?} ", amount);
			pallet_otc::Pallet::<T>::partial_fill_order(RawOrigin::Signed(pallet_acc.clone()).into(), otc_id, amount)?;
		} else {
			log::debug!(
			target: "offchain_worker::settle_otc",
				"calling fill order");
			pallet_otc::Pallet::<T>::fill_order(RawOrigin::Signed(pallet_acc.clone()).into(), otc_id)?;
		};

		let otc_amount_out = <T as Config>::Currency::free_balance(asset_b, &pallet_acc)
			.checked_sub(asset_b_balance_before)
			.unwrap();

		log::debug!(
			target: "offchain_worker::settle_otc",
			"calling router sell: amount_in {:?} ", otc_amount_out);

		// Disable in the benchmarks and use existing weight from the router pallet.
		#[cfg(not(feature = "runtime-benchmarks"))]
		T::Router::sell(
			RawOrigin::Signed(pallet_acc.clone()).into(),
			asset_b,
			asset_a,
			otc_amount_out,
			1,
			route.clone(),
		)
		.map_err(|_| Error::<T>::TradeAmountTooHigh)?;
		// There are 3 possible types of error:
		// min trade limit not reached - we start with the largest possible amount, so we can't increase it more.
		// max trade limit reached - we are interested in this one. We can decrease the amount and try again.
		// some other error - we can't handle this one properly.

		// // Compare OTC and Router price
		let router_price_after = T::Router::spot_price_with_fee(&route).unwrap();
		log::debug!(
			target: "offchain_worker::settle_otc",
			"final router price: {:?}   otc_price: {:?} ",
			router_price_after,
			otc_price
		);

		// in the case of fully fillable orders, the resulting price is not important
		if otc.partially_fillable {
			let price_diff = {
				if otc_price > router_price_after {
					otc_price.saturating_sub(router_price_after)
				} else {
					router_price_after.saturating_sub(otc_price)
				}
			};

			let price_precision = T::PricePrecision::get()
				.checked_mul(&otc_price)
				.ok_or(ArithmeticError::Overflow)?;
			if price_diff > price_precision {
				ensure!(router_price_after <= otc_price, Error::<T>::TradeAmountTooHigh);
				ensure!(router_price_after >= otc_price, Error::<T>::TradeAmountTooLow);
			}
		}

		let asset_a_balance_after_router_trade = <T as Config>::Currency::free_balance(asset_a, &pallet_acc);

		let profit = asset_a_balance_after_router_trade
			.checked_sub(amount)
			.ok_or(ArithmeticError::Overflow)?;

		Self::ensure_min_profit(asset_a, profit)?;

		<T as Config>::Currency::transfer(asset_a, &pallet_acc, &T::ProfitReceiver::get(), profit)?;

		<T as Config>::Currency::withdraw(asset_a, &pallet_acc, amount)?;

		let asset_a_balance_after = <T as Config>::Currency::free_balance(asset_a, &pallet_acc);
		let asset_b_balance_after = <T as Config>::Currency::free_balance(asset_b, &pallet_acc);

		ensure!(
			asset_a_balance_after == asset_a_balance_before,
			Error::<T>::BalanceInconsistency
		);
		ensure!(
			asset_b_balance_after == asset_b_balance_before,
			Error::<T>::BalanceInconsistency
		);

		Self::deposit_event(Event::Executed {
			asset_id: asset_a,
			profit,
		});

		Ok(())
	}

	/// Store the latest block number in the offchain storage.
	/// Returns `true` if `block_number` is newer than the block number stored in the storage.
	fn try_update_last_block_storage(block_number: BlockNumberFor<T>) -> bool {
		let last_update_storage = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA_LAST_UPDATE);
		let last_update = last_update_storage
			.get::<BlockNumberFor<T>>()
			.unwrap_or_default()
			.unwrap_or_default();

		if block_number > last_update {
			last_update_storage.set(&block_number);
			true
		} else {
			false
		}
	}

	/// Sort open OTCs orders and save a list in the offchain storage.
	fn sort_otcs(block_number: BlockNumberFor<T>) {
		log::debug!(
			target: "offchain_worker::sort_otcs",
			"sort_otcs()");

		// acquire offchain worker lock.
		let lock_expiration = Duration::from_millis(LOCK_TIMEOUT_EXPIRATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(SORTED_ORDERS_LOCK, lock_expiration);

		if Self::try_update_last_block_storage(block_number) {
			if let Ok(_guard) = lock.try_lock() {
				let sorted_otcs = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);

				let mut list = vec![];
				for (otc_id, otc) in <pallet_otc::Orders<T>>::iter() {
					let otc_price = FixedU128::checked_from_rational(otc.amount_out, otc.amount_in);

					let route = T::Router::get_route(AssetPair {
						asset_in: otc.asset_out,
						asset_out: otc.asset_in,
					});
					let router_price_before = T::Router::spot_price_with_fee(&route.clone());

					if let (Some(otc_price), Some(router_price)) = (otc_price, router_price_before) {
						// otc's with no arb opportunity are at the end of the list and are not sorted
						let price_diff = otc_price.saturating_sub(router_price);
						list.push((otc_id, otc_price, router_price, price_diff));
					}
				}

				// sort the list by the price diff
				list.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());

				sorted_otcs.set(&list);
			};
		}
	}

	/// Iterate over sorted list of OTCs and try to find arbitrage opportunities.
	fn settle_otcs() {
		log::debug!(
			target: "offchain_worker::settle_otcs",
			"settle OTCs");
		// iterate over sorted OTCs
		let sorted_otcs_storage = StorageValueRef::persistent(OFFCHAIN_WORKER_DATA);
		let sorted_otcs = sorted_otcs_storage
			.get::<Vec<SortedOtcsStorageType>>()
			.unwrap_or_default()
			.unwrap_or_default();

		for (otc_id, otc_price, router_price_before, _price_diff) in sorted_otcs.iter() {
			log::debug!(
			target: "offchain_worker::settle_otcs",
				"test OTC id {:?} ", otc_id);

			if router_price_before > otc_price {
				log::debug!(
			target: "offchain_worker::settle_otcs",
					"no arb, skipping OTC: {:?}", otc_id);
				continue;
			}

			let otc = <pallet_otc::Orders<T>>::get(otc_id).unwrap();
			let route = T::Router::get_route(AssetPair {
				asset_in: otc.asset_out,
				asset_out: otc.asset_in,
			});
			let maybe_amount = Self::try_find_trade_amount(*otc_id, route.clone());
			if let Some(sell_amt) = maybe_amount {
				log::debug!(
				target: "offchain_worker::settle_otcs",
						"Sending TX for OTC id: {:?} amount: {:?}",
						otc_id,
						sell_amt
					);
				let call = Call::settle_otc_order {
					otc_id: *otc_id,
					amount: sell_amt,
					route,
				};
				let _ = SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into());
			}
		}
	}

	/// Try to find the correct amount to close the arbitrage opportunity.
	fn try_find_trade_amount(otc_id: OrderId, route: Vec<Trade<AssetIdOf<T>>>) -> Option<Balance> {
		let otc = <pallet_otc::Orders<T>>::get(otc_id).unwrap();

		// use binary search to determine the correct sell amount
		let mut sell_amt = otc.amount_in; // start by trying to fill the whole order
		let mut sell_amt_up = sell_amt;
		let mut sell_amt_down = 0; // TODO: set to some min trade amount

		let iters = if otc.partially_fillable {
			FILL_SEARCH_ITERATIONS
		} else {
			1
		};
		for i in 0..iters {
			log::debug!(
			target: "offchain_worker::settle_otcs",
				"iteration: {:?}", i);
			log::debug!(
			target: "offchain_worker::settle_otcs::binary_search",
				"\nsell_amt: {:?}\nsell_amt_up: {:?}\nsell_amt_down: {:?}", sell_amt, sell_amt_up, sell_amt_down);
			match Self::settle_otc_order(RawOrigin::None.into(), otc_id, sell_amt, route.clone()) {
				Ok(_) => {
					log::debug!(
					target: "offchain_worker::settle_otcs",
								"Extrinsic executed successfully for OTC id: {:?} amount: {:?}",
								otc_id,
								sell_amt
							);
					return Some(sell_amt);
				}
				Err(error) => {
					if error == Error::<T>::TradeAmountTooHigh.into() {
						log::debug!(
						   target: "offchain_worker::settle_otcs",
							"Extrinsic failed: trade amount too high for OTC id: {:?} amount: {:?}", otc_id, sell_amt);

						sell_amt_up = sell_amt;
					} else if error == Error::<T>::TradeAmountTooLow.into() {
						log::debug!(
						   target: "offchain_worker::settle_otcs",
							"Extrinsic failed: trade amount too low for OTC id: {:?} amount: {:?}", otc_id, sell_amt);

						sell_amt_down = sell_amt;
					} else {
						log::debug!(
						   target: "offchain_worker::settle_otcs",
							"Extrinsic failed with error for OTC id: {:?} amount: {:?} error: {:?}", otc_id, sell_amt, error);
						return None;
					}
				}
			}
			if sell_amt_down == sell_amt_up {
				return None;
			}
			sell_amt = (sell_amt_up + sell_amt_down) / 2;
		}
		None
	}
}
