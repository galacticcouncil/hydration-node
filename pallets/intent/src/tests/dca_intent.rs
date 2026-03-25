use crate::tests::mock::*;
use crate::types::Intent;
use crate::{Error, Event, IntentOwner, Intents};
use frame_support::storage::with_transaction;
use frame_support::{assert_noop, assert_ok};
use hydra_dx_math::ema::EmaPrice;
use ice_support::{DcaData, IntentData, SwapData};
use sp_runtime::{DispatchResult, Permill, TransactionOutcome};

fn dca_intent(amount_in: u128, amount_out: u128, budget: Option<u128>) -> Intent {
	Intent {
		data: IntentData::Dca(DcaData {
			asset_in: HDX,
			asset_out: DOT,
			amount_in,
			amount_out,
			slippage: Permill::from_percent(3),
			budget,
			remaining_budget: 0, // set by add_intent
			period: 10,
			last_execution_block: 0, // set by add_intent
		}),
		deadline: None,
		on_resolved: None,
	}
}

// ---- Submission tests ----

#[test]
fn should_add_dca_intent_with_fixed_budget() {
	let budget = 5 * ONE_HDX;
	let amount_in = ONE_HDX;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			set_block_number(100);

			let _ = with_transaction(|| {
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(amount_in, ONE_DOT, Some(budget)))
					.expect("should work");

				let stored = Intents::<Test>::get(id).unwrap();
				match stored.data {
					IntentData::Dca(dca) => {
						assert_eq!(dca.remaining_budget, budget);
						assert_eq!(dca.last_execution_block, 100);
						assert_eq!(dca.period, 10);
					}
					_ => panic!("expected DCA intent"),
				}

				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved, budget);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_add_dca_intent_with_rolling_budget() {
	let amount_in = ONE_HDX;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			set_block_number(50);

			let _ = with_transaction(|| {
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(amount_in, ONE_DOT, None))
					.expect("should work");

				let stored = Intents::<Test>::get(id).unwrap();
				match stored.data {
					IntentData::Dca(dca) => {
						assert_eq!(dca.remaining_budget, 2 * amount_in);
						assert_eq!(dca.last_execution_block, 50);
					}
					_ => panic!("expected DCA intent"),
				}

				assert_eq!(
					orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved,
					2 * amount_in
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_fail_dca_period_too_small() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let mut intent = dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX));
				if let IntentData::Dca(ref mut d) = intent.data {
					d.period = MIN_DCA_PERIOD - 1;
				}
				assert_noop!(
					crate::Pallet::<Test>::add_intent(ALICE, intent),
					Error::<Test>::InvalidDcaPeriod
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_fail_dca_budget_less_than_trade() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let intent = dca_intent(ONE_HDX, ONE_DOT, Some(ONE_HDX / 2));
				assert_noop!(
					crate::Pallet::<Test>::add_intent(ALICE, intent),
					Error::<Test>::InvalidDcaBudget
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_fail_dca_with_deadline() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let mut intent = dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX));
				intent.deadline = Some(99999);
				// The general deadline check fires first (deadline must be in future)
				assert_noop!(
					crate::Pallet::<Test>::add_intent(ALICE, intent),
					Error::<Test>::InvalidDeadline
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

// ---- Cancellation tests ----

#[test]
fn should_cancel_dca_unreserve_remaining_budget() {
	let budget = 5 * ONE_HDX;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(budget)))
					.expect("should work");

				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved, budget);

				assert_ok!(crate::Pallet::<Test>::cancel_intent(ALICE, id));

				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved, 0);
				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).free, 10 * ONE_HDX);

				assert!(Intents::<Test>::get(id).is_none());
				assert!(IntentOwner::<Test>::get(id).is_none());

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

// ---- get_valid_intents tests ----

#[test]
fn should_not_include_dca_before_period_elapsed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let _id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				// Block 105 < 100 + 10
				set_block_number(105);
				let valid = crate::Pallet::<Test>::get_valid_intents();
				assert!(valid.is_empty());

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_include_dca_after_period_elapsed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				// Block 110 = 100 + 10
				set_block_number(110);
				let valid = crate::Pallet::<Test>::get_valid_intents();
				assert_eq!(valid.len(), 1);
				assert_eq!(valid[0].0, id);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_transform_dca_to_swap_in_get_valid_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let _id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				set_block_number(110);
				let valid = crate::Pallet::<Test>::get_valid_intents();
				assert_eq!(valid.len(), 1);

				match &valid[0].1.data {
					IntentData::Swap(swap) => {
						assert_eq!(swap.asset_in, HDX);
						assert_eq!(swap.asset_out, DOT);
						assert_eq!(swap.amount_in, ONE_HDX);
						assert_eq!(swap.amount_out, ONE_DOT); // hard limit (no oracle)
						assert!(!swap.partial);
					}
					_ => panic!("expected Swap (transformed from DCA)"),
				}

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_use_hard_limit_in_get_valid_intents_with_oracle() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let _id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				// Oracle says 1 HDX = 2 DOT (n/d with d > n means: d asset_in per n asset_out)
				// estimated_out = amount_in * d/n = ONE_HDX * 1/2 = ONE_HDX/2
				set_oracle_price(Some(EmaPrice { n: 2, d: 1 }));
				set_block_number(110);

				let valid = crate::Pallet::<Test>::get_valid_intents();
				assert_eq!(valid.len(), 1);

				match &valid[0].1.data {
					IntentData::Swap(swap) => {
						// get_valid_intents uses hard limit, not oracle effective limit
						assert_eq!(swap.amount_out, ONE_DOT);
					}
					_ => panic!("expected Swap"),
				}

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_use_hard_limit_when_oracle_unavailable() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let _id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				set_block_number(110);
				let valid = crate::Pallet::<Test>::get_valid_intents();
				assert_eq!(valid.len(), 1);

				match &valid[0].1.data {
					IntentData::Swap(swap) => {
						assert_eq!(swap.amount_out, ONE_DOT);
					}
					_ => panic!("expected Swap"),
				}

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

// ---- Resolution tests ----

#[test]
fn should_resolve_dca_trade_and_update_state() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				set_block_number(110);
				// Simulate ICE unlock (happens in submit_solution before intent_resolved)
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, ONE_HDX));
				let resolve = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: ONE_HDX,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve));

				// Intent still exists
				let stored = Intents::<Test>::get(id).unwrap();
				match stored.data {
					IntentData::Dca(dca) => {
						assert_eq!(dca.remaining_budget, 4 * ONE_HDX);
						assert_eq!(dca.last_execution_block, 110);
					}
					_ => panic!("expected DCA intent"),
				}

				// DcaTradeExecuted event
				let events = frame_system::Pallet::<Test>::events();
				assert!(events
					.iter()
					.any(|e| matches!(e.event, RuntimeEvent::IntentPallet(Event::DcaTradeExecuted { .. }))));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_complete_dca_when_budget_exhausted() {
	let amount_in = ONE_HDX;
	let budget = 2 * ONE_HDX;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(amount_in, ONE_DOT, Some(budget)))
					.expect("should work");

				// First trade - simulate ICE unlock
				set_block_number(110);
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, amount_in));
				let resolve1 = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve1));
				assert!(Intents::<Test>::get(id).is_some());

				// Second trade — budget exhausted — simulate ICE unlock
				set_block_number(120);
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, amount_in));
				let resolve2 = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve2));

				assert!(Intents::<Test>::get(id).is_none());
				assert!(IntentOwner::<Test>::get(id).is_none());
				// ICE unlocked 2*amount_in, intent_resolved unreserved remaining (0). Total reserve = 0.
				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved, 0);

				let events = frame_system::Pallet::<Test>::events();
				assert!(events
					.iter()
					.any(|e| matches!(e.event, RuntimeEvent::IntentPallet(Event::DcaCompleted { .. }))));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_validate_dca_hard_limit() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				set_block_number(110);
				// Simulate ICE unlock
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, ONE_HDX));
				let resolve = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: ONE_HDX,
						amount_out: ONE_DOT / 2, // below hard limit
						partial: false,
					}),
				};
				assert_noop!(
					crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve),
					Error::<Test>::LimitViolation
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

// ---- compute_surplus tests ----

#[test]
fn should_compute_surplus_from_hard_limit_for_dca() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				let intent = Intents::<Test>::get(id).unwrap();
				let resolve_data = IntentData::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: ONE_HDX,
					amount_out: 2 * ONE_DOT,
					partial: false,
				});

				// Surplus computed against hard limit (ONE_DOT), not oracle
				let surplus = crate::Pallet::<Test>::compute_surplus(&intent, &resolve_data);
				assert_eq!(surplus, Some(2 * ONE_DOT - ONE_DOT));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_compute_surplus_with_hard_limit_when_no_oracle() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(ONE_HDX, ONE_DOT, Some(5 * ONE_HDX)))
					.expect("should work");

				let intent = Intents::<Test>::get(id).unwrap();
				let resolve_data = IntentData::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: ONE_HDX,
					amount_out: 2 * ONE_DOT,
					partial: false,
				});

				let surplus = crate::Pallet::<Test>::compute_surplus(&intent, &resolve_data);
				assert_eq!(surplus, Some(2 * ONE_DOT - ONE_DOT));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

// ---- Rolling DCA tests ----

#[test]
fn should_rolling_dca_re_reserve_after_trade() {
	let amount_in = ONE_HDX;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10 * ONE_HDX)])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(amount_in, ONE_DOT, None))
					.expect("should work");

				assert_eq!(
					orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved,
					2 * amount_in
				);

				set_block_number(110);
				// Simulate ICE unlock
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, amount_in));
				let resolve = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve));

				let stored = Intents::<Test>::get(id).unwrap();
				match stored.data {
					IntentData::Dca(dca) => {
						// remaining = 2x - 1x = 1x, then re-reserve 1x = 2x
						assert_eq!(dca.remaining_budget, 2 * amount_in);
					}
					_ => panic!("expected DCA"),
				}

				assert_eq!(
					orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved,
					2 * amount_in
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}

#[test]
fn should_complete_rolling_dca_when_free_balance_insufficient() {
	let amount_in = ONE_HDX;
	// Give ALICE 2x + a tiny bit extra so rolling DCA can be created
	// but NOT enough for continuous re-reservation
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 2 * ONE_HDX),
			(BOB, HDX, 10 * ONE_HDX), // holding pot stand-in
		])
		.build()
		.execute_with(|| {
			let _ = with_transaction(|| {
				set_block_number(100);
				let id = crate::Pallet::<Test>::add_intent(ALICE, dca_intent(amount_in, ONE_DOT, None))
					.expect("should work");

				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).free, 0);

				// Simulate ICE: unlock + transfer to holding pot (BOB)
				set_block_number(110);
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, amount_in));
				assert_ok!(orml_tokens::Pallet::<Test>::transfer(
					RuntimeOrigin::signed(ALICE),
					BOB,
					HDX,
					amount_in
				));
				// Now ALICE: free=0, reserved=amount_in

				let resolve = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve));

				// remaining = 2x - 1x = 1x, re-reserve fails (no free), remaining stays 1x
				let stored = Intents::<Test>::get(id).unwrap();
				match stored.data {
					IntentData::Dca(dca) => {
						assert_eq!(dca.remaining_budget, amount_in);
					}
					_ => panic!("expected DCA"),
				}

				// Second trade — simulate ICE: unlock + transfer
				set_block_number(120);
				assert_ok!(crate::Pallet::<Test>::unlock_funds(&ALICE, HDX, amount_in));
				assert_ok!(orml_tokens::Pallet::<Test>::transfer(
					RuntimeOrigin::signed(ALICE),
					BOB,
					HDX,
					amount_in
				));

				let resolve2 = ice_support::ResolvedIntent {
					id,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in,
						amount_out: 2 * ONE_DOT,
						partial: false,
					}),
				};
				assert_ok!(crate::Pallet::<Test>::intent_resolved(&ALICE, &resolve2));

				// DCA completed — removed from storage, no funds left
				assert!(Intents::<Test>::get(id).is_none());
				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).reserved, 0);
				assert_eq!(orml_tokens::Pallet::<Test>::accounts(ALICE, HDX).free, 0);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
}
