use crate::types::Balance;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{ArithmeticError, Perbill, Permill};
use sp_runtime::{Rounding, Saturating};

/// Peg type is a ratio of (numerator, denominator)
pub type PegType = (Balance, Balance);

/// Calculate purchase price for Hollar with collateral asset
/// p_i = (1 + fee_i) * peg_i
pub fn calculate_purchase_price(peg: PegType, fee: Permill) -> PegType {
	// Apply fee to the numerator first: (1 + fee) * peg.0
	let numerator_with_fee = peg.0.saturating_add(fee.mul_floor(peg.0));

	// Return as a ratio (numerator, denominator)
	(numerator_with_fee, peg.1)
}

/// Calculate the amount of Hollar received for a given amount of collateral
/// ΔH = ΔR_i / p_i
pub fn calculate_hollar_amount(
	collateral_amount: Balance,
	purchase_price: PegType,
) -> Result<Balance, ArithmeticError> {
	// Convert purchase_price to a price by dividing numerator by denominator
	let price = purchase_price
		.0
		.checked_div(purchase_price.1)
		.ok_or(ArithmeticError::DivisionByZero)?;

	collateral_amount
		.checked_div(price)
		.ok_or(ArithmeticError::DivisionByZero)
}

/// Calculate imbalance of a stablepool
/// I_i = (H_i - peg_i * R_i) / 2
pub fn calculate_imbalance(
	hollar_reserve: Balance,
	peg: PegType,
	collateral_reserve: Balance,
) -> Result<Balance, ArithmeticError> {
	//TODO: handler negative imbalance correctly


	// Convert peg to a price by dividing numerator by denominator
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
/// p_m = c_i * peg_i
pub fn calculate_max_buy_price(peg: PegType, coefficient: Permill) -> PegType {
	// Apply coefficient to the numerator
	let numerator = coefficient.mul_floor(peg.0);

	// Return the new ratio
	(numerator, peg.1)
}

/// Calculate how much collateral asset user receives for amount of Hollar
/// ΔR_i = p * ΔH
pub fn calculate_collateral_amount(hollar_amount: Balance, price: PegType) -> Option<Balance> {
	multiply_by_rational_with_rounding(hollar_amount, price.0, price.1, Rounding::Down)
}

/// Scale an amount to 18 decimals
pub fn scale_to_18_decimals(amount: Balance, asset_decimals: u8) -> Result<Balance, ArithmeticError> {
	if asset_decimals == 18 {
		return Ok(amount);
	} else if asset_decimals > 18 {
		// Scale down
		let scale_factor = 10u128.saturating_pow((asset_decimals - 18) as u32);
		amount.checked_div(scale_factor).ok_or(ArithmeticError::DivisionByZero)
	} else {
		// Scale up
		let scale_factor = 10u128.saturating_pow((18 - asset_decimals) as u32);
		amount.checked_mul(scale_factor).ok_or(ArithmeticError::Overflow)
	}
}

/// Scale an amount from 18 decimals back to asset's decimals
pub fn scale_from_18_decimals(amount: Balance, asset_decimals: u8) -> Result<Balance, ArithmeticError> {
	if asset_decimals == 18 {
		return Ok(amount);
	} else if asset_decimals > 18 {
		// Scale up
		let scale_factor = 10u128.saturating_pow((asset_decimals - 18) as u32);
		amount.checked_mul(scale_factor).ok_or(ArithmeticError::Overflow)
	} else {
		// Scale down
		let scale_factor = 10u128.saturating_pow((18 - asset_decimals) as u32);
		amount.checked_div(scale_factor).ok_or(ArithmeticError::DivisionByZero)
	}
}
