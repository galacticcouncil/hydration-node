//! Lazy reward pool allocation and conviction-weighted reward calculation.
//!
//! When the first vote is removed from a completed referendum, we:
//! 1. Allocate a reward pool (transfer HDX from GigaReward pot to per-referenda sub-account)
//! 2. Calculate user's share: (weighted_vote / total_weighted_votes) * total_reward
//!
//! The total_weighted_votes snapshot is set lazily on first allocation.
//! It equals the sum of all remaining weighted votes plus the current voter's weighted value
//! (since on_remove_vote already subtracted the current voter before calling us).

use crate::types::{GigaHdxVote, PendingRewardEntry, ReferendaReward, REWARD_MULTIPLIER_SCALE};
use crate::{
	Config, Error, Event, Pallet, PendingRewards, ReferendaRewardPool, ReferendaTotalWeightedVotes, RewardAllocated,
	StuckRewards,
};
use frame_support::pallet_prelude::Get;
use frame_support::traits::{fungibles::Mutate, tokens::Preservation};
use hydradx_traits::gigahdx::{GetTrackId, TrackRewardConfig};
use primitives::Balance;
use sp_arithmetic::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{traits::Zero, Rounding};

/// Allocate reward pool (if needed) and record user's reward.
/// Called from on_remove_vote when status is Completed.
///
/// Note: at this point, the caller (hooks.rs) has already subtracted this vote's
/// weighted value from ReferendaTotalWeightedVotes. We need to account for this
/// when snapshotting the total.
pub fn maybe_allocate_and_record<T: Config>(
	who: &T::AccountId,
	ref_index: u32,
	vote: &GigaHdxVote<frame_system::pallet_prelude::BlockNumberFor<T>>,
) -> Result<(), sp_runtime::DispatchError> {
	// Reward weight uses the GIGAHDX-side portion only — HDX-only vote balance does not
	// contribute to GIGAHDX referenda rewards.
	let weighted_vote =
		vote.gigahdx_lock.saturating_mul(vote.conviction.reward_multiplier() as u128) / REWARD_MULTIPLIER_SCALE;

	maybe_allocate_reward_pool::<T>(ref_index, weighted_vote)?;
	record_user_reward::<T>(who, ref_index, weighted_vote)?;
	Ok(())
}

/// Allocate the reward pool for a referendum (lazy, once).
/// `caller_weighted` is the weighted vote of the voter triggering allocation
/// (already subtracted from ReferendaTotalWeightedVotes by the caller).
fn maybe_allocate_reward_pool<T: Config>(
	ref_index: u32,
	caller_weighted: Balance,
) -> Result<(), sp_runtime::DispatchError> {
	if RewardAllocated::<T>::get(ref_index) {
		return Ok(());
	}

	// Use cached track ID first (populated in on_before_vote), with fallback
	// to live lookup (works while referendum is still Ongoing).
	let track_id =
		crate::ReferendumTracks::<T>::get(ref_index).or_else(|| <T::Referenda as GetTrackId<u32>>::track_id(ref_index));
	let percentage = match track_id {
		Some(tid) => <T::TrackRewards as TrackRewardConfig>::reward_percentage(tid),
		None => return Ok(()),
	};

	let pot_account = Pallet::<T>::giga_reward_pot_account();
	let pot_balance = <T::Currency as frame_support::traits::fungibles::Inspect<T::AccountId>>::balance(
		<T as pallet_gigahdx::Config>::HdxAssetId::get(),
		&pot_account,
	);

	if pot_balance.is_zero() {
		RewardAllocated::<T>::insert(ref_index, true);
		return Ok(());
	}

	let allocation = percentage * pot_balance;
	if allocation.is_zero() {
		RewardAllocated::<T>::insert(ref_index, true);
		return Ok(());
	}

	let referenda_pot = Pallet::<T>::referenda_pot_account(ref_index);

	// Transfer HDX from main reward pot to per-referenda sub-account.
	<T::Currency as Mutate<T::AccountId>>::transfer(
		<T as pallet_gigahdx::Config>::HdxAssetId::get(),
		&pot_account,
		&referenda_pot,
		allocation,
		Preservation::Expendable,
	)?;

	// Snapshot total weighted votes.
	// ReferendaTotalWeightedVotes has already had caller's vote subtracted,
	// so we re-add it to get the true total at referendum completion.
	let remaining_weighted = ReferendaTotalWeightedVotes::<T>::get(ref_index);
	let total_weighted = remaining_weighted.saturating_add(caller_weighted);

	ReferendaRewardPool::<T>::insert(
		ref_index,
		ReferendaReward {
			track_id: track_id.unwrap_or(0),
			total_reward: allocation,
			total_weighted_votes: total_weighted,
			remaining_reward: allocation,
			pot_account: referenda_pot,
		},
	);

	RewardAllocated::<T>::insert(ref_index, true);

	Pallet::<T>::deposit_event(Event::RewardPoolAllocated {
		ref_index,
		track_id: track_id.unwrap_or(0),
		total_reward: allocation,
	});

	Ok(())
}

/// Record a user's reward share for a completed referendum.
fn record_user_reward<T: Config>(
	who: &T::AccountId,
	ref_index: u32,
	weighted_vote: Balance,
) -> Result<(), sp_runtime::DispatchError> {
	let Some(mut pool) = ReferendaRewardPool::<T>::get(ref_index) else {
		return Ok(());
	};

	if pool.total_reward.is_zero() || pool.remaining_reward.is_zero() || pool.total_weighted_votes.is_zero() {
		return Ok(());
	}

	if weighted_vote.is_zero() {
		return Ok(());
	}

	// user_share = weighted_vote * total_reward / total_weighted_votes
	let user_reward = multiply_by_rational_with_rounding(
		weighted_vote,
		pool.total_reward,
		pool.total_weighted_votes,
		Rounding::Down,
	)
	.unwrap_or(0);

	// Cap at remaining reward.
	let user_reward = user_reward.min(pool.remaining_reward);

	if user_reward.is_zero() {
		return Ok(());
	}

	pool.remaining_reward = pool.remaining_reward.saturating_sub(user_reward);
	ReferendaRewardPool::<T>::insert(ref_index, pool);

	// Push pending reward entry, routing to dead-letter queue on overflow.
	let entry = PendingRewardEntry {
		referenda_id: ref_index,
		reward_amount: user_reward,
	};

	let push_result = PendingRewards::<T>::try_mutate(who, |entries| entries.try_push(entry.clone()));

	if push_result.is_err() {
		// PendingRewards full — route to dead-letter queue.
		StuckRewards::<T>::try_mutate(who, |entries| entries.try_push(entry.clone()))
			.map_err(|_| Error::<T>::StuckRewardsFull)?;
		Pallet::<T>::deposit_event(Event::RewardDeferred {
			who: who.clone(),
			ref_index,
			reward_amount: user_reward,
		});
		return Ok(());
	}

	Pallet::<T>::deposit_event(Event::RewardRecorded {
		who: who.clone(),
		ref_index,
		reward_amount: user_reward,
	});

	Ok(())
}
