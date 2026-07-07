use crate::types::*;
use sp_core::{H160, U256};
use std::collections::HashMap;

fn money_market() -> MoneyMarket {
	MoneyMarket {
		pool: H160::zero(),
		oracle: H160::zero(),
		reserves: HashMap::new(),
		poisoned: Vec::new(),
	}
}

fn borrower(total_collateral: U256, total_debt: U256) -> Borrower {
	Borrower {
		configuration: UserConfiguration(U256::zero()),
		address: H160::repeat_byte(0xAB),
		reserves: Vec::new(),
		emode_id: None,
		total_debt,
		total_collateral,
		updated_at: 0,
	}
}

// Mirrors Aave: no debt -> `type(uint256).max` health factor, nothing to liquidate.
#[test]
fn calc_health_factor_should_return_max_when_debt_is_zero() {
	let b = borrower(U256::from(100_000_000u128), U256::zero());

	assert_eq!(b.calc_health_factor(&money_market()).expect("hf"), U256::MAX);
}

// Debt with no collateral (e.g. a simulated full seize) is maximally unhealthy, not an error —
// erroring here silently dropped the only viable liquidation option for deeply underwater
// borrowers in `calculate_liquidation_options`.
#[test]
fn calc_health_factor_should_return_zero_when_collateral_is_zero() {
	let b = borrower(U256::zero(), U256::from(100_000_000u128));

	assert_eq!(b.calc_health_factor(&money_market()).expect("hf"), U256::zero());
}

const TARGET_HF: u128 = 1_001_000_000_000_000_000; // 1.001

fn option(health_factor: U256, marker: u8) -> LiquidationOption {
	LiquidationOption {
		health_factor,
		collateral_asset: H160::repeat_byte(marker),
		debt_asset: H160::repeat_byte(marker),
		debt_to_liquidate: U256::from(1u8),
	}
}

// Partial-to-target design: an option landing just below the target beats a simulated full
// debt repay (HF = U256::MAX) — full repay seizes more collateral than necessary.
#[test]
fn select_best_should_prefer_partial_to_target_over_full_repay() {
	let exact = option(U256::from(1_000_900_000_000_000_000u128), 1);
	let full_repay = option(U256::MAX, 2);

	let best = select_best_liquidation_option(vec![full_repay, exact.clone()], U256::from(TARGET_HF));

	assert_eq!(best, Some(exact));
}

// A full repay must survive as the sole option — it must not be dropped on a divide-by-zero.
#[test]
fn select_best_should_return_full_repay_when_it_is_the_only_option() {
	let full_repay = option(U256::MAX, 2);

	let best = select_best_liquidation_option(vec![full_repay.clone()], U256::from(TARGET_HF));

	assert_eq!(best, Some(full_repay));
}

// When every option overshoots the target, take the smallest overshoot.
#[test]
fn select_best_should_pick_smallest_overshoot_when_all_options_exceed_target() {
	let small_overshoot = option(U256::from(1_200_000_000_000_000_000u128), 1);
	let full_repay = option(U256::MAX, 2);

	let best = select_best_liquidation_option(vec![full_repay, small_overshoot.clone()], U256::from(TARGET_HF));

	assert_eq!(best, Some(small_overshoot));
}

// An option that heals the position (HF > target) beats one that leaves it liquidatable
// (HF < 1.0, e.g. close-factor-capped) — matches v1's behaviour on this case.
#[test]
fn select_best_should_prefer_healing_overshoot_over_unhealthy_partial() {
	let unhealthy_partial = option(U256::from(970_000_000_000_000_000u128), 1);
	let healing_overshoot = option(U256::from(1_050_000_000_000_000_000u128), 2);

	let best = select_best_liquidation_option(
		vec![unhealthy_partial, healing_overshoot.clone()],
		U256::from(TARGET_HF),
	);

	assert_eq!(best, Some(healing_overshoot));
}

// When no option heals the position, take the highest HF (best effort); the per-block re-scan
// drives the next round.
#[test]
fn select_best_should_pick_highest_hf_when_no_option_heals() {
	let worse = option(U256::from(900_000_000_000_000_000u128), 1);
	let better = option(U256::from(970_000_000_000_000_000u128), 2);

	let best = select_best_liquidation_option(vec![better.clone(), worse], U256::from(TARGET_HF));

	assert_eq!(best, Some(better));
}

// Bit layout: one pair per reserve index — bit `2*idx` = debt, bit `2*idx + 1` = collateral.
#[test]
fn user_configuration_uses_any_should_detect_collateral_and_debt_bits() {
	let collateral_at_1 = UserConfiguration(U256::from(0b1000));
	let debt_at_2 = UserConfiguration(U256::from(0b10000));

	assert!(collateral_at_1.uses_any(&[1]));
	assert!(!collateral_at_1.uses_any(&[0, 2]));
	assert!(debt_at_2.uses_any(&[2]));
	assert!(!debt_at_2.uses_any(&[0, 1]));
	assert!(!UserConfiguration(U256::zero()).uses_any(&[0, 1, 2]));
}
