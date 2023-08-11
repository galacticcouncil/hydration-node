use crate::types::Balance;
use num_traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, One};
use sp_arithmetic::{traits::Saturating, FixedPointNumber, FixedU128, Perbill, Permill};
use sp_std::num::NonZeroU128;
use sp_std::ops::Div;

type Period = u128;
type Point = u128;

/// Function calculates new accumulated reward per stake.
///
/// Parameters:
/// - `current_reward_per_stake`: current value of `reward_per_stake`
/// - `pending_rewards`: amount of rewards ready for distribution
/// - `total_stake`: total amount of staked tokens
pub fn calculate_accumulated_rps(
	current_reward_per_stake: FixedU128,
	pending_rewards: Balance,
	total_stake: Balance,
) -> Option<FixedU128> {
	let rps = FixedU128::checked_from_rational(pending_rewards, total_stake)?;
	current_reward_per_stake.checked_add(&rps)
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
) -> Option<Balance> {
	let stake_weighted = current_stake.checked_mul(stake_weight.into())?;
	FixedU128::checked_from_rational(stake_increase, stake_weighted)?
		.min(FixedU128::one())
		.checked_mul_int(points)
}

/// Function calculates period number from block number and period size.
///
/// Parameters:
/// - `period_length`: length of the one period in blocks
/// - `block_number`: block number to calculate period for
pub fn calculate_period_number(period_length: NonZeroU128, block_number: u128) -> Period {
	block_number.div(&period_length.get())
}

/// Function calculates total amount of `Points` user have accumulated until now.
/// Slashed points are subtracted.
///
/// Parameters:
/// - `position_created_at`: period number staking position was created at
/// - `now`: current period number
/// - `time_points_per_period`: number of time points per 1 period
/// - `time_points_weight`: weight of the time points
/// - `action_points`: amount of action points accumulated by user
/// - `action_points_weight`: weight of the action points
/// - `slashed_points`: amount of points to slash from points
pub fn calculate_points(
	position_created_at: Period,
	now: Period,
	time_points_per_period: u8,
	time_points_weight: Permill,
	action_points: Point,
	action_points_weight: Perbill,
	slashed_points: Point,
) -> Option<Point> {
	let time_points = now
		.checked_sub(position_created_at)?
		.checked_mul(time_points_per_period.into())?;
	let time_points_weighted = FixedU128::from(time_points_weight).checked_mul_int(time_points)?;

	FixedU128::from(action_points_weight)
		.checked_mul_int(action_points)?
		.checked_add(time_points_weighted)?
		.checked_sub(slashed_points)
}

/// Implementation of sigmoid function returning values from range [0,1)
///
/// f(x) = (ax)^4/(b + (ax)^4)
///
/// Parameters:
/// - `x`: point on the curve
/// - `a` & `b`: parameters modifying "speed"/slope of the curve
pub fn sigmoid(x: Point, a: FixedU128, b: u32) -> Option<FixedU128> {
	let ax = a.checked_mul(&FixedU128::from(x))?;
	let ax4 = ax.saturating_pow(4);

	let denom = ax4.checked_add(&FixedU128::from(b as u128))?;

	ax4.checked_div(&denom)
}

/// Function calculates amount of rewards.
///
/// - `accumulated_reward_per_stake`: global value of reward per stake
/// - `reward_per_stake`: position's reward per stake
/// - `stake`: staked amount
pub fn calculate_rewards(
	accumulated_reward_per_stake: FixedU128,
	reward_per_stake: FixedU128,
	stake: Balance,
) -> Option<Balance> {
	accumulated_reward_per_stake
		.checked_sub(&reward_per_stake)?
		.checked_mul_int(stake)
}

/// Function calculates `percentage` value from `amount`.
///
/// - `amount` - to calculate value from
/// - `percentage` - percentage we want from `amount`. This value should be less than 1.
pub fn calculate_percentage_amount(amount: u128, percentage: FixedU128) -> Balance {
	percentage.saturating_mul_int(amount)
}
