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

// Aave reserve configuration bitmap: liquidation threshold in bits [16..31], liquidation bonus
// in bits [32..47], decimals in bits [48..55].
fn reserve_config(threshold_bps: u128, bonus_bps: u128, decimals: u8) -> U256 {
	(U256::from(threshold_bps) << 16) | (U256::from(bonus_bps) << 32) | (U256::from(decimals) << 48)
}

fn reserve(idx: usize, addr: u8, price: u128, existential_deposit: u128, configuration: U256) -> Reserve {
	Reserve {
		idx,
		data: ReserveData {
			configuration,
			liquidity_index: 0,
			current_liquidity_rate: 0,
			variable_borrow_index: 0,
			current_variable_borrow_rate: 0,
			last_update_timestamp: 0,
			a_token_address: H160::zero(),
			stable_debt_token_address: H160::zero(),
			variable_debt_token_address: H160::zero(),
		},
		address: H160::repeat_byte(addr),
		asset_id: idx as u32,
		symbol: "TST".to_string(),
		price: U256::from(price),
		existential_deposit,
		emode: None,
	}
}

// A borrower far below HF 1 whose theoretical seize (1050) exceeds the collateral they actually
// hold (100), forcing `calc_debt_to_liquidate` down its clamp branch. Both reserves use the
// oracle's 8 decimals and a price of 1.0, so asset amounts and base-currency amounts coincide and
// the clamp arithmetic stays legible.
fn insufficient_collateral_fixture(collateral_ed: u128, debt_ed: u128) -> (MoneyMarket, Borrower, Reserve, Reserve) {
	let collateral = reserve(0, 0x01, 100_000_000, collateral_ed, reserve_config(8_000, 10_500, 8));
	let debt = reserve(1, 0x02, 100_000_000, debt_ed, reserve_config(0, 10_000, 8));

	let mut reserves = HashMap::new();
	reserves.insert(collateral.address, collateral.clone());
	reserves.insert(debt.address, debt.clone());

	let mm = MoneyMarket {
		pool: H160::zero(),
		oracle: H160::zero(),
		reserves,
		poisoned: Vec::new(),
	};

	let borrower = Borrower {
		configuration: UserConfiguration(U256::zero()),
		address: H160::repeat_byte(0xAB),
		reserves: vec![
			Some(UserReserve {
				collateral: U256::from(100u8),
				debt: U256::zero(),
			}),
			Some(UserReserve {
				collateral: U256::zero(),
				debt: U256::from(1_000u16),
			}),
		],
		emode_id: None,
		total_debt: U256::from(1_000u16),
		total_collateral: U256::from(100u8),
		updated_at: 0,
	};

	(mm, borrower, collateral, debt)
}

// The clamp branch must report the CLAMPED seize, not the pre-clamp theoretical one: shadowing
// the binding left `collateral_amount` at 1050 while `collateral_in_base_currency` was already
// the clamped 100, so the two disagreed by the bonus-inflated overshoot.
#[test]
fn calc_debt_to_liquidate_should_return_clamped_amount_when_collateral_is_insufficient() {
	let (mm, borrower, collateral, debt) = insufficient_collateral_fixture(1, 1);

	let amounts = mm
		.calc_debt_to_liquidate(&borrower, U256::from(TARGET_HF), &collateral, &debt)
		.expect("clamped liquidation amounts");

	assert_eq!(
		amounts,
		LiquidationAmounts {
			debt_amount: U256::from(95u8),
			collateral_amount: U256::from(100u8),
			debt_in_base_currency: U256::from(95u8),
			collateral_in_base_currency: U256::from(100u8),
		}
	);
}

// The ED guard decides on the clamped seize. With ED = 500 the clamped 100 is dust and the
// option must be dropped; reading the pre-clamp 1050 instead passed the guard and submitted a
// liquidation that dusts on-chain.
#[test]
fn calc_debt_to_liquidate_should_fail_when_clamped_seize_is_below_existential_deposit() {
	let (mm, borrower, collateral, debt) = insufficient_collateral_fixture(500, 1);

	let result = mm.calc_debt_to_liquidate(&borrower, U256::from(TARGET_HF), &collateral, &debt);

	assert!(matches!(result, Err(Error::LiquidationBelowED)));
}

// `decimals` is an unvalidated byte of the on-chain configuration word and `uint`'s `pow` panics
// on overflow even in release, so an out-of-range value must surface as an error — a panic here
// unwinds out of the scoped scan threads and kills the worker for the rest of the process life.
#[test]
fn calc_debt_to_liquidate_should_fail_when_debt_decimals_are_out_of_range() {
	let (mm, borrower, collateral, mut debt) = insufficient_collateral_fixture(1, 1);
	debt.data.configuration = reserve_config(0, 10_000, 200);

	let result = mm.calc_debt_to_liquidate(&borrower, U256::from(TARGET_HF), &collateral, &debt);

	assert!(matches!(result, Err(Error::Arithmetic("decimals out of range"))));
}

#[test]
fn calc_debt_to_liquidate_should_fail_when_collateral_decimals_are_out_of_range() {
	let (mm, borrower, mut collateral, debt) = insufficient_collateral_fixture(1, 1);
	collateral.data.configuration = reserve_config(8_000, 10_500, 200);

	let result = mm.calc_debt_to_liquidate(&borrower, U256::from(TARGET_HF), &collateral, &debt);

	assert!(matches!(result, Err(Error::Arithmetic("decimals out of range"))));
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
