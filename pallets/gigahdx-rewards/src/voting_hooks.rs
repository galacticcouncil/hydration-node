// SPDX-License-Identifier: Apache-2.0

//! `VotingHooks` integration for `pallet-gigahdx-rewards`.
//!
//! [`VotingHooksImpl`] is the pallet's own `VotingHooks` implementation:
//! it snapshots votes into `UserVoteRecords` (the source of truth for both
//! reward weighting and the lazily-derived `giga_unstake` commitment). The
//! runtime is responsible for combining this hook with any other `VotingHooks`
//! consumer (typically staking) when wiring
//! `pallet-conviction-voting::Config::VotingHooks`.

use crate::pallet::{
	Config, Event, Pallet, ReferendaRewardPool, ReferendaTotalWeightedVotes, ReferendumTracks, UserVoteRecords,
};
use crate::traits::{ReferendaTrackInspect, TrackRewardTable};
use crate::types::{ReferendaReward, ReferendumIndex, ReferendumLiveTally, UserVoteRecord};
use frame_support::dispatch::DispatchResult;
use frame_support::traits::{Currency, ExistenceRequirement, Get};
use pallet_conviction_voting::{AccountVote, Conviction, Status, VotingHooks};
use primitives::Balance;
use sp_std::marker::PhantomData;

const LOG_TARGET: &str = "gigahdx-rewards::voting_hooks";

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

		// Every vote variant places a `pyconvot` lock on the user's HDX, so we
		// record every variant — even Split / SplitAbstain which earn no rewards
		// — so liquidation's `clear_conflicting_votes` can reach them and the
		// `giga_unstake` commitment guard accounts for the locked HDX. Non-Standard
		// variants are recorded with `Conviction::None` (so `weighted = 0`): they
		// take a `voters_remaining` slot but distort no reward distribution
		// (`record_user_reward` short-circuits to zero on `weighted == 0`).
		let (vote_balance, conviction) = match vote {
			AccountVote::Standard {
				vote: std_vote,
				balance,
			} => (balance, std_vote.conviction),
			AccountVote::Split { aye, nay } => (aye.saturating_add(nay), Conviction::None),
			AccountVote::SplitAbstain { aye, nay, abstain } => {
				(aye.saturating_add(nay).saturating_add(abstain), Conviction::None)
			}
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
				// Edit: voter count unchanged.
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

		Ok(())
	}

	fn on_remove_vote(who: &T::AccountId, ref_index: ReferendumIndex, status: Status) {
		let Some(record) = UserVoteRecords::<T>::take(who, ref_index) else {
			return; // no eligible vote was tracked
		};

		// Pool presence = "allocation has run" idempotency signal. A counted
		// voter that arrives after allocation MUST always be recorded against
		// the pool — regardless of the (possibly pruned, `Status::None`)
		// referendum status — or `voters_remaining` never reaches zero and the
		// pool entry plus this voter's pro-rata share leak permanently.
		if ReferendaRewardPool::<T>::contains_key(ref_index) {
			if let Err(e) = Pallet::<T>::record_user_reward(who, ref_index, &record) {
				debug_assert!(false, "record_user_reward failed in on_remove_vote: {e:?}");
				log::error!(target: LOG_TARGET, "record_user_reward failed for ref {ref_index:?}: {e:?}");
			}
			return;
		}

		// Pre-allocation: drop this voter from the live tally.
		ReferendaTotalWeightedVotes::<T>::mutate_exists(ref_index, |maybe| {
			if let Some(tally) = maybe.as_mut() {
				tally.total_weighted = tally.total_weighted.saturating_sub(record.weighted);
				tally.voters_count = tally.voters_count.saturating_sub(1);
				if tally.voters_count == 0 {
					*maybe = None;
				}
			}
		});

		if matches!(status, Status::Completed) {
			if let Err(e) = maybe_allocate_and_record::<T>(who, ref_index, &record) {
				debug_assert!(false, "maybe_allocate_and_record failed in on_remove_vote: {e:?}");
				log::error!(target: LOG_TARGET, "maybe_allocate_and_record failed for ref {ref_index:?}: {e:?}");
			}
		}
	}

	fn lock_balance_on_unsuccessful_vote(_who: &T::AccountId, _ref_index: ReferendumIndex) -> Option<Balance> {
		// Rewards never locks user balance — the gigahdx unstake commitment is
		// derived lazily from `UserVoteRecords`. Letting the tuple's `or`
		// fallback pass through whatever the other hook (staking) says.
		None
	}

	// `on_before_vote` / `on_remove_vote` short-circuit at `Stakes[who].hdx == 0`,
	// so the conviction-voting `vote` / `remove_vote` benchmarks must make `who`
	// a gigahdx staker for their weight to cover this hook's per-vote work (tally
	// + `UserVoteRecords` write). Seed the stake record directly — the hook only
	// reads `.hdx`, so no money-market / lock setup is needed. (The freeze is no
	// longer maintained here; it's pulled lazily by `giga_unstake` instead.)
	//
	// The one-time-per-referendum allocation path (`Status::Completed` → pot
	// transfer + `record_user_reward`) is not reachable here: the benchmark's
	// poll stays Ongoing, so that bounded cost is paid by — and documented on —
	// the first post-completion remover rather than charged per vote.
	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(who: &T::AccountId) {
		seed_staker_worst_case::<T>(who);
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(who: &T::AccountId) {
		seed_staker_worst_case::<T>(who);
	}
}

#[cfg(feature = "runtime-benchmarks")]
fn seed_staker_worst_case<T: Config>(who: &T::AccountId) {
	pallet_gigahdx::Stakes::<T>::insert(
		who,
		pallet_gigahdx::StakeRecord {
			hdx: 1_000_000_000_000_000,
			gigahdx: 1_000_000_000_000_000,
			..Default::default()
		},
	);
}

/// `giga_unstake`'s freeze guard: the HDX of `who`'s stake currently backing
/// active votes. The same locked HDX backs every concurrent vote, so the
/// commitment is the *largest* reservation, not their sum — `max` over the
/// user's active per-referendum records. Pulled lazily at unstake, never
/// maintained on the voting path.
impl<T: Config> pallet_gigahdx::traits::VotingCommitmentInspect<T::AccountId> for Pallet<T> {
	fn committed_with_count(who: &T::AccountId) -> (Balance, u32) {
		let mut max = 0;
		let mut count = 0u32;
		for record in UserVoteRecords::<T>::iter_prefix_values(who) {
			count = count.saturating_add(1);
			if record.staked_vote_amount > max {
				max = record.staked_vote_amount;
			}
		}
		(max, count)
	}

	fn committed_weight() -> frame_support::weights::Weight {
		// Worst case: a staker holding conviction-voting's `MaxVotes` (25) in
		// every governance track (10) → up to 250 `UserVoteRecords` reads. Keep
		// in sync with the runtime's conviction-voting `MaxVotes` × track count.
		// `giga_unstake` refunds down to the count actually scanned.
		<T as frame_system::Config>::DbWeight::get().reads(250)
	}
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
				track_id,
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
