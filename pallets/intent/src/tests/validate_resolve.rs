use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;

#[test]
fn non_partial_swap_intent_should_work_when_resolved_exactly() {
	ExtBuilder::default().build().execute_with(|| {
		//ExactIn
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let resolve = intent.clone();

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));

		//ExactOut
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let resolve = intent.clone();

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));
	});
}

#[test]
fn non_partial_swap_intent_should_work_when_resolved_better() {
	ExtBuilder::default().build().execute_with(|| {
		//ExactIn
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out += 2 * ONE_HDX;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));

		//ExactOut
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in -= ONE_DOT;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));
	});
}

#[test]
fn partial_swap_intent_should_work_when_resolved_exactly() {
	ExtBuilder::default().build().execute_with(|| {
		//ExactIn
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let resolve = intent.clone();

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));

		//ExactOut
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let resolve = intent.clone();

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));
	});
}

#[test]
fn partial_swap_intent_should_work_when_resolved_better() {
	ExtBuilder::default().build().execute_with(|| {
		//ExactIn
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out += 2 * ONE_HDX;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));

		//ExactOut
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in -= ONE_HDX;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));
	});
}

#[test]
fn partial_should_work_when_resolved_partially() {
	ExtBuilder::default().build().execute_with(|| {
		//ExactIn
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in /= 2;
		r_swap.amount_out /= 2;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));

		//ExactOut
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in /= 2;
		r_swap.amount_out /= 2;

		assert_ok!(IntentPallet::validate_resolve(&intent, &resolve.data));
	});
}

#[test]
fn swap_intent_should_not_work_when_asset_in_does_not_match() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.asset_in = ETH;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::ResolveMismatch
		);
	});
}

#[test]
fn swap_intent_should_not_work_when_asset_out_does_not_match() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.asset_out = ETH;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::ResolveMismatch
		);
	});
}

#[test]
fn swap_intent_should_not_work_when_swap_type_does_not_match() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.swap_type = SwapType::ExactOut;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::ResolveMismatch
		);
	});
}

#[test]
fn swap_intent_should_not_work_when_partiality_does_not_match() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.partial = !r_swap.partial;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::ResolveMismatch
		);
	});
}

#[test]
fn non_partial_swap_exact_in_intent_should_not_work_when_amount_out_is_less_than_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out -= 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn non_partial_swap_exact_in_intent_should_not_work_when_amount_in_is_not_exact() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		//smaller than limit
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in -= 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);

		//bigger than limit
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn non_partial_swap_exact_out_intent_should_not_work_when_amount_in_is_bigger_than_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn non_partial_swap_exact_out_intent_should_not_work_when_amount_out_not_exact() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: false,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		//smaller than limit
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out -= 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);

		//bigger than limit
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_in_should_not_work_when_resolved_fully_and_amount_out_is_less_than_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out -= 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_in_should_not_work_when_amount_in_is_bigger_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_in_should_not_work_when_resolved_partially_and_amount_out_is_less_than_pro_rata_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactIn,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		//NOTE: resolve 50% of intent so amount_out >= pro-rata limit(50%)
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in /= 2;
		r_swap.amount_out = r_swap.amount_out / 2 - 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_out_should_not_work_when_resolved_fully_and_amount_in_is_bigger_than_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_out_should_not_work_when_amount_out_is_bigger_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_out += 1;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}

#[test]
fn partial_swap_exact_out_should_not_work_when_resolved_partially_and_amount_in_is_bigger_than_pro_rata_limit() {
	ExtBuilder::default().build().execute_with(|| {
		let intent = Intent {
			data: IntentData::Swap(SwapData {
				asset_in: DOT,
				asset_out: HDX,
				amount_in: 20_000 * ONE_DOT,
				amount_out: 10_000 * ONE_HDX,
				swap_type: SwapType::ExactOut,
				partial: true,
			}),
			deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
			on_success: None,
			on_failure: None,
		};

		//NOTE: resolve 50% of intent so amount_in <= pro-rata limit(50%)
		let mut resolve = intent.clone();
		let IntentData::Swap(ref mut r_swap) = resolve.data;
		r_swap.amount_in = r_swap.amount_in / 2 + 1;
		r_swap.amount_out /= 2;

		assert_noop!(
			IntentPallet::validate_resolve(&intent, &resolve.data),
			Error::<Test>::LimitViolation
		);
	});
}
