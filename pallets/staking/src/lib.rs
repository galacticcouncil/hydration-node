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

use frame_support::traits::{Currency, Get};
use sp_runtime::{traits::StaticLookup, DispatchResult};

pub mod weights;
pub use pallet::*;
pub use weights::WeightInfo;

type BalanceOf<T> =
	<<T as pallet_democracy::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

type AccountIdLookupOf<T> = <<T as frame_system::Config>::Lookup as StaticLookup>::Source;

#[frame_support::pallet]
pub mod pallet {
	use super::{DispatchResult, *};
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use pallet_democracy::{AccountVote, BoundedCallOf, Conviction, PropIndex, ReferendumIndex};
	use sp_core::H256;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_democracy::Config {
		type WeightInfo: WeightInfo;
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Propose a sensitive action to be taken.
		///
		/// The dispatch origin of this call must be _Signed_ and the sender must
		/// have funds to cover the deposit.
		///
		/// - `proposal_hash`: The hash of the proposal preimage.
		/// - `value`: The amount of deposit (must be at least `MinimumDeposit`).
		///
		/// Emits `Proposed`.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::propose())]
		pub fn propose(
			origin: OriginFor<T>,
			proposal: BoundedCallOf<T>,
			#[pallet::compact] value: BalanceOf<T>,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::propose(origin, proposal, value)
		}

		/// Signals agreement with a particular proposal.
		///
		/// The dispatch origin of this call must be _Signed_ and the sender
		/// must have funds to cover the deposit, equal to the original deposit.
		///
		/// - `proposal`: The index of the proposal to second.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::second())]
		pub fn second(origin: OriginFor<T>, #[pallet::compact] proposal: PropIndex) -> DispatchResult {
			pallet_democracy::Pallet::<T>::second(origin, proposal)
		}

		/// Vote in a referendum. If `vote.is_aye()`, the vote is to enact the proposal;
		/// otherwise it is a vote to keep the status quo.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `ref_index`: The index of the referendum to vote for.
		/// - `vote`: The vote configuration.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::vote_new().max(<T as Config>::WeightInfo::vote_existing()))]
		pub fn vote(
			origin: OriginFor<T>,
			#[pallet::compact] ref_index: ReferendumIndex,
			vote: AccountVote<BalanceOf<T>>,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::vote(origin, ref_index, vote)
		}

		/// Schedule an emergency cancellation of a referendum. Cannot happen twice to the same
		/// referendum.
		///
		/// The dispatch origin of this call must be `CancellationOrigin`.
		///
		/// -`ref_index`: The index of the referendum to cancel.
		///
		/// Weight: `O(1)`.
		#[pallet::call_index(3)]
		#[pallet::weight((<T as Config>::WeightInfo::emergency_cancel(), DispatchClass::Operational))]
		pub fn emergency_cancel(origin: OriginFor<T>, ref_index: ReferendumIndex) -> DispatchResult {
			pallet_democracy::Pallet::<T>::emergency_cancel(origin, ref_index)
		}

		/// Schedule a referendum to be tabled once it is legal to schedule an external
		/// referendum.
		///
		/// The dispatch origin of this call must be `ExternalOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::external_propose())]
		pub fn external_propose(origin: OriginFor<T>, proposal: BoundedCallOf<T>) -> DispatchResult {
			pallet_democracy::Pallet::<T>::external_propose(origin, proposal)
		}

		/// Schedule a majority-carries referendum to be tabled next once it is legal to schedule
		/// an external referendum.
		///
		/// The dispatch of this call must be `ExternalMajorityOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		///
		/// Unlike `external_propose`, blacklisting has no effect on this and it may replace a
		/// pre-scheduled `external_propose` call.
		///
		/// Weight: `O(1)`
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::external_propose_majority())]
		pub fn external_propose_majority(origin: OriginFor<T>, proposal: BoundedCallOf<T>) -> DispatchResult {
			pallet_democracy::Pallet::<T>::external_propose_majority(origin, proposal)
		}

		/// Schedule a negative-turnout-bias referendum to be tabled next once it is legal to
		/// schedule an external referendum.
		///
		/// The dispatch of this call must be `ExternalDefaultOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal.
		///
		/// Unlike `external_propose`, blacklisting has no effect on this and it may replace a
		/// pre-scheduled `external_propose` call.
		///
		/// Weight: `O(1)`
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::external_propose_default())]
		pub fn external_propose_default(origin: OriginFor<T>, proposal: BoundedCallOf<T>) -> DispatchResult {
			pallet_democracy::Pallet::<T>::external_propose_default(origin, proposal)
		}

		/// Schedule the currently externally-proposed majority-carries referendum to be tabled
		/// immediately. If there is no externally-proposed referendum currently, or if there is one
		/// but it is not a majority-carries referendum then it fails.
		///
		/// The dispatch of this call must be `FastTrackOrigin`.
		///
		/// - `proposal_hash`: The hash of the current external proposal.
		/// - `voting_period`: The period that is allowed for voting on this proposal. Increased to
		/// 	Must be always greater than zero.
		/// 	For `FastTrackOrigin` must be equal or greater than `FastTrackVotingPeriod`.
		/// - `delay`: The number of block after voting has ended in approval and this should be
		///   enacted. This doesn't have a minimum amount.
		///
		/// Emits `Started`.
		///
		/// Weight: `O(1)`
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::fast_track())]
		pub fn fast_track(
			origin: OriginFor<T>,
			proposal_hash: H256,
			voting_period: T::BlockNumber,
			delay: T::BlockNumber,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::fast_track(origin, proposal_hash, voting_period, delay)
		}

		/// Veto and blacklist the external proposal hash.
		///
		/// The dispatch origin of this call must be `VetoOrigin`.
		///
		/// - `proposal_hash`: The preimage hash of the proposal to veto and blacklist.
		///
		/// Emits `Vetoed`.
		///
		/// Weight: `O(V + log(V))` where V is number of `existing vetoers`
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::veto_external())]
		pub fn veto_external(origin: OriginFor<T>, proposal_hash: H256) -> DispatchResult {
			pallet_democracy::Pallet::<T>::veto_external(origin, proposal_hash)
		}

		/// Remove a referendum.
		///
		/// The dispatch origin of this call must be _Root_.
		///
		/// - `ref_index`: The index of the referendum to cancel.
		///
		/// # Weight: `O(1)`.
		#[pallet::call_index(9)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_referendum())]
		pub fn cancel_referendum(
			origin: OriginFor<T>,
			#[pallet::compact] ref_index: ReferendumIndex,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::cancel_referendum(origin, ref_index)
		}

		/// Delegate the voting power (with some given conviction) of the sending account.
		///
		/// The balance delegated is locked for as long as it's delegated, and thereafter for the
		/// time appropriate for the conviction's lock period.
		///
		/// The dispatch origin of this call must be _Signed_, and the signing account must either:
		///   - be delegating already; or
		///   - have no voting activity (if there is, then it will need to be removed/consolidated
		///     through `reap_vote` or `unvote`).
		///
		/// - `to`: The account whose voting the `target` account's voting power will follow.
		/// - `conviction`: The conviction that will be attached to the delegated votes. When the
		///   account is undelegated, the funds will be locked for the corresponding period.
		/// - `balance`: The amount of the account's balance to be used in delegating. This must not
		///   be more than the account's current balance.
		///
		/// Emits `Delegated`.
		///
		/// Weight: `O(R)` where R is the number of referendums the voter delegating to has
		///   voted on. Weight is charged as if maximum votes.
		// NOTE: weight must cover an incorrect voting of origin with max votes, this is ensure
		// because a valid delegation cover decoding a direct voting with max votes.
		#[pallet::call_index(10)]
		#[pallet::weight(<T as Config>::WeightInfo::delegate(T::MaxVotes::get()))]
		pub fn delegate(
			origin: OriginFor<T>,
			to: AccountIdLookupOf<T>,
			conviction: Conviction,
			balance: BalanceOf<T>,
		) -> DispatchResultWithPostInfo {
			pallet_democracy::Pallet::<T>::delegate(origin, to, conviction, balance)
		}

		/// Undelegate the voting power of the sending account.
		///
		/// Tokens may be unlocked following once an amount of time consistent with the lock period
		/// of the conviction with which the delegation was issued.
		///
		/// The dispatch origin of this call must be _Signed_ and the signing account must be
		/// currently delegating.
		///
		/// Emits `Undelegated`.
		///
		/// Weight: `O(R)` where R is the number of referendums the voter delegating to has
		///   voted on. Weight is charged as if maximum votes.
		// NOTE: weight must cover an incorrect voting of origin with max votes, this is ensure
		// because a valid delegation cover decoding a direct voting with max votes.
		#[pallet::call_index(11)]
		#[pallet::weight(<T as Config>::WeightInfo::undelegate(T::MaxVotes::get()))]
		pub fn undelegate(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			pallet_democracy::Pallet::<T>::undelegate(origin)
		}

		/// Clears all public proposals.
		///
		/// The dispatch origin of this call must be _Root_.
		///
		/// Weight: `O(1)`.
		#[pallet::call_index(12)]
		#[pallet::weight(<T as Config>::WeightInfo::clear_public_proposals())]
		pub fn clear_public_proposals(origin: OriginFor<T>) -> DispatchResult {
			pallet_democracy::Pallet::<T>::clear_public_proposals(origin)
		}

		/// Unlock tokens that have an expired lock.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `target`: The account to remove the lock on.
		///
		/// Weight: `O(R)` with R number of vote of target.
		#[pallet::call_index(13)]
		#[pallet::weight(<T as Config>::WeightInfo::unlock_set(T::MaxVotes::get()).max(<T as Config>::WeightInfo::unlock_remove(T::MaxVotes::get())))]
		pub fn unlock(origin: OriginFor<T>, target: AccountIdLookupOf<T>) -> DispatchResult {
			pallet_democracy::Pallet::<T>::unlock(origin, target)
		}

		/// Remove a vote for a referendum.
		///
		/// If:
		/// - the referendum was cancelled, or
		/// - the referendum is ongoing, or
		/// - the referendum has ended such that
		///   - the vote of the account was in opposition to the result; or
		///   - there was no conviction to the account's vote; or
		///   - the account made a split vote
		/// ...then the vote is removed cleanly and a following call to `unlock` may result in more
		/// funds being available.
		///
		/// If, however, the referendum has ended and:
		/// - it finished corresponding to the vote of the account, and
		/// - the account made a standard vote with conviction, and
		/// - the lock period of the conviction is not over
		/// ...then the lock will be aggregated into the overall account's lock, which may involve
		/// *overlocking* (where the two locks are combined into a single lock that is the maximum
		/// of both the amount locked and the time is it locked for).
		///
		/// The dispatch origin of this call must be _Signed_, and the signer must have a vote
		/// registered for referendum `index`.
		///
		/// - `index`: The index of referendum of the vote to be removed.
		///
		/// Weight: `O(R + log R)` where R is the number of referenda that `target` has voted on.
		///   Weight is calculated for the maximum number of vote.
		#[pallet::call_index(14)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_vote(T::MaxVotes::get()))]
		pub fn remove_vote(origin: OriginFor<T>, index: ReferendumIndex) -> DispatchResult {
			pallet_democracy::Pallet::<T>::remove_vote(origin, index)
		}

		/// Remove a vote for a referendum.
		///
		/// If the `target` is equal to the signer, then this function is exactly equivalent to
		/// `remove_vote`. If not equal to the signer, then the vote must have expired,
		/// either because the referendum was cancelled, because the voter lost the referendum or
		/// because the conviction period is over.
		///
		/// The dispatch origin of this call must be _Signed_.
		///
		/// - `target`: The account of the vote to be removed; this account must have voted for
		///   referendum `index`.
		/// - `index`: The index of referendum of the vote to be removed.
		///
		/// Weight: `O(R + log R)` where R is the number of referenda that `target` has voted on.
		///   Weight is calculated for the maximum number of vote.
		#[pallet::call_index(15)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_other_vote(T::MaxVotes::get()))]
		pub fn remove_other_vote(
			origin: OriginFor<T>,
			target: AccountIdLookupOf<T>,
			index: ReferendumIndex,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::remove_other_vote(origin, target, index)
		}

		/// Permanently place a proposal into the blacklist. This prevents it from ever being
		/// proposed again.
		///
		/// If called on a queued public or external proposal, then this will result in it being
		/// removed. If the `ref_index` supplied is an active referendum with the proposal hash,
		/// then it will be cancelled.
		///
		/// The dispatch origin of this call must be `BlacklistOrigin`.
		///
		/// - `proposal_hash`: The proposal hash to blacklist permanently.
		/// - `ref_index`: An ongoing referendum whose hash is `proposal_hash`, which will be
		/// cancelled.
		///
		/// Weight: `O(p)` (though as this is an high-privilege dispatch, we assume it has a
		///   reasonable value).
		#[pallet::call_index(16)]
		#[pallet::weight((<T as Config>::WeightInfo::blacklist(), DispatchClass::Operational))]
		pub fn blacklist(
			origin: OriginFor<T>,
			proposal_hash: H256,
			maybe_ref_index: Option<ReferendumIndex>,
		) -> DispatchResult {
			pallet_democracy::Pallet::<T>::blacklist(origin, proposal_hash, maybe_ref_index)
		}

		/// Remove a proposal.
		///
		/// The dispatch origin of this call must be `CancelProposalOrigin`.
		///
		/// - `prop_index`: The index of the proposal to cancel.
		///
		/// Weight: `O(p)` where `p = PublicProps::<T>::decode_len()`
		#[pallet::call_index(17)]
		#[pallet::weight(<T as Config>::WeightInfo::cancel_proposal())]
		pub fn cancel_proposal(origin: OriginFor<T>, #[pallet::compact] prop_index: PropIndex) -> DispatchResult {
			pallet_democracy::Pallet::<T>::cancel_proposal(origin, prop_index)
		}
	}
}
