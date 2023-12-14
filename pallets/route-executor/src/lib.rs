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

use codec::MaxEncodedLen;
use frame_support::storage::with_transaction;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::PalletId;
use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{fungibles::Inspect, Get},
	transactional,
};

use frame_system::pallet_prelude::OriginFor;
use frame_system::{ensure_signed, Origin};
use hydradx_traits::router::{inverse_route, AssetPair, RouteProvider};
pub use hydradx_traits::router::{
	AmmTradeWeights, AmountInAndOut, ExecutorError, PoolType, RouterT, Trade, TradeExecution,
};
use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use sp_runtime::traits::{AccountIdConversion, CheckedDiv};
use sp_runtime::{ArithmeticError, DispatchError, TransactionOutcome};
use sp_std::{vec, vec::Vec};

#[cfg(test)]
mod tests;

pub mod weights;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const MAX_NUMBER_OF_TRADES: u32 = 5;

//TODO: rebenchmark on reference machine

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::fungibles::Mutate;
	use frame_system::pallet_prelude::OriginFor;
	use hydradx_traits::router::ExecutorError;
	use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedDiv};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset id type
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen + AtLeast32BitUnsigned;

		/// Balance type
		type Balance: Parameter
			+ Member
			+ Copy
			+ PartialOrd
			+ MaybeSerializeDeserialize
			+ From<u128>
			+ Default
			+ CheckedSub
			+ CheckedAdd
			+ CheckedDiv;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Currency for checking balances and temporarily minting tokens
		type Currency: Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Self::Balance>
			+ Mutate<Self::AccountId>;

		/// Handlers for AMM pools to calculate and execute trades
		type AMM: TradeExecution<
			<Self as frame_system::Config>::RuntimeOrigin,
			Self::AccountId,
			Self::AssetId,
			Self::Balance,
			Error = DispatchError,
		>;

		/// Pool type used in the default route
		type DefaultRoutePoolType: Get<PoolType<Self::AssetId>>;

		/// Weight information for the extrinsics.
		type WeightInfo: AmmTradeWeights<Trade<Self::AssetId>>;
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
		///The route with trades has been successfully executed
		RouteUpdated { asset_ids: Vec<T::AssetId> },
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
		///The route execution failed in the underlying AMM
		InvalidRouteExecution,
		///The calculation of route trade amounts failed in the underlying AMM
		RouteCalculationFailed,
		///The route is invalid
		InvalidRoute,
		///The route update was not successful
		RouteUpdateIsNotSuccessful,
	}

	/// Storing routes for asset pairs
	#[pallet::storage]
	#[pallet::getter(fn route)]
	pub type Routes<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		AssetPair<T::AssetId>,
		BoundedVec<Trade<T::AssetId>, ConstU32<MAX_NUMBER_OF_TRADES>>,
	>;

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
		/// 		   If not specified, than the on-chain route is used.
		/// 		   If no on-chain is present, then omnipool route is used as default
		///
		/// Emits `RouteExecuted` when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::sell_weight(route))]
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

			let route = Self::get_route_or_default(route, AssetPair::new(asset_in, asset_out))?;

			let user_balance_of_asset_in_before_trade =
				T::Currency::reducible_balance(asset_in, &who, Preservation::Expendable, Fortitude::Polite);
			let user_balance_of_asset_out_before_trade =
				T::Currency::reducible_balance(asset_out, &who, Preservation::Expendable, Fortitude::Polite);

			ensure!(
				user_balance_of_asset_in_before_trade >= amount_in,
				Error::<T>::InsufficientBalance
			);

			let trade_amounts = Self::calculate_sell_trade_amounts(&route, amount_in)?;

			let last_trade_amount = trade_amounts.last().ok_or(Error::<T>::RouteCalculationFailed)?;
			ensure!(
				last_trade_amount.amount_out >= min_amount_out,
				Error::<T>::TradingLimitReached
			);

			for (trade_amount, trade) in trade_amounts.iter().zip(route) {
				let user_balance_of_asset_in_before_trade =
					T::Currency::reducible_balance(trade.asset_in, &who, Preservation::Preserve, Fortitude::Polite);

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
		/// 		   If not specified, than the on-chain route is used.
		/// 		   If no on-chain is present, then omnipool route is used as default
		///
		/// Emits `RouteExecuted` when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::buy_weight(route))]
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

			let route = Self::get_route_or_default(route, AssetPair::new(asset_in, asset_out))?;

			let user_balance_of_asset_in_before_trade =
				T::Currency::reducible_balance(asset_in, &who, Preservation::Preserve, Fortitude::Polite);

			let trade_amounts = Self::calculate_buy_trade_amounts(&route, amount_out)?;

			let last_trade_amount = trade_amounts.last().ok_or(Error::<T>::RouteCalculationFailed)?;
			ensure!(
				last_trade_amount.amount_in <= max_amount_in,
				Error::<T>::TradingLimitReached
			);

			for (trade_amount, trade) in trade_amounts.iter().rev().zip(route) {
				let user_balance_of_asset_out_before_trade =
					T::Currency::reducible_balance(trade.asset_out, &who, Preservation::Expendable, Fortitude::Polite);

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

		/// Sets the on-chain route for a given asset pair.
		///
		/// The new route is validated by being executed in a dry-run mode
		///
		/// If there is no route explicitly set for an asset pair, then we use the omnipool route as default.
		///
		/// When a new route is set, we compare it to the existing (or default) route.
		/// The comparison happens by calculating sell amount_outs for the routes, but also for the inversed routes.
		///
		/// The route is stored in an ordered manner, based on the oder of the ids in the asset pair.
		///
		/// If the route is set successfully, then the fee is payed back.
		///
		/// - `origin`: The origin of the route setter
		/// - `asset_pair`: The identifier of the asset-pair for which the route is set
		/// - `new_route`: Series of [`Trade<AssetId>`] to be executed. A [`Trade<AssetId>`] specifies the asset pair (`asset_in`, `asset_out`) and the AMM (`pool`) in which the trade is executed.
		///
		/// Emits `RouteUpdated` when successful.
		///
		/// Fails with `RouteUpdateIsNotSuccessful` error when failed to set the route
		///
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_route_weight(new_route))]
		#[transactional]
		pub fn set_route(
			origin: OriginFor<T>,
			mut asset_pair: AssetPair<T::AssetId>,
			mut new_route: Vec<Trade<T::AssetId>>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin.clone())?;
			Self::ensure_route_size(new_route.len())?;

			ensure!(
				asset_pair.asset_in == new_route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in,
				Error::<T>::InvalidRoute
			);
			ensure!(
				asset_pair.asset_out == new_route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out,
				Error::<T>::InvalidRoute
			);

			if !asset_pair.is_ordered() {
				asset_pair = asset_pair.ordered_pair();
				new_route = inverse_route(new_route)
			}

			let existing_route = Self::get_route(asset_pair);

			match Self::validate_route(&existing_route) {
				Ok((reference_amount_in, reference_amount_in_for_inverse)) => {
					let inverse_new_route = inverse_route(new_route.to_vec());
					let inverse_existing_route = inverse_route(existing_route.to_vec());

					Self::validate_sell(new_route.clone(), reference_amount_in)?;
					Self::validate_sell(inverse_new_route.clone(), reference_amount_in_for_inverse)?;

					let amount_out_for_existing_route =
						Self::calculate_expected_amount_out(&existing_route, reference_amount_in)?;
					let amount_out_for_new_route =
						Self::calculate_expected_amount_out(&new_route, reference_amount_in)?;

					let amount_out_for_existing_inversed_route =
						Self::calculate_expected_amount_out(&inverse_existing_route, reference_amount_in_for_inverse)?;
					let amount_out_for_new_inversed_route =
						Self::calculate_expected_amount_out(&inverse_new_route, reference_amount_in_for_inverse)?;

					if amount_out_for_new_route > amount_out_for_existing_route
						&& amount_out_for_new_inversed_route > amount_out_for_existing_inversed_route
					{
						return Self::insert_route(asset_pair, new_route);
					}
				}
				Err(_) => {
					Self::validate_route(&new_route)?;

					return Self::insert_route(asset_pair, new_route);
				}
			}

			Err(Error::<T>::RouteUpdateIsNotSuccessful.into())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Pallet account address for do dry-run sell execution as validation
	pub fn router_account() -> T::AccountId {
		PalletId(*b"routerex").into_account_truncating()
	}

	fn ensure_route_size(route_length: usize) -> Result<(), DispatchError> {
		ensure!(
			(route_length as u32) <= MAX_NUMBER_OF_TRADES,
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
		let user_balance_of_asset_out_after_trade =
			T::Currency::reducible_balance(asset_out, &who, Preservation::Expendable, Fortitude::Polite);
		let user_expected_balance_of_asset_out_after_trade = user_balance_of_asset_out_before_trade
			.checked_add(&received_amount)
			.ok_or(ArithmeticError::Overflow)?;

		ensure!(
			user_balance_of_asset_out_after_trade == user_expected_balance_of_asset_out_after_trade,
			Error::<T>::InvalidRouteExecution
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
			let user_balance_of_asset_in_after_trade =
				T::Currency::reducible_balance(asset_in, &who, Preservation::Preserve, Fortitude::Polite);

			ensure!(
				user_balance_of_asset_in_before_trade - spent_amount == user_balance_of_asset_in_after_trade,
				Error::<T>::InvalidRouteExecution
			);
		}
		Ok(())
	}

	fn get_route_or_default(
		route: Vec<Trade<T::AssetId>>,
		asset_pair: AssetPair<T::AssetId>,
	) -> Result<Vec<Trade<T::AssetId>>, DispatchError> {
		let route = if !route.is_empty() {
			route
		} else {
			<Pallet<T> as RouteProvider<T::AssetId>>::get_route(asset_pair)
		};
		Ok(route)
	}

	fn validate_route(route: &[Trade<T::AssetId>]) -> Result<(T::Balance, T::Balance), DispatchError> {
		let reference_amount_in = Self::calculate_reference_amount_in(route)?;
		Self::validate_sell(route.to_vec(), reference_amount_in)?;

		let inverse_route = inverse_route(route.to_vec());
		let reference_amount_in_for_inverse_route = Self::calculate_reference_amount_in(&inverse_route)?;
		Self::validate_sell(inverse_route, reference_amount_in_for_inverse_route)?;

		Ok((reference_amount_in, reference_amount_in_for_inverse_route))
	}

	fn calculate_reference_amount_in(route: &[Trade<T::AssetId>]) -> Result<T::Balance, DispatchError> {
		let first_route = route.first().ok_or(Error::<T>::RouteCalculationFailed)?;
		let asset_b = match first_route.pool {
			PoolType::Omnipool => T::NativeAssetId::get(),
			PoolType::Stableswap(pool_id) => pool_id,
			PoolType::XYK => first_route.asset_out,
			PoolType::LBP => first_route.asset_out,
		};

		let asset_in_liquidity = T::AMM::get_liquidity_depth(first_route.pool, first_route.asset_in, asset_b);

		let liquidity = match asset_in_liquidity {
			Err(ExecutorError::NotSupported) => return Err(Error::<T>::PoolNotSupported.into()),
			Err(ExecutorError::Error(dispatch_error)) => return Err(dispatch_error),
			Ok(liq) => liq,
		};

		let one_percent_asset_in_liquidity = liquidity
			.checked_div(&100u128.into())
			.ok_or(ArithmeticError::Overflow)?;

		Ok(one_percent_asset_in_liquidity)
	}

	fn validate_sell(route: Vec<Trade<T::AssetId>>, amount_in: T::Balance) -> DispatchResult {
		let asset_in = route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in;
		let asset_out = route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out;

		with_transaction(|| {
			let origin: OriginFor<T> = Origin::<T>::Signed(Self::router_account()).into();
			let _ = T::Currency::mint_into(asset_in, &Self::router_account(), amount_in);

			let sell_result = Self::sell(origin, asset_in, asset_out, amount_in, u128::MIN.into(), route.clone());

			TransactionOutcome::Rollback(sell_result)
		})
		.map_err(|_| Error::<T>::InvalidRoute.into())
	}

	fn calculate_expected_amount_out(
		route: &[Trade<<T as Config>::AssetId>],
		amount_in: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		let sell_trade_amounts = Self::calculate_sell_trade_amounts(route, amount_in)?;
		let amount_out = sell_trade_amounts
			.last()
			.ok_or(Error::<T>::RouteCalculationFailed)?
			.amount_out;

		Ok(amount_out)
	}

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

	fn insert_route(asset_pair: AssetPair<T::AssetId>, route: Vec<Trade<T::AssetId>>) -> DispatchResultWithPostInfo {
		let route_as_bounded_vec: BoundedVec<Trade<T::AssetId>, sp_runtime::traits::ConstU32<MAX_NUMBER_OF_TRADES>> =
			route.try_into().map_err(|_| Error::<T>::MaxTradesExceeded)?;

		Routes::<T>::insert(asset_pair, route_as_bounded_vec);

		Self::deposit_event(Event::RouteUpdated {
			asset_ids: asset_pair.to_ordered_vec(),
		});

		Ok(Pays::No.into())
	}
}

impl<T: Config> RouterT<T::RuntimeOrigin, T::AssetId, T::Balance, Trade<T::AssetId>, AmountInAndOut<T::Balance>>
	for Pallet<T>
{
	fn sell(
		origin: T::RuntimeOrigin,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: T::Balance,
		min_amount_out: T::Balance,
		route: Vec<Trade<T::AssetId>>,
	) -> DispatchResult {
		Pallet::<T>::sell(origin, asset_in, asset_out, amount_in, min_amount_out, route)
	}

	fn buy(
		origin: T::RuntimeOrigin,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: T::Balance,
		max_amount_in: T::Balance,
		route: Vec<Trade<T::AssetId>>,
	) -> DispatchResult {
		Pallet::<T>::buy(origin, asset_in, asset_out, amount_out, max_amount_in, route)
	}

	fn calculate_sell_trade_amounts(
		route: &[Trade<T::AssetId>],
		amount_in: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		Pallet::<T>::calculate_sell_trade_amounts(route, amount_in)
	}

	fn calculate_buy_trade_amounts(
		route: &[Trade<T::AssetId>],
		amount_out: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		Pallet::<T>::calculate_buy_trade_amounts(route, amount_out)
	}

	fn set_route(
		origin: T::RuntimeOrigin,
		asset_pair: AssetPair<T::AssetId>,
		route: Vec<Trade<T::AssetId>>,
	) -> DispatchResultWithPostInfo {
		Pallet::<T>::set_route(origin, asset_pair, route)
	}
}

pub struct DummyRouter<T>(PhantomData<T>);
impl<T: Config> RouterT<T::RuntimeOrigin, T::AssetId, T::Balance, Trade<T::AssetId>, AmountInAndOut<T::Balance>>
	for DummyRouter<T>
{
	fn sell(
		_origin: T::RuntimeOrigin,
		_asset_in: T::AssetId,
		_asset_out: T::AssetId,
		_amount_in: T::Balance,
		_min_amount_out: T::Balance,
		_route: Vec<Trade<T::AssetId>>,
	) -> DispatchResult {
		Ok(())
	}

	fn buy(
		_origin: T::RuntimeOrigin,
		_asset_in: T::AssetId,
		_asset_out: T::AssetId,
		_amount_out: T::Balance,
		_max_amount_in: T::Balance,
		_route: Vec<Trade<T::AssetId>>,
	) -> DispatchResult {
		Ok(())
	}

	fn calculate_sell_trade_amounts(
		_route: &[Trade<T::AssetId>],
		amount_in: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		Ok(vec![AmountInAndOut::<T::Balance> {
			amount_in,
			amount_out: amount_in,
		}])
	}

	fn calculate_buy_trade_amounts(
		_route: &[Trade<T::AssetId>],
		amount_out: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		Ok(vec![AmountInAndOut::<T::Balance> {
			amount_in: amount_out,
			amount_out,
		}])
	}

	fn set_route(
		_origin: T::RuntimeOrigin,
		_asset_pair: AssetPair<T::AssetId>,
		_route: Vec<Trade<T::AssetId>>,
	) -> DispatchResultWithPostInfo {
		Ok(Pays::Yes.into())
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

impl<T: Config> RouteProvider<T::AssetId> for Pallet<T> {
	fn get_route(asset_pair: AssetPair<T::AssetId>) -> Vec<Trade<T::AssetId>> {
		let onchain_route = Routes::<T>::get(asset_pair.ordered_pair());

		let default_route = vec![Trade {
			pool: T::DefaultRoutePoolType::get(),
			asset_in: asset_pair.asset_in,
			asset_out: asset_pair.asset_out,
		}];

		match onchain_route {
			Some(route) => {
				if asset_pair.is_ordered() {
					route.to_vec()
				} else {
					inverse_route(route.to_vec())
				}
			}
			None => default_route,
		}
	}
}
