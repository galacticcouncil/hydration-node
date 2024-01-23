// This file is part of HydraDX-node.

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
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

pub use crate::types::{Amount, AssetId, AssetPair, Balance};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::sp_runtime::{
	traits::{AtLeast32BitUnsigned, BlockNumberProvider, Saturating, Zero},
	DispatchError, RuntimeDebug,
};
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{EnsureOrigin, Get, LockIdentifier},
	transactional,
};
use frame_system::ensure_signed;
use frame_system::pallet_prelude::BlockNumberFor;
use hydra_dx_math::types::LBPWeight;
use hydradx_traits::{AMMTransfer, AssetPairAccountIdFor, CanCreatePool, LockedBalance, AMM};
use orml_traits::{MultiCurrency, MultiCurrencyExtended, MultiLockableCurrency};

use scale_info::TypeInfo;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{vec, vec::Vec};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

#[allow(clippy::all)]
pub mod weights;

#[cfg(test)]
mod invariants;

mod trade_execution;
pub mod types;

use weights::WeightInfo;
// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

type BalanceOf<T> = <<T as Config>::MultiCurrency as MultiCurrency<<T as frame_system::Config>::AccountId>>::Balance;
type PoolId<T> = <T as frame_system::Config>::AccountId;

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(Default, RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub enum WeightCurveType {
	#[default]
	Linear,
}

/// Max weight corresponds to 100%
pub const MAX_WEIGHT: LBPWeight = 100_000_000;

/// Max sale duration is 14 days, assuming 6 sec blocks
pub const MAX_SALE_DURATION: u32 = (60 * 60 * 24 / 6) * 14;

/// Lock Identifier for the collected fees
pub const COLLECTOR_LOCK_ID: LockIdentifier = *b"lbpcllct";

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct Pool<AccountId, BlockNumber: AtLeast32BitUnsigned + Copy> {
	/// owner of the pool after `CreatePoolOrigin` creates it
	pub owner: AccountId,

	/// start block
	pub start: Option<BlockNumber>,

	/// end block
	pub end: Option<BlockNumber>,

	/// Asset ids of the tokens (accumulating asset, sold asset) // TODO maybe name them accordingly in struct instead of tuple
	pub assets: (AssetId, AssetId),

	/// initial weight of the asset_a where the minimum value is 0 (equivalent to 0% weight), and the maximum value is 100_000_000 (equivalent to 100% weight)
	pub initial_weight: LBPWeight,

	/// final weights of the asset_a where the minimum value is 0 (equivalent to 0% weight), and the maximum value is 100_000_000 (equivalent to 100% weight)
	pub final_weight: LBPWeight,

	/// weight curve
	pub weight_curve: WeightCurveType,

	/// standard fee amount
	pub fee: (u32, u32),

	/// person that receives the fee
	pub fee_collector: AccountId,

	/// repayment target of the accumulated asset in fee collectors account, when this target is reached fee drops from 20% to fee
	pub repay_target: Balance,
}

impl<AccountId, BlockNumber: AtLeast32BitUnsigned + Copy> Pool<AccountId, BlockNumber> {
	fn new(
		pool_owner: AccountId,
		asset_a: AssetId,
		asset_b: AssetId,
		initial_weight: LBPWeight,
		final_weight: LBPWeight,
		weight_curve: WeightCurveType,
		fee: (u32, u32),
		fee_collector: AccountId,
		repay_target: Balance,
	) -> Self {
		Pool {
			owner: pool_owner,
			start: None,
			end: None,
			assets: (asset_a, asset_b),
			initial_weight,
			final_weight,
			weight_curve,
			fee,
			fee_collector,
			repay_target,
		}
	}
}

pub trait LBPWeightCalculation<BlockNumber: AtLeast32BitUnsigned> {
	fn calculate_weight(
		weight_curve: WeightCurveType,
		start: BlockNumber,
		end: BlockNumber,
		initial_weight: LBPWeight,
		final_weight: LBPWeight,
		at: BlockNumber,
	) -> Option<LBPWeight>;
}

pub struct LBPWeightFunction;
impl<BlockNumber: AtLeast32BitUnsigned> LBPWeightCalculation<BlockNumber> for LBPWeightFunction {
	fn calculate_weight(
		_weight_curve: WeightCurveType,
		start: BlockNumber,
		end: BlockNumber,
		initial_weight: LBPWeight,
		final_weight: LBPWeight,
		at: BlockNumber,
	) -> Option<LBPWeight> {
		hydra_dx_math::lbp::calculate_linear_weights(start, end, initial_weight, final_weight, at).ok()
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi currency for transfer of currencies
		type MultiCurrency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Amount = Amount, Balance = Balance>
			+ MultiLockableCurrency<Self::AccountId>;

		/// Universal locked balance getter for tracking of fee collector balance
		type LockedBalance: LockedBalance<AssetId, Self::AccountId, Balance>;

		/// The origin which can create a new pool
		type CreatePoolOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Function for calculation of LBP weights
		type LBPWeightFunction: LBPWeightCalculation<BlockNumberFor<Self>>;

		/// Mapping of asset pairs to unique pool identities
		type AssetPairAccountId: AssetPairAccountIdFor<AssetId, PoolId<Self>>;

		/// Weight information for the extrinsics
		type WeightInfo: WeightInfo;

		/// Minimum trading limit, sole purpose of this is to keep the math working
		#[pallet::constant]
		type MinTradingLimit: Get<Balance>;

		/// Minimum pool liquidity, sole purpose of this is to keep the math working
		#[pallet::constant]
		type MinPoolLiquidity: Get<Balance>;

		/// Max fraction of pool to sell in single transaction
		#[pallet::constant]
		type MaxInRatio: Get<u128>;

		/// Max fraction of pool to buy in single transaction
		#[pallet::constant]
		type MaxOutRatio: Get<u128>;

		/// The block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			// The exponentiation used in the math can overflow for values smaller than 3
			assert!(T::MaxInRatio::get() >= 3, "LBP: MaxInRatio is set to invalid value.");

			assert!(T::MaxOutRatio::get() >= 3, "LBP: MaxOutRatio is set to invalid value.");
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Pool assets can not be the same
		CannotCreatePoolWithSameAssets,

		/// Account is not a pool owner
		NotOwner,

		/// Sale already started
		SaleStarted,

		/// Sale is still in progress
		SaleNotEnded,

		/// Sale is not running
		SaleIsNotRunning,

		/// Sale duration is too long
		MaxSaleDurationExceeded,

		/// Liquidity being added should not be zero
		CannotAddZeroLiquidity,

		/// Asset balance too low
		InsufficientAssetBalance,

		/// Pool does not exist
		PoolNotFound,

		/// Pool has been already created
		PoolAlreadyExists,

		/// Invalid block range
		InvalidBlockRange,

		/// Calculation error
		WeightCalculationError,

		/// Weight set is out of range
		InvalidWeight,

		/// Can not perform a trade with zero amount
		ZeroAmount,

		/// Trade amount is too high
		MaxInRatioExceeded,

		/// Trade amount is too high
		MaxOutRatioExceeded,

		/// Invalid fee amount
		FeeAmountInvalid,

		/// Trading limit reached
		TradingLimitReached,

		/// An unexpected integer overflow occurred
		Overflow,

		/// Nothing to update
		NothingToUpdate,

		/// Liquidity has not reached the required minimum.
		InsufficientLiquidity,

		/// Amount is less than minimum trading limit.
		InsufficientTradingAmount,

		/// Not more than one fee collector per asset id
		FeeCollectorWithAssetAlreadyUsed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Pool was created by the `CreatePool` origin.
		PoolCreated {
			pool: PoolId<T>,
			data: Pool<T::AccountId, BlockNumberFor<T>>,
		},

		/// Pool data were updated.
		PoolUpdated {
			pool: PoolId<T>,
			data: Pool<T::AccountId, BlockNumberFor<T>>,
		},

		/// New liquidity was provided to the pool.
		LiquidityAdded {
			who: T::AccountId,
			asset_a: AssetId,
			asset_b: AssetId,
			amount_a: BalanceOf<T>,
			amount_b: BalanceOf<T>,
		},

		/// Liquidity was removed from the pool and the pool was destroyed.
		LiquidityRemoved {
			who: T::AccountId,
			asset_a: AssetId,
			asset_b: AssetId,
			amount_a: BalanceOf<T>,
			amount_b: BalanceOf<T>,
		},

		/// Sale executed.
		SellExecuted {
			who: T::AccountId,
			asset_in: AssetId,
			asset_out: AssetId,
			amount: BalanceOf<T>,
			sale_price: BalanceOf<T>,
			fee_asset: AssetId,
			fee_amount: BalanceOf<T>,
		},

		/// Purchase executed.
		BuyExecuted {
			who: T::AccountId,
			asset_out: AssetId,
			asset_in: AssetId,
			amount: BalanceOf<T>,
			buy_price: BalanceOf<T>,
			fee_asset: AssetId,
			fee_amount: BalanceOf<T>,
		},
	}

	/// Details of a pool.
	#[pallet::storage]
	#[pallet::getter(fn pool_data)]
	pub type PoolData<T: Config> =
		StorageMap<_, Blake2_128Concat, PoolId<T>, Pool<T::AccountId, BlockNumberFor<T>>, OptionQuery>;

	/// Storage used for tracking existing fee collectors
	/// Not more than one fee collector per asset possible
	#[pallet::storage]
	pub type FeeCollectorWithAsset<T: Config> =
		StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, AssetId, bool, ValueQuery>;

	#[pallet::extra_constants]
	impl<T: Config> Pallet<T> {
		pub fn repay_fee() -> (u32, u32) {
			(2, 10)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new liquidity bootstrapping pool for given asset pair.
		///
		/// For any asset pair, only one pool can exist at a time.
		///
		/// The dispatch origin for this call must be `T::CreatePoolOrigin`.
		/// The pool is created with initial liquidity provided by the `pool_owner` who must have
		/// sufficient funds free.
		///
		/// The pool starts uninitialized and update_pool call should be called once created to set the start block.
		///
		/// This function should be dispatched from governing entity `T::CreatePoolOrigin`
		///
		/// Parameters:
		/// - `pool_owner`: the future owner of the new pool.
		/// - `asset_a`: { asset_id, amount } Asset ID and initial liquidity amount.
		/// - `asset_b`: { asset_id, amount } Asset ID and initial liquidity amount.
		/// - `initial_weight`: Initial weight of the asset_a. 1_000_000 corresponding to 1% and 100_000_000 to 100%
		/// this should be higher than final weight
		/// - `final_weight`: Final weight of the asset_a. 1_000_000 corresponding to 1% and 100_000_000 to 100%
		/// this should be lower than initial weight
		/// - `weight_curve`: The weight function used to update the LBP weights. Currently,
		/// there is only one weight function implemented, the linear function.
		/// - `fee`: The trading fee charged on every trade distributed to `fee_collector`.
		/// - `fee_collector`: The account to which trading fees will be transferred.
		/// - `repay_target`: The amount of tokens to repay to separate fee_collector account. Until this amount is
		/// reached, fee will be increased to 20% and taken from the pool
		///
		/// Emits `PoolCreated` event when successful.
		///
		/// BEWARE: We are taking the fee from the accumulated asset. If the accumulated asset is sold to the pool,
		/// the fee cost is transferred to the pool. If its bought from the pool the buyer bears the cost.
		/// This increases the price of the sold asset on every trade. Make sure to only run this with
		/// previously illiquid assets.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::create_pool())]
		pub fn create_pool(
			origin: OriginFor<T>,
			pool_owner: T::AccountId,
			asset_a: AssetId,
			asset_a_amount: Balance,
			asset_b: AssetId,
			asset_b_amount: Balance,
			initial_weight: LBPWeight,
			final_weight: LBPWeight,
			weight_curve: WeightCurveType,
			fee: (u32, u32),
			fee_collector: T::AccountId,
			repay_target: Balance,
		) -> DispatchResult {
			T::CreatePoolOrigin::ensure_origin(origin)?;

			ensure!(
				asset_a_amount >= T::MinPoolLiquidity::get() && asset_b_amount >= T::MinPoolLiquidity::get(),
				Error::<T>::InsufficientLiquidity
			);

			ensure!(asset_a != asset_b, Error::<T>::CannotCreatePoolWithSameAssets);

			let asset_pair = AssetPair {
				asset_in: asset_a,
				asset_out: asset_b,
			};

			ensure!(!Self::exists(asset_pair), Error::<T>::PoolAlreadyExists);

			ensure!(
				!<FeeCollectorWithAsset<T>>::contains_key(fee_collector.clone(), asset_a),
				Error::<T>::FeeCollectorWithAssetAlreadyUsed
			);

			ensure!(
				T::MultiCurrency::free_balance(asset_a, &pool_owner) >= asset_a_amount,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(
				T::MultiCurrency::free_balance(asset_b, &pool_owner) >= asset_b_amount,
				Error::<T>::InsufficientAssetBalance
			);

			let pool_data = Pool::new(
				pool_owner.clone(),
				asset_a,
				asset_b,
				initial_weight,
				final_weight,
				weight_curve,
				fee,
				fee_collector.clone(),
				repay_target,
			);

			Self::validate_pool_data(&pool_data)?;

			let pool_id = Self::get_pair_id(asset_pair);

			<PoolData<T>>::insert(&pool_id, &pool_data);
			<FeeCollectorWithAsset<T>>::insert(fee_collector, asset_a, true);

			Self::deposit_event(Event::PoolCreated {
				pool: pool_id.clone(),
				data: pool_data,
			});

			T::MultiCurrency::transfer(asset_a, &pool_owner, &pool_id, asset_a_amount)?;
			T::MultiCurrency::transfer(asset_b, &pool_owner, &pool_id, asset_b_amount)?;

			Self::deposit_event(Event::LiquidityAdded {
				who: pool_id,
				asset_a,
				asset_b,
				amount_a: asset_a_amount,
				amount_b: asset_b_amount,
			});

			Ok(())
		}

		/// Update pool data of a pool.
		///
		/// The dispatch origin for this call must be signed by the pool owner.
		///
		/// The pool can be updated only if the sale has not already started.
		///
		/// At least one of the following optional parameters has to be specified.
		///
		/// Parameters:
		/// - `pool_id`: The identifier of the pool to be updated.
		/// - `start`: The new starting time of the sale. This parameter is optional.
		/// - `end`: The new ending time of the sale. This parameter is optional.
		/// - `initial_weight`: The new initial weight. This parameter is optional.
		/// - `final_weight`: The new final weight. This parameter is optional.
		/// - `fee`: The new trading fee charged on every trade. This parameter is optional.
		/// - `fee_collector`: The new receiver of trading fees. This parameter is optional.
		///
		/// Emits `PoolUpdated` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update_pool_data())]
		pub fn update_pool_data(
			origin: OriginFor<T>,
			pool_id: PoolId<T>,
			pool_owner: Option<T::AccountId>,
			start: Option<BlockNumberFor<T>>,
			end: Option<BlockNumberFor<T>>,
			initial_weight: Option<LBPWeight>,
			final_weight: Option<LBPWeight>,
			fee: Option<(u32, u32)>,
			fee_collector: Option<T::AccountId>,
			repay_target: Option<Balance>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<PoolData<T>>::try_mutate_exists(pool_id.clone(), |maybe_pool| -> DispatchResult {
				// check existence of the pool
				let pool = maybe_pool.as_mut().ok_or(Error::<T>::PoolNotFound)?;

				ensure!(
					start.is_some()
						|| end.is_some() || initial_weight.is_some()
						|| final_weight.is_some()
						|| fee.is_some() || fee_collector.is_some()
						|| repay_target.is_some(),
					Error::<T>::NothingToUpdate
				);

				ensure!(who == pool.owner, Error::<T>::NotOwner);

				ensure!(!Self::has_pool_started(pool), Error::<T>::SaleStarted);

				pool.owner = pool_owner.unwrap_or_else(|| pool.owner.clone());

				pool.start = start.or(pool.start);
				pool.end = end.or(pool.end);

				pool.initial_weight = initial_weight.unwrap_or(pool.initial_weight);

				pool.final_weight = final_weight.unwrap_or(pool.final_weight);

				pool.fee = fee.unwrap_or(pool.fee);

				// Handle update of fee collector - validate and replace old fee collector
				if let Some(updated_fee_collector) = fee_collector {
					FeeCollectorWithAsset::<T>::try_mutate(
						&updated_fee_collector,
						pool.assets.0,
						|collector| -> DispatchResult {
							ensure!(!*collector, Error::<T>::FeeCollectorWithAssetAlreadyUsed);

							<FeeCollectorWithAsset<T>>::remove(&pool.fee_collector, pool.assets.0);
							*collector = true;

							Ok(())
						},
					)?;

					pool.fee_collector = updated_fee_collector;
				}

				pool.repay_target = repay_target.unwrap_or(pool.repay_target);

				Self::validate_pool_data(pool)?;

				Self::deposit_event(Event::PoolUpdated {
					pool: pool_id,
					data: (*pool).clone(),
				});
				Ok(())
			})
		}

		/// Add liquidity to a pool.
		///
		/// Assets to add has to match the pool assets. At least one amount has to be non-zero.
		///
		/// The dispatch origin for this call must be signed by the pool owner.
		///
		/// Parameters:
		/// - `pool_id`: The identifier of the pool
		/// - `amount_a`: The identifier of the asset and the amount to add.
		/// - `amount_b`: The identifier of the second asset and the amount to add.
		///
		/// Emits `LiquidityAdded` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::add_liquidity())]
		pub fn add_liquidity(
			origin: OriginFor<T>,
			amount_a: (AssetId, BalanceOf<T>),
			amount_b: (AssetId, BalanceOf<T>),
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (asset_a, asset_b) = (amount_a.0, amount_b.0);
			let (amount_a, amount_b) = (amount_a.1, amount_b.1);

			let pool_id = Self::pair_account_from_assets(asset_a, asset_b);
			let pool_data = <PoolData<T>>::try_get(&pool_id).map_err(|_| Error::<T>::PoolNotFound)?;

			ensure!(who == pool_data.owner, Error::<T>::NotOwner);

			ensure!(
				!amount_a.is_zero() || !amount_b.is_zero(),
				Error::<T>::CannotAddZeroLiquidity
			);

			if !amount_a.is_zero() {
				ensure!(
					T::MultiCurrency::free_balance(asset_a, &who) >= amount_a,
					Error::<T>::InsufficientAssetBalance
				);
			}

			if !amount_b.is_zero() {
				ensure!(
					T::MultiCurrency::free_balance(asset_b, &who) >= amount_b,
					Error::<T>::InsufficientAssetBalance
				);
			}

			T::MultiCurrency::transfer(asset_a, &who, &pool_id, amount_a)?;
			T::MultiCurrency::transfer(asset_b, &who, &pool_id, amount_b)?;

			Self::deposit_event(Event::LiquidityAdded {
				who: pool_id,
				asset_a,
				asset_b,
				amount_a,
				amount_b,
			});

			Ok(())
		}

		/// Transfer all the liquidity from a pool back to the pool owner and destroy the pool.
		/// The pool data are also removed from the storage.
		///
		/// The pool can't be destroyed during the sale.
		///
		/// The dispatch origin for this call must be signed by the pool owner.
		///
		/// Parameters:
		/// - `amount_a`: The identifier of the asset and the amount to add.
		///
		/// Emits 'LiquidityRemoved' when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_liquidity())]
		pub fn remove_liquidity(origin: OriginFor<T>, pool_id: PoolId<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pool_data = <PoolData<T>>::try_get(&pool_id).map_err(|_| Error::<T>::PoolNotFound)?;

			ensure!(who == pool_data.owner, Error::<T>::NotOwner);

			ensure!(!Self::is_pool_running(&pool_data), Error::<T>::SaleNotEnded);

			let (asset_a, asset_b) = pool_data.assets;

			let amount_a = T::MultiCurrency::free_balance(asset_a, &pool_id);
			let amount_b = T::MultiCurrency::free_balance(asset_b, &pool_id);

			T::MultiCurrency::transfer(asset_a, &pool_id, &who, amount_a)?;
			T::MultiCurrency::transfer(asset_b, &pool_id, &who, amount_b)?;

			if Self::collected_fees(&pool_data) > 0 {
				T::MultiCurrency::remove_lock(COLLECTOR_LOCK_ID, asset_a, &pool_data.fee_collector)?;
			}

			<FeeCollectorWithAsset<T>>::remove(pool_data.fee_collector, pool_data.assets.0);
			<PoolData<T>>::remove(&pool_id);

			Self::deposit_event(Event::LiquidityRemoved {
				who: pool_id,
				asset_a,
				asset_b,
				amount_a,
				amount_b,
			});

			Ok(())
		}

		/// Trade `asset_in` for `asset_out`.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the pool and is
		/// affected by the amount and proportion of the pool assets and the weights.
		///
		/// Trading `fee` is distributed to the `fee_collector`.
		///
		/// Parameters:
		/// - `asset_in`: The identifier of the asset being transferred from the account to the pool.
		/// - `asset_out`: The identifier of the asset being transferred from the pool to the account.
		/// - `amount`: The amount of `asset_in`
		/// - `max_limit`: minimum amount of `asset_out` / amount of asset_out to be obtained from the pool in exchange for `asset_in`.
		///
		/// Emits `SellExecuted` when successful.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::sell())]
		pub fn sell(
			origin: OriginFor<T>,
			asset_in: AssetId,
			asset_out: AssetId,
			amount: BalanceOf<T>,
			max_limit: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::sell(&who, AssetPair { asset_in, asset_out }, amount, max_limit, false)?;

			Ok(())
		}

		/// Trade `asset_in` for `asset_out`.
		///
		/// Executes a swap of `asset_in` for `asset_out`. Price is determined by the pool and is
		/// affected by the amount and the proportion of the pool assets and the weights.
		///
		/// Trading `fee` is distributed to the `fee_collector`.
		///
		/// Parameters:
		/// - `asset_in`: The identifier of the asset being transferred from the account to the pool.
		/// - `asset_out`: The identifier of the asset being transferred from the pool to the account.
		/// - `amount`: The amount of `asset_out`.
		/// - `max_limit`: maximum amount of `asset_in` to be sold in exchange for `asset_out`.
		///
		/// Emits `BuyExecuted` when successful.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::buy())]
		pub fn buy(
			origin: OriginFor<T>,
			asset_out: AssetId,
			asset_in: AssetId,
			amount: BalanceOf<T>,
			max_limit: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			<Self as AMM<_, _, _, _>>::buy(&who, AssetPair { asset_in, asset_out }, amount, max_limit, false)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn calculate_weights(
		pool_data: &Pool<T::AccountId, BlockNumberFor<T>>,
		at: BlockNumberFor<T>,
	) -> Result<(LBPWeight, LBPWeight), DispatchError> {
		let weight_a = T::LBPWeightFunction::calculate_weight(
			pool_data.weight_curve,
			pool_data.start.unwrap_or_else(Zero::zero),
			pool_data.end.unwrap_or_else(Zero::zero),
			pool_data.initial_weight,
			pool_data.final_weight,
			at,
		)
		.ok_or(Error::<T>::WeightCalculationError)?;

		let weight_b = MAX_WEIGHT.saturating_sub(weight_a);

		Ok((weight_a, weight_b))
	}

	fn validate_pool_data(pool_data: &Pool<T::AccountId, BlockNumberFor<T>>) -> DispatchResult {
		let now = T::BlockNumberProvider::current_block_number();

		ensure!(
			match (pool_data.start, pool_data.end) {
				(Some(start), Some(end)) => now < start && start < end,
				(None, None) => true,
				_ => false,
			},
			Error::<T>::InvalidBlockRange
		);

		// duration of the LBP sale should not exceed 2 weeks (assuming 6 sec blocks)
		ensure!(
			pool_data
				.end
				.unwrap_or_default()
				.saturating_sub(pool_data.start.unwrap_or_default())
				< MAX_SALE_DURATION.into(),
			Error::<T>::MaxSaleDurationExceeded
		);

		// zero weight at the beginning or at the end of a sale may cause a problem in the price calculation
		// Minimum allowed weight is 2%. The exponentiation used in the math can overflow when the ration between the weights is higher than 98/2.
		ensure!(
			!pool_data.initial_weight.is_zero()
				&& pool_data.initial_weight < MAX_WEIGHT
				&& pool_data.initial_weight >= MAX_WEIGHT / 50	// 2%
				&& !pool_data.final_weight.is_zero()
				&& pool_data.final_weight < MAX_WEIGHT
				// when initial and final weights are >= 2%, then the weights are also <= 98%
				&& pool_data.final_weight >= MAX_WEIGHT / 50, // 2%,
			// TODO people could leak value out the pool if initial weight is < final weight due to fee structure
			// && pool_data.initial_weight > pool_data.final_weight,
			Error::<T>::InvalidWeight
		);

		ensure!(!pool_data.fee.1.is_zero(), Error::<T>::FeeAmountInvalid);

		Ok(())
	}

	fn get_sorted_weight(
		asset_in: AssetId,
		now: BlockNumberFor<T>,
		pool_data: &Pool<T::AccountId, BlockNumberFor<T>>,
	) -> Result<(LBPWeight, LBPWeight), Error<T>> {
		match Self::calculate_weights(pool_data, now) {
			Ok(weights) => {
				if asset_in == pool_data.assets.0 {
					Ok((weights.0, weights.1))
				} else {
					// swap weights if assets are in different order
					Ok((weights.1, weights.0))
				}
			}
			Err(_) => Err(Error::<T>::InvalidWeight),
		}
	}

	/// return true if now is in interval <pool.start, pool.end>
	fn is_pool_running(pool_data: &Pool<T::AccountId, BlockNumberFor<T>>) -> bool {
		let now = T::BlockNumberProvider::current_block_number();
		match (pool_data.start, pool_data.end) {
			(Some(start), Some(end)) => start <= now && now <= end,
			_ => false,
		}
	}

	/// return true if now is > pool.start and pool has been initialized
	fn has_pool_started(pool_data: &Pool<T::AccountId, BlockNumberFor<T>>) -> bool {
		let now = T::BlockNumberProvider::current_block_number();
		match pool_data.start {
			Some(start) => start <= now,
			_ => false,
		}
	}

	/// returns fees collected and locked in the fee collector account
	/// note: after LBP finishes and liquidity is removed this will be 0
	fn collected_fees(pool: &Pool<T::AccountId, BlockNumberFor<T>>) -> BalanceOf<T> {
		T::LockedBalance::get_by_lock(COLLECTOR_LOCK_ID, pool.assets.0, pool.fee_collector.clone())
	}

	/// repay fee is applied until repay target amount is reached
	fn is_repay_fee_applied(pool: &Pool<T::AccountId, BlockNumberFor<T>>) -> bool {
		Self::collected_fees(pool) < pool.repay_target
	}

	#[transactional]
	fn execute_trade(transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>) -> DispatchResult {
		let pool_account = Self::get_pair_id(transfer.assets);
		let pool = <PoolData<T>>::try_get(&pool_account).map_err(|_| Error::<T>::PoolNotFound)?;

		// Transfer assets between pool and user
		T::MultiCurrency::transfer(
			transfer.assets.asset_in,
			&transfer.origin,
			&pool_account,
			transfer.amount,
		)?;
		T::MultiCurrency::transfer(
			transfer.assets.asset_out,
			&pool_account,
			&transfer.origin,
			transfer.amount_b,
		)?;

		// Fee is deducted from the sent out amount of accumulated asset and transferred to the fee collector
		let (fee_asset, fee_amount) = transfer.fee;
		let fee_payer = if transfer.assets.asset_in == fee_asset {
			&transfer.origin
		} else {
			&pool_account
		};

		T::MultiCurrency::transfer(fee_asset, fee_payer, &pool.fee_collector, fee_amount)?;

		// Resets lock for total of collected fees
		let collected_fee_total = Self::collected_fees(&pool) + fee_amount;
		T::MultiCurrency::set_lock(COLLECTOR_LOCK_ID, fee_asset, &pool.fee_collector, collected_fee_total)?;

		Ok(())
	}

	/// determines fee rate and applies it to the amount
	fn calculate_fees(
		pool: &Pool<T::AccountId, BlockNumberFor<T>>,
		amount: BalanceOf<T>,
	) -> Result<BalanceOf<T>, DispatchError> {
		let fee = if Self::is_repay_fee_applied(pool) {
			Self::repay_fee()
		} else {
			pool.fee
		};
		Ok(hydra_dx_math::fee::calculate_pool_trade_fee(amount, (fee.0, fee.1))
			.ok_or::<Error<T>>(Error::<T>::FeeAmountInvalid)?)
	}

	pub fn pair_account_from_assets(asset_a: AssetId, asset_b: AssetId) -> PoolId<T> {
		T::AssetPairAccountId::from_assets(asset_a, asset_b, "lbp")
	}
}

impl<T: Config> AMM<T::AccountId, AssetId, AssetPair, BalanceOf<T>> for Pallet<T> {
	fn exists(assets: AssetPair) -> bool {
		let pair_account = Self::pair_account_from_assets(assets.asset_in, assets.asset_out);
		<PoolData<T>>::contains_key(&pair_account)
	}

	fn get_pair_id(assets: AssetPair) -> T::AccountId {
		Self::pair_account_from_assets(assets.asset_in, assets.asset_out)
	}

	fn get_share_token(_assets: AssetPair) -> AssetId {
		// No share token in lbp
		AssetId::MAX
	}

	fn get_pool_assets(pool_account_id: &T::AccountId) -> Option<Vec<AssetId>> {
		let maybe_pool = <PoolData<T>>::try_get(pool_account_id);
		if let Ok(pool_data) = maybe_pool {
			Some(vec![pool_data.assets.0, pool_data.assets.1])
		} else {
			None
		}
	}

	/// Calculate spot price for given assets and amount. This method does not modify the storage.
	///
	/// Provided assets must exist in the pool. Panic if an asset does not exist in the pool.
	///
	/// Return 0 if calculation overflows or weights calculation overflows.
	fn get_spot_price_unchecked(asset_a: AssetId, asset_b: AssetId, amount: BalanceOf<T>) -> BalanceOf<T> {
		let pool_id = Self::get_pair_id(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		let asset_a_reserve = T::MultiCurrency::free_balance(asset_a, &pool_id);
		let asset_b_reserve = T::MultiCurrency::free_balance(asset_b, &pool_id);

		let pool_data = match <PoolData<T>>::try_get(&pool_id) {
			Ok(pool) => pool,
			Err(_) => return BalanceOf::<T>::zero(),
		};

		let now = T::BlockNumberProvider::current_block_number();

		// We need to sort weights here if asset_in is not the first asset
		let (weight_in, weight_out) = match Self::get_sorted_weight(asset_a, now, &pool_data) {
			Ok(weights) => weights,
			Err(_) => return BalanceOf::<T>::zero(),
		};

		hydra_dx_math::lbp::calculate_spot_price(asset_a_reserve, asset_b_reserve, weight_in, weight_out, amount)
			.unwrap_or_else(|_| BalanceOf::<T>::zero())
	}

	fn validate_sell(
		who: &T::AccountId,
		assets: AssetPair,
		amount: BalanceOf<T>,
		min_bought: BalanceOf<T>,
		_discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>, DispatchError> {
		ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);
		ensure!(
			T::MultiCurrency::free_balance(assets.asset_in, who) >= amount,
			Error::<T>::InsufficientAssetBalance
		);

		let pool_id = Self::get_pair_id(assets);
		let pool_data = <PoolData<T>>::try_get(&pool_id).map_err(|_| Error::<T>::PoolNotFound)?;

		ensure!(Self::is_pool_running(&pool_data), Error::<T>::SaleIsNotRunning);

		let now = T::BlockNumberProvider::current_block_number();
		let (weight_in, weight_out) = Self::get_sorted_weight(assets.asset_in, now, &pool_data)?;
		let asset_in_reserve = T::MultiCurrency::free_balance(assets.asset_in, &pool_id);
		let asset_out_reserve = T::MultiCurrency::free_balance(assets.asset_out, &pool_id);

		ensure!(
			amount
				<= asset_in_reserve
					.checked_div(T::MaxInRatio::get())
					.ok_or(Error::<T>::Overflow)?,
			Error::<T>::MaxInRatioExceeded
		);

		// LBP fee asset is always accumulated asset
		let fee_asset = pool_data.assets.0;

		// Accumulated asset is sold (in) to the pool for distributed asset (out)
		// Take accumulated asset (in) sans fee from the seller and add to pool
		// Take distributed asset (out) and send to seller
		// Take fee from the seller and send to fee collector
		// Pool bears repay fee
		if fee_asset == assets.asset_in {
			let fee = Self::calculate_fees(&pool_data, amount)?;

			let amount_out = hydra_dx_math::lbp::calculate_out_given_in(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount,
			)
			.map_err(|_| Error::<T>::Overflow)?;

			ensure!(
				amount_out
					<= asset_out_reserve
						.checked_div(T::MaxOutRatio::get())
						.ok_or(Error::<T>::Overflow)?,
				Error::<T>::MaxOutRatioExceeded
			);

			ensure!(min_bought <= amount_out, Error::<T>::TradingLimitReached);

			let amount_without_fee = amount.checked_sub(fee).ok_or(Error::<T>::Overflow)?;

			Ok(AMMTransfer {
				origin: who.clone(),
				assets,
				amount: amount_without_fee,
				amount_b: amount_out,
				discount: false,
				discount_amount: 0_u128,
				fee: (fee_asset, fee),
			})

		// Distributed asset is sold (in) to the pool for accumulated asset (out)
		// Take accumulated asset (out) from the pool sans fee and send to the seller
		// Take distributed asset (in) from the seller and send to pool
		// Take fee from the pool and send to fee collector
		// Seller bears repay fee
		} else {
			let calculated_out = hydra_dx_math::lbp::calculate_out_given_in(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount,
			)
			.map_err(|_| Error::<T>::Overflow)?;

			let fee = Self::calculate_fees(&pool_data, calculated_out)?;
			let amount_out_without_fee = calculated_out.checked_sub(fee).ok_or(Error::<T>::Overflow)?;

			ensure!(
				calculated_out
					<= asset_out_reserve
						.checked_div(T::MaxOutRatio::get())
						.ok_or(Error::<T>::Overflow)?,
				Error::<T>::MaxOutRatioExceeded
			);

			ensure!(min_bought <= amount_out_without_fee, Error::<T>::TradingLimitReached);

			Ok(AMMTransfer {
				origin: who.clone(),
				assets,
				amount,
				amount_b: amount_out_without_fee,
				discount: false,
				discount_amount: 0_u128,
				fee: (fee_asset, fee),
			})
		}
	}

	fn execute_sell(transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>) -> DispatchResult {
		Self::execute_trade(transfer)?;

		Self::deposit_event(Event::<T>::SellExecuted {
			who: transfer.origin.clone(),
			asset_in: transfer.assets.asset_in,
			asset_out: transfer.assets.asset_out,
			amount: transfer.amount,
			sale_price: transfer.amount_b,
			fee_asset: transfer.fee.0,
			fee_amount: transfer.fee.1,
		});

		Ok(())
	}

	fn validate_buy(
		who: &T::AccountId,
		assets: AssetPair,
		amount: BalanceOf<T>,
		max_sold: BalanceOf<T>,
		_discount: bool,
	) -> Result<AMMTransfer<T::AccountId, AssetId, AssetPair, Balance>, DispatchError> {
		ensure!(!amount.is_zero(), Error::<T>::ZeroAmount);

		let pool_id = Self::get_pair_id(assets);
		let pool_data = <PoolData<T>>::try_get(&pool_id).map_err(|_| Error::<T>::PoolNotFound)?;

		ensure!(Self::is_pool_running(&pool_data), Error::<T>::SaleIsNotRunning);

		let now = T::BlockNumberProvider::current_block_number();
		let (weight_in, weight_out) = Self::get_sorted_weight(assets.asset_in, now, &pool_data)?;
		let asset_in_reserve = T::MultiCurrency::free_balance(assets.asset_in, &pool_id);
		let asset_out_reserve = T::MultiCurrency::free_balance(assets.asset_out, &pool_id);

		ensure!(
			amount
				<= asset_out_reserve
					.checked_div(T::MaxOutRatio::get())
					.ok_or(Error::<T>::Overflow)?,
			Error::<T>::MaxOutRatioExceeded
		);

		// LBP fee asset is always accumulated asset
		let fee_asset = pool_data.assets.0;

		// Accumulated asset is bought (out) of the pool for distributed asset (in)
		// Take accumulated asset (out) sans fee from the pool and send to seller
		// Take distributed asset (in) from the seller and add to pool
		// Take fee from the pool and send to fee collector
		// Buyer bears repay fee
		if fee_asset == assets.asset_out {
			let fee = Self::calculate_fees(&pool_data, amount)?;
			let amount_out_plus_fee = amount.checked_add(fee).ok_or(Error::<T>::Overflow)?;

			let calculated_in = hydra_dx_math::lbp::calculate_in_given_out(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount_out_plus_fee,
			)
			.map_err(|_| Error::<T>::Overflow)?;

			ensure!(
				calculated_in
					<= asset_in_reserve
						.checked_div(T::MaxInRatio::get())
						.ok_or(Error::<T>::Overflow)?,
				Error::<T>::MaxInRatioExceeded
			);

			ensure!(
				T::MultiCurrency::free_balance(assets.asset_in, who) >= calculated_in,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(max_sold >= calculated_in, Error::<T>::TradingLimitReached);

			Ok(AMMTransfer {
				origin: who.clone(),
				assets,
				amount: calculated_in,
				amount_b: amount,
				discount: false,
				discount_amount: 0_u128,
				fee: (fee_asset, fee),
			})

		// Distributed asset is bought (out) of the pool for accumulated asset (in)
		// Take accumulated asset (in) sans fee from the buyer and send to pool
		// Take distributed asset (out) from the pool and send to buyer
		// Take fee from the buyer and send to fee collector
		// Pool bears repay fee
		} else {
			let calculated_in = hydra_dx_math::lbp::calculate_in_given_out(
				asset_in_reserve,
				asset_out_reserve,
				weight_in,
				weight_out,
				amount,
			)
			.map_err(|_| Error::<T>::Overflow)?;

			let fee = Self::calculate_fees(&pool_data, calculated_in)?;
			let calculated_in_without_fee = calculated_in.checked_sub(fee).ok_or(Error::<T>::Overflow)?;

			ensure!(
				calculated_in
					<= asset_in_reserve
						.checked_div(T::MaxInRatio::get())
						.ok_or(Error::<T>::Overflow)?,
				Error::<T>::MaxInRatioExceeded
			);

			ensure!(
				T::MultiCurrency::free_balance(assets.asset_in, who) >= calculated_in,
				Error::<T>::InsufficientAssetBalance
			);

			ensure!(max_sold >= calculated_in, Error::<T>::TradingLimitReached);

			Ok(AMMTransfer {
				origin: who.clone(),
				assets,
				amount: calculated_in_without_fee,
				amount_b: amount,
				discount: false,
				discount_amount: 0_u128,
				fee: (fee_asset, fee),
			})
		}
	}

	fn execute_buy(transfer: &AMMTransfer<T::AccountId, AssetId, AssetPair, BalanceOf<T>>) -> DispatchResult {
		Self::execute_trade(transfer)?;

		Self::deposit_event(Event::<T>::BuyExecuted {
			who: transfer.origin.clone(),
			asset_out: transfer.assets.asset_out,
			asset_in: transfer.assets.asset_in,
			amount: transfer.amount,
			buy_price: transfer.amount_b,
			fee_asset: transfer.fee.0,
			fee_amount: transfer.fee.1,
		});
		Ok(())
	}

	fn get_min_trading_limit() -> Balance {
		T::MinTradingLimit::get()
	}

	fn get_min_pool_liquidity() -> Balance {
		T::MinPoolLiquidity::get()
	}

	fn get_max_in_ratio() -> u128 {
		T::MaxInRatio::get()
	}

	fn get_max_out_ratio() -> u128 {
		T::MaxOutRatio::get()
	}

	fn get_fee(pool_account_id: &T::AccountId) -> (u32, u32) {
		let maybe_pool_data = <PoolData<T>>::get(pool_account_id);
		match maybe_pool_data {
			Some(pool_data) => pool_data.fee,
			None => (0, 0),
		}
	}
}

pub struct DisallowWhenLBPPoolRunning<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> CanCreatePool<AssetId> for DisallowWhenLBPPoolRunning<T> {
	fn can_create(asset_a: AssetId, asset_b: AssetId) -> bool {
		let pool_id = Pallet::<T>::pair_account_from_assets(asset_a, asset_b);
		let now = T::BlockNumberProvider::current_block_number();
		match <PoolData<T>>::try_get(&pool_id) {
			// returns true if the pool exists and the sale ended
			Ok(data) => match data.end {
				Some(end) => end < now,
				None => false,
			},
			_ => true,
		}
	}
}
