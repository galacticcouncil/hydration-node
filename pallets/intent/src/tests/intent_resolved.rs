use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

#[test]
fn non_partial_should_remove_intent_and_owner_when_resolved_exactly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: Some(BoundedVec::new()),
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let (id, resolve) = IntentPallet::get_valid_intents()[0].to_owned();
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), who)));
		});
}

#[test]
fn non_partial_should_remove_intent_and_owner_when_resolved_better_than_limits() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
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
			let (id, mut resolve) = IntentPallet::get_valid_intents()[0].to_owned();
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			if r_swap.swap_type == SwapType::ExactIn {
				r_swap.amount_out = r_swap.amount_out + 1_000_000;
			} else {
				r_swap.amount_in = r_swap.amount_in - 1_000_000;
			}

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
		});
}

#[test]
fn non_partial_should_not_work_when_resolved_bellow_limits() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
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
			//NOTE: ExactOut
			let who = BOB;
			let id = 73786976294838206464001_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout out is < than ExactOut
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_out = r_swap.amount_out - 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout out is > than ExactOut
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_out = r_swap.amount_out + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout in is > than amount in limit
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			//NOTE: ExactIn
			let who = ALICE;
			let id = 73786976294838206464000_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout in is < than ExactIn
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in - 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout in is > than ExactIn
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout out is < than amount out limit
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_out = r_swap.amount_out - 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);
		});
}

#[test]
fn should_not_work_when_non_partial_intent_resolved_partially() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
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
			let id = 73786976294838206464001_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = BOB;

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);
		});
}

#[test]
fn partial_intent_should_remove_intent_and_owner_when_resolved_exactly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: Some(BoundedVec::new()),
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let (id, resolve) = IntentPallet::get_valid_intents()[0].to_owned();
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			assert_eq!(get_queued_task(Source::ICE(id)), None);

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), who)));
		});
}

#[test]
fn partial_intent_should_remove_intent_and_owner_when_resolved_fully_and_better_than_limits() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: Some(BoundedVec::new()),
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let (id, mut resolve) = IntentPallet::get_valid_intents()[0].to_owned();
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			if r_swap.swap_type == SwapType::ExactIn {
				r_swap.amount_out = r_swap.amount_out + 1_000_000;
			} else {
				r_swap.amount_in = r_swap.amount_in - 1_000_000;
			}

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), who)));
		});
}

#[test]
fn partial_intent_should_not_remove_intent_and_owner_when_not_resolved_fully() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			let expected_intent = Intent {
				kind: IntentKind::Swap(SwapData {
					asset_in: ETH,
					asset_out: DOT,
					amount_in: ONE_QUINTIL / 2,
					amount_out: 1_500 * ONE_DOT / 2,
					swap_type: SwapType::ExactOut,
					partial: true,
				}),
				deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
				on_success: None,
				on_failure: None,
			};

			assert_eq!(IntentPallet::get_intent(id), Some(expected_intent));
			assert!(IntentPallet::intent_owner(id).is_some());
		});
}

#[test]
fn partial_intent_should_not_work_when_resolved_fully_and_bellow_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			//NOTE: partial ExactOut
			let id = 73786976294838206464001_u128;
			let who = BOB;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			// amount Out > intent.ExactOut
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_out = r_swap.amount_out + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			// amount in > intent.amount_in
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			//NOTE: partial ExactIn
			let who = ALICE;
			let id = 73786976294838206464000_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amount in > intent.exactIn
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in + 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amount in > intent.amount_out
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_out = r_swap.amount_out - 1;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);
		});
}

#[test]
fn partial_intent_should_not_work_when_resolved_partially_and_bellow_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			//NOTE: partial ExactOut
			let id = 73786976294838206464001_u128;
			let who = BOB;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2 + 1; //above limit
			r_swap.amount_out = r_swap.amount_out / 2;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);

			//NOTE: partial ExactIn
			let who = ALICE;
			let id = 73786976294838206464000_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2 - 1; //bellow limit

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::LimitViolation
			);
		});
}

#[test]
fn should_not_work_when_intent_doesnt_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			let non_existing_id = 1;
			assert_noop!(
				IntentPallet::intent_resolved(non_existing_id, &who, &resolve),
				Error::<Test>::IntentNotFound
			);
		});
}

#[test]
fn should_not_work_when_resolved_as_not_an_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let non_owner = CHARLIE;

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			assert_noop!(
				IntentPallet::intent_resolved(id, &non_owner, &resolve),
				Error::<Test>::InvalidOwner
			);
		});
}

#[test]
fn should_not_work_when_intent_expired() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = BOB;

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), resolve.deadline + 1));

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::IntentExpired
			);
		});
}

#[test]
fn should_not_work_when_assets_doesnt_match() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let who = BOB;

			//NOTE: different assetIn
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.asset_in = HDX;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);

			//NOTE: different assetOut
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.asset_out = HDX;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);
		});
}

#[test]
fn should_not_work_when_callbacks_doesnt_match() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let who = BOB;

			//NOTE: different on_success
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			resolve.on_success = Some(BoundedVec::new());

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);

			//NOTE: different on_failure
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			resolve.on_failure = Some(BoundedVec::new());

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);
		});
}

#[test]
fn should_not_work_when_swap_type_doesnt_match() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![(
			ALICE,
			Intent {
				kind: IntentKind::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 100 * ONE_DOT,
					swap_type: SwapType::ExactIn,
					partial: true,
				}),
				deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
				on_success: None,
				on_failure: None,
			},
		)])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464000_u128;
			let who = ALICE;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.swap_type = SwapType::ExactOut;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);
		});
}

#[test]
fn should_not_work_when_partial_doesnt_match() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![(
			ALICE,
			Intent {
				kind: IntentKind::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 100 * ONE_DOT,
					swap_type: SwapType::ExactIn,
					partial: true,
				}),
				deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
				on_success: None,
				on_failure: None,
			},
		)])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464000_u128;
			let who = ALICE;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.partial = !r_swap.partial;

			assert_noop!(
				IntentPallet::intent_resolved(id, &who, &resolve),
				Error::<Test>::ResolveMismatch
			);
		});
}

#[test]
fn non_partial_exact_out_should_unreserve_surplus_when_resolved_better_than_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
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
			let id = 73786976294838206464001_u128;
			let who = BOB;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in - 1_000;

			//NOTE: It's ICE pallet responsibility is to unlock used fund during solution execution. This is
			//to simulate it.
			assert_eq!(
				Currencies::unreserve_named(
					&NAMED_RESERVE_ID,
					resolve.asset_in(),
					&who,
					999_999_999_999_999_000_u128
				),
				Zero::zero()
			);
			// Assert some surplus is left after execution
			assert!(!Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who).is_zero());

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			// Make sure surplus was unlocked
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who),
				Zero::zero()
			);
		});
}

#[test]
fn partial_exact_out_should_unreserve_surplus_when_fully_resolved_better_than_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let who = BOB;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in - 1_000;

			//NOTE: It's ICE pallet responsibility is to unlock used fund during solution execution. This is
			//to simulate it.
			assert_eq!(
				Currencies::unreserve_named(
					&NAMED_RESERVE_ID,
					resolve.asset_in(),
					&who,
					999_999_999_999_999_000_u128
				),
				Zero::zero()
			);
			// Assert some surplus is left after execution
			assert!(!Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who).is_zero());

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			// Make sure surplus was unlocked
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who),
				Zero::zero()
			);
		});
}

#[test]
fn partial_exact_out_should_not_unreserve_funds_when_resolved_patially() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 73786976294838206464001_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			//NOTE: It's ICE pallet responsibility is to unlock used fund during solution execution. This is
			//to simulate it.
			assert_eq!(
				Currencies::unreserve_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who, resolve.amount_in()),
				Zero::zero()
			);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who),
				500_000_000_000_000_000_u128
			);

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve));

			let expected_intent = Intent {
				kind: IntentKind::Swap(SwapData {
					asset_in: ETH,
					asset_out: DOT,
					amount_in: ONE_QUINTIL / 2,
					amount_out: 1_500 * ONE_DOT / 2,
					swap_type: SwapType::ExactOut,
					partial: true,
				}),
				deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
				on_success: None,
				on_failure: None,
			};

			assert_eq!(IntentPallet::get_intent(id), Some(expected_intent.clone()));
			assert!(IntentPallet::intent_owner(id).is_some());
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.asset_in(), &who),
				expected_intent.amount_in()
			);
		});
}

#[test]
fn partial_intent_should_not_queue_callback_when_not_fully_resolved() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 100 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
				},
			),
			(
				BOB,
				Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: true,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: Some(BoundedVec::new()),
					on_failure: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			//NOTE: partial ExactOut
			let id = 73786976294838206464001_u128;
			let who = BOB;
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentKind::Swap(ref mut r_swap) = resolve.kind;
			r_swap.amount_in = r_swap.amount_in / 2;
			r_swap.amount_out = r_swap.amount_out / 2;

			assert_ok!(IntentPallet::intent_resolved(id, &who, &resolve),);

			assert_eq!(get_queued_task(Source::ICE(id)), None);
		});
}
