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

use crate::traits::PayablePercentage;
use crate::types::{Balance, Point, Position, StakingData};
use hydra_dx_math::{staking as math, MathError};
use orml_traits::MultiCurrency;
use sp_core::Get;
use sp_runtime::{
	traits::{BlockNumberProvider, Zero},
	ArithmeticError, DispatchResult, Permill,
};
use sp_runtime::{FixedPointNumber, FixedU128};

pub mod traits;
pub mod types;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::HasCompact;
	use frame_support::pallet_prelude::ValueQuery;
	use frame_support::pallet_prelude::*;
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use sp_runtime::traits::AtLeast32BitUnsigned;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

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
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = Self::AssetId, Balance = Balance>;

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

		/// Number of time points users receive for each period.
		#[pallet::constant]
		type TimePointsPerPeriod: Get<u8>;

		/// Number of periods user can't claim rewards for. User can exit but won't receive rewards
		/// but if he stay longer, he will receive rewards also for these periods.
		#[pallet::constant]
		type UnclaimablePeriods: Get<Self::BlockNumber>;

		//TODO: tinkg about better name
		/// Weight of the actual stake in slash points calculation. Bigger the value lower the calculated slash points.
		#[pallet::constant]
		type CurrentStakeWeight: Get<u8>;

		/// Function returning percentage of rewards to pay based on number of points user
		/// accumulated.
		type PayablePercentage: PayablePercentage<Point, Error = ArithmeticError>;

		/// The block number provider
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = Self::BlockNumber>;

		//TODO: points per action. Will there be different amount of points per different action?

		type PositionItemId: Member + Parameter + Default + Copy + HasCompact + AtLeast32BitUnsigned + MaxEncodedLen;
		//TODO: nft stuff
	}

	#[pallet::storage]
	/// Global stakig state.
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
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Balance too low
		InsufficientBalance,

		/// Staked amount is too low
		InsufficientStake,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}

impl<T: Config> Pallet<T> {
	pub fn do_democracy_vote() -> DispatchResult {
		Ok(())
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
			//No pendig rewards or rewards are too small to distribute
			return Ok(());
		}

		staking.accumulated_reward_per_stake = accumulated_rps;

		//TODO: change this to last balance
		staking.pending_rew = 0;

		Ok(())
	}

	/// This function caluclates `claimable`, `claimable_unpaid`, `unpaid` rewards and `payable_percentage`.
	///
	/// `claimable` - amount use can claim from the `pot`
	/// `claimable_unpaid` - amount to unlock from `accumulated_unpaid_rewards`
	/// `unpaid` - amount of rewards which won't be paid to user
	/// `payable_percentage` - percentage of the rewards that is available to user
	///
	/// Return `(cliamable, claimable_unpaid, unpaid, payable_percentage)`
	fn calculate_rewards(
		position: &Position<T::BlockNumber>,
		accumulated_reward_per_stake: FixedU128,
	) -> Result<(Balance, Balance, Balance, FixedU128), ArithmeticError> {
		let max_rewards =
			math::calcutale_rewards(accumulated_reward_per_stake, position.reward_per_stake, position.stake)
				.map_err(|_| ArithmeticError::Overflow)?;

		//TODO: change to inconsistent state error
		let entered_at_period = math::calculate_period_number(T::PeriodLength::get().into(), position.entered_at)
			.map_err(|_| ArithmeticError::Overflow)?;

		//TODO: change to inconsistent state error
		let current_period = math::calculate_period_number(
			T::PeriodLength::get().into(),
			T::BlockNumberProvider::current_block_number(),
		)
		.map_err(|_| ArithmeticError::Overflow)?;

		let unclaimable_periods: u128 =
			TryInto::try_into(T::UnclaimablePeriods::get()).map_err(|_| ArithmeticError::Overflow)?;
		if current_period.saturating_sub(entered_at_period) <= unclaimable_periods {
			return Ok((Balance::zero(), Balance::zero(), max_rewards, FixedU128::zero()));
		}

		let points = math::calculate_points(
			entered_at_period,
			current_period,
			T::TimePointsPerPeriod::get(),
			T::TimePointsWeight::get(),
			position.action_points,
			T::ActionPointsWeight::get(),
			position.accumulated_slash_points,
		)
		.map_err(|e| ArithmeticError::Overflow)?;

		let payable_percentage = T::PayablePercentage::get(points)?;

		let claimable = payable_percentage
			.checked_mul_int(max_rewards)
			.ok_or(ArithmeticError::Overflow)?;
		let unpaid_rewards = max_rewards.checked_sub(claimable).ok_or(ArithmeticError::Overflow)?;
		let claimable_unpaid_rewards = payable_percentage
			.checked_mul_int(position.accumulated_unpaid_rewards)
			.ok_or(ArithmeticError::Overflow)?;

		Ok((claimable, claimable_unpaid_rewards, unpaid_rewards, payable_percentage))
	}
}
