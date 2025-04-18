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
#![allow(clippy::manual_inspect)]

use codec::MaxEncodedLen;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::{Fortitude, Preservation};
use frame_support::PalletId;
use frame_system::pallet_prelude::OriginFor;
use frame_system::Origin;

use frame_support::{
	ensure,
	pallet_prelude::*,
	traits::{fungibles::Inspect, Get},
	transactional,
};
use hydra_dx_math::support::rational::{round_u512_to_rational, Rounding};
use sp_runtime::traits::Zero;

use frame_system::ensure_signed;
use hydradx_traits::router::{inverse_route, AssetPair, Route, RouteProvider, RouteSpotPriceProvider};
pub use hydradx_traits::router::{
	AmmTradeWeights, AmountInAndOut, ExecutorError, PoolType, RouterT, Trade, TradeExecution,
};

use orml_traits::arithmetic::{CheckedAdd, CheckedSub};
use pallet_broadcast::types::IncrementalIdType;
pub use pallet_broadcast::types::{ExecutionType, Fee};
use sp_core::U512;
use sp_runtime::traits::{AccountIdConversion, CheckedDiv};
use sp_runtime::{ArithmeticError, DispatchError, FixedPointNumber, FixedU128, TokenError};
use sp_std::{vec, vec::Vec};

#[cfg(test)]
mod tests;
pub mod weights;

mod types;

pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const MAX_NUMBER_OF_TRADES: u32 = 9;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::traits::fungibles::Mutate;
	use frame_system::pallet_prelude::OriginFor;
	use hydra_dx_math::ema::EmaPrice;
	use hydradx_traits::router::ExecutorError;
	use hydradx_traits::{OraclePeriod, PriceOracle};
	use sp_runtime::traits::{AtLeast32BitUnsigned, CheckedDiv, Zero};
	use sp_runtime::Saturating;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_broadcast::Config {
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
			+ CheckedDiv
			+ Saturating
			+ Zero;

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

		///Oracle price provider to validate if new route has oracle price data
		type OraclePriceProvider: PriceOracle<Self::AssetId, Price = EmaPrice>;

		/// Oracle's price aggregation period.
		#[pallet::constant]
		type OraclePeriod: Get<OraclePeriod>;

		/// Pool type used in the default route
		type DefaultRoutePoolType: Get<PoolType<Self::AssetId>>;

		/// Origin able to set route without validation
		type ForceInsertOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Weight information for the extrinsics.
		type WeightInfo: AmmTradeWeights<Trade<Self::AssetId>>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		///The route with trades has been successfully executed
		Executed {
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			amount_in: T::Balance,
			amount_out: T::Balance,
			event_id: IncrementalIdType,
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
		///The user has not enough balance to execute the trade
		InsufficientBalance,
		///The calculation of route trade amounts failed in the underlying AMM
		RouteCalculationFailed,
		///The route is invalid
		InvalidRoute,
		///The route update was not successful
		RouteUpdateIsNotSuccessful,
		///Route contains assets that has no oracle data
		RouteHasNoOracle,
		///The route execution failed in the underlying AMM
		InvalidRouteExecution,
		/// Trading same assets is not allowed.
		NotAllowed,
	}

	/// Storing routes for asset pairs
	#[pallet::storage]
	#[pallet::getter(fn route)]
	pub type Routes<T: Config> = StorageMap<_, Blake2_128Concat, AssetPair<T::AssetId>, Route<T::AssetId>>;

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
			route: Route<T::AssetId>,
		) -> DispatchResult {
			Self::do_sell(origin, asset_in, asset_out, amount_in, min_amount_out, route)
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
			route: Route<T::AssetId>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			ensure!(asset_in != asset_out, Error::<T>::NotAllowed);
			Self::ensure_route_size(route.len())?;

			let asset_pair = AssetPair::new(asset_in, asset_out);
			let route = Self::get_route_or_default(route, asset_pair)?;
			Self::ensure_route_arguments(&asset_pair, &route)?;

			let trade_amounts = Self::calculate_buy_trade_amounts(&route, amount_out)?;
			let first_trade = trade_amounts.last().ok_or(Error::<T>::RouteCalculationFailed)?;
			ensure!(first_trade.amount_in <= max_amount_in, Error::<T>::TradingLimitReached);

			let trader_account = Self::router_account();
			pallet_broadcast::Pallet::<T>::set_swapper(who.clone());

			T::Currency::transfer(
				asset_in,
				&who,
				&trader_account.clone(),
				first_trade.amount_in,
				Preservation::Expendable,
			)?;

			let next_event_id = pallet_broadcast::Pallet::<T>::add_to_context(ExecutionType::Router)?;

			for (trade_amount, trade) in trade_amounts.iter().rev().zip(route) {
				let origin: OriginFor<T> = Origin::<T>::Signed(trader_account.clone()).into();
				let execution_result = T::AMM::execute_buy(
					origin.clone(),
					trade.pool,
					trade.asset_in,
					trade.asset_out,
					trade_amount.amount_out,
					trade_amount.amount_in,
				);

				handle_execution_error!(execution_result);
			}

			let amount_out = T::Currency::reducible_balance(
				asset_out,
				&trader_account.clone(),
				Preservation::Expendable,
				Fortitude::Polite,
			);

			T::Currency::transfer(asset_out, &trader_account, &who, amount_out, Preservation::Expendable)?;

			Self::deposit_event(Event::Executed {
				asset_in,
				asset_out,
				amount_in: first_trade.amount_in,
				amount_out,
				event_id: next_event_id,
			});

			pallet_broadcast::Pallet::<T>::remove_from_context()?;
			pallet_broadcast::Pallet::<T>::remove_swapper();

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
			mut new_route: Route<T::AssetId>,
		) -> DispatchResultWithPostInfo {
			let _ = ensure_signed(origin.clone())?;
			Self::ensure_route_size(new_route.len())?;
			Self::ensure_route_arguments(&asset_pair, &new_route)?;
			T::OraclePriceProvider::price(&new_route, T::OraclePeriod::get()).ok_or(Error::<T>::RouteHasNoOracle)?;

			if !asset_pair.is_ordered() {
				asset_pair = asset_pair.ordered_pair();
				new_route = inverse_route(new_route)
			}

			let existing_route = Self::get_route(asset_pair);

			match Self::validate_route(&existing_route.clone()) {
				Ok((reference_amount_in, reference_amount_in_for_inverse)) => {
					let new_route_validation = Self::validate_sell(new_route.clone(), reference_amount_in);

					let inverse_new_route = inverse_route(new_route.clone());
					let inverse_new_route_validation =
						Self::validate_sell(inverse_new_route.clone(), reference_amount_in_for_inverse);

					match (new_route_validation, inverse_new_route_validation) {
						(Ok(_), Ok(_)) => (),
						(Err(_), Ok(amount_out)) => {
							Self::validate_sell(new_route.clone(), amount_out).map(|_| ())?;
						}
						(Ok(amount_out), Err(_)) => {
							Self::validate_sell(inverse_new_route.clone(), amount_out).map(|_| ())?;
						}
						(Err(err), Err(_)) => return Err(err.into()),
					}

					let amount_out_for_existing_route =
						Self::calculate_expected_amount_out(&existing_route, reference_amount_in)?;
					let amount_out_for_new_route =
						Self::calculate_expected_amount_out(&new_route, reference_amount_in)?;

					let inverse_existing_route = inverse_route(existing_route);
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
					Self::validate_route(&new_route.clone())?;

					return Self::insert_route(asset_pair, new_route);
				}
			}

			Err(Error::<T>::RouteUpdateIsNotSuccessful.into())
		}

		/// Force inserts the on-chain route for a given asset pair, so there is no any validation for the route
		///
		/// Can only be called by T::ForceInsertOrigin
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
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::force_insert_route_weight())]
		#[transactional]
		pub fn force_insert_route(
			origin: OriginFor<T>,
			mut asset_pair: AssetPair<T::AssetId>,
			mut new_route: Route<T::AssetId>,
		) -> DispatchResultWithPostInfo {
			T::ForceInsertOrigin::ensure_origin(origin)?;

			if !asset_pair.is_ordered() {
				asset_pair = asset_pair.ordered_pair();
				new_route = inverse_route(new_route)
			}

			Self::insert_route(asset_pair, new_route)
		}

		/// Executes a sell with a series of trades specified in the route.
		/// It sells all reducible user balance of `asset_in`
		/// The price for each trade is determined by the corresponding AMM.
		///
		/// - `origin`: The executor of the trade
		/// - `asset_in`: The identifier of the asset to sell
		/// - `asset_out`: The identifier of the asset to receive
		/// - `min_amount_out`: The minimum amount of `asset_out` to receive.
		/// - `route`: Series of [`Trade<AssetId>`] to be executed. A [`Trade<AssetId>`] specifies the asset pair (`asset_in`, `asset_out`) and the AMM (`pool`) in which the trade is executed.
		/// 		   If not specified, than the on-chain route is used.
		/// 		   If no on-chain is present, then omnipool route is used as default
		///
		/// Emits `RouteExecuted` when successful.
		///
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::sell_weight(route))]
		#[transactional]
		pub fn sell_all(
			origin: OriginFor<T>,
			asset_in: T::AssetId,
			asset_out: T::AssetId,
			min_amount_out: T::Balance,
			route: Route<T::AssetId>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			let amount_in = T::Currency::reducible_balance(asset_in, &who, Preservation::Expendable, Fortitude::Polite);

			Self::do_sell(origin, asset_in, asset_out, amount_in, min_amount_out, route)
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Pallet account address for do dry-run sell execution as validation
	pub fn router_account() -> T::AccountId {
		PalletId(*b"routerex").into_account_truncating()
	}

	fn do_sell(
		origin: T::RuntimeOrigin,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_in: T::Balance,
		min_amount_out: T::Balance,
		route: Route<T::AssetId>,
	) -> Result<(), DispatchError> {
		let who = ensure_signed(origin.clone())?;

		ensure!(asset_in != asset_out, Error::<T>::NotAllowed);
		Self::ensure_route_size(route.len())?;

		let asset_pair = AssetPair::new(asset_in, asset_out);
		let route = Self::get_route_or_default(route, asset_pair)?;
		Self::ensure_route_arguments(&asset_pair, &route)?;

		let trader_account = Self::router_account();

		let user_amount_in_balance =
			T::Currency::reducible_balance(asset_in, &who.clone(), Preservation::Expendable, Fortitude::Polite);
		ensure!(user_amount_in_balance >= amount_in, TokenError::FundsUnavailable);

		T::Currency::transfer(
			asset_in,
			&who,
			&trader_account.clone(),
			amount_in,
			Preservation::Expendable,
		)?;

		let next_event_id = pallet_broadcast::Pallet::<T>::add_to_context(ExecutionType::Router)?;
		pallet_broadcast::Pallet::<T>::set_swapper(who.clone());

		for trade in route.iter() {
			let amount_in_to_sell = T::Currency::reducible_balance(
				trade.asset_in,
				&trader_account.clone(),
				Preservation::Expendable,
				Fortitude::Polite,
			);

			let origin: OriginFor<T> = Origin::<T>::Signed(trader_account.clone()).into();

			let execution_result = T::AMM::execute_sell(
				origin,
				trade.pool,
				trade.asset_in,
				trade.asset_out,
				amount_in_to_sell,
				T::Balance::zero(),
			);

			handle_execution_error!(execution_result);
		}

		let amount_out = T::Currency::reducible_balance(
			asset_out,
			&trader_account.clone(),
			Preservation::Expendable,
			Fortitude::Polite,
		);

		ensure!(amount_out >= min_amount_out, Error::<T>::TradingLimitReached);
		T::Currency::transfer(asset_out, &trader_account, &who, amount_out, Preservation::Expendable)?;

		Self::deposit_event(Event::Executed {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			event_id: next_event_id,
		});

		pallet_broadcast::Pallet::<T>::remove_from_context()?;
		pallet_broadcast::Pallet::<T>::remove_swapper();

		Ok(())
	}

	fn ensure_route_size(route_length: usize) -> Result<(), DispatchError> {
		ensure!(
			(route_length as u32) <= hydradx_traits::router::MAX_NUMBER_OF_TRADES,
			Error::<T>::MaxTradesExceeded
		);

		Ok(())
	}

	fn ensure_route_arguments(
		asset_pair: &AssetPair<T::AssetId>,
		route: &[Trade<T::AssetId>],
	) -> Result<(), DispatchError> {
		ensure!(
			asset_pair.asset_in == route.first().ok_or(Error::<T>::InvalidRoute)?.asset_in,
			Error::<T>::InvalidRoute
		);
		ensure!(
			asset_pair.asset_out == route.last().ok_or(Error::<T>::InvalidRoute)?.asset_out,
			Error::<T>::InvalidRoute
		);

		for i in 0..route.len().saturating_sub(1) {
			let asset_out = route.get(i).ok_or(Error::<T>::InvalidRoute)?.asset_out;
			let next_trade_asset_in = route.get(i.saturating_add(1)).ok_or(Error::<T>::InvalidRoute)?.asset_in;

			ensure!(asset_out == next_trade_asset_in, Error::<T>::InvalidRoute)
		}

		Ok(())
	}

	fn get_route_or_default(
		route: Route<T::AssetId>,
		asset_pair: AssetPair<T::AssetId>,
	) -> Result<Route<T::AssetId>, DispatchError> {
		let route = if !route.is_empty() {
			route
		} else {
			<Pallet<T> as RouteProvider<T::AssetId>>::get_route(asset_pair)
		};
		Ok(route)
	}

	fn validate_route(route: &Route<T::AssetId>) -> Result<(T::Balance, T::Balance), DispatchError> {
		let reference_amount_in = Self::calculate_reference_amount_in(route)?;
		let route_validation = Self::validate_sell(route.clone(), reference_amount_in);

		let inverse_route = inverse_route(route.clone());
		let reference_amount_in_for_inverse_route = Self::calculate_reference_amount_in(&inverse_route)?;
		let inverse_route_validation =
			Self::validate_sell(inverse_route.clone(), reference_amount_in_for_inverse_route);

		match (route_validation, inverse_route_validation) {
			(Ok(_), Ok(_)) => Ok((reference_amount_in, reference_amount_in_for_inverse_route)),
			(Err(_), Ok(amount_out)) => Self::validate_sell(route.clone(), amount_out)
				.map(|_| (amount_out, reference_amount_in_for_inverse_route)),
			(Ok(amount_out), Err(_)) => {
				Self::validate_sell(inverse_route, amount_out).map(|_| (reference_amount_in, amount_out))
			}
			(Err(err), Err(_)) => Err(err),
		}
	}

	// TODO: add missing documentation
	fn calculate_reference_amount_in(route: &[Trade<T::AssetId>]) -> Result<T::Balance, DispatchError> {
		let first_route = route.first().ok_or(Error::<T>::RouteCalculationFailed)?;
		let asset_b = match first_route.pool {
			PoolType::Omnipool => T::NativeAssetId::get(),
			PoolType::Stableswap(pool_id) => pool_id,
			_ => first_route.asset_out,
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

	fn validate_sell(route: Route<T::AssetId>, amount_in: T::Balance) -> Result<T::Balance, DispatchError> {
		// Instead of executing a transaction, just calculate the expected amount out
		let amount_out = Self::calculate_expected_amount_out(route.as_ref(), amount_in)?;

		Ok(amount_out)
	}

	pub fn calculate_expected_amount_out(
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
			let result = T::AMM::calculate_out_given_in(trade.pool, trade.asset_in, trade.asset_out, amount_in);
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

	pub fn calculate_expected_amount_in(
		route: &[Trade<<T as Config>::AssetId>],
		amount_out: T::Balance,
	) -> Result<T::Balance, DispatchError> {
		let sell_trade_amounts = Self::calculate_buy_trade_amounts(route, amount_out)?;
		let amount_in = sell_trade_amounts
			.last()
			.ok_or(Error::<T>::RouteCalculationFailed)?
			.amount_in;

		Ok(amount_in)
	}

	fn calculate_buy_trade_amounts(
		route: &[Trade<T::AssetId>],
		amount_out: T::Balance,
	) -> Result<Vec<AmountInAndOut<T::Balance>>, DispatchError> {
		let mut amount_in_and_outs = Vec::<AmountInAndOut<T::Balance>>::with_capacity(route.len());
		let mut amount_out = amount_out;

		for trade in route.iter().rev() {
			let result = T::AMM::calculate_in_given_out(trade.pool, trade.asset_in, trade.asset_out, amount_out);

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

	fn insert_route(asset_pair: AssetPair<T::AssetId>, route: Route<T::AssetId>) -> DispatchResultWithPostInfo {
		Routes::<T>::insert(asset_pair, route);

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
		route: Route<T::AssetId>,
	) -> DispatchResult {
		Pallet::<T>::sell(origin, asset_in, asset_out, amount_in, min_amount_out, route)
	}

	fn sell_all(
		origin: T::RuntimeOrigin,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		min_amount_out: T::Balance,
		route: Route<T::AssetId>,
	) -> DispatchResult {
		Pallet::<T>::sell_all(origin, asset_in, asset_out, min_amount_out, route)
	}

	fn buy(
		origin: T::RuntimeOrigin,
		asset_in: T::AssetId,
		asset_out: T::AssetId,
		amount_out: T::Balance,
		max_amount_in: T::Balance,
		route: Route<T::AssetId>,
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
		route: Route<T::AssetId>,
	) -> DispatchResultWithPostInfo {
		Pallet::<T>::set_route(origin, asset_pair, route)
	}

	fn force_insert_route(
		origin: T::RuntimeOrigin,
		asset_pair: AssetPair<T::AssetId>,
		route: Route<T::AssetId>,
	) -> DispatchResultWithPostInfo {
		Pallet::<T>::force_insert_route(origin, asset_pair, route)
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
		_route: Route<T::AssetId>,
	) -> DispatchResult {
		Ok(())
	}

	fn sell_all(
		_origin: T::RuntimeOrigin,
		_asset_in: T::AssetId,
		_asset_out: T::AssetId,
		_min_amount_out: T::Balance,
		_route: Route<T::AssetId>,
	) -> sp_runtime::DispatchResult {
		Ok(())
	}

	fn buy(
		_origin: T::RuntimeOrigin,
		_asset_in: T::AssetId,
		_asset_out: T::AssetId,
		_amount_out: T::Balance,
		_max_amount_in: T::Balance,
		_route: Route<T::AssetId>,
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
		_route: Route<T::AssetId>,
	) -> DispatchResultWithPostInfo {
		Ok(Pays::Yes.into())
	}

	fn force_insert_route(
		_origin: T::RuntimeOrigin,
		_asset_pair: AssetPair<T::AssetId>,
		_route: Route<T::AssetId>,
	) -> DispatchResultWithPostInfo {
		Ok(Pays::Yes.into())
	}
}

impl<T: Config> RouteSpotPriceProvider<T::AssetId> for DummyRouter<T> {
	fn spot_price_with_fee(_route: &[Trade<T::AssetId>]) -> Option<FixedU128> {
		Some(FixedU128::from_u32(2))
	}
}

impl<T: Config> RouteProvider<T::AssetId> for DummyRouter<T> {
	fn get_route(asset_pair: AssetPair<T::AssetId>) -> Route<T::AssetId> {
		BoundedVec::truncate_from(vec![Trade {
			pool: T::DefaultRoutePoolType::get(),
			asset_in: asset_pair.asset_in,
			asset_out: asset_pair.asset_out,
		}])
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
	fn get_route(asset_pair: AssetPair<T::AssetId>) -> Route<T::AssetId> {
		let onchain_route = Routes::<T>::get(asset_pair.ordered_pair());

		let default_route = BoundedVec::truncate_from(vec![Trade {
			pool: T::DefaultRoutePoolType::get(),
			asset_in: asset_pair.asset_in,
			asset_out: asset_pair.asset_out,
		}]);

		match onchain_route {
			Some(route) => {
				if asset_pair.is_ordered() {
					route
				} else {
					inverse_route(route)
				}
			}
			None => default_route,
		}
	}
}

impl<T: Config> RouteSpotPriceProvider<T::AssetId> for Pallet<T> {
	fn spot_price_with_fee(route: &[Trade<T::AssetId>]) -> Option<FixedU128> {
		if route.is_empty() {
			return None;
		}

		let mut nominator = U512::from(1u128);
		let mut denominator = U512::from(1u128);

		// We aggregate the prices after every 4 hops to prevent overflow of U512
		for chunk_with_4_hops in route.chunks(4) {
			let mut prices: Vec<FixedU128> = Vec::with_capacity(chunk_with_4_hops.len());
			for trade in chunk_with_4_hops {
				let spot_price_result =
					T::AMM::calculate_spot_price_with_fee(trade.pool, trade.asset_in, trade.asset_out);
				match spot_price_result {
					Ok(spot_price) => prices.push(spot_price),
					Err(_) => return None,
				}
			}

			// Calculate the nominator and denominator for the current chunk
			let chunk_nominator = prices.iter().try_fold(U512::from(1u128), |acc, price| {
				acc.checked_mul(U512::from(price.into_inner()))
			})?;

			let chunk_denominator = prices.iter().try_fold(U512::from(1u128), |acc, _price| {
				acc.checked_mul(U512::from(FixedU128::DIV))
			})?;

			// Combine the chunk results with the final results
			nominator = nominator.checked_mul(chunk_nominator)?;
			denominator = denominator.checked_mul(chunk_denominator)?;
		}

		let rat_as_u128 = round_u512_to_rational((nominator, denominator), Rounding::Nearest);

		FixedU128::checked_from_rational(rat_as_u128.0, rat_as_u128.1)
	}
}
