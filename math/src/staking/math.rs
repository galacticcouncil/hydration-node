use crate::{types::Balance, MathError};
use num_traits::{CheckedAdd, One};
use sp_arithmetic::{FixedPointNumber, FixedU128, Permill};

type BlockNumber = u128;
type Period = u128;
type Point = u128;

/// Function calculates new accumulated rps.
///
/// Parameters:
/// - `pending_rewards`: ammount of rewards ready to distribute
/// - `total_stake`: total amount of tokens staked
/// - `rps`: value added to `rps_now`
pub fn calculate_accumulated_rps(
	rps_now: FixedU128,
	pending_rewards: Balance,
	total_stake: Balance,
) -> Result<FixedU128, MathError> {
	let rps = FixedU128::checked_from_rational(pending_rewards, total_stake).ok_or(MathError::DivisionByZero)?;

	rps_now.checked_add(&rps).ok_or(MathError::Overflow)
}

/// Function calculates amount of points to slash for current stake increase.
/// Caluclated points are rounded down.
///
/// Parameters:
/// - `points`: amount of points user accumulated until this points
/// - `current_stake`: staked amount before increase
/// - `stake_increase`: ammount added to stake
/// - `stake_weight`: weight of `current_stake`. Bigger the weight lower the points slashed
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

/// Fucntion calculated Period number from block number and period size.
///
/// Parameters:
/// - `period_length`: length of the one period in blocks
/// - `block_number`: block number to calculate period for
pub fn calculate_period_number(period_length: BlockNumber, block_number: BlockNumber) -> Result<Period, MathError> {
	block_number.checked_div(period_length).ok_or(MathError::DivisionByZero)
}

/// Function calculates `Points` for payable curve.
///
/// Parameters:
/// - `entered_at`: block nubmer when user entered staking
/// - `now`: current block nubmer
/// - `period_length`: lenght of period in blocks
/// - `time_points_per_period`: number of time points per 1 period
/// - `time_weight`: weight of the time points
/// - `action_points`: amount of acction points accumulated by user
/// - `action_weight`: weight of the action points
/// - `slashed_points`: amount of points to slash from max points.
pub fn calculate_points(
	entered_at: BlockNumber,
	now: BlockNumber,
	period_length: BlockNumber,
	time_points_per_period: u8,
	time_points_weight: Permill,
	action_points: Point,
	action_points_weight: Permill,
	slashed_points: Point,
) -> Result<Point, MathError> {
	let entered_period = calculate_period_number(period_length, entered_at)?;
	let current_period = calculate_period_number(period_length, now)?;

	let time_points = current_period
		.saturating_sub(entered_period)
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
