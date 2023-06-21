// This file is part of pallet-route-executor.

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
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::Get;
use frame_support::transactional;
use frame_system::ensure_signed;
use hydradx_traits::router::TradeExecution;
use hydradx_traits::router::{ExecutorError, PoolType};
use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use scale_info::TypeInfo;
use sp_runtime::DispatchError;
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

pub mod weights;

use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub trait TradeAmountsCalculator<AssetId, Balance> {
	fn calculate_buy_trade_amounts(
		route: &[Trade<AssetId>],
		amount_out: Balance,
	) -> Result<Vec<AmountInAndOut<Balance>>, DispatchError>;

	fn calculate_sell_trade_amounts(
		route: &[Trade<AssetId>],
		amount_in: Balance,
	) -> Result<Vec<AmountInAndOut<Balance>>, DispatchError>;
}

///A single trade for buy/sell, describing the asset pair and the pool type in which the trade is executed
#[derive(Encode, Decode, Debug, Eq, PartialEq, Copy, Clone, TypeInfo, MaxEncodedLen)]
pub struct Trade<AssetId> {
	pub pool: PoolType<AssetId>,
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

pub struct AmountInAndOut<Balance> {
	pub amount_in: Balance,
	pub amount_out: Balance,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::router::ExecutorError;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset id type
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen;

		/// Balance type
		type Balance: Parameter
			+ Member
			+ Copy
			+ PartialOrd
			+ MaybeSerializeDeserialize
			+ Default
			+ CheckedSub
			+ CheckedAdd;

		/// Max limit for the number of trades within a route
		#[pallet::constant]
		type MaxNumberOfTrades: Get<u8>;

		/// Currency for checking balances
		type Currency: Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Self::Balance>;

		/// Handlers for AMM pools to calculate and execute trades
		type AMM: TradeExecution<
			<Self as frame_system::Config>::RuntimeOrigin,
			Self::AccountId,
			Self::AssetId,
			Self::Balance,
			Error = DispatchError,
		>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///The route with trades has been successfully executed
		RouteExecuted {
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: T::Balance,
			amount_out: T::Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		///The trading limit has been reached
		TradingLimitReached,
		///The the max number of trades limit is reached
		MaxTradesExceeded,
		///The AMM pool is not supported for executing trades
		PoolNotSupported,
		/// Route has not trades to be executed
		RouteHasNoTrades,
		///The user has not enough balance to execute the trade
		InsufficientBalance,
		///Unexpected error which should never really happen, but the error case must be handled to prevent panics.
		UnexpectedError,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Executes a sell with a series of trades specified in the route.
		/// The price for each trade is determined by the corresponding AMM.
		///
		/// - `origin`: The executor of the trade
		/// - `asset_in`: The identifier of the asset to sell
		/// - `asset_out`: The identifier of the asset to receive
		/// - `amount_in`: The amount of `asset_in` to sell
		/// - `min_amount_out`: The minimum amount of `asset_out` to receive.
		/// - `route`: Series of [`Trade<AssetId>`] to be executed. A [`Trade<AssetId>`] specifies the asset pair (`asset_in`, `asset_out`) and the AMM (`pool`) in which the trade is executed.
		///
		/// Emits `RouteExecuted` when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::sell(route.len() as u32))]
		#[transactional]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: T::Balance,
			min_amount_out: T::Balance,
			route: Vec<Trade<T::AssetId>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_route_size(route.len())?;

			let user_balance_of_asset_in_before_trade = T::Currency::reducible_balance(asset_in, &who, false);
			let user_balance_of_asset_out_before_trade = T::Currency::reducible_balance(asset_out, &who, false);
			ensure!(
				user_balance_of_asset_in_before_trade >= amount_in,
				Error::<T>::InsufficientBalance
			);

			let trade_amounts = Self::calculate_sell_trade_amounts(&route, amount_in)?;

			let last_trade_amount = trade_amounts.last().ok_or(Error::<T>::UnexpectedError)?;
			ensure!(
				last_trade_amount.amount_out >= min_amount_out,
				Error::<T>::TradingLimitReached
			);

			for (trade_amount, trade) in trade_amounts.iter().zip(route) {
				let user_balance_of_asset_in_before_trade = T::Currency::reducible_balance(trade.asset_in, &who, true);

				let execution_result = T::AMM::execute_sell(
					origin.clone(),
					trade.pool,
					trade.asset_in,
					trade.asset_out,
					trade_amount.amount_in,
					trade_amount.amount_out,
				);

				handle_execution_error!(execution_result);

				Self::ensure_that_user_spent_asset_in(
					who.clone(),
					trade.asset_in,
					user_balance_of_asset_in_before_trade,
					trade_amount.amount_in,
				)?;
			}

			Self::ensure_that_user_received_asset_out(
				who,
				asset_out,
				user_balance_of_asset_out_before_trade,
				last_trade_amount.amount_out,
			)?;

			Self::deposit_event(Event::RouteExecuted {
				asset_in,
				asset_out,
				amount_in,
				amount_out: last_trade_amount.amount_out,
			});

			Ok(())
		}

		/// Executes a buy with a series of trades specified in the route.
		/// The price for each trade is determined by the corresponding AMM.
		///
		/// - `origin`: The executor of the trade
		/// - `asset_in`: The identifier of the asset to be swapped to buy `asset_out`
		/// - `asset_out`: The identifier of the asset to buy
		/// - `amount_out`: The amount of `asset_out` to buy
		/// - `max_amount_in`: The max amount of `asset_in` to spend on the buy.
		/// - `route`: Series of [`Trade<AssetId>`] to be executed. A [`Trade<AssetId>`] specifies the asset pair (`asset_in`, `asset_out`) and the AMM (`pool`) in which the trade is executed.
		///
		/// Emits `RouteExecuted` when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::buy(route.len() as u32))]
		#[transactional]
		pub fn buy(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_out: T::Balance,
			max_amount_in: T::Balance,
			route: Vec<Trade<T::AssetId>>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			Self::ensure_route_size(route.len())?;

			let user_balance_of_asset_in_before_trade = T::Currency::reducible_balance(asset_in, &who, true);

			let trade_amounts = Self::calculate_buy_trade_amounts(&route, amount_out)?;

			let last_trade_amount = trade_amounts.last().ok_or(Error::<T>::UnexpectedError)?;
			ensure!(
				last_trade_amount.amount_in <= max_amount_in,
				Error::<T>::TradingLimitReached
			);

			for (trade_amount, trade) in trade_amounts.iter().rev().zip(route) {
				let user_balance_of_asset_out_before_trade =
					T::Currency::reducible_balance(trade.asset_out, &who, false);

				let execution_result = T::AMM::execute_buy(
					origin.clone(),
					trade.pool,
					trade.asset_in,
					trade.asset_out,
					trade_amount.amount_out,
					trade_amount.amount_in,
				);

				handle_execution_error!(execution_result);

				Self::ensure_that_user_received_asset_out(
					who.clone(),
					trade.asset_out,
					user_balance_of_asset_out_before_trade,
					trade_amount.amount_out,
				)?;
			}

			Self::ensure_that_user_spent_asset_in(
				who,
				asset_in,
				user_balance_of_asset_in_before_trade,
				last_trade_amount.amount_in,
			)?;

			Self::deposit_event(Event::RouteExecuted {
				asset_in,
				asset_out,
				amount_in: last_trade_amount.amount_in,
				amount_out,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn ensure_route_size(route_length: usize) -> Result<(), DispatchError> {
		ensure!(route_length > 0, Error::<T>::RouteHasNoTrades);
		ensure!(
			(route_length as u8) <= T::MaxNumberOfTrades::get(),
			Error::<T>::MaxTradesExceeded
		);

		Ok(())
	}

	fn ensure_that_user_received_asset_out(
		who: T::AccountId,
		asset_out: T::AssetId,
		user_balance_of_asset_out_before_trade: T::Balance,
		received_amount: T::Balance,
	) -> Result<(), DispatchError> {
		let user_balance_of_asset_out_after_trade = T::Currency::reducible_balance(asset_out, &who, false);
		let user_expected_balance_of_asset_out_after_trade = user_balance_of_asset_out_before_trade
			.checked_add(&received_amount)
			.ok_or(Error::<T>::UnexpectedError)?;

		ensure!(
			user_balance_of_asset_out_after_trade == user_expected_balance_of_asset_out_after_trade,
			Error::<T>::UnexpectedError
		);

		Ok(())
	}

	fn ensure_that_user_spent_asset_in(
		who: T::AccountId,
		asset_in: T::AssetId,
		user_balance_of_asset_in_before_trade: T::Balance,
		spent_amount: T::Balance,
	) -> Result<(), DispatchError> {
		if spent_amount < user_balance_of_asset_in_before_trade {
			let user_balance_of_asset_in_after_trade = T::Currency::reducible_balance(asset_in, &who, true);
			ensure!(
				user_balance_of_asset_in_before_trade - spent_amount == user_balance_of_asset_in_after_trade,
				Error::<T>::UnexpectedError
			);
		}
		Ok(())
	}
}

impl<T: Config> TradeAmountsCalculator<T::AssetId, T::Balance> for Pallet<T> {
	fn calculate_sell_trade_amounts(
		route: &[Trade<T::AssetId>],
		amount_in: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		let mut amount_in_and_outs = Vec::<AmountInAndOut<T::Balance>>::with_capacity(route.len());
		let mut amount_in = amount_in;

		for trade in route.iter() {
			let result = T::AMM::calculate_sell(trade.pool, trade.asset_in, trade.asset_out, amount_in);
			match result {
				Err(ExecutorError::NotSupported) => return Err(Error::<T>::PoolNotSupported.into()),
				Err(ExecutorError::Error(dispatch_error)) => return Err(dispatch_error),
				Ok(amount_out) => {
					amount_in_and_outs.push(AmountInAndOut { amount_in, amount_out });
					amount_in = amount_out;
				}
			}
		}

		Ok(amount_in_and_outs)
	}

	fn calculate_buy_trade_amounts(
		route: &[Trade<T::AssetId>],
		amount_out: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		let mut amount_in_and_outs = Vec::<AmountInAndOut<T::Balance>>::with_capacity(route.len());
		let mut amount_out = amount_out;

		for trade in route.iter().rev() {
			let result = T::AMM::calculate_buy(trade.pool, trade.asset_in, trade.asset_out, amount_out);

			match result {
				Err(ExecutorError::NotSupported) => return Err(Error::<T>::PoolNotSupported.into()),
				Err(ExecutorError::Error(dispatch_error)) => return Err(dispatch_error),
				Ok(amount_in) => {
					amount_in_and_outs.push(AmountInAndOut { amount_in, amount_out });
					amount_out = amount_in;
				}
			}
		}

		Ok(amount_in_and_outs)
	}
}

#[macro_export]
macro_rules! handle_execution_error {
	($execution_result:expr) => {{
		if let Err(error) = $execution_result {
			return match error {
				ExecutorError::NotSupported => Err(Error::<T>::PoolNotSupported.into()),
				ExecutorError::Error(dispatch_error) => Err(dispatch_error),
			};
		}
	}};
}
