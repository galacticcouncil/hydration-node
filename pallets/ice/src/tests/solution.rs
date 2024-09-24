use super::*;
use crate::pallet::{Intents, SolutionExecuted, SolutionScore};
use crate::tests::{ExtBuilder, ICE};
use crate::types::{
	BoundedResolvedIntents, BoundedRoute, BoundedTrades, Intent, ResolvedIntent, Swap, SwapType, TradeInstruction,
};
use crate::Error;
use frame_support::pallet_prelude::Hooks;
use frame_support::{assert_noop, assert_ok};

fn create_solution_for_given_intents(intents: Vec<IntentId>) -> (BoundedResolvedIntents, BoundedTrades<AssetId>, u64) {
	// currently only one intent is supported
	let intent_id = intents[0];

	let resolved_intents = vec![ResolvedIntent {
		intent_id,
		amount_in: 100_000_000_000_000,
		amount_out: 200_000_000_000_000,
	}];
	let route = vec![];

	let instructions = vec![TradeInstruction::SwapExactIn {
		asset_in: 100,
		asset_out: 200,
		amount_in: 100_000_000_000_000,
		amount_out: 200_000_000_000_000,
		route: BoundedRoute::try_from(route).unwrap(),
	}];

	let resolved_intents = BoundedResolvedIntents::try_from(resolved_intents).unwrap();
	let trades = BoundedTrades::try_from(instructions).unwrap();
	let score = 1_000_000u64;

	(resolved_intents, trades, score)
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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				trades,
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
			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, trades, score, 2),
				Error::<Test>::InvalidBlockNumber
			);
		});
}

#[test]
fn submit_solution_should_fail_when_score_is_different() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, trades, score + 1, 1),
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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
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
				Intent {
					who: BOB,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));
			let expired_intent_id = get_intent_id(DEFAULT_NOW + 1_000, 1);

			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			// move time forward
			NOW.with(|now| {
				*now.borrow_mut() += 2000;
			});

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				trades,
				score,
				1
			),);
			let intent = Intents::<Test>::get(expired_intent_id);
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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000_000,
					partial: true,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let resolved_intents = vec![ResolvedIntent {
				intent_id,
				amount_in: 100_000_000_000_000 / 2,
				amount_out: 200_000_000_000_000 / 2,
			}];
			let route = vec![];

			let instructions = vec![TradeInstruction::SwapExactIn {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000 / 2,
				amount_out: 200_000_000_000_000 / 2,
				route: BoundedRoute::try_from(route).unwrap(),
			}];

			let resolved_intents = BoundedResolvedIntents::try_from(resolved_intents).unwrap();
			let trades = BoundedTrades::try_from(instructions).unwrap();
			let score = 1_000_000u64;

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				trades,
				score,
				1
			));

			let intent = Intents::<Test>::get(intent_id);
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
		.with_native_amount(ALICE, 1_000_000_000_000)
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, trades, score, 1),
				Error::<Test>::IntentNotFound
			);
		});
}

#[test]
fn submit_solution_should_slash_proposer_when_solution_is_invalid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, trades, score, 1),
				Error::<Test>::IntentNotFound
			);

			//TODO: uncomment when slashing is resolved
			/*
			let balance = Balances::free_balance(&ALICE);
			assert_eq!(balance, 0);
			let balance = Balances::free_balance(&RECEIVER);
			assert_eq!(balance, 1_000_000_000_000);
			 */
		});
}

#[test]
fn on_finalize_should_clear_temporary_storage() {
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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);

			let (resolved_intents, trades, score) = create_solution_for_given_intents(vec![intent_id]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				trades,
				score,
				1
			));

			assert!(SolutionExecuted::<Test>::get());
			ICE::on_finalize(System::block_number());
			assert_eq!(SolutionScore::<Test>::get(), None);
			assert!(!SolutionExecuted::<Test>::get());
		});
}
