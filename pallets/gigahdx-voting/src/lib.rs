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

	/// Maximum number of stuck rewards promoted into `PendingRewards` per `drain_stuck_rewards` call.
	pub(crate) const MAX_DRAIN_PER_CALL: u32 = 16;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_gigahdx::Config {
		/// Native currency (HDX via pallet-balances). Used for HDX portion of voting locks.
		type NativeCurrency: frame_support::traits::LockableCurrency<Self::AccountId, Moment = BlockNumberFor<Self>, Balance = Balance>
			+ frame_support::traits::ReservableCurrency<Self::AccountId, Balance = Balance>
			+ frame_support::traits::fungible::Inspect<Self::AccountId, Balance = Balance>;

		/// Referenda state queries.
		type Referenda: GetReferendumOutcome<u32, BlockNumber = BlockNumberFor<Self>>
			+ GetTrackId<u32, TrackId = u16>;

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

	/// Per-class prior lock split — mirrors pallet-conviction-voting's `prior` field
	/// (one max-aggregate per (account, class)), but two-sided (GIGAHDX + HDX).
	///
	/// Accumulated when a vote on a winning side is removed during the conviction
	/// lock period. Cleared by `rejig` once the prior expires.
	#[pallet::storage]
	#[pallet::getter(fn prior_lock_split)]
	pub type PriorLockSplit<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		u16, // governance class / track id
		PriorSplit<BlockNumberFor<T>>,
		ValueQuery,
	>;

	/// Fallback split snapshot covering paths that don't go through our vote
	/// hooks — primarily `delegate` (and the `undelegate` prior). Conviction-voting
	/// calls `LockableCurrency::extend_lock(amount)` directly for delegation
	/// without firing `on_before_vote`, so we have no per-vote breakdown for the
	/// delegated balance. We snapshot it here at the moment `extend_lock` is
	/// called with an amount that exceeds what active votes + priors already
	/// account for. Single max-aggregate per account, cleared by `remove_lock`.
	#[pallet::storage]
	#[pallet::getter(fn delegation_lock_split)]
	pub type DelegationLockSplit<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, VotingLockSplit, ValueQuery>;

	/// HDX-side spillover from `giga_unstake`. When a user unstakes GIGAHDX
	/// while a prior (or other commitment) still holds G-side locked, the
	/// G-side cap is reduced to fit the post-unstake aToken balance and the
	/// freed commitment moves to the HDX-side native lock. This counter
	/// preserves the total-commitment invariant without mutating per-vote
	/// splits. Cleared by `remove_lock` and `prepare_for_liquidation`.
	#[pallet::storage]
	#[pallet::getter(fn unstake_spillover)]
	pub type UnstakeSpillover<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Balance, ValueQuery>;

	/// Reward pool snapshot per completed referendum.
	#[pallet::storage]
	#[pallet::getter(fn referenda_reward_pool)]
	pub type ReferendaRewardPool<T: Config> = StorageMap<_, Blake2_128Concat, u32, ReferendaReward<T::AccountId>>;

	/// Pending reward entries per account.
	#[pallet::storage]
	#[pallet::getter(fn pending_rewards)]
	pub type PendingRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<PendingRewardEntry, T::MaxVotes>, ValueQuery>;

	/// Dead-letter queue for rewards that could not be inserted into PendingRewards
	/// because it was at capacity. Promote via `drain_stuck_rewards` or on_idle.
	#[pallet::storage]
	pub type StuckRewards<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedVec<PendingRewardEntry, ConstU32<1024>>, ValueQuery>;

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

		/// A reward could not be recorded in `PendingRewards` (at capacity) and was
		/// routed to `StuckRewards`. Promote via `drain_stuck_rewards` or wait for
		/// `on_idle` to do it opportunistically.
		RewardDeferred {
			who: T::AccountId,
			ref_index: u32,
			reward_amount: Balance,
		},

		/// Stuck rewards were promoted into the main queue.
		StuckRewardsDrained { who: T::AccountId, count: u32 },
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
		/// Dead-letter queue full (practically unreachable at cap 1024).
		StuckRewardsFull,
		/// No stuck rewards to drain for the target account.
		NoStuckRewards,
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

		/// Promote stuck rewards from the dead-letter queue into `PendingRewards`.
		///
		/// Permissionless — anyone can call for any target account. Useful after
		/// the account has claimed rewards and made room in `PendingRewards`.
		#[pallet::call_index(1)]
		#[pallet::weight(T::VotingWeightInfo::drain_stuck_rewards())]
		pub fn drain_stuck_rewards(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;

			let mut stuck = StuckRewards::<T>::take(&target);
			ensure!(!stuck.is_empty(), Error::<T>::NoStuckRewards);

			let mut drained: u32 = 0;
			PendingRewards::<T>::try_mutate(&target, |pending| -> DispatchResult {
				while let Some(entry) = stuck.first().cloned() {
					if pending.try_push(entry).is_err() {
						break;
					}
					stuck.remove(0);
					drained = drained.saturating_add(1);
					if drained >= MAX_DRAIN_PER_CALL {
						break;
					}
				}
				Ok(())
			})?;

			if !stuck.is_empty() {
				StuckRewards::<T>::insert(&target, stuck);
			}

			Self::deposit_event(Event::StuckRewardsDrained {
				who: target,
				count: drained,
			});
			Ok(())
		}
	}

	// -----------------------------------------------------------------------
	// Hooks
	// -----------------------------------------------------------------------

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_idle(_n: BlockNumberFor<T>, mut remaining: Weight) -> Weight {
			const PER_ACCOUNT_WEIGHT: Weight = Weight::from_parts(25_000_000, 10_000);
			const MAX_ACCOUNTS_PER_BLOCK: u32 = 5;

			let mut touched: u32 = 0;
			for (who, _) in StuckRewards::<T>::iter() {
				if remaining.any_lt(PER_ACCOUNT_WEIGHT) || touched >= MAX_ACCOUNTS_PER_BLOCK {
					break;
				}
				let _ = Self::drain_stuck_rewards(frame_system::RawOrigin::Signed(who.clone()).into(), who);
				remaining = remaining.saturating_sub(PER_ACCOUNT_WEIGHT);
				touched = touched.saturating_add(1);
			}
			remaining
		}
	}
}

// ---------------------------------------------------------------------------
// View-only `LockSplit::<T>::get(who)` shim. Backed by per-vote splits in
// `GigaHdxVotes` + `PriorLockSplit`, NOT a real storage map. Returned values
// match the effective lock state the user actually faces.
// ---------------------------------------------------------------------------

pub struct LockSplit<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> LockSplit<T> {
	/// Effective per-side lock state — what the user actually faces.
	///
	/// Reads the canonical caps directly:
	/// - `gigahdx_amount`: `GigaHdxVotingLock` (consumed by the 0x0806 EVM lock-manager precompile).
	/// - `hdx_amount`: the user's `pyconvot` entry on `pallet_balances::Locks`.
	pub fn get(who: &T::AccountId) -> VotingLockSplit {
		let gigahdx_amount = GigaHdxVotingLock::<T>::get(who);
		let hdx_amount = Self::pyconvot_native_lock(who);
		VotingLockSplit {
			gigahdx_amount,
			hdx_amount,
		}
	}

	/// H-side max-aggregate over the data sources the adapter wrote into the
	/// `pyconvot` native lock. We can't directly query the lock without
	/// requiring the concrete `pallet_balances` type, so we recompute the same
	/// max-aggregate the adapter does — including UnstakeSpillover.
	fn pyconvot_native_lock(who: &T::AccountId) -> Balance {
		Pallet::<T>::compute_hdx_lock_with_spillover(who)
	}
}

// ---------------------------------------------------------------------------
// Public functions
// ---------------------------------------------------------------------------

impl<T: Config> Pallet<T> {
	/// HDX-side max-aggregate over active votes + priors + delegation snapshot
	/// + unstake spillover. Used by `on_unstake` to apply the H-side native
	/// lock immediately after spilling commitment off the G-side.
	pub fn compute_hdx_lock_with_spillover(who: &T::AccountId) -> Balance {
		let now = frame_system::Pallet::<T>::block_number();
		let mut h_max: Balance = 0;
		for (_ref, v) in GigaHdxVotes::<T>::iter_prefix(who) {
			if v.hdx_lock > h_max {
				h_max = v.hdx_lock;
			}
		}
		for (_class, mut p) in PriorLockSplit::<T>::iter_prefix(who) {
			p.rejig(now);
			if p.is_active() && p.hdx > h_max {
				h_max = p.hdx;
			}
		}
		let delegation = DelegationLockSplit::<T>::get(who);
		if delegation.hdx_amount > h_max {
			h_max = delegation.hdx_amount;
		}
		let spillover = UnstakeSpillover::<T>::get(who);
		if spillover > h_max {
			h_max = spillover;
		}
		h_max
	}

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
					vote.gigahdx_lock.saturating_mul(vote.conviction.reward_multiplier() as u128) / REWARD_MULTIPLIER_SCALE;
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
		let _ = PriorLockSplit::<T>::clear_prefix(who, u32::MAX, None);
		DelegationLockSplit::<T>::remove(who);
		UnstakeSpillover::<T>::remove(who);
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

	fn on_unstake(who: &T::AccountId, gigahdx_amount: Balance) -> DispatchResult {
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

		// Cap GigaHdxVotingLock at the post-unstake aToken balance and spill the
		// remainder to the HDX-side native lock. The 0x0806 LockManager precompile
		// reads GigaHdxVotingLock and would otherwise block AAVE.withdraw later
		// in giga_unstake.
		//
		// This preserves the total-commitment invariant: any G-side cap that no
		// longer fits the user's post-unstake balance moves into UnstakeSpillover,
		// which the adapter folds into the H-side max-aggregate. Per-vote splits
		// stored in GigaHdxVotes / PriorLockSplit are NOT mutated (snapshots
		// remain immutable); the spillover is a separate per-account aggregate.
		use frame_support::traits::fungibles::Inspect as _;
		let current_g_balance =
			<T::Currency as frame_support::traits::fungibles::Inspect<T::AccountId>>::balance(
				<T as pallet_gigahdx::Config>::GigaHdxAssetId::get(),
				who,
			);
		let post_unstake_balance = current_g_balance.saturating_sub(gigahdx_amount);

		// Compute the current G-side max from active votes + priors + delegation snapshot.
		let now = frame_system::Pallet::<T>::block_number();
		let mut g_max: Balance = 0;
		for (_ref, v) in GigaHdxVotes::<T>::iter_prefix(who) {
			if v.gigahdx_lock > g_max {
				g_max = v.gigahdx_lock;
			}
		}
		for (_class, mut p) in PriorLockSplit::<T>::iter_prefix(who) {
			p.rejig(now);
			if p.is_active() && p.gigahdx > g_max {
				g_max = p.gigahdx;
			}
		}
		let delegation = DelegationLockSplit::<T>::get(who);
		if delegation.gigahdx_amount > g_max {
			g_max = delegation.gigahdx_amount;
		}

		if g_max > post_unstake_balance {
			let spill = g_max.saturating_sub(post_unstake_balance);
			UnstakeSpillover::<T>::mutate(who, |s| {
				if spill > *s {
					*s = spill;
				}
			});
			// Pre-shrink the G-side cap so the AAVE precompile permits the burn.
			// The recompute that follows the next vote-adapter call will pick up
			// UnstakeSpillover via the H-side max-aggregate.
			if post_unstake_balance > Zero::zero() {
				GigaHdxVotingLock::<T>::insert(who, post_unstake_balance);
			} else {
				GigaHdxVotingLock::<T>::remove(who);
			}
			// Apply the spillover to the H-side native lock immediately.
			let h_total = Self::compute_hdx_lock_with_spillover(who);
			if h_total > Zero::zero() {
				<T::NativeCurrency as LockableCurrency<T::AccountId>>::set_lock(
					CONVICTION_VOTING_ID,
					who,
					h_total,
					frame_support::traits::WithdrawReasons::all(),
				);
			}
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

	fn on_post_unstake(_who: &T::AccountId) -> DispatchResult {
		// Lock state changes only on vote-related events (vote/remove_vote/unlock),
		// never on plain balance changes. Stake/unstake leave the lock untouched —
		// the per-vote split snapshot stored on `GigaHdxVotes` continues to apply.
		Ok(())
	}
}
