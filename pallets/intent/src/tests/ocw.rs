use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};

#[test]
fn validate_unsingned_should_work_when_intent_is_expired() {
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
			let intent = IntentPallet::get_intent(id).expect("Intent to exist");

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

			let c = Call::cleanup_intent { id };
			assert_eq!(
				IntentPallet::validate_unsigned(TransactionSource::Local, &c),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					provides: vec![(OCW_TAG_PREFIX, Encode::encode(&id)).encode()],
					requires: vec![],
					longevity: 1,
					propagate: false,
				})
			);
		});
}

#[test]
fn validate_unsingned_should_not_work_when_intent_doesnt_exists() {
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
			let id = 0_u128;

			let c = Call::cleanup_intent { id };
			assert_noop!(
				IntentPallet::validate_unsigned(TransactionSource::Local, &c),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsingned_should_not_work_when_intent_is_not_expired() {
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
			let intent = IntentPallet::get_intent(id).expect("Intent to exist");

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline - 1));

			let c = Call::cleanup_intent { id };
			assert_noop!(
				IntentPallet::validate_unsigned(TransactionSource::Local, &c),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsingned_should_not_work_when_tx_is_external() {
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
			let intent = IntentPallet::get_intent(id).expect("Intent to exist");

			assert_ok!(Timestamp::set(RuntimeOrigin::none(), intent.deadline + 1));

			let c = Call::cleanup_intent { id };
			assert_noop!(
				IntentPallet::validate_unsigned(TransactionSource::External, &c),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}
