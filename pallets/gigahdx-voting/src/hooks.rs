//! VotingHooks implementation for GIGAHDX vote tracking.
//!
//! `GigaHdxVotingHooks<T>` implements `pallet_conviction_voting::VotingHooks`.
//! It records GIGAHDX votes, tracks weighted votes per referendum,
//! and triggers reward processing when votes are removed.

use crate::types::{Conviction, GigaHdxVote};
use crate::{Config, Event, GigaHdxVotes, Pallet, ReferendaTotalWeightedVotes, ReferendumTracks};
use frame_support::dispatch::DispatchResult;
use frame_support::pallet_prelude::Get;
use frame_support::traits::fungibles::Inspect;
use hydradx_traits::gigahdx::GetTrackId;
use pallet_conviction_voting::{AccountVote, Status, VotingHooks};
use primitives::Balance;
use sp_runtime::{traits::Zero, Saturating};
use sp_std::marker::PhantomData;

pub struct GigaHdxVotingHooks<T>(PhantomData<T>);

impl<T: Config> VotingHooks<T::AccountId, u32, Balance> for GigaHdxVotingHooks<T> {
	fn on_before_vote(who: &T::AccountId, ref_index: u32, vote: AccountVote<Balance>) -> DispatchResult {
		// Check GIGAHDX balance.
		let gigahdx_balance =
			<T::Currency as Inspect<T::AccountId>>::balance(<T as pallet_gigahdx::Config>::GigaHdxAssetId::get(), who);

		// If no GIGAHDX, this is an HDX-only voter — nothing to track.
		if gigahdx_balance.is_zero() {
			return Ok(());
		}

		// Extract conviction from vote type.
		let conviction = match vote {
			AccountVote::Standard { vote: v, .. } => Conviction::from(v.conviction),
			AccountVote::Split { .. } | AccountVote::SplitAbstain { .. } => Conviction::None,
		};

		// GIGAHDX portion: min(vote.balance(), gigahdx_balance).
		let gigahdx_portion = vote.balance().min(gigahdx_balance);

		// Calculate lock expiry.
		let current_block = frame_system::Pallet::<T>::block_number();
		let lock_periods = conviction.lock_periods();
		let vote_locking_period = <T as crate::pallet::Config>::VoteLockingPeriod::get();
		let total_lock_blocks = vote_locking_period.saturating_mul(lock_periods.into());
		let lock_expires_at = current_block.saturating_add(total_lock_blocks);

		// Calculate new weighted vote.
		let new_weighted = gigahdx_portion.saturating_mul(conviction.reward_multiplier() as u128);

		// If updating an existing vote, subtract old weighted value first.
		if let Some(old_vote) = GigaHdxVotes::<T>::get(who, ref_index) {
			let old_weighted = old_vote
				.amount
				.saturating_mul(old_vote.conviction.reward_multiplier() as u128);
			ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
				*total = total.saturating_sub(old_weighted);
			});
		}

		// Insert/update vote.
		GigaHdxVotes::<T>::insert(
			who,
			ref_index,
			GigaHdxVote {
				amount: gigahdx_portion,
				conviction,
				voted_at: current_block,
				lock_expires_at,
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
		// Take the vote entry — if none, this wasn't a GIGAHDX voter for this ref.
		let Some(vote) = GigaHdxVotes::<T>::take(who, ref_index) else {
			return;
		};

		// Subtract from total weighted votes.
		let weighted = vote.amount.saturating_mul(vote.conviction.reward_multiplier() as u128);
		ReferendaTotalWeightedVotes::<T>::mutate(ref_index, |total| {
			*total = total.saturating_sub(weighted);
		});

		// If referendum completed, allocate rewards and record user's share.
		if status == Status::Completed {
			let _ = crate::rewards::maybe_allocate_and_record::<T>(who, ref_index, &vote);
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
