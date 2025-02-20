use super::*;
use frame_support::testing_prelude::*;
use pallet_intent::types::*;

pub(crate) fn get_next_intent_id(moment: Moment) -> IntentId {
	let increment = pallet_intent::Pallet::<Test>::next_incremental_id();
	pallet_intent::Pallet::<Test>::get_intent_id(moment, increment)
}

fn submit_intent(intent: Intent<AccountId>) -> Result<IntentId, DispatchError> {
	let intent_id = get_next_intent_id(intent.deadline);
	Intents::submit_intent(RuntimeOrigin::signed(intent.who), intent)?;
	Ok(intent_id)
}

fn create_solution(intent_ids: Vec<IntentId>) -> (BoundedResolvedIntents, u64) {
	let mut resolved = vec![];

	for intent_id in intent_ids {
		let intent = Intents::get_intent(intent_id).unwrap();
		resolved.push(ResolvedIntent {
			intent_id,
			amount_in: intent.swap.amount_in,
			amount_out: intent.swap.amount_out,
		});
	}

	let score = 1_000_000 * resolved.len() as u64;

	(BoundedResolvedIntents::truncate_from(resolved), score)
}

#[test]
fn submit_solution_should_work_when_valid_solution_is_submitted() {
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
			let intent = Intent {
				who: ALICE,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			let intent_id = submit_intent(intent).unwrap();

			let (resolved_intents, score) = create_solution(vec![intent_id]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));

			assert_eq!(Tokens::free_balance(100, &ALICE), 0);
			assert_eq!(Tokens::free_balance(200, &ALICE), 200_000_000_000_000);
		});
}

#[test]
fn submit_solution_should_set_execute_flag_correctly() {}
#[test]
fn submit_solution_should_transfer_correct_amounts() {}

#[test]
fn submit_solution_should_update_partial_intent_correctly() {}

#[test]
fn on_finalize_should_reserve_execute_flag_and_score() {}
