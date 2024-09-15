use super::*;
use crate::engine::{BoundedRoute, Instruction};
use crate::pallet::Intents;
use crate::tests::{ExtBuilder, ICE};
use crate::types::{
	BoundedInstructions, BoundedResolvedIntents, Intent, ProposedSolution, ResolvedIntent, Swap, SwapType,
};
use crate::Error;
use frame_support::{assert_noop, assert_ok};

fn create_solution_for_given_intents(intents: Vec<IntentId>) -> (ProposedSolution<AccountId, AssetId>, u64) {
	// TODO: extend to support multiple intents
	// currently only one intent is supported

	let intent_id = intents[0];

	let resolved_intents = vec![ResolvedIntent {
		intent_id,
		amount_in: 100_000_000_000_000,
		amount_out: 200_000_000_000_000,
	}];
	let route = vec![];

	let instructions = vec![
		Instruction::TransferIn {
			who: ALICE,
			asset_id: 100,
			amount: 100_000_000_000_000,
		},
		Instruction::SwapExactIn {
			asset_in: 100,
			asset_out: 200,
			amount_in: 100_000_000_000_000,
			amount_out: 200_000_000_000_000,
			route: BoundedRoute::try_from(route).unwrap(),
		},
		Instruction::TransferOut {
			who: ALICE,
			asset_id: 200,
			amount: 200_000_000_000_000,
		},
	];

	let proposed_solution = ProposedSolution {
		intents: BoundedResolvedIntents::try_from(resolved_intents).unwrap(),
		instructions: BoundedInstructions::try_from(instructions).unwrap(),
	};
	let score = 1_000_000u64;

	(proposed_solution, score)
}

#[test]
fn submit_solution_should_work_when_contains_only_one_intent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (proposed_solution, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				proposed_solution,
				score,
				1
			));
		});
}

#[test]
fn submit_solution_should_fail_when_block_number_is_different() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let (proposed_solution, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), proposed_solution, score, 2),
				Error::<Test>::InvalidBlockNumber
			);
		});
}

#[test]
fn submit_solution_should_fail_when_score_is_different() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let (proposed_solution, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), proposed_solution, score + 1, 1),
				Error::<Test>::InvalidScore
			);
		});
}

#[test]
fn submit_solution_should_clear_expired_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(BOB),
				swap.clone(),
				DEFAULT_NOW + 1_000,
				false,
				None,
				None,
			));
			let expired_intent_id = get_intent_id(DEFAULT_NOW + 1_000, 1);

			let (proposed_solution, score) = create_solution_for_given_intents(vec![intent_id]);

			// move time forward
			NOW.with(|now| {
				*now.borrow_mut() += 2000;
			});

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				proposed_solution,
				score,
				1
			),);
			let intent = Intents::<Test>::get(&expired_intent_id);
			assert_eq!(intent, None);
		});
}

#[test]
fn submit_solution_should_update_partial_resolved_intent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				true,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let resolved_intents = vec![ResolvedIntent {
				intent_id,
				amount_in: 100_000_000_000_000 / 2,
				amount_out: 200_000_000_000_000 / 2,
			}];
			let route = vec![];

			let instructions = vec![
				Instruction::TransferIn {
					who: ALICE,
					asset_id: 100,
					amount: 100_000_000_000_000 / 2,
				},
				Instruction::SwapExactIn {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000 / 2,
					amount_out: 200_000_000_000_000 / 2,
					route: BoundedRoute::try_from(route).unwrap(),
				},
				Instruction::TransferOut {
					who: ALICE,
					asset_id: 200,
					amount: 200_000_000_000_000 / 2,
				},
			];

			let proposed_solution = ProposedSolution {
				intents: BoundedResolvedIntents::try_from(resolved_intents).unwrap(),
				instructions: BoundedInstructions::try_from(instructions).unwrap(),
			};
			let score = 1_000_000u64;

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				proposed_solution,
				score,
				1
			));

			let intent = Intents::<Test>::get(&intent_id);
			assert_eq!(
				intent,
				Some(Intent {
					who: ALICE,
					swap: Swap {
						asset_in: 100,
						asset_out: 200,
						amount_in: 100_000_000_000_000 / 2,
						amount_out: 200_000_000_000_000 / 2,
						swap_type: SwapType::ExactIn,
					},
					deadline: DEFAULT_NOW + 1_000_000,
					partial: true,
					on_success: None,
					on_failure: None,
				})
			);
		});
}

#[test]
fn submit_solution_should_fail_when_intent_does_not_exists() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (proposed_solution, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), proposed_solution, score, 1),
				Error::<Test>::IntentNotFound
			);
		});
}
