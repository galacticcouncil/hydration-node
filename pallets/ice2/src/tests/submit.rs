use super::*;
use frame_support::pallet_prelude::Hooks;
use frame_support::testing_prelude::*;
use pallet_intent::types::*;
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{Rounding, Saturating};
use std::collections::BTreeMap;

pub(crate) fn get_next_intent_id(moment: Moment) -> IntentId {
	let increment = pallet_intent::Pallet::<Test>::next_incremental_id();
	pallet_intent::Pallet::<Test>::get_intent_id(moment, increment)
}

fn submit_intent(intent: Intent<AccountId>) -> Result<IntentId, DispatchError> {
	let intent_id = get_next_intent_id(intent.deadline);
	Intents::submit_intent(RuntimeOrigin::signed(intent.who), intent)?;
	Ok(intent_id)
}

fn create_solution(intent_ids: Vec<(IntentId, Option<(Balance, Balance)>)>) -> (BoundedResolvedIntents, u64) {
	let mut resolved = vec![];
	let mut amounts: BTreeMap<AssetId, (u128, u128)> = BTreeMap::new();

	for (intent_id, resolved_amounts) in intent_ids {
		let intent = Intents::get_intent(intent_id).unwrap();
		let (amount_in, amount_out) = if let Some(given) = resolved_amounts {
			given
		} else {
			(intent.swap.amount_in, intent.swap.amount_out)
		};
		resolved.push(ResolvedIntent {
			intent_id,
			amount_in,
			amount_out,
		});
		amounts
			.entry(intent.swap.asset_in)
			.and_modify(|(v_in, _)| *v_in = v_in.saturating_add(amount_in))
			.or_insert((amount_in, 0u128));
		amounts
			.entry(intent.swap.asset_out)
			.and_modify(|(_, v_out)| *v_out = v_out.saturating_add(amount_out))
			.or_insert((0u128, amount_out));
	}

	let mut hub_amount = resolved.len() as u128 * 1_000_000_000_000u128;

	for (asset_id, (amount_in, amount_out)) in amounts.iter() {
		let matched_amount = (*amount_in).min(*amount_out);
		if matched_amount > 0u128 {
			let price = get_price(*asset_id, LRNA);
			let converted =
				multiply_by_rational_with_rounding(matched_amount, price.0, price.1, Rounding::Down).unwrap();
			hub_amount.saturating_accrue(converted);
		}
	}

	let score = hub_amount / 1_000_000u128;

	(BoundedResolvedIntents::truncate_from(resolved), score as u64)
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

			let (resolved_intents, score) = create_solution(vec![(intent_id, None)]);

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
fn submit_solution_should_work_when_valid_solution_with_matching_amounts_is_submitted() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 200, 100_000_000_000_000)])
		.with_prices(vec![
			((100, 200), (1_000_000_000_000, 2_000_000_000_000)),
			((100, LRNA), (1, 1)),
			((200, LRNA), (1, 1)),
		])
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
			let alice_intent_id = submit_intent(intent).unwrap();

			let swap = Swap {
				asset_in: 200,
				asset_out: 100,
				amount_in: 100_000_000_000_000,
				amount_out: 50_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: BOB,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			let bob_intent_id = submit_intent(intent).unwrap();

			let (resolved_intents, score) = create_solution(vec![(alice_intent_id, None), (bob_intent_id, None)]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));

			assert_eq!(Tokens::free_balance(100, &ALICE), 0);
			assert_eq!(Tokens::free_balance(200, &ALICE), 200_000_000_000_000);
			assert_eq!(Tokens::free_balance(200, &BOB), 0);
			assert_eq!(Tokens::free_balance(100, &BOB), 50_000_000_000_000);
		});
}

#[test]
fn submit_solution_should_work_with_partial_resolved_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 200, 100_000_000_000_000)])
		.with_prices(vec![
			((100, 200), (1_000_000_000_000, 2_000_000_000_000)),
			((100, LRNA), (1, 1)),
			((200, LRNA), (1, 1)),
		])
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
			let alice_intent_id = submit_intent(intent).unwrap();

			let swap = Swap {
				asset_in: 200,
				asset_out: 100,
				amount_in: 100_000_000_000_000,
				amount_out: 50_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: BOB,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: true,
				on_success: None,
				on_failure: None,
			};
			let bob_intent_id = submit_intent(intent).unwrap();

			let (resolved_intents, score) = create_solution(vec![
				(alice_intent_id, None),
				(bob_intent_id, Some((50_000_000_000_000, 25_000_000_000_000))),
			]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));

			assert_eq!(Tokens::free_balance(100, &ALICE), 0);
			assert_eq!(Tokens::free_balance(200, &ALICE), 200_000_000_000_000);
			assert_eq!(Tokens::free_balance(200, &BOB), 50_000_000_000_000);
			assert_eq!(Tokens::free_balance(100, &BOB), 25_000_000_000_000);
		});
}

#[test]
fn submit_solution_should_set_execute_flag_correctly() {
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

			let (resolved_intents, score) = create_solution(vec![(intent_id, None)]);

			assert!(!ICE::solution_executed(), "Solution executed flag should not be set");

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));

			assert!(ICE::solution_executed(), "Solution executed flag should be set");
		});
}

#[test]
fn on_finalize_should_reserve_execute_flag_and_score() {
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

			let (resolved_intents, score) = create_solution(vec![(intent_id, None)]);

			assert!(!ICE::solution_executed(), "Solution executed flag should not be set");
			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));
			assert!(ICE::solution_executed(), "Solution executed flag should be set");

			ICE::on_finalize(System::block_number());
			assert_eq!(ICE::solution_score(), None);
			assert!(!ICE::solution_executed(), "Solution executed flag should reset");
		});
}

#[test]
fn submit_solution_should_clear_resolved_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 200, 100_000_000_000_000)])
		.with_prices(vec![
			((100, 200), (1_000_000_000_000, 2_000_000_000_000)),
			((100, LRNA), (1, 1)),
			((200, LRNA), (1, 1)),
		])
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
			let alice_intent_id = submit_intent(intent).unwrap();

			let swap = Swap {
				asset_in: 200,
				asset_out: 100,
				amount_in: 100_000_000_000_000,
				amount_out: 50_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: BOB,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			let bob_intent_id = submit_intent(intent).unwrap();

			let (resolved_intents, score) = create_solution(vec![(alice_intent_id, None), (bob_intent_id, None)]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));
			assert!(Intents::get_intent(alice_intent_id).is_none());
			assert!(Intents::get_intent(bob_intent_id).is_none());
		});
}

#[test]

fn submit_solution_should_update_partial_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000), (BOB, 200, 100_000_000_000_000)])
		.with_prices(vec![
			((100, 200), (1_000_000_000_000, 2_000_000_000_000)),
			((100, LRNA), (1, 1)),
			((200, LRNA), (1, 1)),
		])
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
			let alice_intent_id = submit_intent(intent).unwrap();

			let swap = Swap {
				asset_in: 200,
				asset_out: 100,
				amount_in: 100_000_000_000_000,
				amount_out: 50_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: BOB,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: true,
				on_success: None,
				on_failure: None,
			};
			let bob_intent_id = submit_intent(intent).unwrap();

			let (resolved_intents, score) = create_solution(vec![
				(alice_intent_id, None),
				(bob_intent_id, Some((50_000_000_000_000, 25_000_000_000_000))),
			]);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::signed(EXECUTOR),
				resolved_intents,
				score,
				1,
			));
			let bob_intent = Intents::get_intent(bob_intent_id).unwrap();
			assert_eq!(
				bob_intent,
				Intent {
					who: BOB,
					swap: Swap {
						asset_in: 200,
						asset_out: 100,
						amount_in: 50_000_000_000_000,
						amount_out: 25_000_000_000_000,
						swap_type: SwapType::ExactIn,
					},
					deadline: DEFAULT_NOW + 1_000_000,
					partial: true,
					on_success: None,
					on_failure: None,
				}
			);

			assert!(Intents::get_intent(alice_intent_id).is_none());
		});
}
