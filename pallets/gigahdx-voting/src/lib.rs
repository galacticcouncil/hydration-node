// This file is part of HydraDX.
// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

//! # GIGAHDX Voting Pallet
//!
//! Voting adapter + conviction-weighted referenda rewards for GIGAHDX.
//!
//! ## Overview
//!
//! This pallet provides:
//! - Custom Currency adapter for conviction-voting (combined GIGAHDX + HDX balance)
//! - VotingHooks implementation to track GIGAHDX votes
//! - Lazy reward pool allocation per referendum
//! - Conviction-weighted reward distribution
//! - GigaHdxHooks implementation for pallet-gigahdx lifecycle
//!
//! ## Extrinsics
//! * `claim_rewards` - Claim pending referenda rewards (HDX → GIGAHDX via stake_rewards)

#![cfg_attr(not(feature = "std"), no_std)]

pub mod adapter;
pub mod combinator;
pub mod hooks;
pub mod rewards;
pub mod types;
pub mod weights;

#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use frame_support::{
	pallet_prelude::*,
	traits::{tokens::Preservation, LockIdentifier, LockableCurrency},
	PalletId,
};
use frame_system::pallet_prelude::*;
use hydradx_traits::gigahdx::{ForceRemoveVote, GetReferendumOutcome, GetTrackId, TrackRewardConfig};
use primitives::Balance;
use sp_runtime::traits::{AccountIdConversion, Zero};

use types::*;

/// pallet-conviction-voting's lock identifier. It's declared there as a private
/// constant so we redeclare it here to force-clear pallet-balances HDX locks
/// during liquidation.
const CONVICTION_VOTING_ID: LockIdentifier = *b"pyconvot";

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_gigahdx::Config {
		/// Native currency (HDX via pallet-balances). Used for HDX portion of voting locks.
		type NativeCurrency: frame_support::traits::LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>, Balance = Balance>
			+ frame_support::traits::ReservableCurrency<Self::AccountId, Balance = Balance>
			+ frame_support::traits::fungible::Inspect<Self::AccountId, Balance = Balance>;

		/// Referenda state queries.
		type Referenda: GetReferendumOutcome<u32> + GetTrackId<u32, TrackId = u16>;

		/// Per-track reward percentage configuration.
		type TrackRewards: TrackRewardConfig;

		/// Force-remove a vote from conviction-voting on behalf of a user.
		type ForceRemoveVote: ForceRemoveVote<Self::AccountId>;

		/// PalletId for the GigaReward pot.
		#[pallet::constant]
		type GigaRewardPotId: Get<PalletId>;

		/// Base locking period for conviction voting.
		#[pallet::constant]
		type VoteLockingPeriod: Get<BlockNumberFor<Self>>;

		/// Maximum active votes per account.
		#[pallet::constant]
		type MaxVotes: Get<u32>;

		/// Weight information for this pallet's extrinsics.
		type VotingWeightInfo: crate::WeightInfo;
	}

	// -----------------------------------------------------------------------
	// Storage
	// -----------------------------------------------------------------------

	/// GIGAHDX votes per account per referendum.
	#[pallet::storage]
	#[pallet::getter(fn gigahdx_votes)]
	pub type GigaHdxVotes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		u32, // referendum index
		GigaHdxVote<BlockNumberFor<T>>,
	>;

	/// Total weighted votes per referendum (sum of amount * conviction_multiplier).
	#[pallet::storage]
	#[pallet::getter(fn referenda_total_weighted_votes)]
	pub type ReferendaTotalWeightedVotes<T: Config> = StorageMap<_, Blake2_128Concat, u32, Balance, ValueQuery>;

	/// GIGAHDX voting lock per account (read by EVM precompile at 0x0806).
	#[pallet::storage]
	#[pallet::getter(fn gigahdx_voting_lock)]
	pub type GigaHdxVotingLock<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Lock split per account: how much of the total lock is in GIGAHDX vs HDX.
	#[pallet::storage]
	#[pallet::getter(fn lock_split)]
	pub type LockSplit<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, VotingLockSplit, ValueQuery>;

	/// Reward pool snapshot per completed referendum.
	#[pallet::storage]
	#[pallet::getter(fn referenda_reward_pool)]
	pub type ReferendaRewardPool<T: Config> = StorageMap<_, Blake2_128Concat, u32, ReferendaReward<T::AccountId>>;

	/// Pending reward entries per account.
	#[pallet::storage]
	#[pallet::getter(fn pending_rewards)]
	pub type PendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<PendingRewardEntry, T::MaxVotes>, ValueQuery>;

	/// Whether reward pool has been allocated for a referendum.
	#[pallet::storage]
	#[pallet::getter(fn reward_allocated)]
	pub type RewardAllocated<T: Config> = StorageMap<_, Blake2_128Concat, u32, bool, ValueQuery>;

	/// Cached track ID per referendum (needed because pallet-referenda drops track
	/// from storage once a referendum completes).
	#[pallet::storage]
	pub type ReferendumTracks<T: Config> = StorageMap<_, Blake2_128Concat, u32, u16>;

	// -----------------------------------------------------------------------
	// Events
	// -----------------------------------------------------------------------

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// GIGAHDX vote recorded for a referendum.
		VoteRecorded {
			who: T::AccountId,
			ref_index: u32,
			amount: Balance,
			conviction: Conviction,
		},

		/// GIGAHDX vote removed for a referendum.
		VoteRemoved { who: T::AccountId, ref_index: u32 },

		/// Reward pool allocated for a completed referendum.
		RewardPoolAllocated {
			ref_index: u32,
			track_id: u16,
			total_reward: Balance,
		},

		/// Individual reward recorded for a user (pending claim).
		RewardRecorded {
			who: T::AccountId,
			ref_index: u32,
			reward_amount: Balance,
		},

		/// Pending rewards claimed and converted to GIGAHDX.
		RewardsClaimed {
			who: T::AccountId,
			total_hdx: Balance,
			gigahdx_received: Balance,
		},

		/// Voting lock updated for an account.
		LockUpdated {
			who: T::AccountId,
			gigahdx_locked: Balance,
			hdx_locked: Balance,
		},

		/// Votes force-removed during unstake or liquidation.
		VotesForceRemoved { who: T::AccountId, count: u32 },
	}

	// -----------------------------------------------------------------------
	// Errors
	// -----------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Zero amount not allowed.
		ZeroAmount,
		/// Arithmetic overflow/underflow.
		Arithmetic,
		/// No pending rewards to claim.
		NoPendingRewards,
		/// Reward pool already allocated for this referendum.
		RewardPoolAlreadyAllocated,
		/// Maximum active votes reached for account.
		MaxVotesReached,
		/// Active votes in ongoing referenda prevent unstaking.
		ActiveVotesPreventUnstake,
	}

	// -----------------------------------------------------------------------
	// Extrinsics
	// -----------------------------------------------------------------------

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Claim all pending referenda rewards.
		/// HDX is transferred from per-referenda sub-accounts to gigapot,
		/// then converted to GIGAHDX via stake_rewards.
		#[pallet::call_index(0)]
		#[pallet::weight(T::VotingWeightInfo::claim_rewards())]
		pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let entries = PendingRewards::<T>::take(&who);
			ensure!(!entries.is_empty(), Error::<T>::NoPendingRewards);

			let gigapot = pallet_gigahdx::Pallet::<T>::gigapot_account_id();
			let hdx_asset = <T as pallet_gigahdx::Config>::HdxAssetId::get();
			let mut total_hdx: Balance = 0;

			for entry in entries.iter() {
				if entry.reward_amount.is_zero() {
					continue;
				}

				let pot_account = Self::referenda_pot_account(entry.referenda_id);

				// Transfer HDX from per-referenda pot to gigapot.
				<T::Currency as frame_support::traits::fungibles::Mutate<T::AccountId>>::transfer(
					hdx_asset,
					&pot_account,
					&gigapot,
					entry.reward_amount,
					Preservation::Expendable,
				)?;

				total_hdx = total_hdx
					.checked_add(entry.reward_amount)
					.ok_or(Error::<T>::Arithmetic)?;
			}

			ensure!(!total_hdx.is_zero(), Error::<T>::NoPendingRewards);

			// Convert HDX to GIGAHDX via pallet-gigahdx.
			let gigahdx_received = pallet_gigahdx::Pallet::<T>::stake_rewards(&who, total_hdx)?;

			Self::deposit_event(Event::RewardsClaimed {
				who,
				total_hdx,
				gigahdx_received,
			});

			Ok(())
		}
	}
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

impl<T: Config> Pallet<T> {
	/// GigaReward pot account (main reward pool).
	pub fn giga_reward_pot_account() -> T::AccountId {
		T::GigaRewardPotId::get().into_account_truncating()
	}

	/// Per-referenda sub-account for reward allocation.
	pub fn referenda_pot_account(ref_index: u32) -> T::AccountId {
		T::GigaRewardPotId::get().into_sub_account_truncating(ref_index)
	}

	/// Force-remove ALL votes for an account (for liquidation).
	/// Removes all tracked votes — no rewards for ongoing referenda.
	pub fn prepare_for_liquidation(who: &T::AccountId) -> DispatchResult {
		let votes: sp_std::vec::Vec<(u32, GigaHdxVote<BlockNumberFor<T>>)> =
			GigaHdxVotes::<T>::iter_prefix(who).collect();

		let count = votes.len() as u32;

		for (ref_index, vote) in votes.iter() {
			let track_id = <T::Referenda as GetTrackId<u32>>::track_id(*ref_index);
			T::ForceRemoveVote::remove_vote(who, track_id, *ref_index)?;

			// Clean up our storage in case on_remove_vote didn't clear it
			// (e.g., if ForceRemoveVote doesn't trigger VotingHooks).
			if GigaHdxVotes::<T>::contains_key(who, ref_index) {
				let weighted =
					vote.amount.saturating_mul(vote.conviction.reward_multiplier() as u128) / REWARD_MULTIPLIER_SCALE;
				ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
					*total = total.saturating_sub(weighted);
				});
				GigaHdxVotes::<T>::remove(who, ref_index);
			}
		}

		// Force-clear the voting lock storage regardless of conviction-period
		// expiry. The EVM precompile at 0x0806 reads GigaHdxVotingLock and the
		// LockableAToken blocks any transfer while it's non-zero — including
		// the liquidation seize. The HDX-side pallet-balances lock is removed
		// too because liquidation cancels the user's governance commitment.
		GigaHdxVotingLock::<T>::remove(who);
		LockSplit::<T>::remove(who);
		<T::NativeCurrency as LockableCurrency<T::AccountId>>::remove_lock(CONVICTION_VOTING_ID, who);

		if count > 0 {
			Self::deposit_event(Event::VotesForceRemoved {
				who: who.clone(),
				count,
			});
		}

		Ok(())
	}
}

// ---------------------------------------------------------------------------
// PrepareForLiquidation implementation
// ---------------------------------------------------------------------------

impl<T: Config> hydradx_traits::gigahdx::PrepareForLiquidation<T::AccountId> for Pallet<T> {
	fn prepare_for_liquidation(who: &T::AccountId) -> DispatchResult {
		Self::prepare_for_liquidation(who)
	}
}

// ---------------------------------------------------------------------------
// GigaHdxHooks implementation
// ---------------------------------------------------------------------------

use hydradx_traits::gigahdx::GigaHdxHooks;

impl<T: Config> GigaHdxHooks<T::AccountId, Balance, BlockNumberFor<T>> for Pallet<T> {
	fn on_stake(_who: &T::AccountId, _hdx_amount: Balance, _gigahdx_received: Balance) -> DispatchResult {
		Ok(())
	}

	fn can_unstake(who: &T::AccountId) -> bool {
		for (ref_index, _vote) in GigaHdxVotes::<T>::iter_prefix(who) {
			if !<T::Referenda as GetReferendumOutcome<u32>>::is_referendum_finished(ref_index) {
				return false;
			}
		}
		true
	}

	fn on_unstake(who: &T::AccountId, _gigahdx_amount: Balance) -> DispatchResult {
		let finished_votes: sp_std::vec::Vec<u32> = GigaHdxVotes::<T>::iter_prefix(who)
			.filter(|(ref_index, _)| <T::Referenda as GetReferendumOutcome<u32>>::is_referendum_finished(*ref_index))
			.map(|(ref_index, _)| ref_index)
			.collect();

		let count = finished_votes.len() as u32;

		for ref_index in finished_votes {
			let track_id = <T::Referenda as GetTrackId<u32>>::track_id(ref_index);
			T::ForceRemoveVote::remove_vote(who, track_id, ref_index)?;
		}

		if count > 0 {
			Self::deposit_event(Event::VotesForceRemoved {
				who: who.clone(),
				count,
			});
		}

		Ok(())
	}

	fn additional_unstake_lock(who: &T::AccountId) -> BlockNumberFor<T> {
		let current_block = frame_system::Pallet::<T>::block_number();

		GigaHdxVotes::<T>::iter_prefix(who)
			.map(|(_, vote)| {
				if vote.lock_expires_at > current_block {
					vote.lock_expires_at - current_block
				} else {
					Zero::zero()
				}
			})
			.max()
			.unwrap_or_else(Zero::zero)
	}
}
