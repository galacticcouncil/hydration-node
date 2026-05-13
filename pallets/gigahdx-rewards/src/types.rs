// SPDX-License-Identifier: Apache-2.0

//! Storage value types for `pallet-gigahdx-rewards`.

use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use pallet_conviction_voting::Conviction;
use primitives::Balance;
use scale_info::TypeInfo;

/// Mirrors `pallet_referenda::ReferendumIndex` (both are `u32`). Declared
/// here so the rewards pallet does not need a direct dependency on
/// `pallet-referenda` — only the runtime wiring does.
pub type ReferendumIndex = u32;

/// Scale for `conviction_reward_multiplier`. Numerators read as percentages.
pub const REWARD_MULTIPLIER_SCALE: u128 = 100;

/// Map conviction → reward weight (percentage of `Locked3x = base`).
/// `Locked3x` (28-day lock) is the unit; shorter locks earn fractions,
/// longer locks earn multiples. `None` earns nothing — voters that don't
/// commit to a lock period don't receive gigahdx-rewards.
///
/// | Conviction | Days lock | Multiplier |
/// |------------|-----------|-----------|
/// | None       | 0         | 0×        |
/// | Locked1x   | 7         | 0.25×     |
/// | Locked2x   | 14        | 0.5×      |
/// | Locked3x   | 28        | 1× (base) |
/// | Locked4x   | 56        | 2×        |
/// | Locked5x   | 112       | 4×        |
/// | Locked6x   | 224       | 8×        |
pub fn conviction_reward_multiplier(conviction: Conviction) -> u128 {
	match conviction {
		Conviction::None => 0,
		Conviction::Locked1x => 25,
		Conviction::Locked2x => 50,
		Conviction::Locked3x => 100,
		Conviction::Locked4x => 200,
		Conviction::Locked5x => 400,
		Conviction::Locked6x => 800,
	}
}

/// Live tally maintained during the voting period for each referendum.
/// Deleted at allocation time; the values move into `ReferendaReward`.
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug, Default)]
pub struct ReferendumLiveTally {
	/// Σ `weighted` over all currently-active `UserVoteRecord`s for this
	/// referendum.
	pub total_weighted: u128,
	/// |{ who : UserVoteRecords[who, ref].is_some() }|. Snapshotted into
	/// `ReferendaReward.voters_remaining` at allocation.
	pub voters_count: u32,
}

/// Frozen per-referendum snapshot, populated on first `on_remove_vote` of
/// a completed referendum. Presence doubles as the "allocation has run"
/// idempotency signal. Deleted when `voters_remaining` reaches zero.
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
pub struct ReferendaReward<TrackId> {
	pub track_id: TrackId,
	/// Allocation snapshot (HDX).
	pub total_reward: Balance,
	/// Frozen denominator for pro-rata math.
	pub total_weighted_votes: u128,
	/// Countdown of voters who still hold a `UserVoteRecord` for this
	/// referendum. When this reaches zero on a per-user payout, the last
	/// claimant scoops `remaining_reward` and the pool entry is deleted.
	pub voters_remaining: u32,
	/// Decremented on each per-user payout. Equals `total_reward` at
	/// allocation; drained to exactly zero by the final claimant.
	pub remaining_reward: Balance,
}

/// Per (user, referendum) snapshot of the eligible vote weight at cast time.
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
pub struct UserVoteRecord {
	/// `min(vote.balance(), Stakes[who].hdx)` at the moment the vote was cast.
	pub staked_vote_amount: Balance,
	/// Conviction at cast time. Stored for off-chain attribution; not read
	/// during reward math (the multiplier is already baked into `weighted`).
	pub conviction: Conviction,
	/// `staked_vote_amount × conviction_reward_multiplier / REWARD_MULTIPLIER_SCALE`.
	pub weighted: u128,
}
