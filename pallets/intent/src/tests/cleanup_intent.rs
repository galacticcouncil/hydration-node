use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;

#[test]
fn should_work_when_intent_is_expired_and_origin_is_none() {
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
					deadline: Some(ONE_SECOND),
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
			let id = 0_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				intent.deadline.expect("intent with deadline") + 1
			));

			//Act
			assert_ok!(IntentPallet::cleanup_intent(RuntimeOrigin::none(), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
		});
}

#[test]
fn should_work_when_intent_is_expired_and_origin_is_signed() {
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
					deadline: Some(ONE_SECOND),
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
			let id = 0_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				intent.deadline.expect("intent with deadline") + 1
			));

			//Act
			assert_ok!(IntentPallet::cleanup_intent(RuntimeOrigin::signed(CHARLIE), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
		});
}

#[test]
fn should_not_work_when_intent_is_not_expired() {
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
					deadline: Some(ONE_SECOND),
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
			let id = 0_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			//Act as signed
			assert_noop!(
				IntentPallet::cleanup_intent(RuntimeOrigin::signed(CHARLIE), id),
				Error::<Test>::IntentActive
			);

			//Assert
			assert!(IntentPallet::get_intent(id).is_some());
			assert!(IntentPallet::intent_owner(id).is_some());
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in()
			);
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			//Act 2 as none origin
			assert_noop!(
				IntentPallet::cleanup_intent(RuntimeOrigin::none(), id),
				Error::<Test>::IntentActive
			);

			//Assert
			assert!(IntentPallet::get_intent(id).is_some());
			assert!(IntentPallet::intent_owner(id).is_some());
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in()
			);
			assert_eq!(get_queued_task(Source::ICE(id)), None);
		});
}

#[test]
fn should_not_collect_fees_when_intent_is_expired() {
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
					deadline: Some(ONE_SECOND),
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
			let id = 0_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				intent.deadline.expect("intent with deadline") + 1
			));

			//Act
			let res = IntentPallet::cleanup_intent(RuntimeOrigin::none(), id);
			assert_eq!(res, Ok(Pays::No.into()));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
		});
}

#[test]
fn should_not_work_when_intent_has_no_deadline() {
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
		])
		.build()
		.execute_with(|| {
			let id = 0_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), 1_000));

			//Act
			assert_noop!(
				IntentPallet::cleanup_intent(RuntimeOrigin::none(), id),
				Error::<Test>::IntentActive
			);
		});
}
