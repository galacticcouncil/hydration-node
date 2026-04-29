use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
use frame_support::sp_runtime::RuntimeDebug;
use scale_info::TypeInfo;

use primitives::Balance;

/// Local conviction enum matching pallet-conviction-voting's Conviction.
/// Provides reward_multiplier() and lock_period_multiplier() for our reward calculations.
#[derive(
	Encode,
	Decode,
	DecodeWithMemTracking,
	Copy,
	Clone,
	Eq,
	PartialEq,
	Ord,
	PartialOrd,
	RuntimeDebug,
	TypeInfo,
	MaxEncodedLen,
	Default,
)]
pub enum Conviction {
	/// 0.1x votes, unlocked. Reward multiplier: 1 (base participation).
	#[default]
	None,
	/// 1x votes, locked for 1 period.
	Locked1x,
	/// 2x votes, locked for 2 periods.
	Locked2x,
	/// 3x votes, locked for 4 periods.
	Locked3x,
	/// 4x votes, locked for 8 periods.
	Locked4x,
	/// 5x votes, locked for 16 periods.
	Locked5x,
	/// 6x votes, locked for 32 periods.
	Locked6x,
}

/// Divisor for reward_multiplier values.
/// None = 1/10 = 0.1x, Locked1x = 10/10 = 1x, etc.
pub const REWARD_MULTIPLIER_SCALE: u128 = 10;

impl Conviction {
	/// Scaled multiplier for reward calculation.
	/// Divide by REWARD_MULTIPLIER_SCALE to get the effective multiplier.
	/// None = 0.1x, Locked1x = 1x, ..., Locked6x = 6x (matches legacy staking).
	pub fn reward_multiplier(self) -> u32 {
		match self {
			Conviction::None => 1,
			Conviction::Locked1x => 10,
			Conviction::Locked2x => 20,
			Conviction::Locked3x => 30,
			Conviction::Locked4x => 40,
			Conviction::Locked5x => 50,
			Conviction::Locked6x => 60,
		}
	}

	/// Number of lock periods (matches pallet-conviction-voting).
	pub fn lock_periods(self) -> u32 {
		match self {
			Conviction::None => 0,
			Conviction::Locked1x => 1,
			Conviction::Locked2x => 2,
			Conviction::Locked3x => 4,
			Conviction::Locked4x => 8,
			Conviction::Locked5x => 16,
			Conviction::Locked6x => 32,
		}
	}
}

impl From<pallet_conviction_voting::Conviction> for Conviction {
	fn from(c: pallet_conviction_voting::Conviction) -> Self {
		match c {
			pallet_conviction_voting::Conviction::None => Conviction::None,
			pallet_conviction_voting::Conviction::Locked1x => Conviction::Locked1x,
			pallet_conviction_voting::Conviction::Locked2x => Conviction::Locked2x,
			pallet_conviction_voting::Conviction::Locked3x => Conviction::Locked3x,
			pallet_conviction_voting::Conviction::Locked4x => Conviction::Locked4x,
			pallet_conviction_voting::Conviction::Locked5x => Conviction::Locked5x,
			pallet_conviction_voting::Conviction::Locked6x => Conviction::Locked6x,
		}
	}
}

/// A tracked GIGAHDX vote for a specific referendum.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct GigaHdxVote<BlockNumber> {
	/// Combined committed amount (GIGAHDX + HDX side).
	pub amount: Balance,
	/// Conviction level chosen by the voter.
	pub conviction: Conviction,
	/// Block number when the vote was cast.
	pub voted_at: BlockNumber,
	/// Block number when the conviction lock expires (0 for Conviction::None).
	pub lock_expires_at: BlockNumber,
	/// GIGAHDX-side contribution as decided at vote-cast time.
	pub gigahdx_lock: Balance,
	/// HDX-side contribution as decided at vote-cast time.
	pub hdx_lock: Balance,
}

/// View-only per-side split snapshot. Computed by `Pallet::lock_split_view`
/// from active `GigaHdxVotes` + `PriorLockSplit` (no separate storage). Useful
/// for inspecting effective lock state in tests and off-chain queries.
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default,
)]
pub struct VotingLockSplit {
	/// Effective GIGAHDX-side cap (mirrors `GigaHdxVotingLock`).
	pub gigahdx_amount: Balance,
	/// Effective HDX-side cap (mirrors the `pyconvot` entry on pallet-balances).
	pub hdx_amount: Balance,
}

/// Per-class prior split — mirrors pallet-conviction-voting's `PriorLock`,
/// but two-sided (GIGAHDX + HDX) and stored per `(account, class)`.
///
/// Single max-aggregate covering finished-and-still-locked votes for that
/// class. Lossy by design (same "overlocking" trade-off upstream makes).
#[derive(
	Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen, Default,
)]
pub struct PriorSplit<BlockNumber: Default> {
	/// GIGAHDX-side max across accumulated finished-conviction priors.
	pub gigahdx: Balance,
	/// HDX-side max across accumulated finished-conviction priors.
	pub hdx: Balance,
	/// Block number at which this prior expires.
	pub until: BlockNumber,
}

impl<BlockNumber: Ord + Copy + Default> PriorSplit<BlockNumber> {
	/// Per-field max-aggregate, mirroring `PriorLock::accumulate`.
	pub fn accumulate(&mut self, until: BlockNumber, gigahdx: Balance, hdx: Balance) {
		if until > self.until {
			self.until = until;
		}
		if gigahdx > self.gigahdx {
			self.gigahdx = gigahdx;
		}
		if hdx > self.hdx {
			self.hdx = hdx;
		}
	}

	/// Zeros the prior once `now >= until`. Mirrors `PriorLock::rejig`.
	pub fn rejig(&mut self, now: BlockNumber) {
		if now >= self.until {
			*self = Self::default();
		}
	}

	/// True iff this prior is still active (until > 0).
	pub fn is_active(&self) -> bool {
		self.until != BlockNumber::default()
	}
}

/// Reward pool snapshot for a completed referendum.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ReferendaReward<AccountId> {
	/// Governance track this referendum belongs to.
	pub track_id: u16,
	/// Total HDX allocated to this referendum's reward pool.
	pub total_reward: Balance,
	/// Snapshot of total weighted votes at allocation time.
	pub total_weighted_votes: Balance,
	/// Remaining HDX reward not yet claimed.
	pub remaining_reward: Balance,
	/// Sub-account holding the HDX reward for this referendum.
	pub pot_account: AccountId,
}

/// A pending reward entry for a user to claim.
#[derive(Encode, Decode, DecodeWithMemTracking, Clone, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct PendingRewardEntry {
	/// The referendum index this reward is for.
	pub referenda_id: u32,
	/// HDX reward amount to claim.
	pub reward_amount: Balance,
}
