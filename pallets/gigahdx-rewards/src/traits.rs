// SPDX-License-Identifier: Apache-2.0

//! Traits used by `pallet-gigahdx-rewards` to abstract over the runtime's
//! referenda configuration and the per-track reward percentage table.

use primitives::Balance;
use sp_runtime::{DispatchError, Permill};

/// Lookup of referendum index → track id.
pub trait ReferendaTrackInspect<RefIdx, TrackId> {
	/// Track id for an ongoing or recently completed referendum. Returns
	/// `None` if the referendum index is unknown (never existed) or the
	/// entry has already been pruned. In the latter case the pallet falls
	/// back to its cached `ReferendumTracks[ref_index]`.
	fn track_of(ref_index: RefIdx) -> Option<TrackId>;
}

/// Per-track reward percentage. Implementations should be `const`-y —
/// the function is called inside `on_remove_vote`, which must be cheap.
pub trait TrackRewardTable<TrackId> {
	/// Fraction of the accumulator pot to allocate to a completed
	/// referendum on this track. Returning `Permill::zero()` is a valid
	/// way to opt a track out of rewards entirely.
	fn reward_percentage(track_id: TrackId) -> Permill;
}

/// Conviction-voting interactions used by `pallet-liquidation` during a
/// gigahdx liquidation. `force_release_vote_lock` exists because
/// `try_remove_vote` does not resync the `pyconvot` lock — leaving the
/// borrower's transfer freeze stale on the seize path.
pub trait ClearConflictingVotes<AccountId> {
	fn clear_conflicting_votes(who: &AccountId, max_remaining_hdx: Balance) -> Result<u32, DispatchError>;

	/// Saturating reduction of the `pyconvot` lock by `amount`.
	fn force_release_vote_lock(who: &AccountId, amount: Balance) -> Result<(), DispatchError>;
}

impl<AccountId> ClearConflictingVotes<AccountId> for () {
	fn clear_conflicting_votes(_who: &AccountId, _max_remaining_hdx: Balance) -> Result<u32, DispatchError> {
		Ok(0)
	}

	fn force_release_vote_lock(_who: &AccountId, _amount: Balance) -> Result<(), DispatchError> {
		Ok(())
	}
}
