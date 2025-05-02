use crate::types::CoefficientRatio;
use crate::types::{Balance, PegType, Price};
use hydra_dx_math::ratio::Ratio;
use num_traits::SaturatingAdd;
use num_traits::SaturatingMul;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{FixedPointNumber, FixedU128, Perbill, Permill};
use sp_runtime::{Rounding, Saturating};

/// Calculate purchase price for Hollar with collateral asset
/// p_i = (1 + fee_i) / peg_i
pub fn calculate_purchase_price(peg: PegType, fee: Permill) -> Price {
	let fee_ratio: Ratio = (fee.deconstruct() as u128, Permill::one().deconstruct() as u128).into();
	let one_ratio: Ratio = Ratio::one();
	let peg_ratio: Ratio = peg.into();

	let price = one_ratio.saturating_add(&fee_ratio).saturating_div(&peg_ratio);
	(price.n, price.d)
}

/// Calculate imbalance of a stablepool
/// I_i = (H_i - peg_i * R_i) / 2
pub fn calculate_imbalance(hollar_reserve: Balance, peg: PegType, collateral_reserve: Balance) -> Option<Balance> {
	let pegged_collateral = multiply_by_rational_with_rounding(collateral_reserve, peg.0, peg.1, Rounding::Down)?;
	// If hollar reserve is less than pegged collateral, we're considering zero imbalance
	// as we only care about positive imbalance (excess Hollar in the pool)
	if hollar_reserve <= pegged_collateral {
		return Some(0);
	}

	Some(hollar_reserve.saturating_sub(pegged_collateral).saturating_div(2))
}

/// Calculate how much Hollar HSM can buy back in a single block
/// B_i = b_i * I_i
pub fn calculate_buyback_limit(imbalance: Balance, b: Perbill) -> Balance {
	b.mul_floor(imbalance)
}

/// Calculate the final buy price with fee adjustment
/// p = p_e / (1 - f_i)
pub fn calculate_buy_price_with_fee(execution_price: Price, buy_back_fee: Permill) -> Option<PegType> {
	if buy_back_fee.is_one() {
		return None;
	}
	let exec_price_ratio: Ratio = execution_price.into();
	let fee_ratio: Ratio = (
		Permill::one().saturating_sub(buy_back_fee).deconstruct() as u128,
		Permill::one().deconstruct() as u128,
	)
		.into();
	let result = exec_price_ratio.saturating_div(&fee_ratio);
	Some((result.n, result.d))
}

/// Calculate max buy price
/// p_m = coefficient * peg
pub fn calculate_max_buy_price(peg: PegType, coefficient: CoefficientRatio) -> Price {
	let peg_ratio: Ratio = peg.into();
	let c_ratio: Ratio = (coefficient.into_inner(), FixedU128::DIV).into();
	let result = peg_ratio.saturating_mul(&c_ratio);
	(result.n, result.d)
}

/// Calculate how much collateral asset user receives for amount of Hollar
/// ΔR_i = p * ΔH
pub fn calculate_collateral_amount(hollar_amount: Balance, price: Price) -> Option<Balance> {
	multiply_by_rational_with_rounding(hollar_amount, price.0, price.1, Rounding::Up)
}

/// Calculate the amount of Hollar received for a given amount of collateral
/// ΔH = ΔR_i / p_i
pub fn calculate_hollar_amount(collateral_amount: Balance, purchase_price: Price) -> Option<Balance> {
	multiply_by_rational_with_rounding(collateral_amount, purchase_price.1, purchase_price.0, Rounding::Down)
}

use primitive_types::U128;
pub fn ensure_max_price(buy_price: Price, max_price: Price) -> bool {
	let buy_price_check = U128::from(buy_price.0).full_mul(U128::from(max_price.1));
	let max_price_check = U128::from(buy_price.1).full_mul(U128::from(max_price.0));
	buy_price_check <= max_price_check
}
