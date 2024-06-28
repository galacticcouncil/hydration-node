use super::*;
use crate::engine::{ExecutionPlan, Instruction, OmniXEngine};
use crate::tests::{ExtBuilder, OmniX};
use crate::types::{
	BoundedInstructions, BoundedPrices, BoundedResolvedIntents, ResolvedIntent, Solution, Swap, SwapType,
};
use frame_support::assert_ok;
use frame_support::pallet_prelude::Weight;

fn create_solution(
	intents: Vec<ResolvedIntent>,
	sell_prices: Vec<(AssetId, (u128, u128))>,
	buy_prices: Vec<(AssetId, (u128, u128))>,
) -> Solution<AccountId, AssetId> {
	let intents = BoundedResolvedIntents::try_from(intents).unwrap();
	let buy_prices = BoundedPrices::try_from(buy_prices).unwrap();
	let sell_prices = BoundedPrices::try_from(sell_prices).unwrap();
	Solution {
		proposer: ALICE,
		intents,
		sell_prices,
		buy_prices,
	}
}

fn create_plan(instructions: Vec<Instruction<AccountId, AssetId>>) -> ExecutionPlan<AccountId, AssetId> {
	let instructions = BoundedInstructions::try_from(instructions).unwrap();
	ExecutionPlan {
		instructions,
		weight: Weight::default(),
	}
}

#[test]
fn test_prepare_solution_with_one_intent() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(OmniX::submit_intent(
			RuntimeOrigin::signed(ALICE),
			Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn
			},
			NOW,
			false,
			None,
			None,
		));

		let intent_id = get_intent_id(NOW, 0);

		let solution = create_solution(
			vec![ResolvedIntent {
				intent_id,
				amount: 100_000_000_000_000,
			}],
			vec![(100, (1, 1)), (200, (1, 1))],
			vec![(100, (2, 1)), (200, (1, 2))],
		);

		let plan = OmniXEngine::<Test, Tokens, DummyTradeExecutor>::prepare_execution_plan(&solution);

		assert!(plan.is_ok());

		let plan = plan.unwrap();

		let expected_plan = create_plan(vec![
			Instruction::TransferIn {
				asset_id: 100,
				who: ALICE,
				amount: 100_000_000_000_000,
			},
			Instruction::HubSwap {
				asset_in: 100,
				asset_out: 1,
				amount_in: 100_000_000_000_000,
				amount_out: 0,
			},
			Instruction::HubSwap {
				asset_in: 1,
				asset_out: 200,
				amount_in: Balance::MAX,
				amount_out: 200_000_000_000_000,
			},
			Instruction::TransferOut {
				asset_id: 200,
				who: ALICE,
				amount: 200_000_000_000_000,
			},
		]);

		assert_eq!(plan, expected_plan);
	});
}

#[test]
fn test_prepare_solution_with_two_intents() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(OmniX::submit_intent(
			RuntimeOrigin::signed(ALICE),
			Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn
			},
			NOW,
			false,
			None,
			None,
		));
		assert_ok!(OmniX::submit_intent(
			RuntimeOrigin::signed(BOB),
			Swap {
				asset_in: 200,
				asset_out: 100,
				amount_in: 200_000_000_000_000,
				amount_out: 100_000_000_000_000,
				swap_type: SwapType::ExactOut
			},
			NOW,
			false,
			None,
			None,
		));

		let intent_id_1 = get_intent_id(NOW, 0);
		let intent_id_2 = get_intent_id(NOW, 1);

		let solution = create_solution(
			vec![
				ResolvedIntent {
					intent_id: intent_id_1,
					amount: 100_000_000_000_000,
				},
				ResolvedIntent {
					intent_id: intent_id_2,
					amount: 100_000_000_000_000,
				},
			],
			vec![(100, (1, 1)), (200, (1, 1))],
			vec![(100, (2, 1)), (200, (1, 2))],
		);

		let plan = OmniXEngine::<Test, Tokens, DummyTradeExecutor>::prepare_execution_plan(&solution);

		assert!(plan.is_ok());

		let plan = plan.unwrap();

		let expected_plan = create_plan(vec![
			Instruction::TransferIn {
				asset_id: 100,
				who: ALICE,
				amount: 100_000_000_000_000,
			},
			Instruction::TransferIn {
				asset_id: 200,
				who: BOB,
				amount: 200_000_000_000_000,
			},
			Instruction::TransferOut {
				asset_id: 200,
				who: ALICE,
				amount: 200_000_000_000_000,
			},
			Instruction::TransferOut {
				asset_id: 100,
				who: BOB,
				amount: 100_000_000_000_000,
			},
		]);

		assert_eq!(plan, expected_plan);
	});
}
