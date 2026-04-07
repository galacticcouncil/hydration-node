use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use pretty_assertions::assert_eq;
use sp_runtime::TransactionOutcome;

#[test]
fn should_work_when_canceled_by_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = 0_u128;
				let intent = IntentPallet::get_intent(id).expect("Intent to exists");
				let owner = ALICE;

				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
					intent.data.amount_in(),
				);

				//Act
				assert_ok!(IntentPallet::cancel_intent(owner, id));

				//Assert
				assert_eq!(IntentPallet::get_intent(id), None);
				assert_eq!(IntentPallet::intent_owner(id), None);
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
					0
				);
				assert_eq!(AccountIntents::<Test>::get(owner, id), None);
				assert_eq!(IntentPallet::account_intent_count(owner), 1); // ALICE still has intent 2
															  // Other intents unaffected
				assert_eq!(AccountIntents::<Test>::get(BOB, 1_u128), Some(()));
				assert_eq!(AccountIntents::<Test>::get(ALICE, 2_u128), Some(()));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_work_when_intent_was_partially_resolved_and_canceled_by_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = 0_u128;
				let mut resolve = IntentPallet::get_intent(id).expect("Intent to exists");
				let owner = ALICE;

				let IntentData::Swap(ref mut r_swap) = resolve.data else {
					panic!("expected Swap");
				};
				r_swap.amount_in /= 2;
				r_swap.amount_out /= 2;

				//NOTE: It's ICE pallet responsibility is to unlock used fund during solution execution. This is
				//to simulate it.
				assert_eq!(
					Currencies::unreserve_named(
						&NAMED_RESERVE_ID,
						resolve.data.asset_in(),
						&owner,
						resolve.data.amount_in()
					),
					0
				);
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.data.asset_in(), &owner),
					5_000_000_000_000_u128
				);
				assert_ok!(IntentPallet::intent_resolved(
					&owner,
					&ResolvedIntent {
						id,
						data: resolve.data.clone()
					},
					0,
				));

				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.data.asset_in(), &owner),
					resolve.data.amount_in(),
				);

				//Act
				assert_ok!(IntentPallet::cancel_intent(owner, id));

				//Assert
				assert_eq!(IntentPallet::get_intent(id), None);
				assert_eq!(IntentPallet::intent_owner(id), None);
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.data.asset_in(), &owner),
					0
				);
				assert_eq!(AccountIntents::<Test>::get(owner, id), None);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_intent_doesnt_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = 9_u128;
				let owner = ALICE;

				//Act & Assert;
				assert_noop!(IntentPallet::cancel_intent(owner, id), Error::<Test>::IntentNotFound);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_not_work_when_canceled_non_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = 0_u128;
				let not_owner = BOB;

				//Act & Assert;
				assert_noop!(IntentPallet::cancel_intent(not_owner, id), Error::<Test>::InvalidOwner);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_work_when_intent_has_no_deadline_and_canceled_by_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						partial: false,
					}),
					deadline: None,
					on_resolved: None,
				},
			),
			(
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = 0_u128;
				let intent = IntentPallet::get_intent(id).expect("Intent to exists");
				let owner = ALICE;

				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
					intent.data.amount_in(),
				);

				//Act
				assert_ok!(IntentPallet::cancel_intent(owner, id));

				//Assert
				assert_eq!(IntentPallet::get_intent(id), None);
				assert_eq!(IntentPallet::intent_owner(id), None);
				assert_eq!(
					Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
					0
				);
				assert_eq!(AccountIntents::<Test>::get(owner, id), None);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}
