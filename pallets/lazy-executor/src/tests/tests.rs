use std::vec;

use super::*;

// use codec::Encode;
use crate::{BoundedCall, CallData, Error, Event, WeightInfo};
use codec::Encode;
use frame_support::{assert_err, assert_noop, assert_ok, weights::Weight};
use pretty_assertions::assert_eq;
use sp_core::Get;

#[test]
fn validate_call_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let weight = MAX_ALLOWED_WEIGHT;
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight,
		});

		//Act
		assert_ok!(LazyExecutor::validate_call(&call));

		//Assert
		//NOTE: validation should not trigger call execution
		assert!(!has_event(
			mock_pallet::Event::<Test>::CallExecuted { who: ALICE, weight }.into()
		));
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_ref_time_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time() + 1, MAX_ALLOWED_WEIGHT.proof_size()),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_proof_size_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time(), MAX_ALLOWED_WEIGHT.proof_size() + 1),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn validate_call_should_should_return_overweight_when_call_is_overweight() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(u64::max_value(), u64::max_value()),
		});

		//Act & assert
		assert_err!(LazyExecutor::validate_call(&call), Error::<Test>::Overweight);
	});
}

#[test]
fn execute_should_queue_call_for_lazy_execution() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(
				MAX_ALLOWED_WEIGHT.ref_time() - 500,
				MAX_ALLOWED_WEIGHT.proof_size() - 1_000,
			),
		});
		let call_data: BoundedCall = call.encode().try_into().unwrap();

		assert_eq!(LazyExecutor::next_id(), 0);
		assert_eq!(LazyExecutor::process_next_id(), 0);
		assert_eq!(LazyExecutor::call_queue(0), None);

		//Act
		assert_ok!(LazyExecutor::execute(ALICE, call_data.clone()));

		//Assert
		assert_eq!(LazyExecutor::next_id(), 1);
		assert_eq!(LazyExecutor::process_next_id(), 0);
		assert_eq!(
			LazyExecutor::call_queue(0),
			Some(CallData {
				origin: ALICE,
				call: call_data,
				created_at: 1,
			})
		);
	});
}

#[test]
fn execute_should_fail_when_bounded_call_cant_be_decoded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		//NOTE: call encoded by PolkadotAPPs with removed last 2 characters
		let call_data: Vec<u8> =
			hex_literal::hex!["070346f0b489ac07cb495852eba68e42250209e4d91f472d37a2fc8e4f0d9c74a828070010a5d4"].into();

		//Act
		assert_noop!(
			LazyExecutor::execute(ALICE, call_data.try_into().unwrap()),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn execute_should_fail_when_bounded_call_is_empty() {
	ExtBuilder::default().build().execute_with(|| {
		//Act & assert
		assert_noop!(
			LazyExecutor::execute(ALICE, Vec::new().try_into().unwrap()),
			Error::<Test>::Corrupted
		);
	});
}

#[test]
fn execute_should_fail_when_call_is_overweighted() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: Weight::from_parts(MAX_ALLOWED_WEIGHT.ref_time() + 1, MAX_ALLOWED_WEIGHT.proof_size()),
		});

		//Act & assert
		assert_noop!(
			LazyExecutor::execute(ALICE, call.encode().try_into().unwrap()),
			Error::<Test>::Overweight
		);
	});
}

#[test]
fn process_queue_should_works_when_queue_is_not_empty() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange
		let call_weight = Weight::from_parts(1_500, 2_000);
		let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
			allowed_origin: vec![ALICE],
			weight: call_weight,
		});
		let call_data: BoundedCall = call.encode().try_into().unwrap();

		assert_eq!(LazyExecutor::next_id(), 0);
		assert_eq!(LazyExecutor::process_next_id(), 0);
		assert_eq!(LazyExecutor::call_queue(0), None);

		assert_ok!(LazyExecutor::execute(ALICE, call_data.clone()));

		assert!(LazyExecutor::call_queue(0).is_some());
		assert_eq!(LazyExecutor::next_id(), 1);
		assert_eq!(LazyExecutor::process_next_id(), 0);

		let remaining_weight = DummyWeightInfo::process_queue_base_weight()
			.checked_mul(2)
			.unwrap()
			.checked_add(&call_weight)
			.unwrap();

		let current_block = 1_000;

		//Act
		let consumed_weight = LazyExecutor::process_queue(current_block, remaining_weight);

		//Assert
		assert_eq!(LazyExecutor::next_id(), 1);
		assert_eq!(LazyExecutor::process_next_id(), 1);
		assert!(LazyExecutor::call_queue(0).is_none());

		let expected_consumed_weight = remaining_weight;
		assert_eq!(expected_consumed_weight, consumed_weight);

		assert!(consumed_weight.all_lte(remaining_weight));

		assert!(has_event(Event::<Test>::Executed { id: 0, result: Ok(()) }.into()));

		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: ALICE,
				weight: call_weight
			}
			.into()
		));
	});
}

#[test]
fn process_queue_should_dispatch_max_calls_when_weight_allows_it() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange

		let calls: Vec<(AccountId, Weight)> = vec![
			(USER1, Weight::from_parts(1_001, 2_001)),
			(USER5, Weight::from_parts(1_005, 2_005)),
			(USER2, Weight::from_parts(1_002, 2_002)),
			(USER3, Weight::from_parts(1_003, 2_003)),
			(USER4, Weight::from_parts(1_004, 2_004)),
		];

		for (origin, call_weight) in calls.clone() {
			let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
				allowed_origin: vec![origin],
				weight: call_weight,
			});
			let call_data: BoundedCall = call.encode().try_into().unwrap();

			assert_ok!(LazyExecutor::execute(origin, call_data));
		}

		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 0);

		let remaining_weight = Weight::from_parts(13_000, 20_000);
		let current_block = 1_000;

		//Act
		let consumed_weight = LazyExecutor::process_queue(current_block, remaining_weight);

		//Assert
		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 3);
		assert!(LazyExecutor::call_queue(0).is_none());
		assert!(LazyExecutor::call_queue(1).is_none());
		assert!(LazyExecutor::call_queue(2).is_none());
		assert!(LazyExecutor::call_queue(3).is_some());
		assert!(LazyExecutor::call_queue(4).is_some());

		let processed_calls_weight = calls[0].1.saturating_add(calls[1].1).checked_add(&calls[2].1).unwrap();

		let m: u8 = <Test as crate::Config>::MaxDispatchedPerBlock::get();
		let expected_consumed_weight = DummyWeightInfo::process_queue_base_weight()
			.saturating_mul(m.into())
			.saturating_add(DummyWeightInfo::process_queue_base_weight())
			.checked_add(&processed_calls_weight)
			.unwrap();

		assert_eq!(expected_consumed_weight, consumed_weight);

		assert!(consumed_weight.all_lte(remaining_weight));

		assert!(has_event(Event::<Test>::Executed { id: 0, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[0].0,
				weight: calls[0].1
			}
			.into()
		));

		assert!(has_event(Event::<Test>::Executed { id: 1, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[1].0,
				weight: calls[1].1
			}
			.into()
		));

		assert!(has_event(Event::<Test>::Executed { id: 2, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[2].0,
				weight: calls[2].1
			}
			.into()
		));
	});
}

#[test]
fn process_queue_should_continue_when_dispatched_call_failed() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange

		let calls: Vec<(AccountId, Weight)> = vec![
			(USER1, Weight::from_parts(1_001, 2_001)),
			(USER5, Weight::from_parts(1_005, 2_005)),
			(USER2, Weight::from_parts(1_002, 2_002)),
			(USER3, Weight::from_parts(1_003, 2_003)),
			(USER4, Weight::from_parts(1_004, 2_004)),
		];

		for (origin, call_weight) in calls.clone() {
			let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
				allowed_origin: if origin == USER5 { vec![] } else { vec![origin] },
				weight: call_weight,
			});
			let call_data: BoundedCall = call.encode().try_into().unwrap();

			assert_ok!(LazyExecutor::execute(origin, call_data));
		}

		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 0);

		let remaining_weight = Weight::from_parts(13_000, 20_000);
		let current_block = 1_000;

		//Act
		let consumed_weight = LazyExecutor::process_queue(current_block, remaining_weight);

		//Assert
		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 3);
		assert!(LazyExecutor::call_queue(0).is_none());
		assert!(LazyExecutor::call_queue(1).is_none());
		assert!(LazyExecutor::call_queue(2).is_none());
		assert!(LazyExecutor::call_queue(3).is_some());
		assert!(LazyExecutor::call_queue(4).is_some());

		let processed_calls_weight = calls[0].1.saturating_add(calls[1].1).checked_add(&calls[2].1).unwrap();

		let m: u8 = <Test as crate::Config>::MaxDispatchedPerBlock::get();
		let expected_consumed_weight = DummyWeightInfo::process_queue_base_weight()
			.saturating_mul(m.into())
			.saturating_add(DummyWeightInfo::process_queue_base_weight())
			.checked_add(&processed_calls_weight)
			.unwrap();

		assert_eq!(expected_consumed_weight, consumed_weight);

		assert!(consumed_weight.all_lte(remaining_weight));

		assert!(has_event(Event::<Test>::Executed { id: 0, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[0].0,
				weight: calls[0].1
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::Executed {
				id: 1,
				result: Err(mock_pallet::Error::<Test>::Forbidden.into())
			}
			.into()
		));

		assert!(!has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[1].0,
				weight: calls[1].1
			}
			.into()
		));

		assert!(has_event(Event::<Test>::Executed { id: 2, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[2].0,
				weight: calls[2].1
			}
			.into()
		));
	});
}

#[test]
fn process_queue_should_stop_when_call_was_added_in_same_block() {
	ExtBuilder::default().build().execute_with(|| {
		//Arange

		let calls: Vec<(AccountId, Weight)> = vec![
			(USER1, Weight::from_parts(1_001, 2_001)),
			(USER5, Weight::from_parts(1_005, 2_005)),
			(USER2, Weight::from_parts(1_002, 2_002)),
			(USER3, Weight::from_parts(1_003, 2_003)),
			(USER4, Weight::from_parts(1_004, 2_004)),
		];

		for (origin, call_weight) in calls.clone() {
			let call = RuntimeCall::MockPallet(MockPalletCall::dummy_call {
				allowed_origin: if origin == USER5 { vec![] } else { vec![origin] },
				weight: call_weight,
			});
			let call_data: BoundedCall = call.encode().try_into().unwrap();

			assert_ok!(LazyExecutor::execute(origin, call_data));
		}

		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 0);

		let remaining_weight = Weight::from_parts(13_000, 20_000);
		let current_block = 1_000;

		//Act
		let consumed_weight = LazyExecutor::process_queue(current_block, remaining_weight);

		//Assert
		assert_eq!(LazyExecutor::next_id(), 5);
		assert_eq!(LazyExecutor::process_next_id(), 3);
		assert!(LazyExecutor::call_queue(0).is_none());
		assert!(LazyExecutor::call_queue(1).is_none());
		assert!(LazyExecutor::call_queue(2).is_none());
		assert!(LazyExecutor::call_queue(3).is_some());
		assert!(LazyExecutor::call_queue(4).is_some());

		let processed_calls_weight = calls[0].1.saturating_add(calls[1].1).checked_add(&calls[2].1).unwrap();

		let m: u8 = <Test as crate::Config>::MaxDispatchedPerBlock::get();
		let expected_consumed_weight = DummyWeightInfo::process_queue_base_weight()
			.saturating_mul(m.into())
			.saturating_add(DummyWeightInfo::process_queue_base_weight())
			.checked_add(&processed_calls_weight)
			.unwrap();

		assert_eq!(expected_consumed_weight, consumed_weight);

		assert!(consumed_weight.all_lte(remaining_weight));

		assert!(has_event(Event::<Test>::Executed { id: 0, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[0].0,
				weight: calls[0].1
			}
			.into()
		));

		assert!(has_event(
			Event::<Test>::Executed {
				id: 1,
				result: Err(mock_pallet::Error::<Test>::Forbidden.into())
			}
			.into()
		));

		assert!(!has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[1].0,
				weight: calls[1].1
			}
			.into()
		));

		assert!(has_event(Event::<Test>::Executed { id: 2, result: Ok(()) }.into()));
		assert!(has_event(
			mock_pallet::Event::<Test>::CallExecuted {
				who: calls[2].0,
				weight: calls[2].1
			}
			.into()
		));
	});
}
