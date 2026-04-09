use crate::tests::mock::*;
use crate::*;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;
use sp_runtime::TransactionOutcome;

fn swap_intent_input(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	deadline: Option<Moment>,
) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: false,
		}),
		deadline,
		on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
	}
}

#[test]
fn should_work_when_intent_is_valid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1));

				//Act
				let r = IntentPallet::add_intent(ALICE, input);
				let id = match r {
					Ok(id) => id,
					_ => {
						panic!("Expected Ok(_). Got {:#?}", r);
					}
				};

				let stored = IntentPallet::get_intent(id).expect("intent should be stored");
				assert_eq!(stored.data.asset_in(), HDX);
				assert_eq!(stored.data.asset_out(), DOT);
				assert_eq!(stored.data.amount_in(), 10 * ONE_HDX);
				assert_eq!(stored.data.amount_out(), 1_000 * ONE_DOT);
				assert_eq!(stored.deadline, Some(MAX_INTENT_DEADLINE - 1));
				assert_eq!(IntentPallet::intent_owner(id), Some(ALICE));
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE),
					10 * ONE_HDX
				);
				assert_eq!(AccountIntents::<Test>::get(ALICE, id), Some(()));
				assert_eq!(IntentPallet::account_intent_count(ALICE), 1);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_deadline_is_less_than_now() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_ok!(Timestamp::set(RuntimeOrigin::none(), 2 * MAX_INTENT_DEADLINE));

				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1));

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidDeadline);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_deadline_bigger_than_max_allowed_intent_duration() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE + 1));

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidDeadline);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_amount_in_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let input = swap_intent_input(HDX, DOT, 0, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1));

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_amount_out_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 0, Some(MAX_INTENT_DEADLINE - 1));

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_asset_in_eq_asset_out() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let input = swap_intent_input(HDX, HDX, 10 * ONE_HDX, 10 * ONE_HDX, Some(MAX_INTENT_DEADLINE - 1));

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_asset_out_is_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let input = swap_intent_input(
					HDX,
					HUB_ASSET_ID,
					10 * ONE_HDX,
					10 * ONE_HDX,
					Some(MAX_INTENT_DEADLINE - 1),
				);

				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_cant_reserve_funds() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1));

				assert_noop!(
					IntentPallet::add_intent(ALICE, input),
					orml_tokens::Error::<Test>::BalanceTooLow
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_amount_in_is_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let ed = DummyRegistry::existential_deposit(HDX).expect("dummy registry to work");

				let input = swap_intent_input(HDX, DOT, ed - 1, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1));

				//Act&Assert
				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_amount_out_is_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let ed = DummyRegistry::existential_deposit(DOT).expect("dummy registry to work");

				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, ed - 1, Some(MAX_INTENT_DEADLINE - 1));

				//Act&Assert
				assert_noop!(IntentPallet::add_intent(ALICE, input), Error::<Test>::InvalidIntent);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_work_when_intent_has_no_deadline() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let input = swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, None);

				//Act
				let r = IntentPallet::add_intent(ALICE, input);
				let id = match r {
					Ok(id) => id,
					_ => {
						panic!("Expected Ok(_). Got {:#?}", r);
					}
				};

				let stored = IntentPallet::get_intent(id).expect("intent should be stored");
				assert_eq!(stored.data.asset_in(), HDX);
				assert_eq!(stored.data.asset_out(), DOT);
				assert_eq!(stored.deadline, None);
				assert_eq!(IntentPallet::intent_owner(id), Some(ALICE));
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE),
					10 * ONE_HDX
				);
				assert_eq!(AccountIntents::<Test>::get(ALICE, id), Some(()));
				assert_eq!(IntentPallet::account_intent_count(ALICE), 1);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn account_intents_index_tracks_multiple_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 100 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				// Create 3 intents for ALICE
				let id0 = IntentPallet::add_intent(
					ALICE,
					swap_intent_input(HDX, DOT, 10 * ONE_HDX, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				)
				.expect("should work");
				let id1 = IntentPallet::add_intent(
					ALICE,
					swap_intent_input(HDX, DOT, 5 * ONE_HDX, 500 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				)
				.expect("should work");
				let id2 = IntentPallet::add_intent(
					ALICE,
					swap_intent_input(ETH, DOT, ONE_QUINTIL, 1_000 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				)
				.expect("should work");

				// Create 1 intent for BOB
				let id3 = IntentPallet::add_intent(
					BOB,
					swap_intent_input(ETH, DOT, ONE_QUINTIL, 1_500 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				)
				.expect("should work");

				// Verify counts
				assert_eq!(IntentPallet::account_intent_count(ALICE), 3);
				assert_eq!(IntentPallet::account_intent_count(BOB), 1);
				assert_eq!(AccountIntents::<Test>::iter_prefix(ALICE).count(), 3);
				assert_eq!(AccountIntents::<Test>::iter_prefix(BOB).count(), 1);

				// Cancel one of ALICE's intents
				assert_ok!(IntentPallet::cancel_intent(ALICE, id1));

				assert_eq!(IntentPallet::account_intent_count(ALICE), 2);
				assert_eq!(AccountIntents::<Test>::iter_prefix(ALICE).count(), 2);
				assert_eq!(AccountIntents::<Test>::get(ALICE, id0), Some(()));
				assert_eq!(AccountIntents::<Test>::get(ALICE, id1), None);
				assert_eq!(AccountIntents::<Test>::get(ALICE, id2), Some(()));

				// BOB unaffected
				assert_eq!(IntentPallet::account_intent_count(BOB), 1);
				assert_eq!(AccountIntents::<Test>::get(BOB, id3), Some(()));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_max_intents_per_account_reached() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1_000 * ONE_HDX), (BOB, HDX, 100 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				// MaxIntentsPerAccount is 5 in mock
				for i in 0..5u128 {
					assert_ok!(IntentPallet::add_intent(
						ALICE,
						swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
					));
					assert_eq!(IntentPallet::account_intent_count(ALICE), (i + 1) as u32);
				}

				// 6th intent should fail
				assert_noop!(
					IntentPallet::add_intent(
						ALICE,
						swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
					),
					Error::<Test>::MaxIntentsReached
				);

				// BOB can still create intents (separate account)
				assert_ok!(IntentPallet::add_intent(
					BOB,
					swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_work_when_intent_canceled_and_slot_freed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 1_000 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				// Fill up to max
				let mut ids = Vec::new();
				for _ in 0..5u128 {
					let id = IntentPallet::add_intent(
						ALICE,
						swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
					)
					.expect("should work");
					ids.push(id);
				}

				// At limit — cannot add
				assert_noop!(
					IntentPallet::add_intent(
						ALICE,
						swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
					),
					Error::<Test>::MaxIntentsReached
				);

				// Cancel one — frees a slot
				assert_ok!(IntentPallet::cancel_intent(ALICE, ids[2]));
				assert_eq!(IntentPallet::account_intent_count(ALICE), 4);

				// Now can add again
				assert_ok!(IntentPallet::add_intent(
					ALICE,
					swap_intent_input(HDX, DOT, ONE_HDX, 100 * ONE_DOT, Some(MAX_INTENT_DEADLINE - 1)),
				));
				assert_eq!(IntentPallet::account_intent_count(ALICE), 5);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}
