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
		data: IntentDataInput::Swap(SwapData {
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

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}
