use crate::MathError;
use crate::MathError::Overflow;

use sp_arithmetic::{
	traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub},
	FixedPointNumber, FixedU128,
};

use crate::to_balance;
use crate::types::Balance;
use core::convert::TryInto;
use primitive_types::U128;

/// This function calculate loyalty multiplier or error.
///
/// `t = periodNow - periodAdded`
/// `num = t + initial_reward_percentage * scale_coef`
/// `denom = t + scale_coef`
///
/// `loyalty_multiplier = num/denom`

pub fn calculate_loyalty_multiplier<Period: num_traits::CheckedSub + TryInto<u32> + TryInto<u128>>(
	periods: Period,
	initial_reward_percentage: FixedU128,
	scale_coef: u32,
) -> Result<FixedU128, MathError> {
	let periods = FixedU128::from(TryInto::<u128>::try_into(periods).map_err(|_e| MathError::Overflow)?);
	let sc_coef = FixedU128::from(scale_coef as u128);

	//t + initial_reward_percentage * scale_coef
	let num = initial_reward_percentage
		.checked_mul(&sc_coef)
		.ok_or(MathError::Overflow)?
		.checked_add(&periods)
		.ok_or(MathError::Overflow)?;

	//t + scale_coef
	let denom = periods.checked_add(&sc_coef).ok_or(MathError::Overflow)?;

	num.checked_div(&denom).ok_or(MathError::Overflow)
}

/// This function calculate and return reward per share or error.
pub fn calculate_accumulated_rps(
	accumulated_rps_now: FixedU128,
	total_shares: Balance,
	reward: Balance,
) -> Result<FixedU128, MathError> {
	let rps = FixedU128::checked_from_rational(reward, total_shares).ok_or(MathError::DivisionByZero)?;

	rps.checked_add(&accumulated_rps_now).ok_or(MathError::Overflow)
}

/// This function calculate and return `(user_rewards, unclaimable_rewards)`.
pub fn calculate_user_reward(
	accumulated_rpvs: FixedU128,
	valued_shares: Balance, // Value of shares at the time of entry in incentivized tokens.
	accumulated_claimed_rewards: Balance,
	accumulated_rpvs_now: FixedU128,
	loyalty_multiplier: FixedU128,
) -> Result<(Balance, Balance), MathError> {
	let max_rewards = calculate_reward(accumulated_rpvs, accumulated_rpvs_now, valued_shares)?;

	if max_rewards == 0 {
		return Ok((0, 0));
	}

	let claimable_rewards = loyalty_multiplier
		.checked_mul_int(max_rewards)
		.ok_or(MathError::Overflow)?;

	let unclaimable_rewards = max_rewards.checked_sub(claimable_rewards).ok_or(MathError::Overflow)?;

	let user_rewards = claimable_rewards
		.checked_sub(accumulated_claimed_rewards)
		.ok_or(MathError::Overflow)?;

	Ok((user_rewards, unclaimable_rewards))
}

/// This function calculate account's valued shares [`Balance`] or error.
pub fn calculate_valued_shares(shares: Balance, incentivized_asset_balance: Balance) -> Result<Balance, MathError> {
	shares
		.checked_mul(incentivized_asset_balance)
		.ok_or(MathError::Overflow)
}

/// This function calculate yield farm's shares amount [`Balance`] in `GlobalFarm` or error.
pub fn calculate_global_farm_shares(valued_shares: Balance, multiplier: FixedU128) -> Result<Balance, MathError> {
	multiplier.checked_mul_int(valued_shares).ok_or(MathError::Overflow)
}

/// General formula to calculate reward. Usage depends on type of rps and shares used for
/// calculations
pub fn calculate_reward(
	accumulated_rps_start: FixedU128,
	accumulated_rps_now: FixedU128,
	shares: Balance,
) -> Result<Balance, MathError> {
	accumulated_rps_now
		.checked_sub(&accumulated_rps_start)
		.ok_or(MathError::Overflow)?
		.checked_mul_int(shares)
		.ok_or(MathError::Overflow)
}

/// This function caluclates yield farm rewards [`Balance`] and rewards per valued shares
/// delta(`delta_rpvs`) [`FixedU128`] or error.
pub fn calculate_yield_farm_rewards(
	yield_farm_rpz: FixedU128,
	global_farm_rpz: FixedU128,
	multiplier: FixedU128,
	total_valued_shares: Balance,
) -> Result<(FixedU128, Balance), MathError> {
	let stake_in_global_farm =
		calculate_global_farm_shares(total_valued_shares, multiplier).map_err(|_| MathError::Overflow)?;

	let yield_farm_rewards =
		calculate_reward(yield_farm_rpz, global_farm_rpz, stake_in_global_farm).map_err(|_| MathError::Overflow)?;

	let delta_rpvs =
		FixedU128::checked_from_rational(yield_farm_rewards, total_valued_shares).ok_or(MathError::Overflow)?;

	Ok((delta_rpvs, yield_farm_rewards))
}

/// This function calculates global-farm rewards [`Balance`] or error.
pub fn calculate_global_farm_rewards<Period: num_traits::CheckedSub + TryInto<u32> + TryInto<u128>>(
	total_shares_z: Balance,
	price_adjustment: FixedU128,
	yield_per_period: FixedU128,
	max_reward_per_period: Balance,
	periods_since_last_update: Period,
) -> Result<Balance, MathError> {
	let total_shares_z_adjusted = price_adjustment
		.checked_mul_int(total_shares_z)
		.ok_or(MathError::Overflow)?;

	let periods = TryInto::<u128>::try_into(periods_since_last_update).map_err(|_e| MathError::Overflow)?;

	//(total_shares_z_adjusted * yield_per_period.into_inner() * periods)/FixedU128::DIV;
	let calculated_rewards = U128::from(total_shares_z_adjusted)
		.full_mul(yield_per_period.into_inner().into())
		.checked_mul(periods.into())
		.ok_or(MathError::Overflow)?
		.checked_div(FixedU128::DIV.into())
		.ok_or(MathError::Overflow)?;

	let rewards = to_balance!(calculated_rewards)?;

	let max_reward_for_periods = max_reward_per_period.checked_mul(periods).ok_or(MathError::Overflow)?;

	Ok(rewards.min(max_reward_for_periods))
}
