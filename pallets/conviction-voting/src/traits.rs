use frame_support::dispatch::DispatchResult;
use ::{Config, PollIndexOf};
use ::{AccountVote, BalanceOf};

pub trait VotingHooks<T> where T: Config {
    fn on_vote(who: &T::AccountId, ref_index: PollIndexOf<T>, vote: AccountVote<BalanceOf<T>>) -> DispatchResult;

    // Called when removed vote is executed.
    // is_finished indicates the state of the referendum = None if referendum is cancelled, Some(bool) if referendum is finished(true) or ongoing(false).
    fn on_remove_vote(who: &T::AccountId, ref_index: PollIndexOf<T>, is_finished: Option<bool>);

    fn remove_vote_locks_if_needed(who: &T::AccountId, ref_index: PollIndexOf<T>) -> Option<BalanceOf<T>>;

    #[cfg(feature = "runtime-benchmarks")]
    fn on_vote_worst_case(_who: &T::AccountId);

    #[cfg(feature = "runtime-benchmarks")]
    fn on_remove_vote_worst_case(_who: &T::AccountId);
}