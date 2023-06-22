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

// TODO
//  * [] - nontransferable nft
//  * [] - deposit for nft
//  * [] - don't allow to skate vested tokens
//  * [] - tests create/increase during UnclaimablePeriods
//  * [] - user can stake already locked token multiple time so he can lock more than he have
//  * [] - remove pending_rew from staking

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use crate::traits::PayablePercentage;
use crate::types::{Balance, Period, Point, Position, StakingData};
use frame_support::ensure;
use frame_support::{
	pallet_prelude::DispatchResult,
	traits::nonfungibles::{Create, InspectEnumerable, Mutate},
};
use hydra_dx_math::staking as math;
use orml_traits::{MultiCurrency, MultiLockableCurrency};
use sp_core::Get;
use sp_runtime::traits::{AccountIdConversion, CheckedAdd, One};
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	ArithmeticError, Permill,
};
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128};

#[cfg(test)]
mod tests;

pub mod traits;
pub mod types;
pub mod weights;

pub use pallet::*;
use pallet_democracy::traits::DemocracyHooks;
use pallet_democracy::{AccountVote, ReferendumIndex};
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::PalletId;
	use frame_support::{pallet_prelude::*, traits::LockIdentifier};
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::AtLeast32BitUnsigned;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::genesis_config]
	#[cfg_attr(feature = "std", derive(Default))]
	pub struct GenesisConfig {}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			let pallet_account = <Pallet<T>>::pot_account_id();

			<T as pallet::Config>::NFTHandler::create_collection(
				&<T as pallet::Config>::NFTCollectionId::get(),
				&pallet_account,
				&pallet_account,
			)
			.unwrap()
		}
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type WeightInfo: WeightInfo;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>
			+ MultiLockableCurrency<Self::AccountId>;

		/// Staking period length in blocks.
		type PeriodLength: Get<Self::BlockNumber>;

		/// Pallet id.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// HDX Asset ID
		#[pallet::constant]
		type HdxAssetId: Get<Self::AssetId>;

		/// Min amount user must stake.
		#[pallet::constant]
		type MinStake: Get<Balance>;

		/// Weight of the time points in total points calculations.
		#[pallet::constant]
		type TimePointsWeight: Get<Permill>;

		/// Weight of the action points in total points calculations.
		#[pallet::constant]
		type ActionPointsWeight: Get<Permill>;
		//TODO: points per action. Will there be different amount of points per different action?

		/// Number of time points users receive for each period.
		#[pallet::constant]
		type TimePointsPerPeriod: Get<u8>;

		/// Number of periods user can't claim rewards for. User can exit but won't receive rewards
		/// but if he stay longer, he will receive rewards also for these periods.
		#[pallet::constant]
		type UnclaimablePeriods: Get<Period>;

		//TODO: tinkg about better name
		/// Weight of the actual stake in slash points calculation. Bigger the value lower the calculated slash points.
		#[pallet::constant]
		type CurrentStakeWeight: Get<u8>;

		/// Function returning percentage of rewards to pay based on number of points user
		/// accumulated.
		type PayablePercentage: PayablePercentage<Point, Error = ArithmeticError>;

		/// The block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		/// Position identifier type.
		type PositionItemId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;

		/// Collection id type
		type CollectionId: TypeInfo + MaxEncodedLen;

		/// NFT collection id
		#[pallet::constant]
		type NFTCollectionId: Get<Self::CollectionId>;

		/// Non fungible handling - mint,burn, check owner
		type NFTHandler: Mutate<Self::AccountId>
			+ Create<Self::AccountId>
			+ InspectEnumerable<Self::AccountId, ItemId = Self::PositionItemId, CollectionId = Self::CollectionId>;
	}

	/// Lock for staked amount by user
	pub(super) const STAKING_LOCK_ID: LockIdentifier = *b"stk_stks";

	#[pallet::storage]
	/// Global staking state.
	#[pallet::getter(fn staking)]
	pub(super) type Staking<T: Config> = StorageValue<_, StakingData, ValueQuery>;

	#[pallet::storage]
	/// User's position state.
	#[pallet::getter(fn positions)]
	pub(super) type Positions<T: Config> = StorageMap<_, Blake2_128Concat, T::PositionItemId, Position<T::BlockNumber>>;

	#[pallet::storage]
	#[pallet::getter(fn next_position_id)]
	/// Position ids sequencer
	pub(super) type NextPositionId<T: Config> = StorageValue<_, T::PositionItemId, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		StakeAdded {
			who: T::AccountId,
			position_id: T::PositionItemId,
			stake: Balance,
			total_stake: Balance,
			locked_rewards: Balance,
			slashed_points: Point,
		},

		RewardsClaimed {
			who: T::AccountId,
			position_id: T::PositionItemId,
			paid_rewards: Balance,
			unlocked_rewards: Balance,
			slashed_points: Point,
			slashed_unpaid_rewards: Balance,
		},

		Unstaked {
			who: T::AccountId,
			position_id: T::PositionItemId,
			unlocked_stake: Balance,
			rewards: Balance,
			unlocked_rewards: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,

		/// Staked amount is too low
		InsufficientStake,

		/// Each user can have max one position
		TooManyPostions,

		/// Position has not been found
		PositionNotFound,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(1_000)]
		pub fn stake(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(amount >= T::MinStake::get(), Error::<T>::InsufficientStake);

			ensure!(
				T::Currency::free_balance(T::HdxAssetId::get(), &who) >= amount,
				Error::<T>::InsufficientBalance
			);

			Staking::<T>::try_mutate(|staking| {
				Self::reward_stakers(staking)?;

				let (position_id, position_new_total_stake, amount_to_lock, locked_rewards, slashed_points) =
					if let Some(position_id) = Self::get_user_position_id(&who)? {
						Positions::<T>::try_mutate(
							position_id,
							|maybe_position| -> Result<(T::PositionItemId, Balance, Balance, Balance, Point), DispatchError> {
								//TODO: inconsistent state
								let position = maybe_position.as_mut().ok_or(Error::<T>::PositionNotFound)?;

                                let current_period = Self::get_current_period()?;
                                let created_at = Self::get_period_number(position.created_at)?;

								let (rewards, slashed_points) = Self::do_increase_stake(position, staking, amount, current_period, created_at)?;

                                let pot = Self::pot_account_id();
                                T::Currency::transfer(T::HdxAssetId::get(), &pot, &who, rewards)?;

								Ok((position_id, position.stake, position.get_total_locked()?, rewards, slashed_points))
							},
						)?
					} else {
						let position_id =
							Self::create_position_and_mint_nft(&who, amount, staking.accumulated_reward_per_stake)?;

						(position_id, amount, amount, 0, 0)
					};

				T::Currency::set_lock(STAKING_LOCK_ID, T::HdxAssetId::get(), &who, amount_to_lock)?;

				staking.add_stake(amount)?;

				Self::deposit_event(Event::StakeAdded {
					who,
					position_id,
					stake: amount,
					total_stake: position_new_total_stake,
					locked_rewards,
					slashed_points,
				});

				Ok(())
			})
		}

		#[pallet::call_index(1)]
		#[pallet::weight(1_000)]
		pub fn claim(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let position_id = Self::get_user_position_id(&who)?;
			ensure!(position_id.is_some(), Error::<T>::PositionNotFound);

			Staking::<T>::try_mutate(|staking| {
				Self::reward_stakers(staking)?;

				Positions::<T>::try_mutate(position_id.unwrap(), |maybe_position| {
					//TODO: inconsistent state
					let position = maybe_position.as_mut().ok_or(Error::<T>::PositionNotFound)?;

					let current_period = Self::get_current_period()?;
					let created_at = Self::get_period_number(position.created_at)?;

					let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, payable_percentage) =
						Self::calculate_rewards(
							position,
							staking.accumulated_reward_per_stake,
							current_period,
							created_at,
						)?;

					let rewards_to_pay = claimable_rewards
						.checked_add(claimable_unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					let pot = Self::pot_account_id();
					T::Currency::transfer(T::HdxAssetId::get(), &pot, &who, rewards_to_pay)?;

					let rewards_to_unlock = payable_percentage
						.checked_mul_int(position.accumulated_locked_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					position.accumulated_locked_rewards = position
						.accumulated_locked_rewards
						.checked_sub(rewards_to_unlock)
						.ok_or(ArithmeticError::Overflow)?;

					position.accumulated_unpaid_rewards = position
						.accumulated_unpaid_rewards
						.checked_add(unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?
						.checked_sub(claimable_unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					let points_to_slash = Self::get_points(position, current_period, created_at)?;
					position.accumulated_slash_points = position
						.accumulated_slash_points
						.checked_add(points_to_slash)
						.ok_or(ArithmeticError::Overflow)?;

					let slashed_unpaid_rewards = position.accumulated_unpaid_rewards;
					position.accumulated_unpaid_rewards = Zero::zero();
					position.reward_per_stake = staking.accumulated_reward_per_stake;

					T::Currency::set_lock(
						STAKING_LOCK_ID,
						T::HdxAssetId::get(),
						&who,
						position.get_total_locked()?,
					)?;

					//return what's left to redistribution, will be removed
					staking.pending_rew = staking
						.pending_rew
						.checked_add(slashed_unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					Self::deposit_event(Event::RewardsClaimed {
						who,
						position_id: position_id.unwrap(),
						paid_rewards: rewards_to_pay,
						unlocked_rewards: rewards_to_unlock,
						slashed_points: points_to_slash,
						slashed_unpaid_rewards,
					});

					Ok(())
				})
			})
		}

		#[pallet::call_index(2)]
		#[pallet::weight(1_000)]
		pub fn unstake(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let position_id = Self::get_user_position_id(&who)?;
			ensure!(position_id.is_some(), Error::<T>::PositionNotFound);

			Staking::<T>::try_mutate(|staking| {
				Self::reward_stakers(staking)?;

				let position_id = position_id.unwrap();
				Positions::<T>::try_mutate_exists(position_id, |maybe_position| {
					//TODO: inconsistent state
					let position = maybe_position.as_mut().ok_or(Error::<T>::PositionNotFound)?;

					let current_period = Self::get_current_period()?;
					let created_at = Self::get_period_number(position.created_at)?;

					let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, _) = Self::calculate_rewards(
						position,
						staking.accumulated_reward_per_stake,
						current_period,
						created_at,
					)?;

					let rewards_to_pay = claimable_rewards
						.checked_add(claimable_unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					let pot = Self::pot_account_id();
					T::Currency::transfer(T::HdxAssetId::get(), &pot, &who, rewards_to_pay)?;

					staking.total_stake = staking
						.total_stake
						.checked_sub(position.stake)
						.ok_or(ArithmeticError::Overflow)?;

					let return_to_pot = position
						.accumulated_unpaid_rewards
						.checked_add(unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?
						.checked_sub(claimable_unpaid_rewards)
						.ok_or(ArithmeticError::Overflow)?;

					//TODO: tmp will be removed
					staking.pending_rew += return_to_pot;

					T::NFTHandler::burn(&T::NFTCollectionId::get(), &position_id, Some(&who))?;
					T::Currency::remove_lock(STAKING_LOCK_ID, T::HdxAssetId::get(), &who)?;

					Self::deposit_event(Event::Unstaked {
						who,
						position_id,
						unlocked_stake: position.stake,
						rewards: rewards_to_pay,
						unlocked_rewards: position.accumulated_locked_rewards,
					});

					*maybe_position = None;

					Ok(())
				})
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn do_democracy_vote() -> DispatchResult {
		Ok(())
	}

	/// Account id holding rewards to pay.
	pub fn pot_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	fn get_user_position_id(who: &T::AccountId) -> Result<Option<T::PositionItemId>, DispatchError> {
		let mut user_position_ids = T::NFTHandler::owned_in_collection(&T::NFTCollectionId::get(), &who);

		let position_id = user_position_ids.next();
		if position_id.is_some() {
			//TODO: change to inconsistent error
			ensure!(user_position_ids.next().is_none(), Error::<T>::TooManyPostions);

			return Ok(position_id);
		}

		Ok(None)
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

		T::NFTHandler::mint_into(&T::NFTCollectionId::get(), &position_id, &who)?;

		Ok(position_id)
	}

	fn do_increase_stake(
		position: &mut Position<T::BlockNumber>,
		staking: &StakingData,
		added_stake: Balance,
		current_period: Period,
		position_created_at: Period,
	) -> Result<(Balance, Point), ArithmeticError> {
		let (claimable_rewards, claimable_unpaid_rewards, unpaid_rewards, _) = Self::calculate_rewards(
			position,
			staking.accumulated_reward_per_stake,
			current_period,
			position_created_at,
		)?;

		let rewards_to_pay = claimable_rewards
			.checked_add(claimable_unpaid_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		//TODO: inconsistent state - this should never fail
		position.accumulated_unpaid_rewards = position
			.accumulated_unpaid_rewards
			.checked_add(unpaid_rewards)
			.ok_or(ArithmeticError::Overflow)?
			.checked_sub(claimable_unpaid_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		position.accumulated_locked_rewards = position
			.accumulated_locked_rewards
			.checked_add(rewards_to_pay)
			.ok_or(ArithmeticError::Overflow)?;

		position.reward_per_stake = staking.accumulated_reward_per_stake;

		let points = Self::get_points(&position, current_period, position_created_at)?;
		let slash_points =
			math::calculate_slashed_points(points, position.stake, added_stake, T::CurrentStakeWeight::get())
				.map_err(|_| ArithmeticError::Overflow)?;

		position.accumulated_slash_points = position
			.accumulated_slash_points
			.checked_add(slash_points)
			.ok_or(ArithmeticError::Overflow)?;

		position.stake = position
			.stake
			.checked_add(added_stake)
			.ok_or(ArithmeticError::Overflow)?;

		Ok((rewards_to_pay, slash_points))
	}

	fn get_next_position_id() -> Result<T::PositionItemId, ArithmeticError> {
		<NextPositionId<T>>::try_mutate(|current_value| -> Result<T::PositionItemId, ArithmeticError> {
			let next_id = *current_value;

			*current_value = current_value
				.checked_add(&T::PositionItemId::one())
				.ok_or(ArithmeticError::Overflow)?;

			Ok(next_id)
		})
	}

	/// This function distributes pending rewards if possible and updates `StakingData`
	fn reward_stakers(staking: &mut StakingData) -> Result<(), ArithmeticError> {
		if Zero::is_zero(&staking.total_stake) {
			return Ok(());
		}

		let pending_rewards = staking.pending_rewards();
		if pending_rewards.is_zero() {
			return Ok(());
		}

		let accumulated_rps = math::calculate_accumulated_rps(
			staking.accumulated_reward_per_stake,
			pending_rewards,
			staking.total_stake,
		)
		.map_err(|_| ArithmeticError::Overflow)?;

		if staking.accumulated_reward_per_stake == accumulated_rps {
			//No pending rewards or rewards are too small to distribute
			return Ok(());
		}

		staking.accumulated_reward_per_stake = accumulated_rps;

		//TODO:
		staking.pending_rew = 0;

		Ok(())
	}

	/// This function calculates total mount of points `position` accumulated until now.
	/// Slash points are removed from returned valued.
	#[inline]
	fn get_points(
		position: &Position<T::BlockNumber>,
		current_period: Period,
		position_created_at: Period,
	) -> Result<Point, ArithmeticError> {
		math::calculate_points(
			position_created_at,
			current_period,
			T::TimePointsPerPeriod::get(),
			T::TimePointsWeight::get(),
			position.action_points,
			T::ActionPointsWeight::get(),
			position.accumulated_slash_points,
		)
		.map_err(|_| ArithmeticError::Overflow)
	}

	#[inline]
	fn get_current_period() -> Result<Period, ArithmeticError> {
		Self::get_period_number(T::BlockNumberProvider::current_block_number())
	}

	#[inline]
	fn get_period_number(block: T::BlockNumber) -> Result<Period, ArithmeticError> {
		//TODO: inconsistent state error
		math::calculate_period_number(T::PeriodLength::get().into(), block).map_err(|_| ArithmeticError::Overflow)
	}

	/// This function calculates `claimable`, `claimable_unpaid`, `unpaid` rewards and `payable_percentage`.
	///
	/// `claimable` - amount use can claim from the `pot`
	/// `claimable_unpaid` - amount to unlock from `accumulated_unpaid_rewards`
	/// `unpaid` - amount of rewards which won't be paid to user
	/// `payable_percentage` - percentage of the rewards that is available to user
	///
	/// Return `(claimable, claimable_unpaid, unpaid, payable_percentage)`
	fn calculate_rewards(
		position: &Position<T::BlockNumber>,
		accumulated_reward_per_stake: FixedU128,
		current_period: Period,
		position_created_at: Period,
	) -> Result<(Balance, Balance, Balance, FixedU128), ArithmeticError> {
		let max_rewards =
			math::calculate_rewards(accumulated_reward_per_stake, position.reward_per_stake, position.stake)
				.map_err(|_| ArithmeticError::Overflow)?;

		if current_period.saturating_sub(position_created_at) <= T::UnclaimablePeriods::get() {
			return Ok((Balance::zero(), Balance::zero(), max_rewards, FixedU128::zero()));
		}

		let points = Self::get_points(position, current_period, position_created_at)?;
		let payable_percentage = T::PayablePercentage::get(points)?;

		let claimable_rewards = payable_percentage
			.checked_mul_int(max_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		let unpaid_rewards = max_rewards
			.checked_sub(claimable_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		let claimable_unpaid_rewards = payable_percentage
			.checked_mul_int(position.accumulated_unpaid_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		Ok((
			claimable_rewards,
			claimable_unpaid_rewards,
			unpaid_rewards,
			payable_percentage,
		))
	}

	//NOTE: this is tmp - will be removed after refactor
	pub fn add_pending_rewards(rewards: Balance) {
		Staking::<T>::try_mutate(|s| -> Result<(), ArithmeticError> {
			s.pending_rew = s.pending_rew + rewards;

			Ok(())
		});
	}

	pub fn process_trade_fee(source: T::AccountId, asset: T::AssetId, amount: Balance) -> DispatchResult {
		Ok(())
	}
}

pub struct SigmoidPercentage<T>(sp_std::marker::PhantomData<T>);

impl<T> PayablePercentage<Point> for SigmoidPercentage<T>
where
	T: Get<FixedU128>,
{
	type Error = ArithmeticError;

	fn get(p: Point) -> Result<FixedU128, Self::Error> {
		let a: FixedU128 = T::get();
		let b: u32 = 40_000;

		math::sigmoid(p, a, b).map_err(|_| ArithmeticError::Overflow)
	}
}

pub struct StakingDemocracy;

impl<AccountId> DemocracyHooks<AccountId, Balance> for StakingDemocracy {
	fn on_vote(who: &AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult {
		Ok(())
	}
}
