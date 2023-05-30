use super::super::test_utils::assert_approx_eq;
use crate::lbp::lbp;
use primitive_types::U256;
use proptest::prelude::*;
use rand::Rng;

const MIN_START_BLOCK: u32 = 0;
const MAX_START_BLOCK: u32 = 10_000_000;
const ONE_DAY_IN_BLOCKS: u32 = 7200;
const ONE_WEEK_IN_BLOCKS: u32 = 50400;

fn start_blocks() -> impl Strategy<Value = u32> {
	MIN_START_BLOCK..MAX_START_BLOCK
}

fn lbp_length() -> impl Strategy<Value = u32> {
	ONE_DAY_IN_BLOCKS..ONE_WEEK_IN_BLOCKS
}

fn initial_weight() -> impl Strategy<Value = u32> {
	1_000_000..10_000_000u32
}

fn final_weight() -> impl Strategy<Value = u32> {
	10_000_001u32..100_000_000u32
}

//Spec: https://www.notion.so/Property-Tests-7b506add39ea48fc8f68ecd18391e30a#9bbed73541c84e45a9855360aeee1f9b
proptest! {
	#![proptest_config(ProptestConfig::with_cases(10000))]
	#[test]
	fn calculate_linear_weights2(
		start_x_block in start_blocks(),
		lbp_length in lbp_length(),
		start_y_weight in initial_weight(),
		end_y_weight in final_weight()) {
		//Arrange
		let end_x_block = start_x_block.checked_add(lbp_length).unwrap();
		let at_block = rand::thread_rng().gen_range(start_x_block..end_x_block);

		//Act
		let weight  = lbp::calculate_linear_weights(start_x_block,end_x_block,start_y_weight,end_y_weight,at_block).unwrap();

		//Assert
		let a1 = U256::from(at_block.checked_sub(start_x_block).unwrap());
		let a2 = U256::from(end_y_weight.checked_sub(start_y_weight).unwrap());

		let b1 = U256::from(weight.checked_sub(start_y_weight).unwrap());
		let b2 = U256::from(end_x_block.checked_sub(start_x_block).unwrap());

		let max_delta = U256::from(lbp_length); //As the rounding error scales linearly with the length of the LB
		assert_approx_eq!(a1*a2, b1*b2, max_delta, "The invariant does not hold")
	}
}
