use super::*;
use crate::pallet::{Intents, SolutionExecuted, SolutionScore};
use crate::tests::{ExtBuilder, ICE};
use crate::types::{BoundedResolvedIntents, Intent, ResolvedIntent, Swap, SwapType};
use crate::{Error, Event, Reason};
use frame_support::pallet_prelude::Hooks;
use frame_support::{assert_noop, assert_ok};

fn generate_intent(
	who: AccountId,
	(asset_in, amount_in): (AssetId, Balance),
	(asset_out, amount_out): (AssetId, Balance),
	deadline: u64,
	partial: bool,
) -> Intent<AccountId> {
	let swap = Swap {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		swap_type: SwapType::ExactIn,
	};
	let intent = Intent {
		who,
		swap,
		deadline,
		partial,
		on_success: None,
		on_failure: None,
	};
	intent
}

#[test]
fn submit_solution_should_fail_when_solution_is_already_executed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			SolutionExecuted::<Test>::put(true);
			let resolved_intents = BoundedResolvedIntents::default();
			let score = 1_000_000u64;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, score, 1),
				Error::<Test>::AlreadyExecuted
			);
		});
}

#[test]
fn submit_solution_should_fail_when_given_solution_is_not_for_current_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let resolved_intents = BoundedResolvedIntents::default();
			let score = 1_000_000u64;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, score, 2),
				Error::<Test>::InvalidBlockNumber
			);
		});
}

#[test]
fn on_finalize_should_clear_temporary_storage() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: ALICE,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			let intent_id = get_next_intent_id(DEFAULT_NOW + 1_000_000);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let (resolved_intents, score) = mock_solution(vec![(intent_id, intent)]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				score,
				1
			));

			assert!(SolutionExecuted::<Test>::get());
			ICE::on_finalize(System::block_number());
			assert_eq!(SolutionScore::<Test>::get(), None);
			assert!(!SolutionExecuted::<Test>::get());
		});
}

#[test]
fn submit_solution_should_fail_when_block_is_not_correct() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let mock_resolved_intent = ResolvedIntent {
				intent_id,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
			};

			assert_noop!(
				ICE::submit_solution(
					RuntimeOrigin::signed(ALICE),
					BoundedResolvedIntents::truncate_from(vec![mock_resolved_intent]),
					1_000_000,
					2
				),
				Error::<Test>::InvalidBlockNumber
			);
		});
}

#[test]
fn submit_solution_should_fail_when_resolved_intents_is_empty() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let resolved_intents = BoundedResolvedIntents::default();
			let score = 1_000_000u64;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, score, 1),
				Error::<Test>::InvalidSolution(Reason::Empty)
			);
		});
}
#[test]
fn submit_should_should_fail_when_resolved_intent_contains_nonexistent_intent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let mock_resolved_intent = ResolvedIntent {
				intent_id: intent_id,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
			};

			assert_noop!(
				ICE::submit_solution(
					RuntimeOrigin::signed(ALICE),
					BoundedResolvedIntents::truncate_from(vec![mock_resolved_intent]),
					1_000_000,
					1
				),
				Error::<Test>::InvalidSolution(Reason::IntentNotFound)
			);
		});
}

#[test]
fn submit_should_should_fail_when_resolved_intent_contains_incorrect_intent_amount_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
				intent_id: inc_id,
				amount_in: 200_000_000_000_000,
				amount_out: 400_000_000_000_000,
			}]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, 1, 1),
				Error::<Test>::InvalidSolution(Reason::IntentAmount)
			);
		});
}

#[test]
fn submit_should_should_fail_when_resolved_intent_has_incorrect_limit_price() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
				intent_id: inc_id,
				amount_in: 100_000_000_000_000,
				amount_out: 10_000_000_000_000,
			}]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, 2, 1),
				Error::<Test>::InvalidSolution(Reason::IntentPrice)
			);
		});
}
#[test]
fn submit_should_should_fail_when_solution_has_incorrect_score() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
				intent_id: inc_id,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
			}]);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::signed(ALICE), resolved_intents, 10, 1),
				Error::<Test>::InvalidSolution(Reason::Score)
			);
		});
}
#[test]
fn submit_solution_should_correctly_execute_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);
			let asset_in_balance = Currencies::free_balance(100, &ALICE);
			let asset_out_balance = Currencies::free_balance(200, &ALICE);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let (resolved_intents, score) = mock_solution(vec![(inc_id, intent.clone())]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				score,
				1
			));
			let new_asset_a_balance = Currencies::free_balance(100, &ALICE);
			let new_asset_b_balance = Currencies::free_balance(200, &ALICE);
			assert_eq!(new_asset_a_balance, asset_in_balance - 100_000_000_000_000);
			assert_eq!(new_asset_b_balance, asset_out_balance + 200_000_000_000_000);
		});
}

#[test]
fn submit_solution_should_correctly_execute_and_update_partial_intent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				true,
			);
			let asset_in_balance = Currencies::free_balance(100, &ALICE);
			let asset_out_balance = Currencies::free_balance(200, &ALICE);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let resolved = ResolvedIntent {
				intent_id: inc_id,
				amount_in: 100_000_000_000_000 / 2,
				amount_out: 200_000_000_000_000 / 2,
			};

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				BoundedResolvedIntents::truncate_from(vec![resolved]),
				1_000_000,
				1
			));
			let new_asset_a_balance = Currencies::total_balance(100, &ALICE);
			let new_asset_b_balance = Currencies::total_balance(200, &ALICE);
			assert_eq!(new_asset_a_balance, asset_in_balance - 100_000_000_000_000 / 2);
			assert_eq!(new_asset_b_balance, asset_out_balance + 200_000_000_000_000 / 2);

			let intent = Intents::<Test>::get(inc_id).unwrap();
			assert_eq!(intent.swap.amount_in, 100_000_000_000_000 / 2);
			assert_eq!(intent.swap.amount_out, 200_000_000_000_000 / 2);
		});
}

#[test]
fn submit_solution_should_clear_expired_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let intent_to_resolve = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				true,
			);

			let expired_intent = generate_intent(
				BOB,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000,
				true,
			);
			let bob_asset_in_balance = Currencies::free_balance(100, &BOB);
			let bob_asset_out_balance = Currencies::free_balance(200, &BOB);

			let alice_inc_id = get_next_intent_id(intent_to_resolve.deadline);
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				intent_to_resolve.clone()
			));
			let bob_inc_id = get_next_intent_id(expired_intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(BOB), expired_intent.clone()));

			let bob_free = Currencies::free_balance(100, &BOB);
			assert_eq!(bob_free, bob_asset_in_balance - 100_000_000_000_000);

			let resolved = ResolvedIntent {
				intent_id: alice_inc_id,
				amount_in: 100_000_000_000_000 / 2,
				amount_out: 200_000_000_000_000 / 2,
			};

			// move time forward
			NOW.with(|now| {
				*now.borrow_mut() += 2000;
			});

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				BoundedResolvedIntents::truncate_from(vec![resolved]),
				1_000_000,
				1
			));
			let intent = Intents::<Test>::get(bob_inc_id);
			assert!(intent.is_none());

			let bob_free = Currencies::free_balance(100, &BOB);
			let bob_out = Currencies::free_balance(200, &BOB);
			assert_eq!(bob_free, bob_asset_in_balance);
			assert_eq!(bob_out, bob_asset_out_balance);
		});
}

#[test]
fn submit_solution_should_set_execute_flag() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let (resolved_intents, score) = mock_solution(vec![(inc_id, intent.clone())]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				score,
				1
			));
			assert!(SolutionExecuted::<Test>::get());
		});
}

#[test]
fn submit_solution_should_deposit_event() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.with_native_amount(ALICE, 1_000_000_000_000)
		.with_prices(vec![((100, 200), (1_000_000_000_000, 2_000_000_000_000))])
		.build()
		.execute_with(|| {
			let intent = generate_intent(
				ALICE,
				(100, 100_000_000_000_000),
				(200, 200_000_000_000_000),
				DEFAULT_NOW + 1_000_000,
				false,
			);

			let inc_id = get_next_intent_id(intent.deadline);
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent.clone()));

			let (resolved_intents, score) = mock_solution(vec![(inc_id, intent.clone())]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(ALICE),
				resolved_intents,
				score,
				1
			));

			expect_events(vec![Event::SolutionExecuted { who: ALICE }.into()]);
		});
}

#[test]
fn submit_solution_should_execute_on_success_callback_when_intent_is_fully_resolved() {
	//TODO: implement
}

#[test]
fn submit_solution_should_execute_on_failure_callback_when_intent_is_expired() {
	//TODO: implement
}
