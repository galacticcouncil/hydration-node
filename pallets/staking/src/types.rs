use crate::traits::ActionData;
use codec::{Decode, Encode, MaxEncodedLen};
use pallet_democracy::ReferendumIndex;
use scale_info::TypeInfo;
use sp_core::bounded::BoundedVec;
use sp_core::Get;
use sp_runtime::RuntimeDebug;
use sp_runtime::{traits::Zero, ArithmeticError, FixedU128};

pub type Balance = u128;
pub type Point = u128;
pub type Period = u128;

pub enum Action {
	DemocracyVote,
}

/// Staking position, represents user's state in staking, e.g. staked amount, slashed points,...
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Position<BlockNumber> {
	/// Staked amount.
	pub(crate) stake: Balance,
	/// Amount of action points user accumulated.
	pub(crate) action_points: Point,
	/// User's reward per stake.
	pub(crate) reward_per_stake: FixedU128,
	/// Block number position was created at.
	pub(crate) created_at: BlockNumber,
	/// Total amount of points to slash.
	pub(crate) accumulated_slash_points: Point,
	/// Amount of rewards that wasn't paid yet.
	pub(crate) accumulated_unpaid_rewards: Balance,
	/// Rewards paid&locked rewards to user from increase stake.
	pub(crate) accumulated_locked_rewards: Balance,
}

impl<BlockNumber> Position<BlockNumber> {
	pub fn new(stake: Balance, reward_per_stake: FixedU128, created_at: BlockNumber) -> Self {
		Self {
			stake,
			action_points: Zero::zero(),
			reward_per_stake,
			created_at,
			accumulated_slash_points: Zero::zero(),
			accumulated_unpaid_rewards: Zero::zero(),
			accumulated_locked_rewards: Zero::zero(),
		}
	}

	/// Returns total amount of tokens locked under the positions.
	/// Returned value is combination of `position.stake` and `accumulated_locked_rewards`.
	pub fn get_total_locked(&self) -> Result<Balance, ArithmeticError> {
		self.stake
			.checked_add(self.accumulated_locked_rewards)
			.ok_or(ArithmeticError::Overflow)
	}

	pub fn get_action_points(&self) -> Point {
		self.action_points
	}
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo, Default)]
pub struct StakingData {
	/// Total amount of tokens staked in staking.
	pub(crate) total_stake: Balance,
	/// Accumulated reward per stake.
	pub(crate) accumulated_reward_per_stake: FixedU128,
	/// Balance of rewards allocated/reserved for stakers in the `pot`.
	pub(crate) pot_reserved_balance: Balance,
}

impl StakingData {
	pub fn add_stake(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_stake = self.total_stake.checked_add(amount).ok_or(ArithmeticError::Overflow)?;
		Ok(())
	}
}

#[derive(Copy, Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo, Default)]
pub enum Conviction {
	#[default]
	None = 0,
	Locked1x = 1,
	Locked2x = 2,
	Locked3x = 3,
	Locked4x = 4,
	Locked5x = 5,
	Locked6x = 6,
}

impl Conviction {
	pub fn multiplier(&self) -> FixedU128 {
		match self {
			//0.1
			Conviction::None => FixedU128::from_inner(100_000_000_000_000_000_u128),
			Conviction::Locked1x => FixedU128::from(1_u128),
			Conviction::Locked2x => FixedU128::from(2_u128),
			Conviction::Locked3x => FixedU128::from(3_u128),
			Conviction::Locked4x => FixedU128::from(4_u128),
			Conviction::Locked5x => FixedU128::from(4_u128),
			Conviction::Locked6x => FixedU128::from(6_u128),
		}
	}

	pub fn max_multiplier() -> FixedU128 {
		Conviction::Locked6x.multiplier()
	}
}

#[derive(Encode, Decode, Copy, Clone, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Vote {
	pub(crate) amount: Balance,
	pub(crate) conviction: Conviction,
}

impl Vote {
	pub fn new(amount: Balance, conviction: Conviction) -> Self {
		Self { amount, conviction }
	}
}

impl ActionData for Vote {
	fn amount(&self) -> Balance {
		self.amount
	}

	fn conviction(&self) -> FixedU128 {
		self.conviction.multiplier()
	}
}

impl<'a> ActionData for &'a Vote {
	fn amount(&self) -> Balance {
		self.amount
	}

	fn conviction(&self) -> FixedU128 {
		self.conviction.multiplier()
	}
}

#[derive(Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
#[codec(mel_bound(skip_type_params(MaxVotes)))]
#[scale_info(skip_type_params(MaxVotes))]
pub struct Voting<MaxVotes: Get<u32>> {
	pub votes: BoundedVec<(ReferendumIndex, Vote), MaxVotes>,
}

impl<MaxVotes: Get<u32>> Default for Voting<MaxVotes> {
	fn default() -> Self {
		Voting {
			votes: Default::default(),
		}
	}
}
