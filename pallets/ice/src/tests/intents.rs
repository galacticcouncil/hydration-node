use super::*;
use crate::tests::{ExtBuilder, ICE};
use crate::types::{Intent, Swap, SwapType};
use crate::Error;
use frame_support::{assert_noop, assert_ok};
use orml_traits::NamedMultiReservableCurrency;

#[test]
fn submit_intent_should_store_correct_intent_information() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			let intent_id = get_intent_id(DEFAULT_NOW + 1_000_000, 0);
			let intent = crate::Pallet::<Test>::get_intent(intent_id);
			assert!(intent.is_some());
			let intent = intent.unwrap();
			let expected_intent = Intent {
				who: ALICE,
				swap,
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_eq!(intent, expected_intent);
		});
}

#[test]
fn submit_intent_should_reserve_amount_in() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_ok!(ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + 1_000_000,
				false,
				None,
				None,
			));

			assert_eq!(
				100_000_000_000_000,
				Currencies::reserved_balance_named(&NamedReserveId::get(), 100, &ALICE)
			);
		});
}

#[test]
fn submit_intent_should_fail_when_deadline_is_not_valid() {
	ExtBuilder::default().build().execute_with(|| {
		let swap = Swap {
			asset_in: 100,
			asset_out: 200,
			amount_in: 100_000_000_000_000,
			amount_out: 200_000_000_000_000,
			swap_type: SwapType::ExactIn,
		};
		// Past
		assert_noop!(
			ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW - 1_000_000,
				false,
				None,
				None,
			),
			Error::<Test>::InvalidDeadline
		);

		// Equal
		assert_noop!(
			ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW,
				false,
				None,
				None,
			),
			Error::<Test>::InvalidDeadline
		);

		// Future
		assert_noop!(
			ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				swap.clone(),
				DEFAULT_NOW + MaxAllowdIntentDuration::get() + 1,
				false,
				None,
				None,
			),
			Error::<Test>::InvalidDeadline
		);
	});
}

#[test]
fn submit_intent_should_fail_when_it_cant_reserve_sufficient_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_noop!(
				ICE::submit_intent(
					RuntimeOrigin::signed(ALICE),
					swap.clone(),
					DEFAULT_NOW + 1_000_000,
					false,
					None,
					None,
				),
				orml_tokens::Error::<Test>::BalanceTooLow
			);
		});
}

#[test]
fn submit_intent_should_fail_when_on_success_call_length_is_exceeded() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let on_success = vec![0u8; (MaxCallData::get() + 1) as usize];
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_noop!(
				ICE::submit_intent(
					RuntimeOrigin::signed(ALICE),
					swap.clone(),
					DEFAULT_NOW + 1_000_000,
					false,
					Some(on_success),
					None,
				),
				Error::<Test>::TooLong
			);
		});
}

#[test]
fn submit_intent_should_fail_when_on_fail_call_length_is_exceeded() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let on_fail = vec![0u8; (MaxCallData::get() + 1) as usize];
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 100_000_000_000_000,
				amount_out: 200_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			assert_noop!(
				ICE::submit_intent(
					RuntimeOrigin::signed(ALICE),
					swap.clone(),
					DEFAULT_NOW + 1_000_000,
					false,
					None,
					Some(on_fail),
				),
				Error::<Test>::TooLong
			);
		});
}
