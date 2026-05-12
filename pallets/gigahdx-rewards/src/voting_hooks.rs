// SPDX-License-Identifier: Apache-2.0

//! `VotingHooks` integration for `pallet-gigahdx-rewards`.
//!
//! [`VotingHooksImpl`] is the pallet's own `VotingHooks` implementation:
//! it snapshots eligible votes into storage and freezes the corresponding
//! gigahdx stake. The runtime is responsible for combining this hook with
//! any other `VotingHooks` consumer (typically staking) when wiring
//! `pallet-conviction-voting::Config::VotingHooks`.

use crate::pallet::{
	Config, Event, Pallet, ReferendaRewardPool, ReferendaTotalWeightedVotes, ReferendumTracks, UserVoteRecords,
};
use crate::traits::{ReferendaTrackInspect, TrackRewardTable};
use crate::types::{ReferendaReward, ReferendumIndex, ReferendumLiveTally, UserVoteRecord};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::{Currency, ExistenceRequirement};
use pallet_conviction_voting::{AccountVote, Status, VotingHooks};
use primitives::Balance;
use sp_std::marker::PhantomData;

/// `VotingHooks` impl for this pallet. The runtime wires this through a
/// tuple adapter alongside any other consumer (typically staking).
pub struct VotingHooksImpl<T>(PhantomData<T>);

impl<T: Config> VotingHooks<T::AccountId, ReferendumIndex, Balance> for VotingHooksImpl<T> {
	fn on_before_vote(who: &T::AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult {
		// Voting hooks must never block voting — saturating arithmetic and
		// `Ok(())` on every defensive branch.
		let staked = pallet_gigahdx::Stakes::<T>::get(who).map(|s| s.hdx).unwrap_or(0);
		if staked == 0 {
			return Ok(());
		}

		// Standard votes only; Split / SplitAbstain have multiple sub-balances
		// without a single principled answer for the eligible amount.
		let (vote_balance, conviction) = match vote {
			AccountVote::Standard {
				vote: std_vote,
				balance,
			} => (balance, std_vote.conviction),
			_ => return Ok(()),
		};

		let staked_vote = vote_balance.min(staked);
		let weighted = Pallet::<T>::weighted(staked_vote, conviction);
		let new_record = UserVoteRecord {
			staked_vote_amount: staked_vote,
			conviction,
			weighted,
		};

		// Already-allocated refs cannot accept new votes (referendum is past
		// Completed), but defensive: cache the track id only while the live
		// tally is still active.
		let live_tally_active = !ReferendaRewardPool::<T>::contains_key(ref_index);
		if live_tally_active && !ReferendumTracks::<T>::contains_key(ref_index) {
			if let Some(track) = T::Referenda::track_of(ref_index) {
				ReferendumTracks::<T>::insert(ref_index, track);
			}
		}

		// Diff against any previous record for (who, ref).
		let prev = UserVoteRecords::<T>::get(who, ref_index);
		match prev {
			Some(prev) => {
				// Edit: unfreeze old, freeze new; voter count unchanged.
				pallet_gigahdx::Pallet::<T>::unfreeze(who, prev.staked_vote_amount);
				if live_tally_active {
					ReferendaTotalWeightedVotes::<T>::mutate_exists(ref_index, |maybe| {
						let tally = maybe.get_or_insert_with(ReferendumLiveTally::default);
						tally.total_weighted = tally.total_weighted.saturating_sub(prev.weighted);
						tally.total_weighted = tally.total_weighted.saturating_add(weighted);
					});
				}
			}
			None => {
				// New record: increment voter count.
				if live_tally_active {
					ReferendaTotalWeightedVotes::<T>::mutate_exists(ref_index, |maybe| {
						let tally = maybe.get_or_insert_with(ReferendumLiveTally::default);
						tally.total_weighted = tally.total_weighted.saturating_add(weighted);
						tally.voters_count = tally.voters_count.saturating_add(1);
					});
				}
			}
		}
		UserVoteRecords::<T>::insert(who, ref_index, new_record);
		pallet_gigahdx::Pallet::<T>::freeze(who, staked_vote);

		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex, status: Status) {
		let Some(record) = UserVoteRecords::<T>::take(who, ref_index) else {
			return; // no eligible vote was tracked
		};
		pallet_gigahdx::Pallet::<T>::unfreeze(who, record.staked_vote_amount);

		// Maintain the live tally only while the ref is still pre-allocation.
		// Pool presence = "allocation has run" idempotency signal.
		if !ReferendaRewardPool::<T>::contains_key(ref_index) {
			ReferendaTotalWeightedVotes::<T>::mutate_exists(ref_index, |maybe| {
				if let Some(tally) = maybe.as_mut() {
					tally.total_weighted = tally.total_weighted.saturating_sub(record.weighted);
					tally.voters_count = tally.voters_count.saturating_sub(1);
					if tally.voters_count == 0 {
						*maybe = None;
					}
				}
			});
		}

		if !matches!(status, Status::Completed) {
			return;
		}

		let _ = maybe_allocate_and_record::<T>(who, ref_index, &record);
	}

	fn lock_balance_on_unsuccessful_vote(_who: &T::AccountId, _ref_index: ReferendumIndex) -> Option<Balance> {
		// Rewards never locks user balance — it operates on the `frozen`
		// field of the gigahdx stake record. Letting the tuple's `or`
		// fallback pass through whatever the other hook (staking) says.
		None
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &T::AccountId) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &T::AccountId) {}
}

/// Allocate the pool on first call for a completed referendum, then credit
/// the caller's per-user share. Idempotent on the allocation step (subsequent
/// callers see `ReferendaRewardPool[ref_index]` populated and skip).
fn maybe_allocate_and_record<T: Config>(
	who: &T::AccountId,
	ref_index: ReferendumIndex,
	record: &UserVoteRecord,
) -> Result<(), frame_support::sp_runtime::DispatchError> {
	if !ReferendaRewardPool::<T>::contains_key(ref_index) {
		let track_id = ReferendumTracks::<T>::get(ref_index).or_else(|| T::Referenda::track_of(ref_index));
		let Some(track_id) = track_id else {
			// No track resolvable — nothing to allocate. Drop silently
			// (this voter forfeits any reward), but keep the live tally
			// intact for any other voters who might still complete.
			return Ok(());
		};

		let pct = T::TrackRewardConfig::reward_percentage(track_id);
		let pot_balance =
			<T as pallet_gigahdx::Config>::NativeCurrency::free_balance(&Pallet::<T>::reward_accumulator_pot());
		let allocation: Balance = pct * pot_balance;

		if allocation > 0 {
			<T as pallet_gigahdx::Config>::NativeCurrency::transfer(
				&Pallet::<T>::reward_accumulator_pot(),
				&Pallet::<T>::allocated_rewards_pot(),
				allocation,
				ExistenceRequirement::AllowDeath,
			)?;
		}

		// Re-add the caller's contribution to the snapshot — `on_remove_vote`
		// already subtracted it from the live tally before delegating here.
		let live = ReferendaTotalWeightedVotes::<T>::get(ref_index).unwrap_or_default();
		let total_weighted = live.total_weighted.saturating_add(record.weighted);
		let voters_remaining = live.voters_count.saturating_add(1);

		ReferendaRewardPool::<T>::insert(
			ref_index,
			ReferendaReward {
				track_id: track_id.clone(),
				total_reward: allocation,
				total_weighted_votes: total_weighted,
				voters_remaining,
				remaining_reward: allocation,
			},
		);

		ReferendaTotalWeightedVotes::<T>::remove(ref_index);
		ReferendumTracks::<T>::remove(ref_index);

		Pallet::<T>::deposit_event(Event::<T>::RewardPoolAllocated {
			ref_index,
			track_id,
			total_reward: allocation,
			total_weighted_votes: total_weighted,
			voters_remaining,
		});
	}

	Pallet::<T>::record_user_reward(who, ref_index, record)?;
	Ok(())
}
