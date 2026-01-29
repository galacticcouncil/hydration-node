use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;
use sp_runtime::traits::BadOrigin;

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
				Intent {
					data: IntentData::Swap(SwapData {
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
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
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
			let id = 73786976294838206464000_u128;
			let intent = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, intent.data.asset_in(), &owner),
				intent.data.amount_in(),
			);

			//Act
			assert_ok!(IntentPallet::cancel_intent(RuntimeOrigin::signed(owner), id));

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
				Intent {
					data: IntentData::Swap(SwapData {
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
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
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
			let id = 73786976294838206464000_u128;
			let mut resolve = IntentPallet::get_intent(id).expect("Intent to exists");
			let owner = ALICE;

			let IntentData::Swap(ref mut r_swap) = resolve.data;
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
				}
			));

			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.data.asset_in(), &owner),
				resolve.data.amount_in(),
			);

			//Act
			assert_ok!(IntentPallet::cancel_intent(RuntimeOrigin::signed(owner), id));

			//Assert
			assert_eq!(IntentPallet::get_intent(id), None);
			assert_eq!(IntentPallet::intent_owner(id), None);
			assert_eq!(
				Currencies::reserved_balance_named(&NAMED_RESERVE_ID, resolve.data.asset_in(), &owner),
				0
			);
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
				Intent {
					data: IntentData::Swap(SwapData {
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
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: BTC,
						amount_in: 30 * ONE_QUINTIL,
						amount_out: ONE_QUINTIL,
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
			let id = 9_u128;
			let owner = ALICE;

			//Act & Assert;
			assert_noop!(
				IntentPallet::cancel_intent(RuntimeOrigin::signed(owner), id),
				Error::<Test>::IntentNotFound
			);
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
				Intent {
					data: IntentData::Swap(SwapData {
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
			let id = 73786976294838206464000_u128;
			let non_owner = BOB;

			//Act & Assert;
			assert_noop!(
				IntentPallet::cancel_intent(RuntimeOrigin::signed(non_owner), id),
				Error::<Test>::InvalidOwner
			);
		});
}

#[test]
fn should_not_work_when_origin_is_none() {
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
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
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
			let id = 73786976294838206464000_u128;

			//Act & Assert;
			assert_noop!(IntentPallet::cancel_intent(RuntimeOrigin::none(), id), BadOrigin);
		});
}
