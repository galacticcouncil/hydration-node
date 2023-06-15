use crate::{types::Balance, MathError};
use num_traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One};
use sp_arithmetic::{traits::Saturating, FixedPointNumber, FixedU128, Permill};

type Period = u128;
type Point = u128;

/// Function calculates new accumulated reward per stake.
///
/// Parameters:
/// - `current_reward_per_stake`: current value of `reward_per_stake`
/// - `pending_rewards`: amount of rewards ready to distribute
/// - `total_stake`: total amount of tokens staked
pub fn calculate_accumulated_rps(
	current_reward_per_stake: FixedU128,
	pending_rewards: Balance,
	total_stake: Balance,
) -> Result<FixedU128, MathError> {
	let rps = FixedU128::checked_from_rational(pending_rewards, total_stake).ok_or(MathError::DivisionByZero)?;

	current_reward_per_stake.checked_add(&rps).ok_or(MathError::Overflow)
}

/// Function calculates amount of points to slash for current stake increase.
/// Calculated points are rounded down.
///
/// Parameters:
/// - `points`: amount of points user accumulated until this now
/// - `current_stake`: staked amount before stake increase
/// - `stake_increase`: amount added to stake
/// - `stake_weight`: weight of `current_stake`. Bigger the weight lower the slashed points
pub fn calculate_slashed_points(
	points: Point,
	current_stake: Balance,
	stake_increase: Balance,
	stake_weight: u8,
) -> Result<Balance, MathError> {
	let stake_weighted = current_stake
		.checked_mul(stake_weight.into())
		.ok_or(MathError::Overflow)?;

	FixedU128::checked_from_rational(stake_increase, stake_weighted)
		.ok_or(MathError::DivisionByZero)?
		.min(FixedU128::one())
		.checked_mul_int(points)
		.ok_or(MathError::Overflow)
}

/// Function calculates period number from block number and period size.
///
/// Parameters:
/// - `period_length`: length of the one period in blocks
/// - `block_number`: block number to calculate period for
pub fn calculate_period_number<BlockNumber: num_traits::CheckedDiv + TryInto<u32> + TryInto<u128>>(
	period_length: BlockNumber,
	block_number: BlockNumber,
) -> Result<Period, MathError> {
	TryInto::try_into(
		block_number
			.checked_div(&period_length)
			.ok_or(MathError::DivisionByZero)?,
	)
	.map_err(|_| MathError::Overflow)
}

/// Function calculates total amount of `Points` user have accumulated until now.
/// Slashed points are subtracted.
///
/// Parameters:
/// - `entered_at`: period number when user entered staking
/// - `now`: current period number
/// - `time_points_per_period`: number of time points per 1 period
/// - `time_weight`: weight of the time points
/// - `action_points`: amount of action points accumulated by user
/// - `action_weight`: weight of the action points
/// - `slashed_points`: amount of points to slash from max points
pub fn calculate_points(
	entered_at: Period,
	now: Period,
	time_points_per_period: u8,
	time_points_weight: Permill,
	action_points: Point,
	action_points_weight: Permill,
	slashed_points: Point,
) -> Result<Point, MathError> {
	let time_points = now
		.checked_sub(entered_at)
		.ok_or(MathError::Overflow)?
		.checked_mul(time_points_per_period.into())
		.ok_or(MathError::Overflow)?;

	let time_points_weighted = FixedU128::from(time_points_weight)
		.checked_mul_int(time_points)
		.ok_or(MathError::Overflow)?;

	FixedU128::from(action_points_weight)
		.checked_mul_int(action_points)
		.ok_or(MathError::Overflow)?
		.checked_add(time_points_weighted)
		.ok_or(MathError::Overflow)?
		.checked_sub(slashed_points)
		.ok_or(MathError::Overflow)
}

/// Implementation of sigmoid function returning values from range (0,1)
///
/// f(x) = (ax)^4/(b + (ax)^4)
///
/// Parameters:
/// - `x`: point on the curve
/// - `a` & `b`: parameters modifying "speed" and slope of the curve
pub fn sigmoid(x: Point, a: FixedU128, b: u32) -> Result<FixedU128, MathError> {
	let ax = a.checked_mul(&FixedU128::from(x)).ok_or(MathError::Overflow)?;

	let ax4 = ax.saturating_pow(4);

	let denom = ax4
		.checked_add(&FixedU128::from(b as u128))
		.ok_or(MathError::Overflow)?;

	ax4.checked_div(&denom).ok_or(MathError::Overflow)
}

/// Function calculates amount of rewards.
///
/// - `accumulated_reward_per_stake`: global value of reward per stake
/// - `reward_per_stake`: positon's reward per stake
/// - `stake`: staked amount
pub fn calculate_rewards(
	accumulated_reward_per_stake: FixedU128,
	reward_per_stake: FixedU128,
	stake: Balance,
) -> Result<Balance, MathError> {
	accumulated_reward_per_stake
		.checked_sub(&reward_per_stake)
		.ok_or(MathError::Overflow)?
		.checked_mul_int(stake)
		.ok_or(MathError::Overflow)

	//TODO: tests
}
