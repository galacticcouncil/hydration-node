use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::RuntimeDebug;
use scale_info::TypeInfo;
use sp_runtime::{traits::Zero, ArithmeticError, FixedU128};

pub type Balance = u128;
//TODO: I don't think we need u128 I think u32 should be enough
pub type Point = u128;
pub type Period = u128;

/// Staking position, represents user's state in staking, eg. staked amount, slashed points,
/// votes...
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct Position<BlockNumber> {
	/// Staked amount
	pub(crate) stake: Balance,
	/// Amount of action points user accumulated
	pub(crate) action_points: Point,
	/// User's reward per stake
	pub(crate) reward_per_stake: FixedU128,
	/// Block number position was created
	pub(crate) created_at: BlockNumber,
	/// Total amount of points to slash
	pub(crate) accumulated_slash_points: Point,
	/// Amount of rewards that wasn't paid yet
	pub(crate) accumulated_unpaid_rewards: Balance,
	/// Rewards paid&locked to user after skate increase
	pub(crate) accumulated_locked_rewards: Balance,
	//TODO:
	//pub(crate) votest: BoundedVec<Vote, T::MaxVotesPerPositon>
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
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo, Default)]
pub struct StakingData {
	/// Total amount of tokens staked in staking.
	pub(crate) total_stake: Balance,
	/// Accumulated reward per stake
	pub(crate) accumulated_reward_per_stake: FixedU128,

	//TODO: get rid of this and use balance on the account
	pub(crate) pending_rew: Balance,
}

impl StakingData {
	pub fn pending_rewards(&self) -> Balance {
		//TODO: rewrite this to use balance
		self.pending_rew
	}

	pub fn add_stake(&mut self, amount: Balance) -> Result<(), ArithmeticError> {
		self.total_stake = self.total_stake.checked_add(amount).ok_or(ArithmeticError::Overflow)?;
		Ok(())
	}
}
