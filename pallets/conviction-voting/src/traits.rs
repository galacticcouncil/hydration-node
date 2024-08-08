use crate::AccountVote;
use frame_support::dispatch::DispatchResult;

pub trait VotingHooks<AccountId, Index, Balance> {
	fn on_vote(who: &AccountId, ref_index: Index, vote: AccountVote<Balance>) -> DispatchResult;

	// Called when removed vote is executed.
	// is_finished indicates the state of the referendum = None if referendum is cancelled, Some(bool) if referendum is finished(true) or ongoing(false).
	fn on_remove_vote(who: &AccountId, ref_index: Index, is_finished: Option<bool>);

	// Called when removed vote is executed and vote is in opposition.
	// Returns the amount that should be locked for the conviction time.
	fn get_amount_to_lock_for_remove_vote(who: &AccountId, ref_index: Index) -> Option<Balance>;

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &AccountId);

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &AccountId);
}

impl<A, I, B> VotingHooks<A, I, B> for () {
	fn on_vote(_who: &A, _ref_index: I, _vote: AccountVote<B>) -> DispatchResult {
		Ok(())
	}

	fn on_remove_vote(_who: &A, _ref_index: I, _is_finished: Option<bool>) {}

	fn get_amount_to_lock_for_remove_vote(_who: &A, _ref_index: I) -> Option<B> {
		None
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(_who: &A) {}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(_who: &A) {}
}
