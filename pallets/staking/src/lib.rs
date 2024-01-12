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
//! # Staking Pallet

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use crate::traits::{ActionData, DemocracyReferendum, PayablePercentage, VestingDetails};
use crate::types::{Action, Balance, Period, Point, Position, StakingData, Voting};
use frame_support::ensure;
use frame_support::{
	pallet_prelude::DispatchResult,
	pallet_prelude::*,
	traits::nonfungibles::{Create, Inspect, InspectEnumerable, Mutate},
	traits::{DefensiveOption, LockIdentifier},
};
use frame_system::pallet_prelude::BlockNumberFor;
use hydra_dx_math::staking as math;
use orml_traits::{GetByKey, MultiCurrency, MultiLockableCurrency};
use sp_core::Get;
use sp_runtime::traits::{AccountIdConversion, CheckedAdd, One};
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	Perbill, Permill, SaturatedConversion,
};
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128};
use sp_std::num::NonZeroU128;

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod integrations;
pub mod traits;
pub mod types;
pub mod weights;

pub use pallet::*;
use types::Conviction;
pub use weights::WeightInfo;

/// Lock ID for staked assets.
pub const STAKING_LOCK_ID: LockIdentifier = *b"stk_stks";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::traits::{DemocracyReferendum, Freeze};
	use crate::types::Voting;
	use codec::HasCompact;
	use frame_support::PalletId;
	use frame_system::{ensure_signed, pallet_prelude::*};
	use orml_traits::GetByKey;
	use sp_runtime::traits::AtLeast32BitUnsigned;

	/// Current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Origin to initialize staking.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Identifier for the class of asset.
		type AssetId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Multi currency mechanism.
		type Currency: MultiLockableCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

		/// Staking period length in blocks.
		#[pallet::constant]
		type PeriodLength: Get<BlockNumberFor<Self>>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Native Asset ID.
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Min amount user must stake.
		#[pallet::constant]
		type MinStake: Get<Balance>;

		/// Weight of the time points in total points calculations.
		#[pallet::constant]
		type TimePointsWeight: Get<Permill>;

		/// Weight of the action points in total points calculations.
		#[pallet::constant]
		type ActionPointsWeight: Get<Perbill>;

		/// Number of time points users receive for each period.
		#[pallet::constant]
		type TimePointsPerPeriod: Get<u8>;

		/// Number of periods user can't claim rewards for. User can exit but won't receive any rewards.
		/// If he stay longer than `UnclaimablePeriods` he will receive rewards also for these periods.
		#[pallet::constant]
		type UnclaimablePeriods: Get<Period>;

		/// Weight of the actual stake in slash points calculation. Bigger the value lower the calculated slash points.
		#[pallet::constant]
		type CurrentStakeWeight: Get<u8>;

		/// Max amount of votes the user can have at any time.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// NFT collection id.
		#[pallet::constant]
		type NFTCollectionId: Get<Self::CollectionId>;

		/// Function returning percentage of rewards to pay based on the number of points user
		/// accumulated.
		type PayablePercentage: PayablePercentage<Point>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Position identifier type.
		type PositionItemId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Collection id type.
		type CollectionId: TypeInfo + MaxEncodedLen;

		/// Provides ability to freeze a collection.
		type Collections: Freeze<Self::AccountId, Self::CollectionId>;

		/// Non fungible handling - mint, burn, check owner.
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ Inspect<Self::AccountId, ItemId = Self::PositionItemId, CollectionId = Self::CollectionId>
			+ InspectEnumerable<Self::AccountId, ItemId = Self::PositionItemId, CollectionId = Self::CollectionId>;

		/// Max amount of action points user can receive for action. Users receives
		/// percentage of this based on how much of staking power they used. e.g. for democracy
		/// vote it is percentage of stake used for voting.
		type MaxPointsPerAction: GetByKey<Action, u32>;

		/// Democracy referendum state.
		type ReferendumInfo: DemocracyReferendum;

		/// Provides information about amount of vested tokens.
		type Vesting: VestingDetails<Self::AccountId, Balance>;

		#[cfg(feature = "runtime-benchmarks")]
		/// Max mumber of locks per account.  It's used in on_vote_worst_case benchmarks.
		type MaxLocks: Get<u32>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// Global staking state.
	#[pallet::getter(fn staking)]
	pub(super) type Staking<T: Config> = StorageValue<_, StakingData, ValueQuery>;

	#[pallet::storage]
	/// User's position state.
	#[pallet::getter(fn positions)]
	pub(super) type Positions<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PositionItemId, Position<BlockNumberFor<T>>>;

	#[pallet::storage]
	/// Position ids sequencer.
	#[pallet::getter(fn next_position_id)]
	pub(super) type NextPositionId<T: Config> = StorageValue<_, T::PositionItemId, ValueQuery>;

	#[pallet::storage]
	/// List of position votes.
	#[pallet::getter(fn position_votes)]
	pub(super) type PositionVotes<T: Config> =
		StorageMap<_, Blake2_128Concat, T::PositionItemId, Voting<T::MaxVotes>, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// New staking position was created and NFT was minted.
		PositionCreated {
			who: T::AccountId,
			position_id: T::PositionItemId,
			stake: Balance,
		},

		/// Staked amount for existing position was increased.
		StakeAdded {
			who: T::AccountId,
			position_id: T::PositionItemId,
			stake: Balance,
			total_stake: Balance,
			locked_rewards: Balance,
			slashed_points: Point,
			payable_percentage: FixedU128,
		},

		/// Rewards were claimed.
		RewardsClaimed {
			who: T::AccountId,
			position_id: T::PositionItemId,
			paid_rewards: Balance,
			unlocked_rewards: Balance,
			slashed_points: Point,
			slashed_unpaid_rewards: Balance,
			payable_percentage: FixedU128,
		},

		/// Staked amount was withdrawn and NFT was burned.
		Unstaked {
			who: T::AccountId,
			position_id: T::PositionItemId,
			unlocked_stake: Balance,
		},

		/// Staking was initialized.
		StakingInitialized { non_dustable_balance: Balance },

		/// Staking's `accumulated_reward_per_stake` was updated.
		AccumulatedRpsUpdated {
			accumulated_rps: FixedU128,
			total_stake: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Balance is too low.
		InsufficientBalance,

		/// Staked amount is too low.
		InsufficientStake,

		/// Staking position has not been found.
		PositionNotFound,

		/// Maximum amount of votes were reached for staking position.
		MaxVotesReached,

		/// Staking is not initialized.
		NotInitialized,

		/// Staking is already initialized.
		AlreadyInitialized,

		/// Arithmetic error.
		Arithmetic,

		/// Pot's balance is zero.
		MissingPotBalance,

		/// Account's position already exists.
		PositionAlreadyExists,

		/// Signer is not an owner of the staking position.
		Forbidden,

		/// Remove vote is not allowed when referendum is finished and staking position exists.
		RemoveVoteNotAllowed,

		/// Action cannot be completed because unexpected error has occurred. This should be reported
		/// to protocol maintainers.
		InconsistentState(InconsistentStateError),
	}

	// NOTE: these errors should never happen.
	#[derive(Encode, Decode, Eq, PartialEq, TypeInfo, frame_support::PalletError, RuntimeDebug)]
	pub enum InconsistentStateError {
		/// Position was not found in storage but NFT does exists.
		PositionNotFound,

		/// Calculated `pending_rewards` are less than 0.
		NegativePendingRewards,

		/// Calculated `accumulated_unpaid_rewards` are less than 0.
		NegativeUnpaidRewards,

		/// Multiple positions exits for single account.
		TooManyPositions,

		/// Arithmetic error.
		Arithmetic,
	}

	impl<T> From<InconsistentStateError> for Error<T> {
		fn from(e: InconsistentStateError) -> Error<T> {
			Error::<T>::InconsistentState(e)
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Staking pallet initialization. This call will reserved `pot`'s balance to prevent
		/// account dusting and start collecting fees from trades as rewards.
		///
		/// `pot`’s account has to have a balance which will be reserved to prevent account dusting.
		///
		/// Emits `StakingInitialized` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::initialize_staking())]
		pub fn initialize_staking(origin: OriginFor<T>) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			ensure!(!Self::is_initialized(), Error::<T>::AlreadyInitialized);

			let pallet_account = <Pallet<T>>::pot_account_id();
			let pot_balance = T::Currency::free_balance(T::NativeAssetId::get(), &pallet_account);
			ensure!(!pot_balance.is_zero(), Error::<T>::MissingPotBalance);

			//Offsetting `pot_reserved_balance` to prevent `pot` dusting.
			Staking::<T>::put(StakingData {
				pot_reserved_balance: pot_balance,
				..Default::default()
			});

			Self::deposit_event(Event::StakingInitialized {
				non_dustable_balance: pot_balance,
			});

			T::NFTHandler::create_collection(&T::NFTCollectionId::get(), &pallet_account, &pallet_account)?;
			T::Collections::freeze_collection(pallet_account, T::NFTCollectionId::get())
		}

		/// Stake `amount` into a new staking position.
		///
		/// `stake` locks specified `amount` into staking and creates new NFT representing staking
		/// position.
		/// Users can stake `NativeAssetId` balance which is not vested or already staked.
		///
		/// Staking pallet must be initialized otherwise extrinsic will fail with error.
		///
		/// Parameters:
		/// - `amount`: Amount of native asset to be staked. `amount` can't be vested or already
		/// staked
		///
		/// Emits `PositionCreated` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::stake())]
		pub fn stake(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(amount >= T::MinStake::get(), Error::<T>::InsufficientStake);

			ensure!(Self::is_initialized(), Error::<T>::NotInitialized);

			ensure!(
				Self::get_user_position_id(&who)?.is_none(),
				Error::<T>::PositionAlreadyExists
			);

			Staking::<T>::try_mutate(|staking| {
				Self::update_rewards(staking)?;

				Self::ensure_stakeable_balance(&who, amount, None)?;
				let position_id =
					Self::create_position_and_mint_nft(&who, amount, staking.accumulated_reward_per_stake)?;

				T::Currency::set_lock(STAKING_LOCK_ID, T::NativeAssetId::get(), &who, amount)?;

				staking.add_stake(amount)?;

				Self::deposit_event(Event::PositionCreated {
					who,
					position_id,
					stake: amount,
				});

				Ok(())
			})
		}

		/// Extrinsic to increase staked amount of existing staking position by specified `amount`.
		///
		/// `increase_stake` increases staked amount of position specified by `postion_id` by the
		/// `amount` specified by the user.
		/// Staking position must exist and `origin` has to be the owner of the position.
		/// Users can stake tokens which are not vested or already staked.
		/// Position's params e.g points are updated to offset stake increase and rewards
		/// accumulated until this point are paid and locked to the user.
		///
		/// Parameters:
		/// - `position_id`: The identifier of the position which stake will be increased.
		/// - `amount`: Amount of native asset to be added to staked amount. `amount` can't be vested or
		/// already staked
		///
		/// Emits `StakeAdded` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::increase_stake())]
		pub fn increase_stake(origin: OriginFor<T>, position_id: T::PositionItemId, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(amount >= T::MinStake::get(), Error::<T>::InsufficientStake);

			ensure!(Self::is_initialized(), Error::<T>::NotInitialized);

			ensure!(Self::is_owner(&who, position_id), Error::<T>::Forbidden);

			Staking::<T>::try_mutate(|staking| {
				Self::update_rewards(staking)?;

				Positions::<T>::try_mutate(position_id, |maybe_position| {
					let position = maybe_position
						.as_mut()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::PositionNotFound.into())?;

					Self::ensure_stakeable_balance(&who, amount, Some(position))?;

					Self::process_votes(position_id, position)?;

					let current_period = Self::get_current_period()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;
					let created_at = Self::get_period_number(position.created_at)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, payable_percentage) =
						Self::calculate_rewards(
							position,
							staking.accumulated_reward_per_stake,
							current_period,
							created_at,
						)
						.ok_or(Error::<T>::Arithmetic)?;

					let rewards = claimable_rewards
						.checked_add(claimable_unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					let pot = Self::pot_account_id();
					T::Currency::transfer(T::NativeAssetId::get(), &pot, &who, rewards)?;

					position.accumulated_unpaid_rewards = position
						.accumulated_unpaid_rewards
						.checked_add(unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?;
					position.accumulated_unpaid_rewards = position
						.accumulated_unpaid_rewards
						.checked_sub(claimable_unpaid_rewards)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::NegativeUnpaidRewards.into())?;

					position.accumulated_locked_rewards = position
						.accumulated_locked_rewards
						.checked_add(rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					position.reward_per_stake = staking.accumulated_reward_per_stake;

					let points =
						Self::get_points(position, current_period, created_at).ok_or(Error::<T>::Arithmetic)?;
					let slash_points =
						math::calculate_slashed_points(points, position.stake, amount, T::CurrentStakeWeight::get())
							.ok_or(Error::<T>::Arithmetic)?;

					position.accumulated_slash_points = position
						.accumulated_slash_points
						.checked_add(slash_points)
						.ok_or(Error::<T>::Arithmetic)?;

					position.stake = position.stake.checked_add(amount).ok_or(Error::<T>::Arithmetic)?;

					staking.pot_reserved_balance = staking
						.pot_reserved_balance
						.checked_sub(rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					staking.add_stake(amount)?;

					T::Currency::set_lock(
						STAKING_LOCK_ID,
						T::NativeAssetId::get(),
						&who,
						position.get_total_locked()?,
					)?;

					Self::deposit_event(Event::StakeAdded {
						who,
						position_id,
						stake: amount,
						total_stake: position.stake,
						locked_rewards: rewards,
						slashed_points: slash_points,
						payable_percentage,
					});

					Ok(())
				})
			})
		}

		/// Claim rewards accumulated for specific staking position.
		///
		/// Function calculates amount of rewards to pay for specified staking position based on
		/// the amount of points position accumulated. Function also unlocks all the rewards locked
		/// from `increase_stake` based on the amount of the points.
		///
		/// This action is penalized by removing all the points and returning allocated unpaid rewards
		/// for redistribution.
		///
		/// Parameters:
		/// - `position_id`: The identifier of the position to claim rewards for.
		///
		/// Emits `RewardsClaimed` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::claim())]
		pub fn claim(origin: OriginFor<T>, position_id: T::PositionItemId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_initialized(), Error::<T>::NotInitialized);

			ensure!(Self::is_owner(&who, position_id), Error::<T>::Forbidden);

			Staking::<T>::try_mutate(|staking| {
				Self::update_rewards(staking)?;

				Positions::<T>::try_mutate(position_id, |maybe_position| {
					let position = maybe_position
						.as_mut()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::PositionNotFound.into())?;

					Self::process_votes(position_id, position)?;

					let current_period = Self::get_current_period()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;
					let created_at = Self::get_period_number(position.created_at)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, payable_percentage) =
						Self::calculate_rewards(
							position,
							staking.accumulated_reward_per_stake,
							current_period,
							created_at,
						)
						.ok_or(Error::<T>::Arithmetic)?;

					let rewards_to_pay = claimable_rewards
						.checked_add(claimable_unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					let pot = Self::pot_account_id();
					T::Currency::transfer(T::NativeAssetId::get(), &pot, &who, rewards_to_pay)?;

					let rewards_to_unlock = position.accumulated_locked_rewards;
					position.accumulated_locked_rewards = Zero::zero();

					position.accumulated_unpaid_rewards = position
						.accumulated_unpaid_rewards
						.checked_add(unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					position.accumulated_unpaid_rewards = position
						.accumulated_unpaid_rewards
						.checked_sub(claimable_unpaid_rewards)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::NegativeUnpaidRewards.into())?;

					let points_to_slash =
						Self::get_points(position, current_period, created_at).ok_or(Error::<T>::Arithmetic)?;
					position.accumulated_slash_points = position
						.accumulated_slash_points
						.checked_add(points_to_slash)
						.ok_or(Error::<T>::Arithmetic)?;

					let slashed_unpaid_rewards =
						if current_period.saturating_sub(created_at) > T::UnclaimablePeriods::get() {
							let p = position.accumulated_unpaid_rewards;
							position.accumulated_unpaid_rewards = Zero::zero();
							p
						} else {
							Zero::zero()
						};
					position.reward_per_stake = staking.accumulated_reward_per_stake;

					T::Currency::set_lock(
						STAKING_LOCK_ID,
						T::NativeAssetId::get(),
						&who,
						position.get_total_locked()?,
					)?;

					staking.pot_reserved_balance = staking
						.pot_reserved_balance
						.checked_sub(rewards_to_pay)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?
						.checked_sub(slashed_unpaid_rewards)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					Self::deposit_event(Event::RewardsClaimed {
						who,
						position_id,
						paid_rewards: rewards_to_pay,
						unlocked_rewards: rewards_to_unlock,
						slashed_points: points_to_slash,
						slashed_unpaid_rewards,
						payable_percentage,
					});

					Ok(())
				})
			})
		}

		/// Function pays rewards, unlocks all the staked assets and destroys staking position
		/// specified by `position_id`.
		///
		/// Function calculates and pays latest rewards, unlocks all the locked rewards and staked
		/// tokens for staking position and burns NFT representing staking position.
		/// Unpaid allocated rewards are returned to the Staking for redistribution.
		///
		/// Parameters:
		/// - `position_id`: The identifier of the position to be destroyed.
		///
		/// Emits `RewardsClaimed` and `Unstaked` events when successful.
		///
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::unstake())]
		pub fn unstake(origin: OriginFor<T>, position_id: T::PositionItemId) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(Self::is_initialized(), Error::<T>::NotInitialized);

			ensure!(Self::is_owner(&who, position_id), Error::<T>::Forbidden);

			Staking::<T>::try_mutate(|staking| {
				Self::update_rewards(staking)?;

				Positions::<T>::try_mutate_exists(position_id, |maybe_position| {
					let position = maybe_position
						.as_mut()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::PositionNotFound.into())?;

					Self::process_votes(position_id, position)?;

					let current_period = Self::get_current_period()
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;
					let created_at = Self::get_period_number(position.created_at)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, payable_percentage) =
						Self::calculate_rewards(
							position,
							staking.accumulated_reward_per_stake,
							current_period,
							created_at,
						)
						.ok_or(Error::<T>::Arithmetic)?;

					let rewards_to_pay = claimable_rewards
						.checked_add(claimable_unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?;

					let pot = Self::pot_account_id();
					T::Currency::transfer(T::NativeAssetId::get(), &pot, &who, rewards_to_pay)?;

					staking.total_stake = staking
						.total_stake
						.checked_sub(position.stake)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					let return_to_pot = position
						.accumulated_unpaid_rewards
						.checked_add(unpaid_rewards)
						.ok_or(Error::<T>::Arithmetic)?
						.checked_sub(claimable_unpaid_rewards)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					staking.pot_reserved_balance = staking
						.pot_reserved_balance
						.checked_sub(rewards_to_pay)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?
						.checked_sub(return_to_pot)
						.defensive_ok_or::<Error<T>>(InconsistentStateError::Arithmetic.into())?;

					T::NFTHandler::burn(&T::NFTCollectionId::get(), &position_id, Some(&who))?;
					T::Currency::remove_lock(STAKING_LOCK_ID, T::NativeAssetId::get(), &who)?;

					Self::deposit_event(Event::RewardsClaimed {
						who: who.clone(),
						position_id,
						paid_rewards: rewards_to_pay,
						unlocked_rewards: position.accumulated_locked_rewards,
						slashed_points: Self::get_points(position, current_period, created_at)
							.ok_or(Error::<T>::Arithmetic)?,
						slashed_unpaid_rewards: return_to_pot,
						payable_percentage,
					});

					Self::deposit_event(Event::Unstaked {
						who,
						position_id,
						unlocked_stake: position.stake,
					});

					PositionVotes::<T>::remove(position_id);
					*maybe_position = None;

					Ok(())
				})
			})
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	/// Account id holding rewards to pay.
	pub fn pot_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	fn ensure_stakeable_balance(
		who: &T::AccountId,
		stake: Balance,
		position: Option<&Position<BlockNumberFor<T>>>,
	) -> Result<(), DispatchError> {
		let free_balance = T::Currency::free_balance(T::NativeAssetId::get(), who);
		let staked = position
			.map(|p| p.stake.saturating_add(p.accumulated_locked_rewards))
			.unwrap_or_default();
		let vested = T::Vesting::locked(who.clone());

		//NOTE: locks overlay so vested + staked can be bigger than free_balance
		let stakeable = free_balance
			.checked_sub(vested)
			.ok_or(Error::<T>::Arithmetic)?
			.saturating_sub(staked);

		ensure!(stakeable >= stake, Error::<T>::InsufficientBalance);

		Ok(())
	}

	/// Returns staking `PositionItemId` for `who` or `None`.
	pub fn get_user_position_id(who: &T::AccountId) -> Result<Option<T::PositionItemId>, DispatchError> {
		let mut user_position_ids = T::NFTHandler::owned_in_collection(&T::NFTCollectionId::get(), who);

		let position_id = user_position_ids.next();
		if position_id.is_some() {
			ensure!(
				user_position_ids.next().is_none(),
				Error::<T>::InconsistentState(InconsistentStateError::TooManyPositions)
			);
		}

		Ok(position_id)
	}

	fn is_owner(who: &T::AccountId, id: T::PositionItemId) -> bool {
		if let Some(owner) =
			<T as pallet::Config>::NFTHandler::owner(&<T as pallet::Config>::NFTCollectionId::get(), &id)
		{
			owner == *who
		} else {
			false
		}
	}

	fn create_position_and_mint_nft(
		who: &T::AccountId,
		staked_amount: Balance,
		accumulated_reward_per_stake: FixedU128,
	) -> Result<T::PositionItemId, DispatchError> {
		let position_id = Self::get_next_position_id()?;
		Positions::<T>::insert(
			position_id,
			Position::new(
				staked_amount,
				accumulated_reward_per_stake,
				T::BlockNumberProvider::current_block_number(),
			),
		);

		T::NFTHandler::mint_into(&T::NFTCollectionId::get(), &position_id, who)?;

		Ok(position_id)
	}

	fn get_next_position_id() -> Result<T::PositionItemId, DispatchError> {
		<NextPositionId<T>>::try_mutate(|current_value| -> Result<T::PositionItemId, DispatchError> {
			let next_id = *current_value;

			*current_value = current_value
				.checked_add(&T::PositionItemId::one())
				.ok_or(Error::<T>::Arithmetic)?;

			Ok(next_id)
		})
	}

	/// This function "distributes" pending rewards if possible and updates `StakingData`
	pub(crate) fn update_rewards(staking: &mut StakingData) -> Result<(), DispatchError> {
		if staking.total_stake.is_zero() {
			return Ok(());
		}

		let pending_rewards = T::Currency::free_balance(T::NativeAssetId::get(), &Self::pot_account_id())
			.checked_sub(staking.pot_reserved_balance)
			.defensive_ok_or::<Error<T>>(InconsistentStateError::NegativePendingRewards.into())?;

		if pending_rewards.is_zero() {
			return Ok(());
		}

		let accumulated_rps = math::calculate_accumulated_rps(
			staking.accumulated_reward_per_stake,
			pending_rewards,
			staking.total_stake,
		)
		.ok_or(Error::<T>::Arithmetic)?;

		if staking.accumulated_reward_per_stake == accumulated_rps {
			// No pending rewards or rewards are too small to distribute.
			return Ok(());
		}

		staking.accumulated_reward_per_stake = accumulated_rps;
		staking.pot_reserved_balance = staking
			.pot_reserved_balance
			.checked_add(pending_rewards)
			.ok_or(Error::<T>::Arithmetic)?;

		Self::deposit_event(Event::AccumulatedRpsUpdated {
			accumulated_rps: staking.accumulated_reward_per_stake,
			total_stake: staking.total_stake,
		});

		Ok(())
	}

	/// This function calculates total amount of points `position` accumulated until now.
	/// Slash points are subtracted from returned value.
	#[inline]
	fn get_points(
		position: &Position<BlockNumberFor<T>>,
		current_period: Period,
		position_created_at: Period,
	) -> Option<Point> {
		math::calculate_points(
			position_created_at,
			current_period,
			T::TimePointsPerPeriod::get(),
			T::TimePointsWeight::get(),
			position.action_points,
			T::ActionPointsWeight::get(),
			position.accumulated_slash_points,
		)
	}

	#[inline]
	fn get_current_period() -> Option<Period> {
		Self::get_period_number(T::BlockNumberProvider::current_block_number())
	}

	#[inline]
	fn get_period_number(block: BlockNumberFor<T>) -> Option<Period> {
		Some(math::calculate_period_number(
			NonZeroU128::try_from(T::PeriodLength::get().saturated_into::<u128>()).ok()?,
			block.saturated_into(),
		))
	}

	/// This function calculates `claimable`, `claimable_unpaid`, `unpaid` rewards and `payable_percentage`.
	///
	/// `claimable` - amount of rewards user can claim from the `pot`
	/// `claimable_unpaid` - amount to unlock from user's `accumulated_unpaid_rewards`
	/// `unpaid` - amount of rewards which won't be paid to user
	/// `payable_percentage` - percentage of the rewards that is available to user
	///
	/// Return `(claimable, claimable_unpaid, unpaid, payable_percentage)`
	fn calculate_rewards(
		position: &Position<BlockNumberFor<T>>,
		accumulated_reward_per_stake: FixedU128,
		current_period: Period,
		position_created_at: Period,
	) -> Option<(Balance, Balance, Balance, FixedU128)> {
		let max_rewards =
			math::calculate_rewards(accumulated_reward_per_stake, position.reward_per_stake, position.stake)?;

		if current_period.saturating_sub(position_created_at) <= T::UnclaimablePeriods::get() {
			return Some((Balance::zero(), Balance::zero(), max_rewards, FixedU128::zero()));
		}

		let points = Self::get_points(position, current_period, position_created_at)?;
		let payable_percentage = T::PayablePercentage::get(points)?;

		let claimable_rewards = math::calculate_percentage_amount(max_rewards, payable_percentage);

		let unpaid_rewards = max_rewards.checked_sub(claimable_rewards)?;

		let claimable_unpaid_rewards =
			math::calculate_percentage_amount(position.accumulated_unpaid_rewards, payable_percentage);

		Some((
			claimable_rewards,
			claimable_unpaid_rewards,
			unpaid_rewards,
			payable_percentage,
		))
	}

	/// Transfer given fee to pot account.
	/// Returns amount of unused fee.
	pub fn process_trade_fee(
		source: T::AccountId,
		asset: T::AssetId,
		amount: Balance,
	) -> Result<Balance, DispatchError> {
		if asset == T::NativeAssetId::get() && Self::is_initialized() {
			T::Currency::transfer(asset, &source, &Self::pot_account_id(), amount)?;
			Ok(amount)
		} else {
			Ok(Balance::zero())
		}
	}

	pub(crate) fn process_votes(
		position_id: T::PositionItemId,
		position: &mut Position<BlockNumberFor<T>>,
	) -> DispatchResult {
		PositionVotes::<T>::mutate(position_id, |voting| {
			let max_position_vote = Conviction::max_multiplier().saturating_mul_int(position.stake);

			voting.votes.retain(|(ref_idx, vote)| {
				if T::ReferendumInfo::is_referendum_finished(*ref_idx) {
					let points = Self::calculate_points_for_action(Action::DemocracyVote, vote, max_position_vote);
					position.action_points = position.action_points.saturating_add(points);
					false
				} else {
					true
				}
			});
		});
		Ok(())
	}

	/// Returns amount of action points user receives for action.
	///
	/// params:
	/// - action: action for which points are calculated
	/// - data: action's data necessary for points calculation
	/// - action_max_value: max value that can be used by user for `action`. It is used to calculate
	/// percentage of points user will receive based on how much action's power user used.
	fn calculate_points_for_action<V: ActionData>(action: Action, data: V, action_max_value: Balance) -> Balance {
		data.conviction()
			.saturating_mul_int(data.amount())
			.saturating_mul(T::MaxPointsPerAction::get(&action) as u128)
			.saturating_div(action_max_value)
	}

	#[inline]
	fn is_initialized() -> bool {
		Staking::<T>::exists()
	}
}

impl<T: Config> Pallet<T> {
	pub fn get_position(position_id: T::PositionItemId) -> Option<Position<BlockNumberFor<T>>> {
		Positions::<T>::get(position_id)
	}

	pub fn get_position_votes(position_id: T::PositionItemId) -> Voting<T::MaxVotes> {
		PositionVotes::<T>::get(position_id)
	}
}

pub struct SigmoidPercentage<T, B>(sp_std::marker::PhantomData<(T, B)>);

impl<T, B: Get<u32>> PayablePercentage<Point> for SigmoidPercentage<T, B>
where
	T: Get<FixedU128>,
{
	fn get(p: Point) -> Option<FixedU128> {
		let a: FixedU128 = T::get();
		let b: u32 = B::get();

		math::sigmoid(p, a, b)
	}
}
