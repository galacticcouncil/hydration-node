use crate::types::Balance;
use crate::types::CoefficientRatio;
use hydra_dx_math::ratio::Ratio;
use num_traits::SaturatingAdd;
use num_traits::SaturatingMul;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{ArithmeticError, Perbill, Permill};
use sp_runtime::{Rounding, Saturating};

/// Peg type is a ratio of (numerator, denominator)
pub type PegType = (Balance, Balance);

/// Calculate purchase price for Hollar with collateral asset
/// p_i = (1 + fee_i) * peg_i
pub fn calculate_purchase_price(peg: PegType, fee: Permill) -> PegType {
	let fee_ratio: Ratio = (fee.deconstruct() as u128, Permill::one().deconstruct() as u128).into();
	let one_ratio: Ratio = Ratio::one();
	let peg_ratio: Ratio = peg.into();

	let price = one_ratio.saturating_add(&fee_ratio).saturating_mul(&peg_ratio);
	(price.n, price.d)
}

/// Calculate the amount of Hollar received for a given amount of collateral
/// ΔH = ΔR_i / p_i
pub fn calculate_hollar_amount(collateral_amount: Balance, purchase_price: PegType) -> Option<Balance> {
	multiply_by_rational_with_rounding(collateral_amount, purchase_price.1, purchase_price.0, Rounding::Down)
}

/// Calculate imbalance of a stablepool
/// I_i = (H_i - peg_i * R_i) / 2
pub fn calculate_imbalance(
	hollar_reserve: Balance,
	peg: PegType,
	collateral_reserve: Balance,
) -> Result<Balance, ArithmeticError> {
	// Convert peg to a price by dividing numerator by denominator
	//TODO: this is incorrect! should be fixed when decimals taken into account
	let peg_price = peg.0.checked_div(peg.1).ok_or(ArithmeticError::DivisionByZero)?;

	let pegged_collateral = peg_price
		.checked_mul(collateral_reserve)
		.ok_or(ArithmeticError::Overflow)?;

	// If hollar reserve is less than pegged collateral, we're considering zero imbalance
	// as we only care about positive imbalance (excess Hollar in the pool)
	if hollar_reserve <= pegged_collateral {
		return Ok(0);
	}

	hollar_reserve
		.checked_sub(pegged_collateral)
		.ok_or(ArithmeticError::Underflow)?
		.checked_div(2)
		.ok_or(ArithmeticError::DivisionByZero)
}

/// Calculate how much Hollar HSM can buy back in a single block
/// B_i = b_i * I_i
pub fn calculate_buyback_limit(imbalance: Balance, b: Perbill) -> Balance {
	b.mul_floor(imbalance)
}

/// Calculate the final buy price with fee adjustment
/// p = p_e / (1 - f_i)
pub fn calculate_buy_price_with_fee(
	execution_price: PegType,
	buy_back_fee: Permill,
) -> Result<PegType, ArithmeticError> {
	if buy_back_fee == Permill::one() {
		return Err(ArithmeticError::DivisionByZero);
	}

	let denominator = Permill::one().saturating_sub(buy_back_fee);
	let denominator_value = denominator.deconstruct() as Balance;

	// Scale the numerator by dividing by (1 - fee)
	let scaled_numerator = execution_price
		.0
		.checked_mul(Permill::one().deconstruct() as Balance)
		.ok_or(ArithmeticError::Overflow)?
		.checked_div(denominator_value)
		.ok_or(ArithmeticError::DivisionByZero)?;

	Ok((scaled_numerator, execution_price.1))
}

/// Calculate max buy price
/// p_m = coefficient * peg
/// Where coefficient is now a Ratio instead of Permill
pub fn calculate_max_buy_price(peg: PegType, coefficient: CoefficientRatio) -> PegType {
	// Multiply the two ratios
	// For (a,b) * (c,d) = (a*c, b*d)
	let numerator = peg.0.saturating_mul(coefficient.0);
	let denominator = peg.1.saturating_mul(coefficient.1);

	// Return the new ratio
	(numerator, denominator)
}

/// Calculate how much collateral asset user receives for amount of Hollar
/// ΔR_i = p * ΔH
pub fn calculate_collateral_amount(hollar_amount: Balance, price: PegType) -> Option<Balance> {
	multiply_by_rational_with_rounding(hollar_amount, price.0, price.1, Rounding::Up)
}
