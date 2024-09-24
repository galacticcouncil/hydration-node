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
			let intent = Intent {
				who: ALICE,
				swap: swap.clone(),
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent,));

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
			let intent = Intent {
				who: ALICE,
				swap,
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_ok!(ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent,));

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
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW - 1_000_000,
					partial: false,
					on_success: None,
					on_failure: None,
				},
			),
			Error::<Test>::InvalidDeadline
		);

		// Equal
		assert_noop!(
			ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW,
					partial: false,
					on_success: None,
					on_failure: None,
				}
			),
			Error::<Test>::InvalidDeadline
		);

		// Future
		assert_noop!(
			ICE::submit_intent(
				RuntimeOrigin::signed(ALICE),
				Intent {
					who: ALICE,
					swap: swap.clone(),
					deadline: DEFAULT_NOW + MaxAllowdIntentDuration::get() + 1,
					partial: false,
					on_success: None,
					on_failure: None,
				}
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
					Intent {
						who: ALICE,
						swap: swap.clone(),
						deadline: DEFAULT_NOW + 1_000_000,
						partial: false,
						on_success: None,
						on_failure: None,
					},
				),
				orml_tokens::Error::<Test>::BalanceTooLow
			);
		});
}

#[test]
fn test_correct_intent_id() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(crate::Pallet::<Test>::get_intent_id(100, 0), 1844674407370955161600);
		assert_eq!(crate::Pallet::<Test>::get_intent_id(100, 1), 1844674407370955161601);
		assert_eq!(crate::Pallet::<Test>::get_intent_id(100, 2), 1844674407370955161602);

		assert_eq!(crate::Pallet::<Test>::get_intent_id(100, 0), 1844674407370955161600);
		assert_eq!(crate::Pallet::<Test>::get_intent_id(101, 1), 1863121151444664713217);
		assert_eq!(crate::Pallet::<Test>::get_intent_id(102, 2), 1881567895518374264834);

		assert_eq!(
			crate::Pallet::<Test>::get_intent_id(DEFAULT_NOW, 0),
			31172125326516865653853388800000
		);
		assert_eq!(
			crate::Pallet::<Test>::get_intent_id(DEFAULT_NOW, 1),
			31172125326516865653853388800001
		);
		assert_eq!(
			crate::Pallet::<Test>::get_intent_id(DEFAULT_NOW, 2),
			31172125326516865653853388800002
		);

		// revert example
		let intent_id = 31172125326516865653853388800001u128;
		let deadline = (intent_id >> 64) as u64;
		let increment = intent_id as u64;
		assert_eq!(deadline, DEFAULT_NOW);
		assert_eq!(increment, 1);
	});
}

#[test]
fn submit_intent_should_fail_when_amount_in_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 0,
				amount_out: 1_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: ALICE,
				swap: swap,
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_noop!(
				ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent,),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn submit_intent_should_fail_when_amount_out_is_zero() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: 200,
				amount_in: 1_000_000_000_000,
				amount_out: 0,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: ALICE,
				swap: swap,
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_noop!(
				ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent,),
				Error::<Test>::InvalidIntent
			);
		});
}

#[test]
fn submit_intent_should_fail_when_asset_out_is_hub_asset() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 100, 100_000_000_000_000)])
		.build()
		.execute_with(|| {
			let swap = Swap {
				asset_in: 100,
				asset_out: HubAssetId::get(),
				amount_in: 1_000_000_000_000,
				amount_out: 1_000_000_000_000,
				swap_type: SwapType::ExactIn,
			};
			let intent = Intent {
				who: ALICE,
				swap: swap,
				deadline: DEFAULT_NOW + 1_000_000,
				partial: false,
				on_success: None,
				on_failure: None,
			};
			assert_noop!(
				ICE::submit_intent(RuntimeOrigin::signed(ALICE), intent,),
				Error::<Test>::InvalidIntent
			);
		});
}
