use super::*;
use crate::engine::{ICEEngine, Instruction};
use crate::tests::{ExtBuilder, ICE};
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

			let plan = ICEEngine::<Test, Tokens, DummyTradeExecutor>::validate_solution(&mut solution);

			assert!(plan.is_ok());
		});
}
