// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2022  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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
// Abbr:
//  rpvs - reward per valued share
//  rpz - reward per share in global farm

// Notion spec naming map:
// * shares                 -> s
// * total_shares           -> S
// * valued_shares          -> s'
// * total_valued_shares    -> S'
// * stake_in_global_pool   -> z
// * total_shares_z         -> Z
// * multiplier             -> m

//! # Liquidity mining pallet
//!
//! ## Overview
//!
//! This pallet provides functionality for a liquidity mining program with a time incentive (loyalty
//! factor) and multiple incentives scheme.
//! Users are rewarded for each period they stay in the liq. mining program.
//!
//! Reward per one period is derived from the user's loyalty factor which grows with time (periods)
//! the user is in the liq. mining program and the amount of LP shares the user locked into deposit.
//! User's loyalty factor is reset if the user exits and reenters liquidity mining.
//! User can claim rewards without resetting loyalty factor, only withdrawing shares
//! is penalized by loyalty factor reset.
//! The user is rewarded from the next period after they enters.
//!
//! Multiple Incentives
//!
//! This feature allows users to redeposit already deposited LP shares to multiple yield farms and
//! receive incentives from these farms.
//! Deposit in yield farm is called "farm entry".
//! Maximal number of redepositing same LP shares is configured by variable: `MaxFarmEntriesPerDeposit`.
//! Set `MaxFarmEntriesPerDeposit` to `1` to disable multiple incentives scheme. !!!NEVER set this
//! variable to `0`.
//! LP shares can be redeposited only to different yield farms running liquidity mining for same
//! pair of assets.
//!
//! Notes:
//! * LP shares are returned ONLY if deposit is destroyed - withdrawing LP shares can
//! be used to "free slot" for re-lock LP shares to different yield farm. Withdrawing LP shares result in
//! resetting loyalty factor for yield farm user is withdrawing from(other farm entries in the
//! deposit are not affected). If deposit has no more farm entries, deposit is destroyed and LP
//! shares are returned back to user.
//! * `YieldFarm` -  can be in the 3 states: [`Active`, `Stopped`, `Terminated`]
//!     * `Active` - liquidity mining is running, users are able to deposit, claim and withdraw LP
//!     shares. `YieldFarm` is rewarded from `GlobalFarm` in this state.
//!     * `Stopped` - liquidity mining is stopped. Users can claim and withdraw LP shares from the
//!     farm. Users CAN'T deposit new LP shares to stopped farm. Stopped farm is not rewarded from the
//!     `GlobalFarm`.
//!     Note: stopped farm can be resumed or destroyed.
//!     * `Terminated` - liquidity mining is ended. User's CAN'T deposit or claim rewards from
//!     stopped farm. Users CAN only withdraw LP shares(without rewards).
//!     `YieldFarm` must be stopped before it can be terminated. Terminated farm stays in the storage
//!     until last farm's entry is withdrawn. Last withdrawn from yield farm will remove terminated
//!     farm from the storage.
//!     Note: Terminated farm CAN'T be resumed.
//! * `GlobalFarm` - can be in the 2 states: [`Active`, `Terminated`]
//!     * `Active` - liquidity mining program is running, new yield farms can be added to the
//!     global farm.
//!     * `Terminated` - liquidity mining program is ended. Yield farms can't be added to the global
//!     farm. Global farm MUST be empty(all yield farms in the global farm must be destroyed)
//!     before it can be destroyed. Destroying global farm transfer undistributed rewards to farm's
//!     owner. Terminated global farm stay in the storage until all yield farms are removed from
//!     the storage. Last yield farm removal from storage triggers global farm removal from
//!     storage.
//!     Note: Terminated global farm CAN'T be resumed.
//! * Pot - account holding all rewards allocated for all `YieldFarm`s from all `GlobalFarm`s.
//!   User's rewards are transferred from `pot`'s account to user's accounts.
//!

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::derive_partial_eq_without_eq)]

#[cfg(test)]
mod tests;
mod types;

pub use pallet::*;

pub use crate::types::{
	Balance, DefaultPriceAdjustment, DepositData, DepositId, FarmId, FarmMultiplier, FarmState, GlobalFarmData,
	GlobalFarmId, LoyaltyCurve, YieldFarmData, YieldFarmEntry, YieldFarmId,
};
use codec::{Decode, Encode, FullCodec};
use frame_support::{
	defensive,
	pallet_prelude::*,
	require_transactional,
	sp_runtime::{
		traits::{AccountIdConversion, BlockNumberProvider, MaybeSerializeDeserialize, One, Zero},
		RuntimeDebug,
	},
	traits::{Defensive, DefensiveOption},
	PalletId,
};

use frame_system::pallet_prelude::BlockNumberFor;
use sp_runtime::ArithmeticError;

use hydra_dx_math::liquidity_mining as math;
use hydradx_traits::{liquidity_mining::PriceAdjustment, pools::DustRemovalAccountWhitelist, registry::Registry};
use orml_traits::{GetByKey, MultiCurrency};
use scale_info::TypeInfo;
use sp_arithmetic::{
	fixed_point::FixedU128,
	traits::{CheckedAdd, CheckedDiv, CheckedSub},
	Perquintill,
};
use sp_std::{
	convert::{From, Into, TryInto},
	vec::Vec,
};

type PeriodOf<T> = BlockNumberFor<T>;

//WARN: MIN_YIELD_FARM_MULTIPLIER.check_mul_int(MIN_DEPOSIT) >= 1. This rule is important otherwise
//non-zero deposit can result in a zero stake in global-farm and farm can be falsely identified as
//empty. https://github.com/galacticcouncil/warehouse/issues/127
/// Min value farm's owner can set as `min_deposit`
pub(crate) const MIN_DEPOSIT: Balance = 1_000;
/// Min value farm's owner can set as yield-farm's `multiplier`
pub(crate) const MIN_YIELD_FARM_MULTIPLIER: FixedU128 = FixedU128::from_inner(1_000_000_000_000_000);

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T, I = ()>(PhantomData<(T, I)>);

	#[pallet::hooks]
	impl<T: Config<I>, I: 'static> Hooks<BlockNumberFor<T>> for Pallet<T, I> {
		fn integrity_test() {
			assert!(
				T::MaxFarmEntriesPerDeposit::get().ge(&1_u32),
				"`T::MaxFarmEntriesPerDeposit` must be greater or equal to 1"
			);
		}
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config<I>, I: 'static = ()> {
		#[serde(skip)]
		pub _phantom: PhantomData<(T, I)>,
	}

	#[pallet::genesis_build]
	impl<T: Config<I>, I: 'static> BuildGenesisConfig for GenesisConfig<T, I> {
		fn build(&self) {
			let pot = <Pallet<T, I>>::pot_account_id().unwrap();

			T::NonDustableWhitelistHandler::add_account(&pot).unwrap();
		}
	}

	#[pallet::config]
	pub trait Config<I: 'static = ()>: frame_system::Config + TypeInfo {
		type RuntimeEvent: From<Event<Self, I>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type.
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + MaxEncodedLen + Into<u32>;

		/// Currency for transfers.
		type MultiCurrency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Minimum total rewards to distribute from global farm during liquidity mining.
		#[pallet::constant]
		type MinTotalFarmRewards: Get<Balance>;

		/// Minimum number of periods to run liquidity mining program.
		#[pallet::constant]
		type MinPlannedYieldingPeriods: Get<BlockNumberFor<Self>>;

		/// The block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Id used to identify amm pool in liquidity mining pallet.
		type AmmPoolId: Parameter + Member + Clone + FullCodec + MaxEncodedLen;

		/// Maximum number of yield farms same LP shares can be re/deposited into. This value always
		/// MUST BE >= 1.         
		#[pallet::constant]
		type MaxFarmEntriesPerDeposit: Get<u32>;

		/// Max number of yield farms can exist in global farm. This includes all farms in the
		/// storage(active, stopped, terminated).
		#[pallet::constant]
		type MaxYieldFarmsPerGlobalFarm: Get<u32>;

		/// Asset Registry - used to check if asset is correctly registered in asset registry and
		/// provides information about existential deposit of the asset.
		type AssetRegistry: Registry<Self::AssetId, Vec<u8>, Balance, DispatchError> + GetByKey<Self::AssetId, Balance>;

		/// Account whitelist manager to exclude pool accounts from dusting mechanism.
		type NonDustableWhitelistHandler: DustRemovalAccountWhitelist<Self::AccountId, Error = DispatchError>;

		type PriceAdjustment: PriceAdjustment<
			GlobalFarmData<Self, I>,
			Error = DispatchError,
			PriceAdjustment = FixedU128,
		>;
	}

	#[pallet::error]
	#[cfg_attr(test, derive(Eq, PartialEq))]
	pub enum Error<T, I = ()> {
		/// Global farm does not exist.
		GlobalFarmNotFound,

		/// Yield farm does not exist.
		YieldFarmNotFound,

		/// Multiple claims in the same period is not allowed.
		DoubleClaimInPeriod,

		/// Liquidity mining is canceled.
		LiquidityMiningCanceled,

		/// Liquidity mining is not canceled.
		LiquidityMiningIsActive,

		/// Liquidity mining is in `active` or `terminated` state and action cannot be completed.
		LiquidityMiningIsNotStopped,

		/// LP shares amount is not valid.
		InvalidDepositAmount,

		/// Account is not allowed to perform action.
		Forbidden,

		/// Yield farm multiplier can't be 0.
		InvalidMultiplier,

		/// Yield farm with given `amm_pool_id` already exists in global farm.
		YieldFarmAlreadyExists,

		/// Loyalty curve's initial reward percentage is not valid. Valid range is: [0, 1).
		InvalidInitialRewardPercentage,

		/// One or more yield farms exist in global farm.
		GlobalFarmIsNotEmpty,

		/// Farm's `incentivized_asset` is missing in provided asset pair.
		MissingIncentivizedAsset,

		/// Reward currency balance is not sufficient.
		InsufficientRewardCurrencyBalance,

		/// Blocks per period can't be 0.
		InvalidBlocksPerPeriod,

		/// Yield per period can't be 0.
		InvalidYieldPerPeriod,

		/// Total rewards is less than `MinTotalFarmRewards`.
		InvalidTotalRewards,

		/// Planned yielding periods is less than `MinPlannedYieldingPeriods`.
		InvalidPlannedYieldingPeriods,

		/// Maximum number of locks reached for deposit.
		MaxEntriesPerDeposit,

		/// Trying to lock LP shares into already locked yield farm.
		DoubleLock,

		/// Yield farm entry doesn't exist for given deposit.
		YieldFarmEntryNotFound,

		/// Max number of yield farms in global farm was reached. Global farm can't accept new
		/// yield farms until some yield farm is not removed from storage.
		GlobalFarmIsFull,

		/// Invalid min. deposit was set for global farm.
		InvalidMinDeposit,

		/// Price adjustment multiplier can't be 0.
		InvalidPriceAdjustment,

		/// Account creation from id failed.
		ErrorGetAccountId,

		/// Value of deposited shares amount in reward currency is bellow min. limit.
		IncorrectValuedShares,

		/// `reward_currency` is not registered in asset registry.
		RewardCurrencyNotRegistered,

		/// `incentivized_asset` is not registered in asset registry.
		IncentivizedAssetNotRegistered,

		/// Action cannot be completed because unexpected error has occurred. This should be reported
		/// to protocol maintainers.
		InconsistentState(InconsistentStateError),
	}

	//NOTE: these errors should never happen.
	#[derive(Encode, Decode, Eq, PartialEq, TypeInfo, frame_support::PalletError, RuntimeDebug)]
	pub enum InconsistentStateError {
		/// Yield farm does not exist.
		YieldFarmNotFound,

		/// Global farm does not exist.
		GlobalFarmNotFound,

		/// Liquidity mining is `stopped` or `terminated`.
		LiquidityIsNotActive,

		/// Global farm is terminated.
		GlobalFarmIsNotActive,

		/// Deposit does not exist.
		DepositNotFound,

		/// Period calculation overflow.
		InvalidPeriod,

		/// Rewards allocated for yield-farm are lower then calculated rewards.
		NotEnoughRewardsInYieldFarm,

		/// Global-farm's `live_yield_farms_count` calculation overflow.
		InvalidLiveYielFarmsCount,

		/// Global-farm's `total_yield_farms_count` calculation overflow.
		InvalidTotalYieldFarmsCount,

		/// Yield-farm's entries count calculation overflow.
		InvalidYieldFarmEntriesCount,

		/// Yield-farm's `total_shares` calculation overflow.
		InvalidTotalShares,

		/// Yield-farm's `valued_shares` calculation overflow.
		InvalidValuedShares,

		/// Global-farm's `total_shares_z` calculation overflow.
		InvalidTotalSharesZ,

		/// Global-farm's `paid_accumulated_rewards` calculation overflow.
		InvalidPaidAccumulatedRewards,

		/// `FarmId` can't be 0.
		InvalidFarmId,

		/// Loyalty multiplier can't be greater than one.
		InvalidLoyaltyMultiplier,
	}

	impl<T, I> From<InconsistentStateError> for Error<T, I> {
		fn from(e: InconsistentStateError) -> Error<T, I> {
			Error::<T, I>::InconsistentState(e)
		}
	}

	/// Id sequencer for `GlobalFarm` and `YieldFarm`.
	#[pallet::storage]
	#[pallet::getter(fn last_farm_id)]
	pub type FarmSequencer<T: Config<I>, I: 'static = ()> = StorageValue<_, FarmId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn deposit_id)]
	pub type DepositSequencer<T: Config<I>, I: 'static = ()> = StorageValue<_, DepositId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn global_farm)]
	pub type GlobalFarm<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Blake2_128Concat, GlobalFarmId, GlobalFarmData<T, I>, OptionQuery>;

	/// Yield farm details.
	#[pallet::storage]
	#[pallet::getter(fn yield_farm)]
	pub type YieldFarm<T: Config<I>, I: 'static = ()> = StorageNMap<
		_,
		(
			NMapKey<Blake2_128Concat, T::AmmPoolId>,
			NMapKey<Blake2_128Concat, GlobalFarmId>,
			NMapKey<Blake2_128Concat, YieldFarmId>,
		),
		YieldFarmData<T, I>,
		OptionQuery,
	>;

	/// Deposit details.
	#[pallet::storage]
	#[pallet::getter(fn deposit)]
	pub type Deposit<T: Config<I>, I: 'static = ()> =
		StorageMap<_, Twox64Concat, DepositId, DepositData<T, I>, OptionQuery>;

	/// Active(farms able to receive LP shares deposits) yield farms.
	#[pallet::storage]
	#[pallet::getter(fn active_yield_farm)]
	pub type ActiveYieldFarm<T: Config<I>, I: 'static = ()> =
		StorageDoubleMap<_, Blake2_128Concat, T::AmmPoolId, Blake2_128Concat, GlobalFarmId, YieldFarmId>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config<I>, I: 'static = ()> {
		/// Global farm accumulated reward per share was updated.
		GlobalFarmAccRPZUpdated {
			global_farm_id: GlobalFarmId,
			accumulated_rpz: FixedU128,
			total_shares_z: Balance,
		},

		/// Yield farm accumulated reward per valued share was updated.
		YieldFarmAccRPVSUpdated {
			global_farm_id: GlobalFarmId,
			yield_farm_id: YieldFarmId,
			accumulated_rpvs: FixedU128,
			total_valued_shares: Balance,
		},

		/// Global farm has no more rewards to distribute in the moment.
		AllRewardsDistributed { global_farm_id: GlobalFarmId },
	}

	#[pallet::call]
	impl<T: Config<I>, I: 'static> Pallet<T, I> {}
}

impl<T: Config<I>, I: 'static> Pallet<T, I> {
	/// Create a new liquidity mining program with provided parameters.
	///
	/// `owner` account has to have at least `total_rewards` balance. These funds will be
	/// transferred from `owner` to farm account.
	///
	/// Returns: `(GlobalFarmId, max reward per period)`
	///
	/// Parameters:
	/// - `total_rewards`: total rewards planned to distribute. These rewards will be
	/// distributed between all yield farms in the global farm.
	/// - `planned_yielding_periods`: planned number of periods to distribute `total_rewards`.
	/// WARN: THIS IS NOT HARD DEADLINE. Not all rewards have to be distributed in
	/// `planned_yielding_periods`. Rewards are distributed based on the situation in the yield
	/// farm and can be distributed in a longer, though never in a shorter, time frame.
	/// - `blocks_per_period`:  number of blocks in a single period. Min. number of blocks per
	/// period is 1.
	/// - `incentivized_asset`: asset to be incentivized in AMM pools. All yield farms added into
	/// global farm must to have `incentivized_asset` in their pair.
	/// - `reward_currency`: payoff currency of rewards.
	/// - `owner`: liq. mining farm owner.
	/// - `yield_per_period`: percentage return on `reward_currency` of all pools.
	/// - `min_deposit`: minimum amount of LP shares to be deposited into liquidity mining by each user.
	/// - `price_adjustment`: price adjustment between `incentivized_asset` and `reward_currency`.
	/// This value should be `1` if `incentivized_asset` and `reward_currency` are the same.
	#[allow(clippy::too_many_arguments)]
	#[require_transactional]
	fn create_global_farm(
		total_rewards: Balance,
		planned_yielding_periods: PeriodOf<T>,
		blocks_per_period: BlockNumberFor<T>,
		incentivized_asset: T::AssetId,
		reward_currency: T::AssetId,
		owner: T::AccountId,
		yield_per_period: Perquintill,
		min_deposit: Balance,
		price_adjustment: FixedU128,
	) -> Result<(GlobalFarmId, Balance), DispatchError> {
		Self::validate_create_global_farm_data(
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			yield_per_period,
			min_deposit,
			price_adjustment,
		)?;

		ensure!(
			T::AssetRegistry::exists(reward_currency),
			Error::<T, I>::RewardCurrencyNotRegistered
		);
		ensure!(
			T::AssetRegistry::exists(incentivized_asset),
			Error::<T, I>::IncentivizedAssetNotRegistered
		);

		T::MultiCurrency::ensure_can_withdraw(reward_currency, &owner, total_rewards)
			.map_err(|_| Error::<T, I>::InsufficientRewardCurrencyBalance)?;

		let planned_periods =
			TryInto::<u128>::try_into(planned_yielding_periods).map_err(|_| ArithmeticError::Overflow)?;
		let max_reward_per_period = total_rewards
			.checked_div(planned_periods)
			.ok_or(ArithmeticError::DivisionByZero)?;
		let current_period = Self::get_current_period(blocks_per_period)?;
		let farm_id = Self::get_next_farm_id()?;

		let global_farm = GlobalFarmData::new(
			farm_id,
			current_period,
			reward_currency,
			yield_per_period,
			planned_yielding_periods,
			blocks_per_period,
			owner,
			incentivized_asset,
			max_reward_per_period,
			min_deposit,
			price_adjustment,
		);

		<GlobalFarm<T, I>>::insert(global_farm.id, &global_farm);

		let global_farm_account = Self::farm_account_id(global_farm.id)?;

		T::NonDustableWhitelistHandler::add_account(&global_farm_account)?;
		T::MultiCurrency::transfer(reward_currency, &global_farm.owner, &global_farm_account, total_rewards)?;

		Ok((farm_id, max_reward_per_period))
	}

	/// Update global farm's price adjustment.
	///  
	/// Only farm's owner can perform this action.
	///
	/// Parameters:
	/// - `who`: farm's owner
	/// - `global_farm_id`: global farm id.
	/// - `price_adjustment`: new price adjustment value.
	fn update_global_farm_price_adjustment(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		price_adjustment: FixedU128,
	) -> Result<(), DispatchError> {
		ensure!(!price_adjustment.is_zero(), Error::<T, I>::InvalidPriceAdjustment);

		<GlobalFarm<T, I>>::try_mutate(global_farm_id, |maybe_global_farm| {
			let global_farm = maybe_global_farm.as_mut().ok_or(Error::<T, I>::GlobalFarmNotFound)?;

			ensure!(global_farm.state.is_active(), Error::<T, I>::GlobalFarmNotFound);

			ensure!(who == global_farm.owner, Error::<T, I>::Forbidden);

			let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
			Self::sync_global_farm(global_farm, current_period)?;

			global_farm.price_adjustment = price_adjustment;

			Ok(())
		})
	}

	/// Terminate existing liquidity mining program. Undistributed rewards are transferred to
	/// owner(`who`).
	///
	/// Only farm's owner can perform this action.
	///
	/// WARN: To successfully terminate a global farm, farm have to be empty(all yield farms in the
	/// global farm must be terminated)
	///
	/// Returns: `(reward currency, undistributed rewards, destination account)`
	///
	/// Parameters:
	/// - `who`: farm's owner.
	/// - `farm_id`: id of farm to be terminated.
	#[require_transactional]
	fn terminate_global_farm(
		who: T::AccountId,
		farm_id: GlobalFarmId,
	) -> Result<(T::AssetId, Balance, T::AccountId), DispatchError> {
		<GlobalFarm<T, I>>::try_mutate_exists(farm_id, |maybe_global_farm| {
			let global_farm = maybe_global_farm.as_mut().ok_or(Error::<T, I>::GlobalFarmNotFound)?;

			ensure!(who == global_farm.owner, Error::<T, I>::Forbidden);

			ensure!(global_farm.state.is_active(), Error::<T, I>::GlobalFarmNotFound);

			ensure!(!global_farm.has_live_farms(), Error::<T, I>::GlobalFarmIsNotEmpty);

			let global_farm_account = Self::farm_account_id(global_farm.id)?;
			let undistributed_rewards =
				T::MultiCurrency::free_balance(global_farm.reward_currency, &global_farm_account);

			T::MultiCurrency::transfer(
				global_farm.reward_currency,
				&global_farm_account,
				&who,
				undistributed_rewards,
			)?;

			//Mark for removal from storage on last `YieldFarm` in the farm removed.
			global_farm.state = FarmState::Terminated;

			//NOTE: Nothing can be send to this account because `YieldFarm`'s has to be terminated
			//first so it can be dusted.
			T::NonDustableWhitelistHandler::remove_account(&global_farm_account)?;

			let reward_currency = global_farm.reward_currency;
			if global_farm.can_be_removed() {
				*maybe_global_farm = None;
			}

			Ok((reward_currency, undistributed_rewards, who))
		})
	}

	/// Add yield farm to global farm and start liquidity mining for given assets pair.
	///  
	/// Only farm owner can perform this action.
	///
	/// One of the AMM assets has to be `incentivized_token`. Same AMM can be
	/// in the same farm only once.
	///
	/// Returns: `(YieldFarmId)`
	///
	/// Parameters:
	/// - `who`: farm's owner
	/// - `global_farm_id`: farm id to which a yield farm will be added.
	/// - `multiplier`: yield farm multiplier.
	/// - `loyalty_curve`: curve to calculate loyalty multiplier to distribute rewards to users
	/// with time incentive. `None` means no loyalty multiplier.
	/// - `amm_pool_id`: identifier of the AMM pool.
	/// - `assets`: list of assets in the AMM pool. One of this assets must be incentivized asset
	#[require_transactional]
	fn create_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		multiplier: FarmMultiplier,
		loyalty_curve: Option<LoyaltyCurve>,
		amm_pool_id: T::AmmPoolId,
		assets: Vec<T::AssetId>,
	) -> Result<YieldFarmId, DispatchError> {
		ensure!(
			multiplier >= MIN_YIELD_FARM_MULTIPLIER,
			Error::<T, I>::InvalidMultiplier
		);

		if let Some(ref curve) = loyalty_curve {
			ensure!(
				curve.initial_reward_percentage.lt(&FixedU128::one()),
				Error::<T, I>::InvalidInitialRewardPercentage
			);
		}

		<GlobalFarm<T, I>>::try_mutate(
			global_farm_id,
			|maybe_global_farm| -> Result<YieldFarmId, DispatchError> {
				let global_farm = maybe_global_farm.as_mut().ok_or(Error::<T, I>::GlobalFarmNotFound)?;

				//This is basically same as farm not found.
				ensure!(global_farm.state.is_active(), Error::<T, I>::GlobalFarmNotFound);

				ensure!(who == global_farm.owner, Error::<T, I>::Forbidden);

				ensure!(!global_farm.is_full(), Error::<T, I>::GlobalFarmIsFull);

				ensure!(
					assets.contains(&global_farm.incentivized_asset),
					Error::<T, I>::MissingIncentivizedAsset
				);

				<ActiveYieldFarm<T, I>>::try_mutate(amm_pool_id.clone(), global_farm_id, |maybe_active_yield_farm| {
					ensure!(maybe_active_yield_farm.is_none(), Error::<T, I>::YieldFarmAlreadyExists);

					let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
					Self::sync_global_farm(global_farm, current_period)?;

					let yield_farm_id = Self::get_next_farm_id()?;

					let yield_farm =
						YieldFarmData::new(yield_farm_id, current_period, loyalty_curve.clone(), multiplier);

					<YieldFarm<T, I>>::insert((amm_pool_id, global_farm_id, yield_farm_id), yield_farm);
					global_farm.increase_yield_farm_counts()?;

					*maybe_active_yield_farm = Some(yield_farm_id);

					Ok(yield_farm_id)
				})
			},
		)
	}

	/// Update yield farm's multiplier.
	///  
	/// Only farm's owner can perform this action.
	///
	/// Returns: `(YieldFarmId)`
	///
	/// Parameters:
	/// - `who`: farm's owner
	/// - `global_farm_id`: global farm id in which yield farm will be updated.
	/// - `multiplier`: new yield farm multiplier.
	/// - `amm_pool_id`: identifier of the AMM pool.
	#[require_transactional]
	fn update_yield_farm_multiplier(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: T::AmmPoolId,
		multiplier: FarmMultiplier,
	) -> Result<YieldFarmId, DispatchError> {
		ensure!(
			multiplier >= MIN_YIELD_FARM_MULTIPLIER,
			Error::<T, I>::InvalidMultiplier
		);

		let yield_farm_id =
			Self::active_yield_farm(amm_pool_id.clone(), global_farm_id).ok_or(Error::<T, I>::YieldFarmNotFound)?;

		<YieldFarm<T, I>>::try_mutate((amm_pool_id, global_farm_id, yield_farm_id), |maybe_yield_farm| {
			//NOTE: yield-farm must exist if it's in the active_yield_farm storage.
			let yield_farm = maybe_yield_farm
				.as_mut()
				.defensive_ok_or::<Error<T, I>>(InconsistentStateError::YieldFarmNotFound.into())?;

			ensure!(yield_farm.state.is_active(), Error::<T, I>::LiquidityMiningCanceled);

			<GlobalFarm<T, I>>::try_mutate(global_farm_id, |maybe_global_farm| {
				//NOTE: global-farm must exist if yield-farm exists.
				let global_farm = maybe_global_farm
					.as_mut()
					.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

				ensure!(who == global_farm.owner, Error::<T, I>::Forbidden);

				let old_stake_in_global_farm =
					math::calculate_global_farm_shares(yield_farm.total_valued_shares, yield_farm.multiplier)
						.map_err(|_| ArithmeticError::Overflow)?;

				let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
				Self::sync_global_farm(global_farm, current_period)?;
				Self::sync_yield_farm(yield_farm, global_farm, current_period)?;

				let new_stake_in_global_farm =
					math::calculate_global_farm_shares(yield_farm.total_valued_shares, multiplier)
						.map_err(|_| ArithmeticError::Overflow)?;

				global_farm.remove_stake(old_stake_in_global_farm)?;
				global_farm.add_stake(new_stake_in_global_farm)?;

				yield_farm.multiplier = multiplier;

				Ok(yield_farm.id)
			})
		})
	}

	/// Stop liquidity mining for specific yield farm.
	///
	/// This function claims rewards from `GlobalFarm` for the last time and stops yield farm
	/// incentivization from a `GlobalFarm`. Users will be able to only claim and withdraw LP
	/// shares after calling this function.
	/// `deposit_lp_shares()` is not allowed on stopped yield farm.
	///
	/// Returns: `(YieldFarmId)`
	///  
	/// Only farm owner can perform this action.
	///
	/// Parameters:
	/// - `who`: farm's owner.
	/// - `global_farm_id`: farm id in which yield farm will be stopped.
	/// - `amm_pool_id`: identifier of the AMM pool.
	#[require_transactional]
	fn stop_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: T::AmmPoolId,
	) -> Result<YieldFarmId, DispatchError> {
		<ActiveYieldFarm<T, I>>::try_mutate_exists(
			amm_pool_id.clone(),
			global_farm_id,
			|maybe_active_yield_farm_id| -> Result<YieldFarmId, DispatchError> {
				let yield_farm_id = maybe_active_yield_farm_id
					.as_ref()
					.ok_or(Error::<T, I>::YieldFarmNotFound)?;

				<YieldFarm<T, I>>::try_mutate(
					(amm_pool_id, global_farm_id, yield_farm_id),
					|maybe_yield_farm| -> Result<(), DispatchError> {
						//NOTE: yield-farm must exist if it's in the active_yield_farm storage.
						let yield_farm = maybe_yield_farm
							.as_mut()
							.defensive_ok_or::<Error<T, I>>(InconsistentStateError::YieldFarmNotFound.into())?;

						//NOTE: inactive yield-farm can't be in the active_yield_farm storage.
						ensure!(
							yield_farm.state.is_active(),
							Self::defensive_err(Error::<T, I>::InconsistentState(
								InconsistentStateError::LiquidityIsNotActive
							))
						);

						<GlobalFarm<T, I>>::try_mutate(global_farm_id, |maybe_global_farm| {
							//NOTE: global-farm must exist when yield-farm exists.
							let global_farm = maybe_global_farm
								.as_mut()
								.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

							ensure!(global_farm.owner == who, Error::<T, I>::Forbidden);

							let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
							Self::sync_global_farm(global_farm, current_period)?;
							Self::sync_yield_farm(yield_farm, global_farm, current_period)?;

							let old_stake_in_global_farm = math::calculate_global_farm_shares(
								yield_farm.total_valued_shares,
								yield_farm.multiplier,
							)
							.map_err(|_| ArithmeticError::Overflow)?;

							global_farm.remove_stake(old_stake_in_global_farm)?;

							yield_farm.state = FarmState::Stopped;
							yield_farm.multiplier = FarmMultiplier::default();

							Ok(())
						})
					},
				)?;

				let yield_farm_id = *yield_farm_id;
				//Remove yield farm from active farms storage.
				*maybe_active_yield_farm_id = None;

				Ok(yield_farm_id)
			},
		)
	}

	/// Resume liquidity mining for stopped yield farm.
	///
	/// This function resume incentivization from `GlobalPool` and restore full functionality
	/// for yield farm. Users will be able to deposit, claim and withdraw again.
	///
	/// Yield farm is not rewarded for the time it was stopped.
	///
	/// Only farm's owner can perform this action.
	///
	/// Parameters:
	/// - `who`: farm's owner
	/// - `global_farm_id`: farm id in which yield farm will be resumed.
	/// - `yield_farm_id`: id of yield farm to resume.
	/// - `amm_pool_id`: identifier of the AMM pool.
	/// - `multiplier`: yield farm's multiplier.
	#[require_transactional]
	fn resume_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: T::AmmPoolId,
		multiplier: FarmMultiplier,
	) -> Result<(), DispatchError> {
		ensure!(
			multiplier >= MIN_YIELD_FARM_MULTIPLIER,
			Error::<T, I>::InvalidMultiplier
		);

		<ActiveYieldFarm<T, I>>::try_mutate(amm_pool_id.clone(), global_farm_id, |maybe_active_yield_farm_id| {
			ensure!(
				maybe_active_yield_farm_id.is_none(),
				Error::<T, I>::YieldFarmAlreadyExists
			);

			<YieldFarm<T, I>>::try_mutate((amm_pool_id, global_farm_id, yield_farm_id), |maybe_yield_farm| {
				let yield_farm = maybe_yield_farm.as_mut().ok_or(Error::<T, I>::YieldFarmNotFound)?;

				//Active or terminated yield farms can't be resumed.
				ensure!(
					yield_farm.state.is_stopped(),
					Error::<T, I>::LiquidityMiningIsNotStopped
				);

				<GlobalFarm<T, I>>::try_mutate(global_farm_id, |maybe_global_farm| {
					//NOTE: global-farm must exist if yield-farm exists.
					let global_farm = maybe_global_farm
						.as_mut()
						.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

					ensure!(global_farm.owner == who, Error::<T, I>::Forbidden);

					let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
					Self::sync_global_farm(global_farm, current_period)?;

					//NOTE: this should never fail.
					let stopped_periods = current_period
						.checked_sub(&yield_farm.updated_at)
						.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())?;

					let new_stake_in_global_farm =
						math::calculate_global_farm_shares(yield_farm.total_valued_shares, multiplier)
							.map_err(|_| ArithmeticError::Overflow)?;

					global_farm.add_stake(new_stake_in_global_farm)?;

					yield_farm.accumulated_rpz = global_farm.accumulated_rpz;
					yield_farm.updated_at = current_period;
					yield_farm.state = FarmState::Active;
					yield_farm.multiplier = multiplier;
					yield_farm.total_stopped = yield_farm
						.total_stopped
						.checked_add(&stopped_periods)
						.ok_or(ArithmeticError::Overflow)?;

					//add yield farm to active farms.
					*maybe_active_yield_farm_id = Some(yield_farm.id);

					Ok(())
				})
			})
		})
	}

	/// This function marks an yield farm ready for removal from storage when it's empty. Users will
	/// be able to only withdraw shares(without claiming rewards from yield farm). Unpaid rewards
	/// will be transferred back to global farm and will be used to distribute to other yield farms.
	///
	/// Yield farm must be stopped before calling this function.
	///
	/// Only farm's owner can perform this action. Yield farm stays in the storage until it's
	/// empty(all farm entries are withdrawn). Last withdrawn from yield farm trigger removing from
	/// the storage.
	///
	/// Parameters:
	/// - `who`: farm's owner.
	/// - `global_farm_id`: farm id from which yield farm will be removed.
	/// - `yield_farm_id`: yield farm id of farm to terminate.
	/// - `amm_pool_id`: identifier of the AMM pool.
	#[require_transactional]
	fn terminate_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: T::AmmPoolId,
	) -> Result<(), DispatchError> {
		ensure!(
			<ActiveYieldFarm<T, I>>::get(amm_pool_id.clone(), global_farm_id) != Some(yield_farm_id),
			Error::<T, I>::LiquidityMiningIsActive
		);

		<GlobalFarm<T, I>>::try_mutate_exists(global_farm_id, |maybe_global_farm| {
			let global_farm = maybe_global_farm.as_mut().ok_or(Error::<T, I>::GlobalFarmNotFound)?;

			ensure!(global_farm.owner == who, Error::<T, I>::Forbidden);

			<YieldFarm<T, I>>::try_mutate_exists(
				(amm_pool_id, global_farm_id, yield_farm_id),
				|maybe_yield_farm| -> Result<(), DispatchError> {
					let yield_farm = maybe_yield_farm.as_mut().ok_or(Error::<T, I>::YieldFarmNotFound)?;

					//Only stopped farms can be resumed.
					ensure!(
						yield_farm.state.is_stopped(),
						Error::<T, I>::LiquidityMiningIsNotStopped
					);

					//Transfer yield-farm's unpaid rewards back to global farm.
					let global_farm_account = Self::farm_account_id(global_farm.id)?;
					let pot = Self::pot_account_id().ok_or(Error::<T, I>::ErrorGetAccountId)?;

					global_farm.accumulated_paid_rewards = global_farm
						.accumulated_paid_rewards
						.checked_sub(yield_farm.left_to_distribute)
						.ok_or(ArithmeticError::Overflow)?;

					T::MultiCurrency::transfer(
						global_farm.reward_currency,
						&pot,
						&global_farm_account,
						yield_farm.left_to_distribute,
					)?;

					yield_farm.left_to_distribute = Zero::zero();
					//Delete yield farm.
					yield_farm.state = FarmState::Terminated;
					global_farm.decrease_live_yield_farm_count()?;

					//Cleanup if it's possible
					if yield_farm.can_be_removed() {
						global_farm.decrease_total_yield_farm_count()?;

						*maybe_yield_farm = None;
					}

					Ok(())
				},
			)?;

			Ok(())
		})
	}

	/// Deposit LP shares to a yield farm.
	///
	/// This function creates new deposit farm entry in the yield farm.
	///
	/// Returns: `(DepositId)`
	///
	/// Parameters:
	/// - `global_farm_id`: global farm identifier.
	/// - `yield_farm_id`: yield farm identifier depositing to.
	/// - `amm_pool_id`: identifier of the AMM pool.
	/// - `shares_amount`: amount of LP shares user want to deposit.
	/// - `get_token_value_of_lp_shares`: callback function returning amount of
	/// `incentivized_asset` behind `lp_shares`.
	#[require_transactional]
	fn deposit_lp_shares(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: T::AmmPoolId,
		shares_amount: Balance,
		get_token_value_of_lp_shares: impl Fn(T::AssetId, T::AmmPoolId, Balance) -> Result<Balance, DispatchError>,
	) -> Result<DepositId, DispatchError> {
		let mut deposit = DepositData::new(shares_amount, amm_pool_id);

		Self::do_deposit_lp_shares(
			&mut deposit,
			global_farm_id,
			yield_farm_id,
			get_token_value_of_lp_shares,
		)?;

		//Save deposit to storage.
		let deposit_id = Self::get_next_deposit_id()?;
		<Deposit<T, I>>::insert(deposit_id, deposit);

		Ok(deposit_id)
	}

	/// This function create yield farm entry for existing deposit. LP shares are not transferred
	/// and amount of LP shares is based on existing deposit.
	///
	/// This function DOESN'T create new deposit.
	///
	/// Returns: `(redeposited shares amount, amm pool id)`
	///
	/// Parameters:
	/// - `global_farm_id`: global farm identifier.
	/// - `yield_farm_id`: yield farm identifier redepositing to.
	/// - `deposit_id`: identifier of the AMM pool.
	/// - `get_token_value_of_lp_shares`: callback function returning amount of
	/// `incentivized_asset` behind `lp_shares`.
	fn redeposit_lp_shares(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		deposit_id: DepositId,
		get_token_value_of_lp_shares: impl Fn(T::AssetId, T::AmmPoolId, Balance) -> Result<Balance, DispatchError>,
	) -> Result<(Balance, T::AmmPoolId), DispatchError> {
		<Deposit<T, I>>::try_mutate(deposit_id, |maybe_deposit| {
			//NOTE: At this point deposit existence and owner must be checked by pallet calling this
			//function so this should never happen.
			let deposit = maybe_deposit
				.as_mut()
				.defensive_ok_or::<Error<T, I>>(InconsistentStateError::DepositNotFound.into())?;

			Self::do_deposit_lp_shares(deposit, global_farm_id, yield_farm_id, get_token_value_of_lp_shares)?;

			Ok((deposit.shares, deposit.amm_pool_id.clone()))
		})
	}

	/// Claim rewards from yield farm for given deposit.
	///
	/// This function calculate user rewards from yield farm and transfer rewards to `who`
	/// account. Claiming in the same period is configured by `check_double_claim` parameter.
	/// Second claim in the same period result in `0` claims. This is desirable for in case we need
	/// `unclaimable_rewards` e.g. for `withdraw_lp_shares()`
	///
	/// WARN: User have to use `withdraw_shares()` if yield farm is terminated.
	///
	/// Returns: `(GlobalFarmId, reward currency, claimed amount, unclaimable amount)`
	///
	/// Parameters:
	/// - `who`: destination account to receive rewards.
	/// - `deposit_id`: id representing deposit in the yield farm.
	/// - `yield_farm_id`: identifier of yield farm to withdrawn from.
	/// - `check_double_claim`: fn failed on second claim in the same period if set to `true`.
	#[require_transactional]
	fn claim_rewards(
		who: T::AccountId,
		deposit_id: DepositId,
		yield_farm_id: YieldFarmId,
		fail_on_doubleclaim: bool,
	) -> Result<(GlobalFarmId, T::AssetId, Balance, Balance), DispatchError> {
		<Deposit<T, I>>::try_mutate(deposit_id, |maybe_deposit| {
			//NOTE: At this point deposit existence and owner must be checked by pallet calling this
			//function so this should never happen.
			let deposit = maybe_deposit
				.as_mut()
				.defensive_ok_or::<Error<T, I>>(InconsistentStateError::DepositNotFound.into())?;

			let amm_pool_id = deposit.amm_pool_id.clone();
			let farm_entry = deposit
				.get_yield_farm_entry(yield_farm_id)
				.ok_or(Error::<T, I>::YieldFarmEntryNotFound)?;

			<YieldFarm<T, I>>::try_mutate(
				(amm_pool_id, farm_entry.global_farm_id, yield_farm_id),
				|maybe_yield_farm| {
					//NOTE: yield-farm must exist if yield-farm-entry exists.
					let yield_farm = maybe_yield_farm
						.as_mut()
						.defensive_ok_or::<Error<T, I>>(InconsistentStateError::YieldFarmNotFound.into())?;

					ensure!(
						!yield_farm.state.is_terminated(),
						Error::<T, I>::LiquidityMiningCanceled
					);

					<GlobalFarm<T, I>>::try_mutate(farm_entry.global_farm_id, |maybe_global_farm| {
						//NOTE: global-farm must exist if yield-farm exists.
						let global_farm = maybe_global_farm
							.as_mut()
							.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

						let current_period = Self::get_current_period(global_farm.blocks_per_period)?;
						//Double claim should be allowed in some case e.g withdraw_lp_shares need
						//`unclaimable_rewards` returned by this function.
						if fail_on_doubleclaim {
							ensure!(
								farm_entry.updated_at != current_period,
								Error::<T, I>::DoubleClaimInPeriod
							);
						}

						Self::sync_global_farm(global_farm, current_period)?;
						Self::sync_yield_farm(yield_farm, global_farm, current_period)?;

						//NOTE: this should never fail yield-farm's stopped must be >= entry's
						//stopped
						let delta_stopped =
							yield_farm
								.total_stopped
								.checked_sub(&farm_entry.stopped_at_creation)
								.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())?;

						//NOTE: yield-farm's `updated_at` is updated to current period if it's
						//possible so this should be ok.
						let periods = yield_farm
							.updated_at
							.checked_sub(&farm_entry.entered_at)
							.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())?
							.checked_sub(&delta_stopped)
							.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())?;

						let loyalty_multiplier =
							Self::get_loyalty_multiplier(periods, yield_farm.loyalty_curve.clone())?;

						let (rewards, unclaimable_rewards) = math::calculate_user_reward(
							farm_entry.accumulated_rpvs,
							farm_entry.valued_shares,
							farm_entry.accumulated_claimed_rewards,
							yield_farm.accumulated_rpvs,
							loyalty_multiplier,
						)
						.map_err(|_| ArithmeticError::Overflow)?;

						if !rewards.is_zero() {
							yield_farm.left_to_distribute = yield_farm
								.left_to_distribute
								.checked_sub(rewards)
								.defensive_ok_or::<Error<T, I>>(
									InconsistentStateError::NotEnoughRewardsInYieldFarm.into(),
								)?;

							farm_entry.accumulated_claimed_rewards = farm_entry
								.accumulated_claimed_rewards
								.checked_add(rewards)
								.ok_or(ArithmeticError::Overflow)?;

							farm_entry.updated_at = current_period;

							let pot = Self::pot_account_id().ok_or(Error::<T, I>::ErrorGetAccountId)?;
							T::MultiCurrency::transfer(global_farm.reward_currency, &pot, &who, rewards)?;
						}

						Ok((
							global_farm.id,
							global_farm.reward_currency,
							rewards,
							unclaimable_rewards,
						))
					})
				},
			)
		})
	}

	/// Withdraw LP shares from yield farm. This function can be used to free slot for yield
	/// farm entry in the deposit or to destroy deposit and return LP shares if deposit has no more
	/// farm entries.
	///
	/// WARNING: This function doesn't automatically claim rewards for user. Caller of this
	/// function must call `claim_rewards()` first if claiming is desirable.
	///
	/// !!!LP shares are transferred back to user only when deposit is destroyed.
	///
	/// This function transfer user's unclaimable rewards back to global farm.
	///
	/// Returns: `(GlobalFarmId, withdrawn amount, true if deposit was destroyed)`
	///
	/// Parameters:
	/// - `deposit_id`: id representing deposit in the yield farm.
	/// - `yield_farm_id`: identifier yield farm to withdrawn from.
	/// - `unclaimable_rewards`: amount of rewards user will not be able to claim because of early
	/// exit from liquidity mining program.
	#[require_transactional]
	fn withdraw_lp_shares(
		deposit_id: DepositId,
		yield_farm_id: YieldFarmId,
		unclaimable_rewards: Balance,
	) -> Result<(GlobalFarmId, Balance, bool), DispatchError> {
		<Deposit<T, I>>::try_mutate_exists(deposit_id, |maybe_deposit| {
			//NOTE: At this point deposit existence and owner must be checked by pallet calling this
			//function so this should never fail.
			let deposit = maybe_deposit
				.as_mut()
				.defensive_ok_or::<Error<T, I>>(InconsistentStateError::DepositNotFound.into())?;

			let farm_entry = deposit.remove_yield_farm_entry(yield_farm_id)?;
			let amm_pool_id = deposit.amm_pool_id.clone();

			<GlobalFarm<T, I>>::try_mutate_exists(
				farm_entry.global_farm_id,
				|maybe_global_farm| -> Result<(), DispatchError> {
					//NOTE: global-farm must exist if yield-farm-entry exists.
					let global_farm = maybe_global_farm
						.as_mut()
						.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

					<YieldFarm<T, I>>::try_mutate_exists(
						(&amm_pool_id, farm_entry.global_farm_id, yield_farm_id),
						|maybe_yield_farm| -> Result<(), DispatchError> {
							//NOTE: yield-farm must exist if yield-farm-entry exists.
							let yield_farm = maybe_yield_farm
								.as_mut()
								.defensive_ok_or::<Error<T, I>>(InconsistentStateError::YieldFarmNotFound.into())?;

							yield_farm.total_shares = yield_farm
								.total_shares
								.checked_sub(deposit.shares)
								.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidTotalShares.into())?;

							yield_farm.total_valued_shares = yield_farm
								.total_valued_shares
								.checked_sub(farm_entry.valued_shares)
								.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidValuedShares.into())?;

							// yield farm's stake in global farm is set to `0` when farm is
							// stopped and yield farm have to be stopped before it's terminated so
							// this update is only required for active farms.
							if yield_farm.state.is_active() {
								let deposit_stake_in_global_farm =
									math::calculate_global_farm_shares(farm_entry.valued_shares, yield_farm.multiplier)
										.map_err(|_| ArithmeticError::Overflow)?;

								global_farm.remove_stake(deposit_stake_in_global_farm)?;
							}

							//NOTE: this should never happen. It's the responsibility of a pallet
							//which is using this function to provide `unclaimable_rewards == 0`
							//if yield-farm is not claimable.
							ensure!(
								unclaimable_rewards.is_zero() || !yield_farm.state.is_terminated(),
								Self::defensive_err(Error::<T, I>::InconsistentState(
									InconsistentStateError::NotEnoughRewardsInYieldFarm,
								))
							);
							if !unclaimable_rewards.is_zero() {
								yield_farm.left_to_distribute = yield_farm
									.left_to_distribute
									.checked_sub(unclaimable_rewards)
									.defensive_ok_or::<Error<T, I>>(
										InconsistentStateError::NotEnoughRewardsInYieldFarm.into(),
									)?;

								global_farm.accumulated_paid_rewards = global_farm
									.accumulated_paid_rewards
									.checked_sub(unclaimable_rewards)
									.defensive_ok_or::<Error<T, I>>(
										InconsistentStateError::InvalidPaidAccumulatedRewards.into(),
									)?;

								let global_farm_account = Self::farm_account_id(global_farm.id)?;
								let pot = Self::pot_account_id().ok_or(Error::<T, I>::ErrorGetAccountId)?;

								T::MultiCurrency::transfer(
									global_farm.reward_currency,
									&pot,
									&global_farm_account,
									unclaimable_rewards,
								)?;
							}

							yield_farm.decrease_entries_count()?;
							if yield_farm.can_be_removed() {
								global_farm.decrease_total_yield_farm_count()?;

								*maybe_yield_farm = None;
							}

							Ok(())
						},
					)?;

					if global_farm.can_be_removed() {
						*maybe_global_farm = None;
					}

					Ok(())
				},
			)?;

			let withdrawn_amount = deposit.shares;
			let mut deposit_destroyed = false;
			if deposit.can_be_removed() {
				*maybe_deposit = None;

				deposit_destroyed = true;
			}

			Ok((farm_entry.global_farm_id, withdrawn_amount, deposit_destroyed))
		})
	}

	/// Helper function to create yield farm entry.
	#[require_transactional]
	fn do_deposit_lp_shares(
		deposit: &mut DepositData<T, I>,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		get_token_value_of_lp_shares: impl Fn(T::AssetId, T::AmmPoolId, Balance) -> Result<Balance, DispatchError>,
	) -> Result<(), DispatchError> {
		//LP shares can be locked only once in the same yield farm.
		ensure!(
			deposit.search_yield_farm_entry(yield_farm_id).is_none(),
			Error::<T, I>::DoubleLock
		);

		<YieldFarm<T, I>>::try_mutate(
			(deposit.amm_pool_id.clone(), global_farm_id, yield_farm_id),
			|maybe_yield_farm| {
				let yield_farm = maybe_yield_farm.as_mut().ok_or(Error::<T, I>::YieldFarmNotFound)?;

				ensure!(yield_farm.state.is_active(), Error::<T, I>::LiquidityMiningCanceled);

				<GlobalFarm<T, I>>::try_mutate(global_farm_id, |maybe_global_farm| {
					//NOTE: global-farm must exists if yield-farm exists.
					let global_farm = maybe_global_farm
						.as_mut()
						.defensive_ok_or::<Error<T, I>>(InconsistentStateError::GlobalFarmNotFound.into())?;

					ensure!(
						deposit.shares.ge(&global_farm.min_deposit),
						Error::<T, I>::InvalidDepositAmount,
					);

					//NOTE: If yield-farm is active also global-farm MUST be active.
					ensure!(
						global_farm.state.is_active(),
						Self::defensive_err(Error::<T, I>::InconsistentState(
							InconsistentStateError::GlobalFarmNotFound
						))
					);

					let current_period = Self::get_current_period(global_farm.blocks_per_period)?;

					Self::sync_global_farm(global_farm, current_period)?;
					Self::sync_yield_farm(yield_farm, global_farm, current_period)?;

					let valued_shares = get_token_value_of_lp_shares(
						global_farm.incentivized_asset,
						deposit.amm_pool_id.clone(),
						deposit.shares,
					)?;

					ensure!(
						valued_shares >= global_farm.min_deposit,
						Error::<T, I>::IncorrectValuedShares
					);

					let deposit_stake_in_global_farm =
						math::calculate_global_farm_shares(valued_shares, yield_farm.multiplier)
							.map_err(|_| ArithmeticError::Overflow)?;

					yield_farm.total_shares = yield_farm
						.total_shares
						.checked_add(deposit.shares)
						.ok_or(ArithmeticError::Overflow)?;

					yield_farm.total_valued_shares = yield_farm
						.total_valued_shares
						.checked_add(valued_shares)
						.ok_or(ArithmeticError::Overflow)?;

					global_farm.add_stake(deposit_stake_in_global_farm)?;

					let farm_entry = YieldFarmEntry::new(
						global_farm.id,
						yield_farm.id,
						valued_shares,
						yield_farm.accumulated_rpvs,
						current_period,
						yield_farm.total_stopped,
					);

					deposit.add_yield_farm_entry(farm_entry)?;

					//Increment farm's entries count
					yield_farm.increase_entries_count()?;

					Ok(())
				})
			},
		)
	}

	/// This function returns new unused `FarmId` usable for yield global farm or error.
	fn get_next_farm_id() -> Result<FarmId, ArithmeticError> {
		FarmSequencer::<T, I>::try_mutate(|current_id| {
			*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			Ok(*current_id)
		})
	}

	/// This function returns new unused `DepositId` or error.
	fn get_next_deposit_id() -> Result<DepositId, ArithmeticError> {
		DepositSequencer::<T, I>::try_mutate(|current_id| {
			*current_id = current_id.checked_add(1).ok_or(ArithmeticError::Overflow)?;

			Ok(*current_id)
		})
	}

	/// Account id holding rewards allocated from all global farms for all yield farms.
	pub fn pot_account_id() -> Option<T::AccountId> {
		T::PalletId::get().try_into_account()
	}

	/// This function returns account from `FarmId` or error.
	///
	/// WARN: farm_id = 0 is same as `T::PalletId::get().into_account()`. 0 is not valid value.
	pub fn farm_account_id(farm_id: FarmId) -> Result<T::AccountId, Error<T, I>> {
		Self::validate_farm_id(farm_id)?;

		match T::PalletId::get().try_into_sub_account(farm_id) {
			Some(account) => Ok(account),
			None => Err(Error::<T, I>::ErrorGetAccountId),
		}
	}

	/// This function returns current period number or error.
	fn get_current_period(blocks_per_period: BlockNumberFor<T>) -> Result<PeriodOf<T>, Error<T, I>> {
		Self::get_period_number(T::BlockNumberProvider::current_block_number(), blocks_per_period)
	}

	/// This function returns period number from block number(`block`) and `blocks_per_period` or error.
	fn get_period_number(
		block: BlockNumberFor<T>,
		blocks_per_period: BlockNumberFor<T>,
	) -> Result<PeriodOf<T>, Error<T, I>> {
		block
			.checked_div(&blocks_per_period)
			.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())
	}

	/// This function returns loyalty multiplier or error.
	fn get_loyalty_multiplier(periods: PeriodOf<T>, curve: Option<LoyaltyCurve>) -> Result<FixedU128, DispatchError> {
		let curve = match curve {
			Some(v) => v,
			None => return Ok(FixedU128::one()), //no loyalty curve mean no loyalty multiplier
		};

		let m = math::calculate_loyalty_multiplier(periods, curve.initial_reward_percentage, curve.scale_coef)
			.map_err(|_| ArithmeticError::Overflow)?;

		ensure!(
			m.le(&FixedU128::one()),
			Self::defensive_err(Error::<T, I>::InconsistentState(
				InconsistentStateError::InvalidLoyaltyMultiplier
			))
		);

		Ok(m)
	}

	/// This function calculates and updates `accumulated_rpz` and all associated properties of
	/// `global_farm` if conditions are met.
	/// Returns the reward transferred to the pot.
	#[require_transactional]
	fn sync_global_farm(
		global_farm: &mut GlobalFarmData<T, I>,
		current_period: PeriodOf<T>,
	) -> Result<Balance, DispatchError> {
		// Inactive farm should not be updated
		if !global_farm.state.is_active() {
			return Ok(Zero::zero());
		}

		// Farm should be updated only once in the same period.
		if global_farm.updated_at == current_period {
			return Ok(Zero::zero());
		}

		// Nothing to update if there is no stake in the farm.
		if global_farm.total_shares_z.is_zero() {
			global_farm.updated_at = current_period;
			return Ok(Zero::zero());
		}

		let global_farm_account = Self::farm_account_id(global_farm.id)?;
		let reward_currency_ed = T::AssetRegistry::get(&global_farm.reward_currency);
		let left_to_distribute = T::MultiCurrency::free_balance(global_farm.reward_currency, &global_farm_account)
			.saturating_sub(reward_currency_ed);

		// Number of periods since last farm update.
		let periods_since_last_update: Balance = TryInto::<u128>::try_into(
			current_period
				.checked_sub(&global_farm.updated_at)
				.defensive_ok_or::<Error<T, I>>(InconsistentStateError::InvalidPeriod.into())?,
		)
		.map_err(|_| ArithmeticError::Overflow)?;

		if let Ok(price_adjustment) = T::PriceAdjustment::get(global_farm) {
			global_farm.price_adjustment = price_adjustment;
		}

		// Calculate reward for all periods since last update capped by balance of `GlobalFarm`
		// account.
		let reward = math::calculate_global_farm_rewards(
			global_farm.total_shares_z,
			//NOTE: Fallback. Last saved value should be used if oracle is not available.
			global_farm.price_adjustment,
			global_farm.yield_per_period.into(),
			global_farm.max_reward_per_period,
			periods_since_last_update,
		)
		.map_err(|_| ArithmeticError::Overflow)?
		.min(left_to_distribute);

		if !reward.is_zero() {
			let pot = Self::pot_account_id().ok_or(Error::<T, I>::ErrorGetAccountId)?;
			T::MultiCurrency::transfer(global_farm.reward_currency, &global_farm_account, &pot, reward)?;

			global_farm.accumulated_rpz =
				math::calculate_accumulated_rps(global_farm.accumulated_rpz, global_farm.total_shares_z, reward)
					.map_err(|_| ArithmeticError::Overflow)?;

			global_farm.pending_rewards = global_farm
				.pending_rewards
				.checked_add(reward)
				.ok_or(ArithmeticError::Overflow)?;
		} else {
			Pallet::<T, I>::deposit_event(Event::AllRewardsDistributed {
				global_farm_id: global_farm.id,
			});
		}

		global_farm.updated_at = current_period;

		Pallet::<T, I>::deposit_event(Event::GlobalFarmAccRPZUpdated {
			global_farm_id: global_farm.id,
			accumulated_rpz: global_farm.accumulated_rpz,
			total_shares_z: global_farm.total_shares_z,
		});

		Ok(reward)
	}

	/// This function calculates and updates `accumulated_rpvz` and all associated properties of
	/// `YieldFarm` if conditions are met. It also calculates yield-farm's rewards from `GlobalFarm`.
	/// NOTE: Yield-farm's rewards are staying in the `pot`.
	#[require_transactional]
	fn sync_yield_farm(
		yield_farm: &mut YieldFarmData<T, I>,
		global_farm: &mut GlobalFarmData<T, I>,
		current_period: BlockNumberFor<T>,
	) -> Result<(), DispatchError> {
		if !yield_farm.state.is_active() {
			return Ok(());
		}

		if yield_farm.updated_at == current_period {
			return Ok(());
		}

		if yield_farm.total_valued_shares.is_zero() {
			//NOTE: This is important to prevent rewarding of the farms for emtpy periods and it
			//also prevents the first user getting more rewards than the second user.
			yield_farm.accumulated_rpz = global_farm.accumulated_rpz;
			yield_farm.updated_at = current_period;

			return Ok(());
		}

		let (delta_rpvs, yield_farm_rewards) = math::calculate_yield_farm_rewards(
			yield_farm.accumulated_rpz,
			global_farm.accumulated_rpz,
			yield_farm.multiplier,
			yield_farm.total_valued_shares,
		)
		.map_err(|_| ArithmeticError::Overflow)?;

		yield_farm.accumulated_rpz = global_farm.accumulated_rpz;

		global_farm.accumulated_paid_rewards = global_farm
			.accumulated_paid_rewards
			.checked_add(yield_farm_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		global_farm.pending_rewards = global_farm
			.pending_rewards
			.checked_sub(yield_farm_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		yield_farm.accumulated_rpvs = yield_farm
			.accumulated_rpvs
			.checked_add(&delta_rpvs)
			.ok_or(ArithmeticError::Overflow)?;

		yield_farm.updated_at = current_period;

		yield_farm.left_to_distribute = yield_farm
			.left_to_distribute
			.checked_add(yield_farm_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		Pallet::<T, I>::deposit_event(Event::YieldFarmAccRPVSUpdated {
			global_farm_id: global_farm.id,
			yield_farm_id: yield_farm.id,
			accumulated_rpvs: yield_farm.accumulated_rpvs,
			total_valued_shares: yield_farm.total_valued_shares,
		});

		Ok(())
	}

	/// This function returns an error if `farm_id` is not valid.
	fn validate_farm_id(farm_id: FarmId) -> Result<(), Error<T, I>> {
		if farm_id.is_zero() {
			return Err(InconsistentStateError::InvalidFarmId.into()).defensive();
		}

		Ok(())
	}

	/// This function is used to validate input data before creating new global farm.
	fn validate_create_global_farm_data(
		total_rewards: Balance,
		planned_yielding_periods: PeriodOf<T>,
		blocks_per_period: BlockNumberFor<T>,
		yield_per_period: Perquintill,
		min_deposit: Balance,
		price_adjustment: FixedU128,
	) -> DispatchResult {
		ensure!(min_deposit.ge(&MIN_DEPOSIT), Error::<T, I>::InvalidMinDeposit);

		ensure!(!price_adjustment.is_zero(), Error::<T, I>::InvalidPriceAdjustment);

		ensure!(
			total_rewards >= T::MinTotalFarmRewards::get(),
			Error::<T, I>::InvalidTotalRewards
		);

		ensure!(
			planned_yielding_periods >= T::MinPlannedYieldingPeriods::get(),
			Error::<T, I>::InvalidPlannedYieldingPeriods
		);

		ensure!(!blocks_per_period.is_zero(), Error::<T, I>::InvalidBlocksPerPeriod);

		ensure!(!yield_per_period.is_zero(), Error::<T, I>::InvalidYieldPerPeriod);

		Ok(())
	}

	// Claiming from `YieldFarm` is not possible(will fail) if yield farm is terminated or has no
	// entries.
	fn is_yield_farm_claimable(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: T::AmmPoolId,
	) -> bool {
		if let Some(yield_farm) = Self::yield_farm((amm_pool_id, global_farm_id, yield_farm_id)) {
			return !yield_farm.state.is_terminated() && yield_farm.has_entries();
		}

		false
	}

	// This function returns `GlobalFarmId` from deposit's farm entry or `None` if deposit or farm
	// entry doesn't exists.
	fn get_global_farm_id(id: DepositId, yield_farm_id: YieldFarmId) -> Option<GlobalFarmId> {
		if let Some(mut deposit) = Self::deposit(id) {
			if let Some(farm_entry) = deposit.get_yield_farm_entry(yield_farm_id) {
				return Some(farm_entry.global_farm_id);
			}
		}

		None
	}

	#[inline(always)]
	fn defensive_err(e: Error<T, I>) -> Error<T, I> {
		defensive!(e);
		e
	}
}

impl<T: Config<I>, I: 'static> hydradx_traits::liquidity_mining::Mutate<T::AccountId, T::AssetId, BlockNumberFor<T>>
	for Pallet<T, I>
{
	type Error = DispatchError;

	type AmmPoolId = T::AmmPoolId;
	type Balance = Balance;
	type Period = PeriodOf<T>;
	type LoyaltyCurve = LoyaltyCurve;

	fn create_global_farm(
		total_rewards: Self::Balance,
		planned_yielding_periods: Self::Period,
		blocks_per_period: BlockNumberFor<T>,
		incentivized_asset: T::AssetId,
		reward_currency: T::AssetId,
		owner: T::AccountId,
		yield_per_period: Perquintill,
		min_deposit: Self::Balance,
		price_adjustment: FixedU128,
	) -> Result<(GlobalFarmId, Self::Balance), Self::Error> {
		Self::create_global_farm(
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			incentivized_asset,
			reward_currency,
			owner,
			yield_per_period,
			min_deposit,
			price_adjustment,
		)
	}

	/// This function should be used when external source(e.g. oracle) is used for `price_adjustment`
	/// or if `incentivized_asset` and `reward_currency` can't be different.
	fn create_global_farm_without_price_adjustment(
		total_rewards: Self::Balance,
		planned_yielding_periods: Self::Period,
		blocks_per_period: BlockNumberFor<T>,
		incentivized_asset: T::AssetId,
		reward_currency: T::AssetId,
		owner: T::AccountId,
		yield_per_period: Perquintill,
		min_deposit: Self::Balance,
	) -> Result<(GlobalFarmId, Self::Balance), Self::Error> {
		Self::create_global_farm(
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			incentivized_asset,
			reward_currency,
			owner,
			yield_per_period,
			min_deposit,
			//NOTE: `price_adjustment` == 1 is same as no `price_adjustment`
			FixedU128::one(),
		)
	}

	fn update_global_farm_price_adjustment(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		price_adjustment: FixedU128,
	) -> Result<(), Self::Error> {
		Self::update_global_farm_price_adjustment(who, global_farm_id, price_adjustment)
	}

	fn terminate_global_farm(
		who: T::AccountId,
		global_farm_id: u32,
	) -> Result<(T::AssetId, Self::Balance, T::AccountId), Self::Error> {
		Self::terminate_global_farm(who, global_farm_id)
	}

	fn create_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		multiplier: FixedU128,
		loyalty_curve: Option<Self::LoyaltyCurve>,
		amm_pool_id: Self::AmmPoolId,
		assets: Vec<T::AssetId>,
	) -> Result<YieldFarmId, Self::Error> {
		Self::create_yield_farm(who, global_farm_id, multiplier, loyalty_curve, amm_pool_id, assets)
	}

	fn update_yield_farm_multiplier(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: Self::AmmPoolId,
		multiplier: FixedU128,
	) -> Result<YieldFarmId, Self::Error> {
		Self::update_yield_farm_multiplier(who, global_farm_id, amm_pool_id, multiplier)
	}

	fn stop_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<u32, Self::Error> {
		Self::stop_yield_farm(who, global_farm_id, amm_pool_id)
	}

	fn resume_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
		multiplier: FixedU128,
	) -> Result<(), Self::Error> {
		Self::resume_yield_farm(who, global_farm_id, yield_farm_id, amm_pool_id, multiplier)
	}

	fn terminate_yield_farm(
		who: T::AccountId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<(), Self::Error> {
		Self::terminate_yield_farm(who, global_farm_id, yield_farm_id, amm_pool_id)
	}

	fn deposit_lp_shares<F: Fn(T::AssetId, Self::AmmPoolId, Self::Balance) -> Result<Self::Balance, Self::Error>>(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
		shares_amount: Self::Balance,
		get_token_value_of_lp_shares: F,
	) -> Result<DepositId, Self::Error> {
		Self::deposit_lp_shares(
			global_farm_id,
			yield_farm_id,
			amm_pool_id,
			shares_amount,
			get_token_value_of_lp_shares,
		)
	}

	fn redeposit_lp_shares<F: Fn(T::AssetId, Self::AmmPoolId, Self::Balance) -> Result<Self::Balance, Self::Error>>(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		deposit_id: DepositId,
		get_token_value_of_lp_shares: F,
	) -> Result<(Self::Balance, Self::AmmPoolId), Self::Error> {
		Self::redeposit_lp_shares(global_farm_id, yield_farm_id, deposit_id, get_token_value_of_lp_shares)
	}

	fn claim_rewards(
		who: T::AccountId,
		deposit_id: DepositId,
		yield_farm_id: YieldFarmId,
	) -> Result<(GlobalFarmId, T::AssetId, Self::Balance, Self::Balance), Self::Error> {
		let fail_on_doubleclaim = true;
		Self::claim_rewards(who, deposit_id, yield_farm_id, fail_on_doubleclaim)
	}

	fn withdraw_lp_shares(
		who: T::AccountId,
		deposit_id: DepositId,
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> Result<(Self::Balance, Option<(T::AssetId, Self::Balance, Self::Balance)>, bool), Self::Error> {
		let claim_data = if Self::is_yield_farm_claimable(global_farm_id, yield_farm_id, amm_pool_id) {
			let fail_on_doubleclaim = false;
			let (_, reward_currency, claimed, unclaimable) =
				Self::claim_rewards(who, deposit_id, yield_farm_id, fail_on_doubleclaim)?;

			Some((reward_currency, claimed, unclaimable))
		} else {
			None
		};

		let unclaimable = claim_data.map_or(Zero::zero(), |(_, _, unclaimable)| unclaimable);
		let (_, withdrawn_amount, deposit_destroyed) =
			Self::withdraw_lp_shares(deposit_id, yield_farm_id, unclaimable)?;

		Ok((withdrawn_amount, claim_data, deposit_destroyed))
	}

	fn is_yield_farm_claimable(
		global_farm_id: GlobalFarmId,
		yield_farm_id: YieldFarmId,
		amm_pool_id: Self::AmmPoolId,
	) -> bool {
		Self::is_yield_farm_claimable(global_farm_id, yield_farm_id, amm_pool_id)
	}

	fn get_global_farm_id(deposit_id: DepositId, yield_farm_id: YieldFarmId) -> Option<u32> {
		Self::get_global_farm_id(deposit_id, yield_farm_id)
	}
}
