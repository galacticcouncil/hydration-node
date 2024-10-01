use super::*;
use crate::engine::ICEEngine;
use crate::tests::{ExtBuilder, ICE};
use crate::types::{
	BoundedInstructions, BoundedResolvedIntents, BoundedRoute, BoundedTrades, Instruction, Intent, ResolvedIntent,
	Solution, Swap, SwapType, TradeInstruction,
};
use frame_support::assert_ok;

fn create_solution(
	intents: Vec<ResolvedIntent>,
	trades: Vec<TradeInstruction<AssetId>>,
) -> (BoundedResolvedIntents, BoundedTrades<AssetId>) {
	let intents = BoundedResolvedIntents::try_from(intents).unwrap();
	let trades = BoundedTrades::try_from(trades).unwrap();
	(intents, trades)
}

#[test]
fn preparee_solution_should_work_when_solution_contains_one_intent_swap_exact_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Intent {
					who: ALICE,
					swap: Swap {
						asset_in: 100,
						asset_out: 200,
						amount_in: 100_000_000_000_000,
						amount_out: 200_000_000_000_000,
						swap_type: SwapType::ExactIn,
					},
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (intents, trades) = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![],
			);

			let r = ICEEngine::<Test>::prepare_solution(intents, trades, 1000000);
			assert_ok!(r);
		});
}

#[test]
fn preparee_solution_should_return_correct_result_when_solution_is_valid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Intent {
					who: ALICE,
					swap: Swap {
						asset_in: 100,
						asset_out: 200,
						amount_in: 100_000_000_000_000,
						amount_out: 200_000_000_000_000,
						swap_type: SwapType::ExactIn,
					},
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (intents, trades) = create_solution(
				vec![ResolvedIntent {
					intent_id,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
				}],
				vec![TradeInstruction::SwapExactIn {
					asset_in: 100,
					asset_out: 200,
					amount_in: 100_000_000_000_000,
					amount_out: 200_000_000_000_000,
					route: BoundedRoute::try_from(vec![]).unwrap(),
				}],
			);

			let r = ICEEngine::<Test>::prepare_solution(intents, trades, 1000000);
			assert_ok!(&r);
			let solution = r.unwrap();
			let expected_intents = BoundedResolvedIntents::try_from(vec![ResolvedIntent {
				intent_id,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
			}])
			.unwrap();
			let expected_trades = BoundedInstructions::try_from(vec![
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
					route: BoundedRoute::try_from(vec![]).unwrap(),
				},
				Instruction::TransferOut {
					who: ALICE,
					asset_id: 200,
					amount: 200_000_000_000_000,
				},
			])
			.unwrap();
			assert_eq!(
				solution,
				Solution {
					intents: expected_intents,
					instructions: expected_trades,
				}
			);
		});
}

#[test]
fn validate_solution_should_fail_when_resolved_intent_does_exist() {}

#[test]
fn validate_solution_should_fail_when_resolved_intent_is_already_past_deadline() {}

#[test]
fn validate_solution_should_fail_when_limit_price_is_not_respected_in_partial_intent() {}
