use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

#[test]
fn should_work_with_intent_without_deadline() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: None,
					on_resolved: Some(BoundedVec::new()),
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1;
			let resolve = IntentPallet::get_intent(1).expect("intent to exist");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exist");
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			));

			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(get_queued_task(Source::ICE(id)), Some((Source::ICE(id), who)));
		});
}

#[test]
fn non_partial_should_remove_intent_and_owner_when_resolved_exactly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: Some(BoundedVec::new()),
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1;
			let resolve = IntentPallet::get_intent(1).expect("intent to exist");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exist");
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			));

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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
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
			let (id, mut resolve) = IntentPallet::get_valid_intents()[0].to_owned();
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_out += 1_000_000;

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			));

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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
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
			let who = ALICE;
			let id = 0_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout in is < than ExactIn
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in -= 1;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout in is > than ExactIn
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in += 1;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amout out is < than amount out limit
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_out -= 1;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
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
			let id = 1_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = BOB;

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out /= 2;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: Some(BoundedVec::new()),
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1;
			let resolve = IntentPallet::get_intent(id).expect("intent to exit");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exist");

			assert_eq!(get_queued_task(Source::ICE(id)), None);

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			),);

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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: Some(BoundedVec::new()),
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1;
			let mut resolve = IntentPallet::get_intent(1).expect("intent to exist");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exist");
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_out += 1_000_000;

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			),);

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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out /= 2;

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			),);

			let expected_intent = Intent {
				data: IntentData::Swap(SwapData {
					asset_in: ETH,
					asset_out: DOT,
					amount_in: ONE_QUINTIL / 2,
					amount_out: 1_500 * ONE_DOT / 2,
					partial: true,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
				on_resolved: None,
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let who = ALICE;
			let id = 0_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amount in > intent.exactIn
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in += 1;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
				Error::<Test>::LimitViolation
			);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			//amount in > intent.amount_out
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_out -= 1;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let who = ALICE;
			let id = 0_u128;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out = r_swap.amount_out / 2 - 1; //bellow limit

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = IntentPallet::intent_owner(id).expect("intent owner to exists");

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out /= 2;

			let non_existing_id = 1_000_000_000_000_000_u128;
			assert_noop!(
				IntentPallet::intent_resolved(
					&who,
					&ResolvedIntent {
						id: non_existing_id,
						data: resolve.data
					}
				),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let non_owner = CHARLIE;

			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out /= 2;

			assert_noop!(
				IntentPallet::intent_resolved(&non_owner, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let who = BOB;

			assert_ok!(Timestamp::set(
				RuntimeOrigin::none(),
				resolve.deadline.expect("intent with deadline") + 1
			));

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let who = BOB;

			//NOTE: different assetIn
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.asset_in = HDX;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
				Error::<Test>::ResolveMismatch
			);

			//NOTE: different assetOut
			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.asset_out = HDX;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
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
				data: IntentData::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 10 * ONE_HDX,
					amount_out: 100 * ONE_DOT,
					partial: true,
				}),
				deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
				on_resolved: None,
			},
		)])
		.build()
		.execute_with(|| {
			let id = 0_u128;
			let who = ALICE;

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.partial = !r_swap.partial;

			assert_noop!(
				IntentPallet::intent_resolved(&who, &ResolvedIntent { id, data: resolve.data }),
				Error::<Test>::ResolveMismatch
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
					data: IntentData::Swap(SwapData {
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
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: DOT,
						amount_in: ONE_QUINTIL,
						amount_out: 1_500 * ONE_DOT,
						partial: true,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: Some(BoundedVec::new()),
				},
			),
		])
		.build()
		.execute_with(|| {
			let id = 1_u128;
			let who = BOB;
			assert_eq!(get_queued_task(Source::ICE(id)), None);

			let mut resolve = IntentPallet::get_intent(id).expect("intent to exists");
			let IntentData::Swap(ref mut r_swap) = resolve.data else {
				panic!("expected Swap");
			};
			r_swap.amount_in /= 2;
			r_swap.amount_out /= 2;

			assert_ok!(IntentPallet::intent_resolved(
				&who,
				&ResolvedIntent { id, data: resolve.data }
			));

			assert_eq!(get_queued_task(Source::ICE(id)), None);
		});
}
