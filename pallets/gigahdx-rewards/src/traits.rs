// SPDX-License-Identifier: Apache-2.0

//! Traits used by `pallet-gigahdx-rewards` to abstract over the runtime's
//! referenda configuration and the per-track reward percentage table.

use sp_runtime::Permill;

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
