use crate::{AccountVote, ReferendumIndex};
use frame_support::dispatch::DispatchResult;

pub trait DemocracyHooks<AccountId, Balance> {
	fn on_vote(who: &AccountId, ref_index: ReferendumIndex, vote: AccountVote<Balance>) -> DispatchResult;
	fn on_remove_vote(who: &AccountId, ref_index: ReferendumIndex) -> DispatchResult;
}

impl<AccountId, Balance> DemocracyHooks<AccountId, Balance> for () {
	fn on_vote(_who: &AccountId, _ref_index: ReferendumIndex, _vote: AccountVote<Balance>) -> DispatchResult {
		Ok(())
	}

	fn on_remove_vote(_who: &AccountId, _ref_index: ReferendumIndex) -> DispatchResult {
		Ok(())
	}
}
