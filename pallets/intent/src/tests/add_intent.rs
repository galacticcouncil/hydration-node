use crate::tests::mock::*;
use crate::*;
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;
use sp_runtime::TransactionOutcome;

#[test]
fn should_work_when_intent_is_valid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, ETH, 5 * ONE_QUINTIL)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, HDX, &ALICE), 0);
				assert_eq!(Intents::<Test>::iter_keys().count(), 0);

				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 1_000 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				//Act
				let r = IntentPallet::add_intent(ALICE, intent_0.clone());
				let id = match r {
					Ok(id) => id,
					_ => {
						assert!(false, "Expected Ok(_). Got {:#?}", r);
						0
					}
				};

				assert_eq!(IntentPallet::get_intent(id), Some(intent_0));
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

				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 1_000 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(
					IntentPallet::add_intent(ALICE, intent_0),
					Error::<Test>::InvalidDeadline
				);
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
				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 1_000 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE + 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(
					IntentPallet::add_intent(ALICE, intent_0),
					Error::<Test>::InvalidDeadline
				);
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
				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 0,
						amount_out: 1_000 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(IntentPallet::add_intent(ALICE, intent_0), Error::<Test>::InvalidIntent);
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
				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 0,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(IntentPallet::add_intent(ALICE, intent_0), Error::<Test>::InvalidIntent);
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
				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: HDX,
						amount_in: 10 * ONE_HDX,
						amount_out: 10 * ONE_HDX,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(IntentPallet::add_intent(ALICE, intent_0), Error::<Test>::InvalidIntent);
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
				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: HUB_ASSET_ID,
						amount_in: 10 * ONE_HDX,
						amount_out: 10 * ONE_HDX,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(IntentPallet::add_intent(ALICE, intent_0), Error::<Test>::InvalidIntent);
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

				let intent_0 = Intent {
					kind: IntentKind::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10 * ONE_HDX,
						amount_out: 1_000 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - 1,
					on_success: Some(BoundedVec::truncate_from(b"success".to_vec())),
					on_failure: Some(BoundedVec::truncate_from(b"failure".to_vec())),
				};

				assert_noop!(
					IntentPallet::add_intent(ALICE, intent_0),
					orml_tokens::Error::<Test>::BalanceTooLow
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}
