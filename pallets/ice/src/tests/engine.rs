use super::*;
use crate::engine::ICEEngine;
use crate::tests::{ExtBuilder, ICE};
use crate::types::{
	BoundedInstructions, BoundedResolvedIntents, Instruction, ResolvedIntent, Solution, Swap, SwapType,
};
use frame_support::assert_ok;

fn create_solution(
	intents: Vec<ResolvedIntent>,
	instructions: Vec<Instruction<AccountId, AssetId>>,
) -> Solution<AccountId, AssetId> {
	let intents = BoundedResolvedIntents::try_from(intents).unwrap();
	let instructions = BoundedInstructions::try_from(instructions).unwrap();
	Solution {
		proposer: ALICE,
		intents,
		instructions,
		score: 1000000,
	}
}

#[test]
fn validate_solution_should_work_when_solution_contains_one_intent_swap_exact_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Swap {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
					swap_type: SwapType::ExactIn
				},
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let solution = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![],
			);

			assert_ok!(ICEEngine::<Test>::validate_solution(&solution));
		});
}

#[test]
fn validate_solution_should_fail_when_solution_does_not_correctly_transfer_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Swap {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
					swap_type: SwapType::ExactIn
				},
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let mut solution = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![],
			);

			assert!(ICEEngine::<Test>::validate_solution(&mut solution).is_err());
		});
}

#[test]
fn validate_solution_should_fail_when_solution_does_not_correctly_transfer_out() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Swap {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
					swap_type: SwapType::ExactIn
				},
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let mut solution = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![],
			);

			assert!(ICEEngine::<Test>::validate_solution(&mut solution).is_err());
		});
}

#[test]
fn validate_solution_should_fail_when_solution_contains_intent_updated_but_not_resolved() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Swap {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
					swap_type: SwapType::ExactIn
				},
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let mut solution = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![],
			);

			assert!(ICEEngine::<Test>::validate_solution(&mut solution).is_err());
		});
}

#[test]
fn validate_solution_should_return_correct_matched_amounts() {}

#[test]
fn validate_solution_should_fail_when_resolved_intent_does_exist() {}

#[test]
fn validate_solution_should_fail_when_resolved_intent_is_already_past_deadline() {}

#[test]
fn validate_solution_should_fail_when_limit_price_is_not_respected_in_partial_intent() {}

#[test]
fn validate_solution_should_fail_when_contains_incorrect_transfer_in_amount_in_exact_in_swap() {}

#[test]
fn validate_solution_should_fail_when_contains_incorrect_transfer_in_amount_in_exact_out_swap() {}

#[test]
fn validate_solution_should_fail_when_contains_incorrect_transfer_out_amount_in_exact_in_swap() {}

#[test]
fn validate_solution_should_fail_when_contains_incorrect_transfer_out_amount_in_exact_out_swap() {}
