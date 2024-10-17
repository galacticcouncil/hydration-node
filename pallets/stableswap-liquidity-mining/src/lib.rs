// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Omnipool liquidity mining pallet
//!
//! ## Overview
//!
//! This pallet provides functionality for a liquidity mining program with a time incentive (loyalty
//! factor) and multiple incentives scheme for Omnipools AMM.
//!
//! This pallet is build on top of the [pallet-liquidity-mining]
//! (https://github.com/galacticcouncil/warehouse/tree/main/liquidity-mining). This liquidity
//! mining pallet doesn't allow to specify `incentized_asset`. `valued_shares` are always valued in
//! [LRNA]. Farm's owner is responsible for managing exchange rate(`lrna_price_adjustment`) between [LRNA] and
//! `reward_currency`.
//!
//! ### Terminology
//!
//! * **LP:**  liquidity provider
//! * **Position:** omnipool's LP position
//! * **Deposit:** omnipool's position(LP shares) locked in the liquidity mining

#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

#[cfg(test)]
mod tests;

pub mod migration;
pub mod weights;

use frame_support::{
	ensure,
	pallet_prelude::{DispatchError, DispatchResult},
	require_transactional,
	sp_runtime::traits::{AccountIdConversion, Zero},
	traits::DefensiveOption,
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate, Transfer},
		Get,
	},
	PalletId,
};
use frame_system::{
	ensure_signed,
	pallet_prelude::{BlockNumberFor, OriginFor},
};
use hydra_dx_math::ema::EmaPrice as Price;
use hydradx_traits::{
	liquidity_mining::{GlobalFarmId, Mutate as LiquidityMiningMutate, YieldFarmId},
	oracle::{AggregatedPriceOracle, OraclePeriod, Source},
};
use orml_traits::MultiCurrency;
pub use pallet::*;
use pallet_ema_oracle::OracleError;
use pallet_liquidity_mining::{FarmMultiplier, LoyaltyCurve};
use pallet_stableswap::types::AssetAmount;
use primitive_types::U256;
use primitives::Balance;
use primitives::{CollectionId, ItemId as DepositId};
use sp_runtime::{ArithmeticError, FixedU128, Perquintill};
use sp_std::vec;
pub use weights::WeightInfo;

type StableswapPallet<T> = pallet_stableswap::Pallet<T>;
type PeriodOf<T> = BlockNumberFor<T>;

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T> {
		#[serde(skip)]
		pub _marker: PhantomData<T>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			let pallet_account = <Pallet<T>>::account_id();

			<T as pallet::Config>::NFTHandler::create_collection(
				&<T as pallet::Config>::NFTCollectionId::get(),
				&pallet_account,
				&pallet_account,
			)
			.unwrap()
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_stableswap::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Currency for transfers.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// The origin account that can create new liquidity mining program.
		type CreateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Pallet id.
		type PalletId: Get<PalletId>;

		/// NFT collection id for liquidity mining's deposit nfts.
		#[pallet::constant]
		type NFTCollectionId: Get<CollectionId>;

		/// Non fungible handling
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, ItemId = DepositId, CollectionId = CollectionId>
			+ Transfer<Self::AccountId>;

		/// Liquidity mining handler for managing liquidity mining functionalities
		type LiquidityMiningHandler: LiquidityMiningMutate<
			Self::AccountId,
			Self::AssetId,
			BlockNumberFor<Self>,
			Error = DispatchError,
			AmmPoolId = Self::AssetId,
			Balance = Balance,
			LoyaltyCurve = LoyaltyCurve,
			Period = PeriodOf<Self>,
		>;

		/// Identifier of oracle data soruce
		#[pallet::constant]
		type OracleSource: Get<Source>;

		/// Oracle's price aggregation period.
		#[pallet::constant]
		type OraclePeriod: Get<OraclePeriod>;

		/// Oracle providing price of LRNA/{Asset} used to calculate `valued_shares`.
		type PriceOracle: AggregatedPriceOracle<Self::AssetId, BlockNumberFor<Self>, Price, Error = OracleError>;

		/// Maximum number of farm entries per deposit.
		type MaxFarmEntriesPerDeposit: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New global farm was created.
		GlobalFarmCreated {
			id: GlobalFarmId,
			owner: T::AccountId,
			total_rewards: Balance,
			reward_currency: T::AssetId,
			yield_per_period: Perquintill,
			planned_yielding_periods: PeriodOf<T>,
			blocks_per_period: BlockNumberFor<T>,
			max_reward_per_period: Balance,
			min_deposit: Balance,
			lrna_price_adjustment: FixedU128,
		},

		/// Global farm was updated
		GlobalFarmUpdated {
			id: GlobalFarmId,
			planned_yielding_periods: PeriodOf<T>,
			yield_per_period: Perquintill,
			min_deposit: Balance,
		},

		/// Global farm was terminated.
		GlobalFarmTerminated {
			global_farm_id: GlobalFarmId,
			who: T::AccountId,
			reward_currency: T::AssetId,
			undistributed_rewards: Balance,
		},

		/// New yield farm was added to the farm.
		YieldFarmCreated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			multiplier: FarmMultiplier,
			loyalty_curve: Option<LoyaltyCurve>,
		},

		/// Yield farm multiplier was updated.
		YieldFarmUpdated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId, //TODO: possibly rename to pool id
			who: T::AccountId,
			multiplier: FarmMultiplier,
		},

		/// Yield farm for `asset_id` was stopped.
		YieldFarmStopped {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
		},

		/// Yield farm for `asset_id` was resumed.
		YieldFarmResumed {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
			multiplier: FarmMultiplier,
		},

		/// Yield farm was terminated from the global farm.
		YieldFarmTerminated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			asset_id: T::AssetId,
			who: T::AccountId,
		},

		/// New LP shares(LP position) were deposited.
		SharesDeposited {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			deposit_id: DepositId,
			asset_id: T::AssetId,
			who: T::AccountId,
			shares_amount: Balance,
		},

		/// Already locked LP shares were redeposited to another yield farm.
		SharesRedeposited {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			deposit_id: DepositId,
			asset_id: T::AssetId,
			who: T::AccountId,
			shares_amount: Balance,
		},

		/// Rewards were claimed.
		RewardClaimed {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			who: T::AccountId,
			claimed: Balance,
			reward_currency: T::AssetId,
			deposit_id: DepositId,
		},

		/// LP shares were withdrawn.
		SharesWithdrawn {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
			who: T::AccountId,
			amount: Balance,
			deposit_id: DepositId,
		},

		/// All LP shares were unlocked and NFT representing deposit was destroyed.
		DepositDestroyed { who: T::AccountId, deposit_id: DepositId },
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Asset is not in the omnipool.
		AssetNotFound,

		/// Signed account is not owner of the deposit.
		Forbidden,

		/// Rewards to claim are 0.
		ZeroClaimedRewards,

		/// Action cannot be completed because unexpected error has occurred. This should be reported
		/// to protocol maintainers.
		InconsistentState(InconsistentStateError),

		/// Oracle could not be found for requested assets.
		OracleNotAvailable,

		/// Oracle providing `price_adjustment` could not be found for requested assets.
		PriceAdjustmentNotAvailable,

		/// The extrinsic is disabled for now.
		Disabled,

		/// No farms specified to join
		NoFarmEntriesSpecified,

		///Deposit data not found
		DepositDataNotFound,

		/// No global farm - yield farm pairs specified to join
		NoFarmsSpecified,
	}

	//NOTE: these errors should never happen.
	#[derive(Encode, Decode, Eq, PartialEq, TypeInfo, frame_support::PalletError, RuntimeDebug)]
	pub enum InconsistentStateError {
		/// Deposit data not found.
		DepositDataNotFound,
	}

	impl<T> From<InconsistentStateError> for Error<T> {
		fn from(e: InconsistentStateError) -> Error<T> {
			Error::<T>::InconsistentState(e)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		//TODO: readjust doc for all extrnsics
		/// Create a new liquidity mining program with provided parameters.
		///
		/// `owner` account has to have at least `total_rewards` balance. These funds will be
		/// transferred from `owner` to farm account.
		///
		/// The dispatch origin for this call must be `T::CreateOrigin`.
		/// !!!WARN: `T::CreateOrigin` has power over funds of `owner`'s account and it should be
		/// configured to trusted origin e.g Sudo or Governance.
		///
		/// Parameters:
		/// - `origin`: account allowed to create new liquidity mining program(root, governance).
		/// - `total_rewards`: total rewards planned to distribute. These rewards will be
		/// distributed between all yield farms in the global farm.
		/// - `planned_yielding_periods`: planned number of periods to distribute `total_rewards`.
		/// WARN: THIS IS NOT HARD DEADLINE. Not all rewards have to be distributed in
		/// `planned_yielding_periods`. Rewards are distributed based on the situation in the yield
		/// farms and can be distributed in a longer, though never in a shorter, time frame.
		/// - `blocks_per_period`:  number of blocks in a single period. Min. number of blocks per
		/// period is 1.
		/// - `reward_currency`: payoff currency of rewards.
		/// - `owner`: liq. mining farm owner. This account will be able to manage created
		/// liquidity mining program.
		/// - `yield_per_period`: percentage return on `reward_currency` of all farms.
		/// - `min_deposit`: minimum amount of LP shares to be deposited into the liquidity mining by each user.
		/// - `lrna_price_adjustment`: price adjustment between `[LRNA]` and `reward_currency`.
		///
		/// Emits `GlobalFarmCreated` when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::create_global_farm())]
		pub fn create_global_farm(
			origin: OriginFor<T>,
			total_rewards: Balance,
			planned_yielding_periods: PeriodOf<T>,
			blocks_per_period: BlockNumberFor<T>,
			incentivized_asset: T::AssetId,
			reward_currency: T::AssetId,
			owner: T::AccountId,
			yield_per_period: Perquintill,
			min_deposit: Balance,
			lrna_price_adjustment: FixedU128,
		) -> DispatchResult {
			<T as pallet::Config>::CreateOrigin::ensure_origin(origin)?;

			let (id, max_reward_per_period) = T::LiquidityMiningHandler::create_global_farm(
				total_rewards,
				planned_yielding_periods,
				blocks_per_period,
				incentivized_asset,
				reward_currency,
				owner.clone(),
				yield_per_period,
				min_deposit,
				lrna_price_adjustment,
			)?;

			Self::deposit_event(Event::GlobalFarmCreated {
				id,
				owner,
				total_rewards,
				reward_currency,
				yield_per_period,
				planned_yielding_periods,
				blocks_per_period,
				max_reward_per_period,
				min_deposit,
				lrna_price_adjustment,
			});

			Ok(())
		}

		/// Terminate existing liq. mining program.
		///
		/// Only farm owner can perform this action.
		///
		/// WARN: To successfully terminate a global farm, farm have to be empty
		/// (all yield farms in the global farm must be terminated).
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: id of global farm to be terminated.
		///
		/// Emits `GlobalFarmTerminated` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::terminate_global_farm())]
		pub fn terminate_global_farm(origin: OriginFor<T>, global_farm_id: GlobalFarmId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (reward_currency, undistributed_rewards, who) =
				T::LiquidityMiningHandler::terminate_global_farm(who, global_farm_id)?;

			Self::deposit_event(Event::GlobalFarmTerminated {
				global_farm_id,
				who,
				reward_currency,
				undistributed_rewards,
			});

			Ok(())
		}

		/// Create yield farm for given `asset_id` in the omnipool.
		///
		/// Only farm owner can perform this action.
		///
		/// Asset with `asset_id` has to be registered in the omnipool.
		/// At most one `active` yield farm can exist in one global farm for the same `asset_id`.
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: global farm id to which a yield farm will be added.
		/// - `asset_id`: id of a asset in the omnipool. Yield farm will be created
		/// for this asset and user will be able to lock LP shares into this yield farm immediately.
		/// - `multiplier`: yield farm's multiplier.
		/// - `loyalty_curve`: curve to calculate loyalty multiplier to distribute rewards to users
		/// with time incentive. `None` means no loyalty multiplier.
		///
		/// Emits `YieldFarmCreated` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::create_yield_farm())]
		pub fn create_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			pool_id: T::AssetId,
			multiplier: FarmMultiplier,
			loyalty_curve: Option<LoyaltyCurve>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let pool = StableswapPallet::<T>::pool(pool_id).ok_or(pallet_stableswap::Error::<T>::PoolNotFound)?;

			let yield_farm_id = T::LiquidityMiningHandler::create_yield_farm(
				who,
				global_farm_id,
				multiplier,
				loyalty_curve.clone(),
				pool_id,
				pool.assets.to_vec(),
			)?;

			Self::deposit_event(Event::YieldFarmCreated {
				global_farm_id,
				yield_farm_id,
				asset_id: pool_id,
				multiplier,
				loyalty_curve,
			});

			Ok(())
		}

		/// Update yield farm's multiplier.
		///
		/// Only farm owner can perform this action.
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: global farm id in which yield farm will be updated.
		/// - `asset_id`: id of the asset identifying yield farm in the global farm.
		/// - `multiplier`: new yield farm's multiplier.
		///
		/// Emits `YieldFarmUpdated` event when successful.
		///
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::update_yield_farm())]
		pub fn update_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			pool_id: T::AssetId,
			multiplier: FarmMultiplier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			StableswapPallet::<T>::pool(pool_id).ok_or(pallet_stableswap::Error::<T>::PoolNotFound)?;

			let yield_farm_id = T::LiquidityMiningHandler::update_yield_farm_multiplier(
				who.clone(),
				global_farm_id,
				pool_id,
				multiplier,
			)?;

			Self::deposit_event(Event::YieldFarmUpdated {
				global_farm_id,
				yield_farm_id,
				asset_id: pool_id,
				multiplier,
				who,
			});

			Ok(())
		}

		/// Stop liquidity miming for specific yield farm.
		///
		/// This function claims rewards from `GlobalFarm` last time and stop yield farm
		/// incentivization from a `GlobalFarm`. Users will be able to only withdraw
		/// shares(with claiming) after calling this function.
		/// `deposit_shares()` is not allowed on stopped yield farm.
		///
		/// Only farm owner can perform this action.
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: farm id in which yield farm will be canceled.
		/// - `asset_id`: id of the asset identifying yield farm in the global farm.
		///
		/// Emits `YieldFarmStopped` event when successful.
		///
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::stop_yield_farm())]
		pub fn stop_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			pool_id: T::AssetId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let yield_farm_id = T::LiquidityMiningHandler::stop_yield_farm(who.clone(), global_farm_id, pool_id)?;

			Self::deposit_event(Event::YieldFarmStopped {
				global_farm_id,
				yield_farm_id,
				asset_id: pool_id,
				who,
			});

			Ok(())
		}

		/// Resume incentivization of the asset represented by yield farm.
		///
		/// This function resume incentivization of the asset from the `GlobalFarm` and
		/// restore full functionality or the yield farm. Users will be able to deposit,
		/// claim and withdraw again.
		///
		/// WARN: Yield farm(and users) is NOT rewarded for time it was stopped.
		///
		/// Only farm owner can perform this action.
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: global farm id in which yield farm will be resumed.
		/// - `yield_farm_id`: id of the yield farm to be resumed.
		/// - `asset_id`: id of the asset identifying yield farm in the global farm.
		/// - `multiplier`: yield farm multiplier.
		///
		/// Emits `YieldFarmResumed` event when successful.
		///
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::resume_yield_farm())]
		pub fn resume_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
			multiplier: FarmMultiplier,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			StableswapPallet::<T>::pool(pool_id).ok_or(pallet_stableswap::Error::<T>::PoolNotFound)?;

			T::LiquidityMiningHandler::resume_yield_farm(
				who.clone(),
				global_farm_id,
				yield_farm_id,
				pool_id,
				multiplier,
			)?;

			Self::deposit_event(Event::<T>::YieldFarmResumed {
				global_farm_id,
				yield_farm_id,
				asset_id: pool_id,
				who,
				multiplier,
			});

			Ok(())
		}

		/// Terminate yield farm.
		///
		/// This function marks a yield farm as ready to be removed from storage when it's empty. Users will
		/// be able to only withdraw shares(without claiming rewards from yield farm). Unpaid rewards
		/// will be transferred back to global farm and it will be used to distribute to other yield farms.
		///
		/// Yield farm must be stopped before it can be terminated.
		///
		/// Only global farm's owner can perform this action. Yield farm stays in the storage until it's
		/// empty(all farm entries are withdrawn). Last withdrawn from yield farm trigger removing from
		/// the storage.
		///
		/// Parameters:
		/// - `origin`: global farm's owner.
		/// - `global_farm_id`: global farm id in which yield farm should be terminated.
		/// - `yield_farm_id`: id of yield farm to be terminated.
		/// - `asset_id`: id of the asset identifying yield farm.
		///
		/// Emits `YieldFarmTerminated` event when successful.
		///
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::terminate_yield_farm())]
		pub fn terminate_yield_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;

			//NOTE: don't check asset existence in the omnipool, owner must be able to terminate yield farm.
			T::LiquidityMiningHandler::terminate_yield_farm(who.clone(), global_farm_id, yield_farm_id, pool_id)?;

			Self::deposit_event(Event::YieldFarmTerminated {
				global_farm_id,
				yield_farm_id,
				asset_id: pool_id,
				who,
			});

			Ok(())
		}

		/// Deposit omnipool position(LP shares) to a liquidity mining.
		///
		/// This function transfers omnipool position from `origin` to pallet's account and mint NFT for
		/// `origin` account. Minted NFT represents deposit in the liquidity mining. User can
		/// deposit omnipool position as a whole(all the LP shares in the position).
		///
		/// Parameters:
		/// - `origin`: owner of the omnipool position to deposit into the liquidity mining.
		/// - `global_farm_id`: id of global farm to which user wants to deposit LP shares.
		/// - `yield_farm_id`: id of yield farm to deposit to.
		/// - `position_id`: id of the omnipool position to be deposited into the liquidity mining.
		///
		/// Emits `SharesDeposited` event when successful.
		///
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::deposit_shares().saturating_add(T::PriceOracle::get_price_weight()))]
		pub fn deposit_shares(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
			shares_amount: Balance,
		) -> DispatchResult {
			Self::do_deposit_shares(origin, global_farm_id, yield_farm_id, pool_id, shares_amount)?;

			Ok(())
		}

		/// Redeposit LP shares in the already locked omnipool position.
		///
		/// This function create yield farm entry for existing deposit. Amount of redeposited LP
		/// shares is same as amount shares which are already deposited in the deposit.
		///
		/// This function DOESN'T create new deposit(NFT).
		///
		/// Parameters:
		/// - `origin`: owner of the deposit to redeposit.
		/// - `global_farm_id`: id of the global farm to which user wants to redeposit LP shares.
		/// - `yield_farm_id`: id of the yield farm to redeposit to.
		/// - `deposit_id`: identifier of the deposit to redeposit.
		///
		/// Emits `SharesRedeposited` event when successful.
		///
		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::redeposit_shares().saturating_add(T::PriceOracle::get_price_weight()))]
		pub fn redeposit_shares(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
			deposit_id: DepositId,
		) -> DispatchResult {
			let owner = Self::ensure_nft_owner(origin, deposit_id)?;

			//NOTE: pallet should be owner of the omnipool position at this point.
			/*let lp_position = StableswapPallet::<T>::load_position(position_id, Self::account_id())?;
			ensure!(
				StableswapPallet::<T>::exists(lp_position.asset_id),
				Error::<T>::AssetNotFound
			);*/

			let (shares_amount, deposit_amm_pool_id) = T::LiquidityMiningHandler::redeposit_lp_shares(
				global_farm_id,
				yield_farm_id,
				deposit_id,
				Self::get_value_in_asset,
			)?;

			Self::deposit_event(Event::SharesRedeposited {
				global_farm_id,
				yield_farm_id,
				deposit_id,
				asset_id: pool_id,
				who: owner,
				shares_amount,
			});

			Ok(())
		}

		/// Note: This extrinsic is disabled
		///
		/// Claim rewards from liquidity mining program for deposit represented by the `deposit_id`.
		///
		/// This function calculate user rewards from liquidity mining and transfer rewards to `origin`
		/// account. Claiming multiple time the same period is not allowed.
		///
		/// Parameters:
		/// - `origin`: owner of deposit.
		/// - `deposit_id`: id of the deposit to claim rewards for.
		/// - `yield_farm_id`: id of the yield farm to claim rewards from.
		///
		/// Emits `RewardClaimed` event when successful.
		///
		#[pallet::call_index(10)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		pub fn claim_rewards(
			_origin: OriginFor<T>,
			_deposit_id: DepositId,
			_yield_farm_id: YieldFarmId,
		) -> DispatchResult {
			return Err(Error::<T>::Disabled.into());
		}

		/// Withdraw LP shares from liq. mining with reward claiming if possible.
		///
		/// List of possible cases of transfers of LP shares and claimed rewards:
		///
		/// * yield farm is active(yield farm is not stopped) - claim and transfer rewards(if it
		/// wasn't claimed in this period) and transfer LP shares.
		/// * liq. mining is stopped - claim and transfer rewards(if it
		/// wasn't claimed in this period) and transfer LP shares.
		/// * yield farm was terminated - only LP shares will be transferred.
		/// * farm was terminated - only LP shares will be transferred.
		///
		/// User's unclaimable rewards will be transferred back to global farm's account.
		///
		/// Parameters:
		/// - `origin`: account owner of deposit(nft).
		/// - `deposit_id`: nft id representing deposit in the yield farm.
		/// - `yield_farm_id`: yield farm identifier to dithdraw shares from.
		/// - `asset_pair`: asset pair identifying yield farm in global farm.
		///
		/// Emits:
		/// * `RewardClaimed` if claim happen
		/// * `SharesWithdrawn` event when successful
		#[pallet::call_index(11)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_shares())]
		pub fn withdraw_shares(
			origin: OriginFor<T>,
			deposit_id: DepositId,
			yield_farm_id: YieldFarmId,
			pool_id: T::AssetId,
		) -> DispatchResult {
			let owner = Self::ensure_nft_owner(origin, deposit_id)?;

			let global_farm_id = T::LiquidityMiningHandler::get_global_farm_id(deposit_id, yield_farm_id)
				.ok_or(Error::<T>::DepositDataNotFound)?;

			let (withdrawn_amount, claim_data, is_destroyed) = T::LiquidityMiningHandler::withdraw_lp_shares(
				owner.clone(),
				deposit_id,
				global_farm_id,
				yield_farm_id,
				pool_id.clone(),
			)?;

			if let Some((reward_currency, claimed, _)) = claim_data {
				if !claimed.is_zero() {
					Self::deposit_event(Event::RewardClaimed {
						global_farm_id,
						yield_farm_id,
						who: owner.clone(),
						claimed,
						reward_currency,
						deposit_id,
					});
				}
			}

			let lp_token = pool_id;
			if !withdrawn_amount.is_zero() {
				Self::deposit_event(Event::SharesWithdrawn {
					global_farm_id,
					yield_farm_id,
					pool_id: lp_token,
					who: owner.clone(),
					amount: withdrawn_amount,
					deposit_id,
				});
			}

			if is_destroyed {
				Self::unlock_lp_tokens(lp_token, &owner, withdrawn_amount)?;
				T::NFTHandler::burn(&T::NFTCollectionId::get(), &deposit_id, Some(&owner))?;

				Self::deposit_event(Event::DepositDestroyed { who: owner, deposit_id });
			}

			Ok(())
		}

		/// This extrinsic updates global farm's main parameters.
		///
		/// The dispatch origin for this call must be `T::CreateOrigin`.
		/// !!!WARN: `T::CreateOrigin` has power over funds of `owner`'s account and it should be
		/// configured to trusted origin e.g Sudo or Governance.
		///
		/// Parameters:
		/// - `origin`: account allowed to create new liquidity mining program(root, governance).
		/// - `global_farm_id`: id of the global farm to update.
		/// - `planned_yielding_periods`: planned number of periods to distribute `total_rewards`.
		/// - `yield_per_period`: percentage return on `reward_currency` of all farms.
		/// - `min_deposit`: minimum amount of LP shares to be deposited into the liquidity mining by each user.
		///
		/// Emits `GlobalFarmUpdated` event when successful.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as Config>::WeightInfo::update_global_farm())]
		pub fn update_global_farm(
			origin: OriginFor<T>,
			global_farm_id: GlobalFarmId,
			planned_yielding_periods: crate::PeriodOf<T>,
			yield_per_period: Perquintill,
			min_deposit: Balance,
		) -> DispatchResult {
			T::CreateOrigin::ensure_origin(origin)?;

			T::LiquidityMiningHandler::update_global_farm(
				global_farm_id,
				planned_yielding_periods,
				yield_per_period,
				min_deposit,
			)?;

			Self::deposit_event(Event::GlobalFarmUpdated {
				id: global_farm_id,
				planned_yielding_periods,
				yield_per_period,
				min_deposit,
			});

			Ok(())
		}

		/// Join multiple farms with a given share amount
		///
		/// The share is deposited to the first farm of the specified fams,
		/// and then redeposit the shares to the remaining farms
		///
		/// Parameters:
		/// - `origin`: account depositing LP shares. This account has to have at least
		/// - `farm_entries`: list of global farm id and yield farm id pairs to join
		/// - `asset_pair`: asset pair identifying LP shares user wants to deposit.
		/// - `shares_amount`: amount of LP shares user wants to deposit.
		///
		/// Emits `SharesDeposited` event for the first farm entry
		/// Emits `SharesRedeposited` event for each farm entry after the first one
		#[pallet::call_index(13)]
		#[pallet::weight(<T as Config>::WeightInfo::deposit_shares())] //TODO: add proper weight, dynamic one based on farm
		pub fn join_farms(
			origin: OriginFor<T>,
			farm_entries: BoundedVec<(GlobalFarmId, YieldFarmId), T::MaxFarmEntriesPerDeposit>,
			pool_id: T::AssetId,
			shares_amount: Balance,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;

			let (global_farm_id, yield_farm_id) = farm_entries.first().ok_or(Error::<T>::NoFarmsSpecified)?;
			let deposit_id = Self::do_deposit_shares(origin, *global_farm_id, *yield_farm_id, pool_id, shares_amount)?;

			for (global_farm_id, yield_farm_id) in farm_entries.into_iter().skip(1) {
				let (redeposited_amount, _) = T::LiquidityMiningHandler::redeposit_lp_shares(
					global_farm_id,
					yield_farm_id,
					deposit_id,
					Self::get_value_in_asset,
				)?;

				Self::deposit_event(Event::SharesRedeposited {
					global_farm_id,
					yield_farm_id,
					asset_id: pool_id,
					who: who.clone(),
					shares_amount: redeposited_amount,
					deposit_id,
				});
			}

			Ok(())
		}

		/// Add liquidity to XYK pool and join multiple farms with a given share amount
		///
		/// The share is deposited to the first farm of the specified entries,
		/// and then redeposit the shares to the remaining farms
		///
		/// Parameters:
		/// - `origin`: account depositing LP shares. This account has to have at least
		/// - `asset_a`: asset id of the first asset in the pair
		/// - `asset_b`: asset id of the second asset in the pair
		/// - `amount_a`: amount of the first asset to deposit
		/// - `amount_b_max_limit`: maximum amount of the second asset to deposit
		/// - `farm_entries`: list of global farm id and yield farm id pairs to join
		///
		/// Emits `SharesDeposited` event for the first farm entry
		/// Emits `SharesRedeposited` event for each farm entry after the first one
		#[pallet::call_index(14)]
		#[pallet::weight(<T as Config>::WeightInfo::deposit_shares())] //TODO: add proper weight, dynamic one based on farm
		pub fn add_liquidity_and_join_farms(
			origin: OriginFor<T>,
			pool_id: T::AssetId,
			assets: Vec<AssetAmount<T::AssetId>>,
			farm_entries: BoundedVec<(GlobalFarmId, YieldFarmId), T::MaxFarmEntriesPerDeposit>,
		) -> DispatchResult {
			let who = ensure_signed(origin.clone())?;
			ensure!(!farm_entries.is_empty(), Error::<T>::NoFarmsSpecified);

			StableswapPallet::<T>::add_liquidity(origin.clone(), pool_id, assets)?;

			let shares_amount = <T as Config>::Currency::free_balance(pool_id, &who);

			Self::join_farms(origin, farm_entries, pool_id, shares_amount)?;

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Account ID of the pot holding all the locked omnipool's positions(NFTs). This account
	/// is also owner of the NFT collection used to mint liqudity mining's NFTs.
	pub fn account_id() -> T::AccountId {
		<T as pallet::Config>::PalletId::get().into_account_truncating()
	}

	/// This function transfers omnipool's position NFT to liquidity mining's account. This
	/// function also saves mapping of the deposit's id to omnipool position's id.
	fn lock_lp_tokens(lp_token: T::AssetId, who: &T::AccountId, amount: Balance) -> Result<(), DispatchError> {
		let service_account_for_lp_shares = Self::account_id();

		<T as Config>::Currency::transfer(lp_token, who, &service_account_for_lp_shares, amount)
	}

	fn unlock_lp_tokens(lp_token: T::AssetId, who: &T::AccountId, amount: Balance) -> Result<(), DispatchError> {
		let service_account_for_lp_shares = Self::account_id();

		<T as Config>::Currency::transfer(lp_token, &service_account_for_lp_shares, who, amount)
	}

	/// This function returns value of shares in incentivized asset.
	fn get_value_in_asset(
		incentivized_asset: T::AssetId,
		pool_id: T::AssetId,
		shares: Balance,
	) -> Result<Balance, DispatchError> {
		let (price, _) = T::PriceOracle::get_price(
			incentivized_asset,
			pool_id,
			T::OraclePeriod::get(),
			T::OracleSource::get(),
		)
		.map_err(|_| Error::<T>::OracleNotAvailable)?;

		let position_value: u128 = U256::from(shares)
			.checked_mul(price.n.into())
			.ok_or(ArithmeticError::Overflow)?
			.checked_div(price.d.into())
			.ok_or(ArithmeticError::DivisionByZero)?
			.try_into()
			.map_err(|_| ArithmeticError::Overflow)?;

		Ok(position_value)
	}

	/// This function check if origin is signed and returns account if account is owner of the
	/// deposit.
	fn ensure_nft_owner(origin: OriginFor<T>, deposit_id: DepositId) -> Result<T::AccountId, DispatchError> {
		let who = ensure_signed(origin)?;

		let nft_owner =
			<T as pallet::Config>::NFTHandler::owner(&<T as pallet::Config>::NFTCollectionId::get(), &deposit_id)
				.ok_or(Error::<T>::Forbidden)?;

		ensure!(nft_owner == who, Error::<T>::Forbidden);

		Ok(who)
	}

	#[require_transactional]
	fn do_deposit_shares(
		origin: OriginFor<T>,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		pool_id: T::AssetId,
		shares_amount: Balance,
	) -> Result<DepositId, DispatchError> {
		let who = ensure_signed(origin)?;

		ensure!(
			<T as Config>::Currency::ensure_can_withdraw(pool_id, &who, shares_amount).is_ok(),
			pallet_stableswap::Error::<T>::InsufficientBalance
		);

		let deposit_id = T::LiquidityMiningHandler::deposit_lp_shares(
			global_farm_id,
			yield_farm_id,
			pool_id,
			shares_amount,
			Self::get_value_in_asset,
		)?;

		Self::lock_lp_tokens(pool_id, &who, shares_amount)?;
		T::NFTHandler::mint_into(&T::NFTCollectionId::get(), &deposit_id, &who)?;

		Self::deposit_event(Event::SharesDeposited {
			global_farm_id,
			yield_farm_id,
			deposit_id,
			asset_id: pool_id,
			who,
			shares_amount,
		});

		Ok(deposit_id)
	}
}
