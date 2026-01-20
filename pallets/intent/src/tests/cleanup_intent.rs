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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: ONE_SECOND,
					on_success: None,
					on_failure: Some(BoundedVec::new()),
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 18446744073709551616000_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

			//Act
			assert_ok!(IntentPallet::cleanup_intent(RuntimeOrigin::none(), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), owner)));
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: ONE_SECOND,
					on_success: None,
					on_failure: Some(BoundedVec::new()),
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 18446744073709551616000_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

			//Act
			assert_ok!(IntentPallet::cleanup_intent(RuntimeOrigin::signed(CHARLIE), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), owner)));
		});
}

#[test]
fn should_work_when_intent_is_expired_and_intent_has_on_failure() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 100 * ONE_HDX),
			(ALICE, ETH, 30 * ONE_QUINTIL),
			(BOB, ETH, 5 * ONE_QUINTIL),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 18446744073709551616000_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

			//Act
			assert_ok!(IntentPallet::cleanup_intent(RuntimeOrigin::signed(CHARLIE), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				0
			);
			assert_eq!(get_queued_task(Source::ICE(id)), None);
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 18446744073709551616000_u128;
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: ONE_SECOND,
					on_success: None,
					on_failure: Some(BoundedVec::new()),
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 18446744073709551616000_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(get_queued_task(Source::ICE(id)), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

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
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), owner)));
		});
}
