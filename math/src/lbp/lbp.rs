use core::convert::{TryFrom, TryInto};
use primitive_types::U256;

use crate::types::{AssetId, Balance, LBPWeight};
use crate::{
	ensure, to_balance, to_lbp_weight, to_u256, MathError,
	MathError::{Overflow, ZeroDuration, ZeroReserve},
};

use core::convert::From;
use fixed::types::U32F96;
use num_traits::{CheckedMul, CheckedSub, Zero};
use sp_arithmetic;
use sp_arithmetic::helpers_128bit::multiply_by_rational_with_rounding;
use sp_arithmetic::{FixedPointNumber, FixedU128, Rounding};

/// Calculating spot price given reserve of selling asset and reserve of buying asset.
/// Formula : BUY_RESERVE * AMOUNT / SELL_RESERVE
///
/// - `in_reserve` - reserve amount of selling asset
/// - `out_reserve` - reserve amount of buying asset
/// - `in_weight` - pool weight of selling asset
/// - `out_Weight` - pool weight of buying asset
/// - `amount` - amount
///
/// Returns None in case of error
pub fn calculate_spot_price(
	in_reserve: Balance,
	out_reserve: Balance,
	in_weight: LBPWeight,
	out_weight: LBPWeight,
	amount: Balance,
) -> Result<Balance, MathError> {
	// If any is 0 - let's not progress any further.
	ensure!(in_reserve != 0, ZeroReserve);

	if amount == 0 || out_reserve == 0 {
		return to_balance!(0);
	}

	let (amount, out_reserve, in_reserve, out_weight, in_weight) =
		to_u256!(amount, out_reserve, in_reserve, out_weight, in_weight);

	let spot_price = amount
		.checked_mul(out_reserve)
		.ok_or(Overflow)?
		.checked_mul(in_weight)
		.ok_or(Overflow)?
		.checked_div(in_reserve.checked_mul(out_weight).ok_or(Overflow)?)
		.ok_or(Overflow)?;

	to_balance!(spot_price)
}

/// Calculating spot price given reserve of selling asset and reserve of buying asset.
/// The calculation includes the fee of the pool
///
/// - `in_reserve` - reserve amount of selling asset
/// - `out_reserve` - reserve amount of buying asset
/// - `in_weight` - pool weight of selling asset
/// - `out_Weight` - pool weight of buying asset
/// - `fee_asset` - the fee asset of the pool (it is usually the asset at the first index within the pool)
/// - `asset_out` - the asset id of the buying asset
/// - `fee` - fee rate of the pool.
///
/// NOTE: the fee should be passed based on the fact if repay fee should be applied or not.
///
/// Returns MathError in case of error
pub fn calculate_spot_price_with_fee(
	in_reserve: Balance,
	out_reserve: Balance,
	in_weight: LBPWeight,
	out_weight: LBPWeight,
	fee_asset: AssetId,
	asset_out: AssetId,
	fee: Option<(u32, u32)>,
) -> Result<FixedU128, MathError> {
	ensure!(in_reserve != 0 || out_reserve != 0, ZeroReserve);

	let n = out_reserve.checked_mul(in_weight.into()).ok_or(Overflow)?;
	let d = in_reserve.checked_mul(out_weight.into()).ok_or(Overflow)?;

	let spot_price_without_fee = FixedU128::checked_from_rational(n, d).ok_or(DivisionByZero)?;

	if let Some((fee_n, fee_d)) = fee {
		let spot_price_with_fee = if fee_asset == asset_out {
			//Buyer bears fee, the fee is deducted from asset out, making the asset_out/asset_in price cheaper
			//So we decrease the price by multipling with (1-f) to reflect correct amount out after the fee deduction
			let fee = FixedU128::checked_from_rational(fee_n, fee_d).ok_or(DivisionByZero)?;
			let fee_multiplier = FixedU128::from_rational(1, 1).checked_sub(&fee).ok_or(Overflow)?;

			spot_price_without_fee.checked_mul(&fee_multiplier).ok_or(Overflow)?
		} else {
			//Pool bears repay fee
			//Fee does not change spot price as user receives the whole amount out based on whole amount in.
			//For the trade, amount_in minus fee is transferred from the user, then in the end the fee is transferred to fee-collector
			spot_price_without_fee
		};

		return Ok(spot_price_with_fee);
	}

	Ok(spot_price_without_fee)
}

use crate::MathError::DivisionByZero;
use num_traits::One;

/// Calculating selling price given reserve of selling asset and reserve of buying asset.
///
/// - `in_reserve` - reserve amount of selling asset
/// - `out_reserve` - reserve amount of buying asset
/// - `in_weight` - pool weight of selling asset
/// - `out_weight` - pool weight of buying asset
/// - `amount` - amount
///
/// Returns None in case of error
pub fn calculate_out_given_in(
	in_reserve: Balance,
	out_reserve: Balance,
	in_weight: LBPWeight,
	out_weight: LBPWeight,
	amount: Balance,
) -> Result<Balance, MathError> {
	if amount.is_zero() {
		return Ok(0u128);
	}

	let weight_ratio = div_to_fixed(in_weight.into(), out_weight.into(), Rounding::Down).ok_or(Overflow)?;

	let new_in_reserve = in_reserve.checked_add(amount).ok_or(Overflow)?;
	// This ratio being closer to one (i.e. rounded up) minimizes the impact of the asset
	// that was sold to the pool, i.e. 'amount'
	let ir = div_to_fixed(in_reserve, new_in_reserve, Rounding::Up).ok_or(Overflow)?;

	let ir: U32F96 = crate::transcendental::pow(ir, weight_ratio).map_err(|_| Overflow)?;

	let new_out_reserve_calc = mul_to_balance(out_reserve, ir, Rounding::Up).ok_or(Overflow)?;

	out_reserve.checked_sub(new_out_reserve_calc).ok_or(Overflow)
}

/// Calculating buying price given reserve of selling asset and reserve of buying asset.
/// Formula :
///
/// - `in_reserve` - reserve amount of selling asset
/// - `out_reserve` - reserve amount of buying asset
/// - `in_weight` - pool weight of selling asset
/// - `out_weight` - pool weight of buying asset
/// - `amount` - buy amount
///
/// Returns None in case of error
pub fn calculate_in_given_out(
	in_reserve: Balance,
	out_reserve: Balance,
	in_weight: LBPWeight,
	out_weight: LBPWeight,
	amount: Balance,
) -> Result<Balance, MathError> {
	let weight_ratio = div_to_fixed(out_weight.into(), in_weight.into(), Rounding::Down).ok_or(Overflow)?;

	let new_out_reserve = out_reserve.checked_sub(amount).ok_or(Overflow)?;

	let y = div_to_fixed(out_reserve, new_out_reserve, Rounding::Up).ok_or(Overflow)?;

	let y1: U32F96 = crate::transcendental::pow(y, weight_ratio).map_err(|_| Overflow)?;

	let y2 = y1.checked_sub(U32F96::one()).ok_or(Overflow)?;

	let r = mul_to_balance(in_reserve, y2, Rounding::Up).ok_or(Overflow)?;

	// Mysterious off-by-one error popped up in tests. Rounding this up to cover all possible errors.
	Ok(r.saturating_add(1))
}

/// Calculating weight at any given block in an interval using linear interpolation.
///
/// - `start_x` - beginning of an interval
/// - `end_x` - end of an interval
/// - `start_y` - initial weight
/// - `end_y` - final weight
/// - `at` - block number at which to calculate the weight
pub fn calculate_linear_weights<BlockNumber: num_traits::CheckedSub + TryInto<u32> + TryInto<u128>>(
	start_x: BlockNumber,
	end_x: BlockNumber,
	start_y: LBPWeight,
	end_y: LBPWeight,
	at: BlockNumber,
) -> Result<LBPWeight, MathError> {
	let d1 = end_x.checked_sub(&at).ok_or(Overflow)?;
	let d2 = at.checked_sub(&start_x).ok_or(Overflow)?;
	let dx = end_x.checked_sub(&start_x).ok_or(Overflow)?;

	let dx: u32 = dx.try_into().map_err(|_| Overflow)?;
	// if dx fits into u32, d1 and d2 fit into u128
	let d1: u128 = d1.try_into().map_err(|_| Overflow)?;
	let d2: u128 = d2.try_into().map_err(|_| Overflow)?;

	ensure!(dx != 0, ZeroDuration);

	let (start_y, end_y, d1, d2) = to_u256!(start_y, end_y, d1, d2);

	let left_part = start_y.checked_mul(d1).ok_or(Overflow)?;
	let right_part = end_y.checked_mul(d2).ok_or(Overflow)?;
	let result = (left_part.checked_add(right_part).ok_or(Overflow)?)
		.checked_div(dx.into())
		.ok_or(Overflow)?;

	to_lbp_weight!(result)
}

/// Create a fixed point number based on two `u128` values. Divides the values and rounds according to `r`.
pub(crate) fn div_to_fixed(num: u128, denom: u128, r: Rounding) -> Option<U32F96> {
	let bits = multiply_by_rational_with_rounding(num, U32F96::one().to_bits(), denom, r)?;
	Some(U32F96::from_bits(bits))
}

/// Multiply a `balance` with a `fixed` number and return a balance. Rounds the implicit division by `r`.
pub(crate) fn mul_to_balance(balance: u128, fixed: U32F96, r: Rounding) -> Option<Balance> {
	multiply_by_rational_with_rounding(balance, fixed.to_bits(), U32F96::one().to_bits(), r)
}
