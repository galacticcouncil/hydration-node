//! CombinedVotingHooks — combines two VotingHooks implementations.
//!
//! pallet-conviction-voting doesn't provide a tuple impl for VotingHooks,
//! so we need this explicit combinator.

use frame_support::dispatch::DispatchResult;
use pallet_conviction_voting::{AccountVote, Status, VotingHooks};
use sp_std::marker::PhantomData;

/// Combines two VotingHooks implementations. Both are called for each hook.
pub struct CombinedVotingHooks<A, B>(PhantomData<(A, B)>);

impl<AccountId, Index, Balance, A, B> VotingHooks<AccountId, Index, Balance> for CombinedVotingHooks<A, B>
where
	A: VotingHooks<AccountId, Index, Balance>,
	B: VotingHooks<AccountId, Index, Balance>,
	Balance: Copy + Ord,
	Index: Copy,
{
	fn on_before_vote(who: &AccountId, ref_index: Index, vote: AccountVote<Balance>) -> DispatchResult {
		A::on_before_vote(who, ref_index, vote)?;
		B::on_before_vote(who, ref_index, vote)?;
		Ok(())
	}

	fn on_remove_vote(who: &AccountId, ref_index: Index, status: Status) {
		A::on_remove_vote(who, ref_index, status);
		B::on_remove_vote(who, ref_index, status);
	}

	fn lock_balance_on_unsuccessful_vote(who: &AccountId, ref_index: Index) -> Option<Balance> {
		let a = A::lock_balance_on_unsuccessful_vote(who, ref_index);
		let b = B::lock_balance_on_unsuccessful_vote(who, ref_index);
		match (a, b) {
			(Some(x), Some(y)) => Some(x.max(y)),
			(Some(x), None) => Some(x),
			(None, Some(y)) => Some(y),
			(None, None) => None,
		}
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_vote_worst_case(who: &AccountId) {
		A::on_vote_worst_case(who);
		B::on_vote_worst_case(who);
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn on_remove_vote_worst_case(who: &AccountId) {
		A::on_remove_vote_worst_case(who);
		B::on_remove_vote_worst_case(who);
	}
}
