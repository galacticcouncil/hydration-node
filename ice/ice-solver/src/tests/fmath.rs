//! Exactness and failure-mode tests for the shared arithmetic and the pair
//! flow analysis.

use crate::common::{analyze_pair_flow, calc_amount_out, mul_div, mul_div_balance, FlowDirection};
use hydra_dx_math::types::Ratio;
use sp_core::U256;

fn u256(v: u128) -> U256 {
	U256::from(v)
}

#[test]
fn mul_div_should_return_none_when_divisor_is_zero() {
	assert_eq!(mul_div(u256(10), u256(10), U256::zero()), None);
	assert_eq!(mul_div_balance(10, 10, 0), None);
}

#[test]
fn mul_div_should_stay_exact_when_the_product_exceeds_u256() {
	// a·b needs 262 bits, far past U256. A split-and-recombine fallback computes
	// (a/c)·b = 2048 and then drops the (a%c)·b/c correction because that
	// product overflows too — losing the remaining 1023.
	let c = U256::one() << 250;
	let a = (U256::one() << 251) + ((U256::one() << 250) - U256::one());
	let b = u256(1_024);
	assert_eq!(mul_div(a, b, c), Some(u256(3_071)));
}

#[test]
fn mul_div_should_return_none_when_the_quotient_exceeds_u256() {
	assert_eq!(mul_div(U256::MAX, u256(2), U256::one()), None);
}

#[test]
fn mul_div_balance_should_return_none_when_the_quotient_exceeds_u128() {
	assert_eq!(mul_div_balance(u128::MAX, u128::MAX, 1), None);
	assert_eq!(mul_div_balance(u128::MAX, 2, 2), Some(u128::MAX));
}

#[test]
fn calc_amount_out_should_stay_exact_when_intermediate_products_exceed_u256() {
	// price_in.n · price_out.d = 2^200 + 2^190 and price_in.d · price_out.n =
	// 2^200, so the exact answer is amount·(1 + 2^-10). amount·numerator is
	// 2^300: the direct order overflows and every recombination that divides
	// early loses the 2^90 correction term.
	let amount: u128 = 1 << 100;
	let price_in = Ratio::new(1_025u128 << 95, 1u128 << 100);
	let price_out = Ratio::new(1u128 << 100, 1u128 << 95);
	assert_eq!(
		calc_amount_out(amount, &price_in, &price_out),
		Some((1u128 << 100) + (1u128 << 90)),
	);
}

#[test]
fn calc_amount_out_should_return_none_when_result_exceeds_u128() {
	let price_in = Ratio::new(u128::MAX, 1);
	let price_out = Ratio::new(1, u128::MAX);
	assert_eq!(calc_amount_out(u128::MAX, &price_in, &price_out), None);
}

#[test]
fn calc_amount_out_should_return_none_when_a_price_denominator_is_zero() {
	let price_in = Ratio::new(1, 1);
	assert_eq!(calc_amount_out(100, &price_in, &Ratio::new(0, 1)), None);
}

#[test]
fn calc_amount_out_should_convert_at_the_price_ratio_when_values_are_small() {
	// asset in is worth 2 units of the numeraire, asset out 1 → 1 in buys 2 out.
	assert_eq!(calc_amount_out(100, &Ratio::new(2, 1), &Ratio::new(1, 1)), Some(200));
}

#[test]
fn analyze_pair_flow_should_report_single_direction_when_only_one_side_has_volume() {
	let p = Ratio::new(1, 1);
	assert!(matches!(
		analyze_pair_flow(100, 0, &p, &p),
		Some(FlowDirection::SingleForward { amount: 100 })
	));
	assert!(matches!(
		analyze_pair_flow(0, 100, &p, &p),
		Some(FlowDirection::SingleBackward { amount: 100 })
	));
}

#[test]
fn analyze_pair_flow_should_report_perfect_cancel_when_values_match() {
	let p = Ratio::new(1, 1);
	assert!(matches!(
		analyze_pair_flow(100, 100, &p, &p),
		Some(FlowDirection::PerfectCancel {
			a_as_b: 100,
			b_as_a: 100
		})
	));
}

#[test]
fn analyze_pair_flow_should_route_the_excess_when_one_side_is_larger() {
	let p = Ratio::new(1, 1);
	assert!(matches!(
		analyze_pair_flow(150, 100, &p, &p),
		Some(FlowDirection::ExcessForward {
			scarce_out: 100,
			direct_match: 100,
			net_sell: 50
		})
	));
	assert!(matches!(
		analyze_pair_flow(100, 150, &p, &p),
		Some(FlowDirection::ExcessBackward {
			scarce_out: 100,
			direct_match: 100,
			net_sell: 50
		})
	));
}

#[test]
fn analyze_pair_flow_should_return_none_when_the_pair_cannot_be_valued() {
	// The conversion of A into B does not fit 128 bits. Reading that as a
	// zero-valued flow would classify the pair as one-sided excess and pay the
	// scarce side nothing.
	let pa = Ratio::new(u128::MAX, 1);
	let pb = Ratio::new(1, u128::MAX);
	assert!(analyze_pair_flow(u128::MAX, 100, &pa, &pb).is_none());
}
