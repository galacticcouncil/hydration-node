//! VotingHooks implementation for GIGAHDX vote tracking.
//!
//! `GigaHdxVotingHooks<T>` implements `pallet_conviction_voting::VotingHooks`.
//! It records GIGAHDX votes, tracks weighted votes per referendum,
//! and triggers reward processing when votes are removed.

use crate::types::{Conviction, GigaHdxVote, REWARD_MULTIPLIER_SCALE};
use crate::{Config, Event, GigaHdxVotes, Pallet, PriorLockSplit, ReferendaTotalWeightedVotes, ReferendumTracks};
use frame_support::defensive;
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::Get;
use frame_support::traits::fungibles::Inspect;
use hydradx_traits::gigahdx::{GetReferendumOutcome, GetTrackId, ReferendumOutcome};
use pallet_conviction_voting::{AccountVote, Status, VotingHooks};
use primitives::Balance;
use sp_runtime::Saturating;
use sp_std::marker::PhantomData;

pub struct GigaHdxVotingHooks<T>(PhantomData<T>);

impl<T: Config> VotingHooks<T::AccountId, u32, Balance> for GigaHdxVotingHooks<T> {
	fn on_before_vote(who: &T::AccountId, ref_index: u32, vote: AccountVote<Balance>) -> DispatchResult {
		// Per-vote split is the cornerstone of the lock model: GIGAHDX-side first, HDX-side
		// for any remainder. Snapshot at vote-cast time so the adapter's per-side max-aggregate
		// stays correct even after balance changes (stake/unstake/transfer) that the lock must
		// not react to.
		let gigahdx_balance =
			<T::Currency as Inspect<T::AccountId>>::balance(<T as pallet_gigahdx::Config>::GigaHdxAssetId::get(), who);

		let combined = vote.balance();
		let gigahdx_portion = combined.min(gigahdx_balance);
		let hdx_portion = combined.saturating_sub(gigahdx_portion);

		// Extract conviction from vote type.
		let conviction = match vote {
			AccountVote::Standard { vote: v, .. } => Conviction::from(v.conviction),
			AccountVote::Split { .. } | AccountVote::SplitAbstain { .. } => Conviction::None,
		};

		// Calculate lock expiry.
		let current_block = frame_system::Pallet::<T>::block_number();
		let lock_periods = conviction.lock_periods();
		let vote_locking_period = <T as crate::pallet::Config>::VoteLockingPeriod::get();
		let total_lock_blocks = vote_locking_period.saturating_mul(lock_periods.into());
		let lock_expires_at = current_block.saturating_add(total_lock_blocks);

		// Calculate new weighted vote (scaled: divide by REWARD_MULTIPLIER_SCALE).
		// Reward weighting only counts the GIGAHDX-portion — HDX-only voters don't earn
		// GIGAHDX referenda rewards.
		let new_weighted =
			gigahdx_portion.saturating_mul(conviction.reward_multiplier() as u128) / REWARD_MULTIPLIER_SCALE;

		// If updating an existing vote, subtract old weighted value first.
		if let Some(old_vote) = GigaHdxVotes::<T>::get(who, ref_index) {
			let old_weighted = old_vote
				.gigahdx_lock
				.saturating_mul(old_vote.conviction.reward_multiplier() as u128)
				/ REWARD_MULTIPLIER_SCALE;
			ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
				*total = total.saturating_sub(old_weighted);
			});
		}

		// Insert/update vote with the per-side split snapshot.
		GigaHdxVotes::<T>::insert(
			who,
			ref_index,
			GigaHdxVote {
				amount: combined,
				conviction,
				voted_at: current_block,
				lock_expires_at,
				gigahdx_lock: gigahdx_portion,
				hdx_lock: hdx_portion,
			},
		);

		// Add weighted vote.
		ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
			*total = total.saturating_add(new_weighted);
		});

		Pallet::<T>::deposit_event(Event::VoteRecorded {
			who: who.clone(),
			ref_index,
			amount: gigahdx_portion,
			conviction,
		});

		// Cache track ID so we can look it up after the referendum completes
		// (pallet-referenda drops the track from Ongoing once it finishes).
		if !ReferendumTracks::<T>::contains_key(ref_index) {
			if let Some(track) = <T::Referenda as GetTrackId<u32>>::track_id(ref_index) {
				ReferendumTracks::<T>::insert(ref_index, track);
			}
		}

		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: u32, status: Status) {
		// Take the vote entry — if none, this wasn't a tracked voter for this ref.
		let Some(vote) = GigaHdxVotes::<T>::take(who, ref_index) else {
			return;
		};

		// Subtract from total weighted votes (weight is GIGAHDX-portion only).
		let weighted =
			vote.gigahdx_lock.saturating_mul(vote.conviction.reward_multiplier() as u128) / REWARD_MULTIPLIER_SCALE;
		ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
			*total = total.saturating_sub(weighted);
		});

		// On completion: mirror upstream's PriorLock accumulation. Upstream covers
		// two paths:
		//   - winning side with conviction → `prior.accumulate(unlock_at, balance)`.
		//   - losing side with conviction → also `prior.accumulate`, gated by
		//     `lock_balance_on_unsuccessful_vote` returning `Some`.
		// Our `lock_balance_on_unsuccessful_vote` impl always returns `Some(amount)` for
		// existing votes, so for our purposes both paths reduce to: "any conviction>0
		// vote on a completed referendum keeps a prior lock alive".
		//
		// Cancelled / TimedOut / Killed referenda don't keep any lock — only Approved
		// or Rejected outcomes do.
		if status == Status::Completed {
			let lock_periods = vote.conviction.lock_periods();
			if lock_periods > 0 {
				let outcome = <T::Referenda as GetReferendumOutcome<u32>>::referendum_outcome(ref_index);
				let lock_after_completion =
					matches!(outcome, ReferendumOutcome::Approved | ReferendumOutcome::Rejected);
				if lock_after_completion {
					let class = ReferendumTracks::<T>::get(ref_index)
						.or_else(|| <T::Referenda as GetTrackId<u32>>::track_id(ref_index));
					let end = <T::Referenda as GetReferendumOutcome<u32>>::end_block(ref_index);
					if let (Some(class), Some(end)) = (class, end) {
						let lock_period_blocks =
							<T as crate::pallet::Config>::VoteLockingPeriod::get().saturating_mul(lock_periods.into());
						let unlock_at = end.saturating_add(lock_period_blocks);
						let now = frame_system::Pallet::<T>::block_number();
						if now < unlock_at {
							PriorLockSplit::<T>::mutate(who, class, |p| {
								p.accumulate(unlock_at, vote.gigahdx_lock, vote.hdx_lock);
							});
						}
					}
				}
			}

			// Existing rewards processing path stays unchanged.
			if let Err(e) = crate::rewards::maybe_allocate_and_record::<T>(who, ref_index, &vote) {
				// With StuckRewards dead-letter queue in place, capacity errors cannot
				// reach here — any failure now is an unexpected dispatch error. Do NOT
				// re-insert the vote: pallet-conviction-voting has already cleared it
				// and re-inserting would desync storage.
				defensive!("maybe_allocate_and_record unexpected error: {:?}", e);
			}
		}

		Pallet::<T>::deposit_event(Event::VoteRemoved {
			who: who.clone(),
			ref_index,
		});
	}

	fn lock_balance_on_unsuccessful_vote(who: &T::AccountId, ref_index: u32) -> Option<Balance> {
		GigaHdxVotes::<T>::get(who, ref_index).map(|vote| vote.amount)
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &T::AccountId) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &T::AccountId) {}
}
