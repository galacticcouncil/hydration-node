use super::*;
use crate::engine::{Instruction, OmniXEngine};
use crate::tests::{ExtBuilder, OmniX};
use crate::types::{BoundedInstructions, BoundedResolvedIntents, ResolvedIntent, Solution, Swap, SwapType};
use frame_support::assert_ok;
use frame_support::pallet_prelude::Weight;

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
		score: 0,
		weight: Default::default(),
	}
}

#[test]
fn test_validate_solution_with_one_intent() {
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

		let mut solution = create_solution(
			vec![ResolvedIntent {
				intent_id,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
			}],
			vec![],
		);

		let plan = OmniXEngine::<Test, Tokens, DummyTradeExecutor>::validate_solution(&mut solution);

		assert!(plan.is_ok());
	});
}
