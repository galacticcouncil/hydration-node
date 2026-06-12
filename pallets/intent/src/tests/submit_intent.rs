use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;
use sp_runtime::traits::BadOrigin;

#[test]
fn should_work_when_origin_signed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let id: IntentId = 0;
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act
			assert_ok!(IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0));

			let stored = IntentPallet::get_intent(id).expect("intent should be stored");
			assert_eq!(stored.data.asset_in(), HDX);
			assert_eq!(stored.data.asset_out(), DOT);
			assert_eq!(stored.data.amount_in(), 10 * ONE_HDX);
			assert_eq!(stored.deadline, Some(MAX_INTENT_DEADLINE - 1));
			assert_eq!(IntentPallet::intent_owner(id), Some(ALICE));
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE),
				10 * ONE_HDX
			);
			assert_eq!(AccountIntents::<Test>::get(ALICE, id), Some(()));
			assert_eq!(IntentPallet::account_intent_count(ALICE), 1);
		});
}

#[test]
fn should_work_when_intent_has_no_deadline() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let id: IntentId = 0;
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: None,
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act
			assert_ok!(IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0));

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
		});
}

#[test]
fn should_not_work_when_origin_is_none() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let id: IntentId = 92215273624474048528384;
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act
			assert_noop!(IntentPallet::submit_intent(RuntimeOrigin::none(), intent_0), BadOrigin);
		});
}

#[test]
fn should_not_work_when_deadline_is_less_than_now() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			assert_ok!(Timestamp::set(RuntimeOrigin::none(), 2 * MAX_INTENT_DEADLINE));

			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidDeadline
			);
		});
}

#[test]
fn should_not_work_when_deadline_bigger_than_max_allowed_intent_duration() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE + 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidDeadline
			);
		});
}

#[test]
fn should_not_work_when_amount_in_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 0,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn should_not_work_when_amount_out_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 0,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn should_not_work_when_asset_in_eq_asset_out() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: HDX,
					amount_in: 10 * ONE_HDX,
					amount_out: 10 * ONE_HDX,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn should_not_work_when_asset_out_is_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: HUB_ASSET_ID,
					amount_in: 10 * ONE_HDX,
					amount_out: 10 * ONE_HDX,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn should_not_work_when_cant_reserve_funds() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0),
				orml_tokens::Error::<Test>::BalanceTooLow
			);
		});
}

#[test]
fn should_work_when_intent_is_partial() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let intent_0 = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 1_000 * ONE_DOT,
					partial: true,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act&assert
			assert_ok!(IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent_0));
		});
}

#[test]
fn should_not_work_when_amount_in_is_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let id: IntentId = 92215273624474048528384;
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let ed = DummyRegistry::existential_deposit(HDX).expect("dummy registry to work");

			let intent = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: ed - 1,
					amount_out: 1_000 * ONE_DOT,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act&Assert
			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn should_not_work_when_amount_out_is_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let id: IntentId = 92215273624474048528384;
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
			assert_eq!(Intents::<Test>::iter_keys().count(), 0);

			let ed = DummyRegistry::existential_deposit(DOT).expect("dummy registry to work");

			let intent = IntentInput {
				data: IntentDataInput::Swap(SwapParams {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: ed - 1,
					partial: false,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - 1),
				on_resolved: Some(BoundedVec::truncate_from(b"success".to_vec())),
			};

			//Act&Assert
			assert_noop!(
				IntentPallet::submit_intent(RuntimeOrigin::signed(ALICE), intent),
				Error::<Test>::InvalidIntent
			);
		});
}
