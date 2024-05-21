use crate::{AccountVote, ReferendumIndex};
use frame_support::dispatch::DispatchResult;

pub trait DemocracyHooks<AccountId, Balance> {
	fn on_vote(who: &AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult;

	// Called when removed vote is executed.
	// is_finished indicates the state of the referendum = None if referendum is cancelled, Some(bool) if referendum is finished(true) or ongoing(false).
	fn on_remove_vote(who: &AccountId, ref_index: ReferendumIndex, is_finished: Option<bool>);

	fn remove_vote_locks_if_needed(who: &AccountId, ref_index: ReferendumIndex) -> Option<Balance>;

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &AccountId);

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &AccountId);
}

impl<AccountId, Balance> DemocracyHooks<AccountId, Balance> for () {
	fn on_vote(_who: &AccountId, _ref_index: ReferendumIndex, _vote: AccountVote<Balance>) -> DispatchResult {
		Ok(())
	}

	fn on_remove_vote(_who: &AccountId, _ref_index: ReferendumIndex, _is_finished: Option<bool>) {}

	fn remove_vote_locks_if_needed(_who: &AccountId, _ref_index: ReferendumIndex) -> Option<Balance> {
		None
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &AccountId) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &AccountId) {}
}
