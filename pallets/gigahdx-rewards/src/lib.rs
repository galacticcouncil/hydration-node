// This file is part of https://github.com/galacticcouncil/hydration-node

// Copyright (C) 2025  Intergalactic, Limited (GIB).
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

//! # pallet-gigahdx-rewards
//!
//! Distributes HDX rewards to `pallet-gigahdx` stakers based on their
//! conviction-voting activity.
//!
//! - Two HDX-only pot accounts: an accumulator (externally funded) and an
//!   allocated pot that holds per-referendum allocations.
//! - On every vote by a gigahdx staker, the reward weight is snapshotted
//!   into `UserVoteRecords` and the corresponding stake is frozen on the
//!   gigahdx side to prevent stake → vote → unstake exploits.
//! - First `on_remove_vote` for a completed referendum lazily transfers
//!   `track_pct × accumulator_balance` into the allocated pot and snapshots
//!   the frozen denominator.
//! - Per-user shares are pro-rata against that frozen denominator. The last
//!   claimant scoops any remaining dust, draining the pool to exactly zero
//!   and triggering storage cleanup.
//! - `claim_rewards` atomically compounds the user's accumulated HDX back
//!   into their gigahdx position.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod traits;
pub mod types;
pub mod voting_hooks;
pub mod weights;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

#[frame_support::pallet]
pub mod pallet {
	use crate::traits::{ReferendaTrackInspect, TrackRewardTable};
	use crate::types::{ReferendaReward, ReferendumIndex, ReferendumLiveTally, UserVoteRecord};
	pub use crate::weights::WeightInfo;
	use codec::HasCompact;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use frame_support::sp_runtime::traits::AccountIdConversion;
	use frame_support::sp_runtime::Rounding;
	use frame_support::traits::{Currency, ExistenceRequirement};
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
	use primitives::Balance;
	use scale_info::TypeInfo;
	use sp_std::fmt::Debug;

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> + pallet_gigahdx::Config {
		type TrackId: Parameter
			+ Member
			+ Copy
			+ MaybeSerializeDeserialize
			+ Debug
			+ MaxEncodedLen
			+ TypeInfo
			+ Ord
			+ HasCompact;

		/// Lookup of referendum → track id.
		type Referenda: ReferendaTrackInspect<ReferendumIndex, Self::TrackId>;

		/// Track id → reward percentage table.
		type TrackRewardConfig: TrackRewardTable<Self::TrackId>;

		/// PalletId of the externally-funded accumulator pot. Source of
		/// every per-referendum allocation. The allocated-rewards pot is
		/// derived deterministically as a sub-account of this id.
		#[pallet::constant]
		type RewardPotPalletId: Get<PalletId>;

		type WeightInfo: WeightInfo;
	}

	/// Live tally maintained during the voting period. Deleted at allocation
	/// time; the values move into `ReferendaRewardPool`.
	#[pallet::storage]
	pub type ReferendaTotalWeightedVotes<T: Config> =
		StorageMap<_, Blake2_128Concat, ReferendumIndex, ReferendumLiveTally, OptionQuery>;

	/// Track id cached at vote time. Deleted at allocation; lives on in
	/// `ReferendaRewardPool[ref].track_id`.
	#[pallet::storage]
	pub type ReferendumTracks<T: Config> = StorageMap<_, Blake2_128Concat, ReferendumIndex, T::TrackId, OptionQuery>;

	/// Per-referendum frozen snapshot. Presence doubles as "allocation has
	/// run." Deleted when the last voter claims their share.
	#[pallet::storage]
	pub type ReferendaRewardPool<T: Config> =
		StorageMap<_, Blake2_128Concat, ReferendumIndex, ReferendaReward<T::TrackId>, OptionQuery>;

	/// Per (user, referendum) snapshot of the eligible vote weight at cast
	/// time. Updated on vote edits; taken on `on_remove_vote`.
	#[pallet::storage]
	pub type UserVoteRecords<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		ReferendumIndex,
		UserVoteRecord,
		OptionQuery,
	>;

	/// Running sum of HDX owed to `who` across all completed referenda.
	#[pallet::storage]
	pub type PendingRewards<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Rewards for a completed referendum have been allocated from the
		/// accumulator pot.
		RewardPoolAllocated {
			ref_index: ReferendumIndex,
			track_id: T::TrackId,
			total_reward: Balance,
			total_weighted_votes: u128,
			voters_remaining: u32,
		},
		/// A user's share of a referendum's reward pool was added to their
		/// `PendingRewards`.
		UserRewardRecorded {
			who: T::AccountId,
			ref_index: ReferendumIndex,
			reward_amount: Balance,
		},
		/// A user converted their accumulated HDX rewards into GIGAHDX via
		/// `claim_rewards`.
		RewardsClaimed {
			who: T::AccountId,
			total_hdx: Balance,
			gigahdx_received: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `PendingRewards[who]` is zero.
		NoPendingRewards,
		/// Not enough rewards in the allocated pot to cover the amount owed.
		PotInsufficient,
		/// Arithmetic overflow during share computation.
		Overflow,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Convert the caller's accumulated HDX rewards into GIGAHDX by staking
		/// them on the caller's behalf.
		///
		/// The full `PendingRewards[who]` sum is taken atomically. The HDX is
		/// transferred from the allocated-rewards pot to the caller's free
		/// balance, then `pallet_gigahdx::do_stake` is called to mint GIGAHDX
		/// into the caller's stake position.
		///
		/// Parameters:
		/// - `origin`: signed by the user claiming.
		///
		/// Emits `RewardsClaimed` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let total = PendingRewards::<T>::take(&who);
			ensure!(total > 0, Error::<T>::NoPendingRewards);

			<T as pallet_gigahdx::Config>::NativeCurrency::transfer(
				&Self::allocated_rewards_pot(),
				&who,
				total,
				ExistenceRequirement::AllowDeath,
			)
			.map_err(|_| Error::<T>::PotInsufficient)?;

			let stake_before = pallet_gigahdx::Stakes::<T>::get(&who).map(|s| s.gigahdx).unwrap_or(0);
			pallet_gigahdx::Pallet::<T>::do_stake(&who, total)?;
			let stake_after = pallet_gigahdx::Stakes::<T>::get(&who).map(|s| s.gigahdx).unwrap_or(0);
			let gigahdx_received = stake_after.saturating_sub(stake_before);

			Self::deposit_event(Event::RewardsClaimed {
				who,
				total_hdx: total,
				gigahdx_received,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Account id of the externally-funded accumulator pot.
		pub fn reward_accumulator_pot() -> T::AccountId {
			T::RewardPotPalletId::get().into_account_truncating()
		}

		/// Account id of the allocated-rewards pot. Derived deterministically
		/// as a sub-account of the accumulator pot. The discriminator is
		/// non-zero so the sub-account does not collide with the parent
		/// (zero-padded) account.
		pub fn allocated_rewards_pot() -> T::AccountId {
			T::RewardPotPalletId::get().into_sub_account_truncating(*b"alc")
		}

		/// Compute weighted contribution: `staked_vote × multiplier / 10`,
		/// where multiplier follows the reward table in `crate::types`.
		pub(crate) fn weighted(staked_vote: Balance, conviction: pallet_conviction_voting::Conviction) -> u128 {
			let mult = crate::types::conviction_reward_multiplier(conviction);
			multiply_by_rational_with_rounding(staked_vote, mult, crate::types::REWARD_MULTIPLIER_SCALE, Rounding::Down)
				.unwrap_or(0)
		}

		/// Per-user share: pro-rata weighted vote against the frozen pool,
		/// with last-claimer dust scoop. Returns the amount credited to
		/// `PendingRewards[who]`. Mutates / deletes `ReferendaRewardPool` as
		/// required.
		pub(crate) fn record_user_reward(
			who: &T::AccountId,
			ref_index: ReferendumIndex,
			record: &UserVoteRecord,
		) -> Result<Balance, DispatchError> {
			let Some(mut pool) = ReferendaRewardPool::<T>::take(ref_index) else {
				return Ok(0);
			};

			debug_assert!(pool.voters_remaining > 0, "record_user_reward with empty pool");
			if pool.voters_remaining == 0 {
				return Ok(0);
			}
			pool.voters_remaining = pool.voters_remaining.saturating_sub(1);

			let user_reward: Balance = if pool.voters_remaining == 0 {
				let r = pool.remaining_reward;
				pool.remaining_reward = 0;
				r
			} else if pool.total_weighted_votes == 0 {
				0
			} else {
				let share = multiply_by_rational_with_rounding(
					record.weighted,
					pool.total_reward,
					pool.total_weighted_votes,
					Rounding::Down,
				)
				.ok_or(Error::<T>::Overflow)?;
				let capped = share.min(pool.remaining_reward);
				pool.remaining_reward = pool.remaining_reward.saturating_sub(capped);
				capped
			};

			if user_reward > 0 {
				PendingRewards::<T>::mutate(who, |b| *b = b.saturating_add(user_reward));
				Self::deposit_event(Event::UserRewardRecorded {
					who: who.clone(),
					ref_index,
					reward_amount: user_reward,
				});
			}

			if pool.voters_remaining > 0 {
				ReferendaRewardPool::<T>::insert(ref_index, pool);
			}

			Ok(user_reward)
		}
	}
}
