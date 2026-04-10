use crate::hyperbridge_cleanup::Stage;
use crate::mock::*;
use crate::{CleanupEnabled, CleanupStage, Event, ExtraGas};
use frame_support::dispatch::{DispatchErrorWithPostInfo, Pays};
use frame_support::{assert_noop, assert_ok, dispatch::PostDispatchInfo};
use orml_tokens::Error;
use orml_traits::MultiCurrency;
use sp_runtime::{
	traits::{BlakeTwo256, Hash},
	DispatchError,
};

#[test]
fn dispatch_as_treasury_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE,
			currency_id: HDX,
			amount: 1_000,
		}));

		let call_hash = BlakeTwo256::hash_of(&call);
		let treasury_balance_before = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_ok!(Dispatcher::dispatch_as_treasury(RuntimeOrigin::root(), call));

		let treasury_balance_after = Tokens::free_balance(HDX, &TreasuryAccount::get());

		assert_eq!(treasury_balance_after, treasury_balance_before - 1_000);

		expect_events(vec![Event::TreasuryManagerCallDispatched {
			call_hash,
			result: Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes,
			}),
		}
		.into()]);
	});
}

#[test]
fn dispatch_as_treasury_should_fail_when_bad_origin() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
			remark: vec![1],
		}));

		assert_noop!(
			Dispatcher::dispatch_as_treasury(RuntimeOrigin::signed(ALICE), call),
			DispatchError::BadOrigin
		);
		expect_events(vec![]);
	});
}

#[test]
fn dispatch_with_extra_gas_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: 1_000,
		}));

		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);
		let bob_initial_balance = Tokens::free_balance(HDX, &BOB);
		let extra_gas = 1_000_000_000;

		// Act
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE),
			call,
			extra_gas
		));

		// Assert
		// Check balance was transferred
		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance - 1_000);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance + 1_000);

		// Verify storage was cleaned up
		assert_eq!(Dispatcher::extra_gas(), 0);
	});
}

#[test]
fn dispatch_with_extra_gas_should_fail_when_call_fails() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange - try to transfer more than available balance
		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);
		let bob_initial_balance = Tokens::free_balance(HDX, &BOB);

		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: alice_initial_balance + 1, // more than ALICE has
		}));

		let extra_gas = 1_000_000_000;

		let r = DispatchErrorWithPostInfo {
			post_info: PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes,
			},
			error: Error::<Test>::BalanceTooLow.into(),
		};
		// Act
		assert_noop!(
			Dispatcher::dispatch_with_extra_gas(RuntimeOrigin::signed(ALICE), call, extra_gas),
			r
		);

		// Assert
		// Check no balance was transferred
		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance);

		// Verify storage was cleaned up even after failure
		assert_eq!(Dispatcher::extra_gas(), 0u64);
	});
}

#[test]
fn get_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should return 0 when no limit is set
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Set a gas limit through dispatch
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark { remark: vec![] }));
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(ALICE),
			call,
			1000
		));

		// Should return 0 after dispatch (storage is cleaned)
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Manually insert a gas limit
		ExtraGas::<Test>::set(500u64);
		assert_eq!(Dispatcher::extra_gas(), 500);
	});
}

#[test]
fn decrease_gas_limit_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Should do nothing when no limit is set
		Dispatcher::decrease_extra_gas(100);
		assert_eq!(Dispatcher::extra_gas(), 0);

		// Set initial gas limit
		ExtraGas::<Test>::set(1000u64);

		// Decrease by zero should not change anything
		Dispatcher::decrease_extra_gas(0);
		assert_eq!(Dispatcher::extra_gas(), 1000);

		// Decrease by some amount
		Dispatcher::decrease_extra_gas(300);
		assert_eq!(Dispatcher::extra_gas(), 700);

		// Decrease by more than remaining should remove the entry
		Dispatcher::decrease_extra_gas(800);
		assert_eq!(Dispatcher::extra_gas(), 0);
		assert_eq!(ExtraGas::<Test>::get(), 0u64);

		// Set initial gas limit again
		ExtraGas::<Test>::set(1000u64);

		// Decrease by exact amount should remove the entry
		Dispatcher::decrease_extra_gas(1000);
		assert_eq!(Dispatcher::extra_gas(), 0);
		assert_eq!(ExtraGas::<Test>::get(), 0u64);
	});
}

#[test]
fn dispatch_as_emergency_admin_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		// Arrange
		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: ALICE,
			currency_id: HDX,
			amount: 1_000,
		}));

		let call_hash = BlakeTwo256::hash_of(&call);
		let admin_balance_before = Tokens::free_balance(HDX, &crate::mock::EmergencyAdminAccount::get());

		assert_ok!(Dispatcher::dispatch_as_emergency_admin(RuntimeOrigin::root(), call));

		let admin_balance_after = Tokens::free_balance(HDX, &crate::mock::EmergencyAdminAccount::get());

		assert_eq!(admin_balance_after, admin_balance_before - 1_000);

		expect_events(vec![Event::EmergencyAdminCallDispatched {
			call_hash,
			result: Ok(PostDispatchInfo {
				actual_weight: None,
				pays_fee: Pays::Yes,
			}),
		}
		.into()]);
	});
}

#[test]
fn dispatch_as_emergency_admin_should_fail_when_bad_origin() {
	ExtBuilder::default().build().execute_with(|| {
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark_with_event {
			remark: vec![1],
		}));

		assert_noop!(
			Dispatcher::dispatch_as_emergency_admin(RuntimeOrigin::signed(ALICE), call),
			DispatchError::BadOrigin
		);
		expect_events(vec![]);
	});
}

#[test]
fn dispatch_with_fee_payer_should_set_and_clear_fee_payer() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(get_fee_payer(), None);

		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: 1_000,
		}));

		assert_ok!(Dispatcher::dispatch_with_fee_payer(RuntimeOrigin::signed(ALICE), call,));

		assert_eq!(get_fee_payer(), None);
	});
}

#[test]
fn dispatch_with_fee_payer_should_clear_on_failure() {
	ExtBuilder::default().build().execute_with(|| {
		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);

		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: alice_initial_balance + 1, // more than ALICE has
		}));

		assert!(Dispatcher::dispatch_with_fee_payer(RuntimeOrigin::signed(ALICE), call).is_err());

		assert_eq!(get_fee_payer(), None);
	});
}

#[test]
fn dispatch_with_fee_payer_should_forward_dispatch_result() {
	ExtBuilder::default().build().execute_with(|| {
		let alice_initial_balance = Tokens::free_balance(HDX, &ALICE);
		let bob_initial_balance = Tokens::free_balance(HDX, &BOB);

		let call = Box::new(RuntimeCall::Tokens(orml_tokens::Call::transfer {
			dest: BOB,
			currency_id: HDX,
			amount: 500,
		}));

		assert_ok!(Dispatcher::dispatch_with_fee_payer(RuntimeOrigin::signed(ALICE), call,));

		assert_eq!(Tokens::free_balance(HDX, &ALICE), alice_initial_balance - 500);
		assert_eq!(Tokens::free_balance(HDX, &BOB), bob_initial_balance + 500);
	});
}

#[test]
fn dispatch_with_fee_payer_should_require_signed_origin() {
	ExtBuilder::default().build().execute_with(|| {
		let call = Box::new(RuntimeCall::System(frame_system::Call::remark { remark: vec![] }));

		assert_noop!(
			Dispatcher::dispatch_with_fee_payer(RuntimeOrigin::root(), call),
			DispatchError::BadOrigin
		);
	});
}

mod hyperbridge_cleanup_tests {
	use super::*;
	use frame_support::storage::unhashed;
	use frame_support::weights::Weight;

	fn insert_keys(prefix: &[u8; 32], count: u32) {
		for i in 0..count {
			let mut key = prefix.to_vec();
			key.extend_from_slice(&i.to_le_bytes());
			unhashed::put(&key, &i);
		}
	}

	fn count_keys(prefix: &[u8; 32]) -> u32 {
		let mut count = 0u32;
		let mut iter = sp_io::storage::next_key(prefix);
		while let Some(key) = iter {
			if !key.starts_with(prefix) {
				break;
			}
			count += 1;
			iter = sp_io::storage::next_key(&key);
		}
		count
	}

	fn run_on_idle(weight: Weight) -> Weight {
		<Dispatcher as frame_support::traits::Hooks<u64>>::on_idle(1, weight)
	}

	fn assert_cleanup_complete() {
		assert!(
			!CleanupEnabled::<Test>::get(),
			"CleanupEnabled must be false after completion"
		);
		assert_eq!(
			CleanupStage::<Test>::get(),
			None,
			"CleanupStage must be None after completion"
		);
		for stage in [
			Stage::StateCommitments,
			Stage::StateMachineUpdateTime,
			Stage::RelayChainStateCommitments,
		] {
			assert_eq!(
				count_keys(&stage.storage_prefix()),
				0,
				"{stage:?} must be empty after cleanup",
			);
		}
	}

	#[test]
	fn pause_cleanup_disables() {
		ExtBuilder::default().build().execute_with(|| {
			// DefaultTrue - cleanup is active right after build
			assert!(CleanupEnabled::<Test>::get());

			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), true));
			assert!(!CleanupEnabled::<Test>::get());
		});
	}

	#[test]
	fn resume_cleanup_enables() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), true));
			assert!(!CleanupEnabled::<Test>::get());

			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), false));
			assert!(CleanupEnabled::<Test>::get());
		});
	}

	#[test]
	fn on_idle_does_nothing_when_disabled() {
		ExtBuilder::default().build().execute_with(|| {
			let prefix = Stage::StateCommitments.storage_prefix();
			insert_keys(&prefix, 5);

			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), true));

			let used = run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(used, MockDbWeight::get().reads(1));
			assert_eq!(count_keys(&prefix), 5, "keys must not be touched when disabled");
		});
	}

	#[test]
	fn on_idle_returns_one_read_worth_weight_when_disabled() {
		ExtBuilder::default().build().execute_with(|| {
			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), true));
			let used = run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(used, MockDbWeight::get().reads(1));
		});
	}

	#[test]
	fn cleanup_works_on_empty_storage() {
		ExtBuilder::default().build().execute_with(|| {
			// DefaultTrue - no explicit enable needed.
			// Run on_idle enough times to exhaust all three empty stages.
			for _ in 0..10 {
				run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
				if !CleanupEnabled::<Test>::get() {
					break;
				}
			}

			assert_cleanup_complete();
		});
	}

	#[test]
	fn cleanup_processes_stages_in_order() {
		ExtBuilder::default().build().execute_with(|| {
			let p1 = Stage::StateCommitments.storage_prefix();
			let p2 = Stage::StateMachineUpdateTime.storage_prefix();
			let p3 = Stage::RelayChainStateCommitments.storage_prefix();
			insert_keys(&p1, 3);
			insert_keys(&p2, 3);
			insert_keys(&p3, 3);

			// Stage is None initially - on_idle uses unwrap_or(StateCommitments).
			assert!(CleanupStage::<Test>::get().is_none());

			// First on_idle clears stage 1 and advances.
			run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(count_keys(&p1), 0, "stage 1 must be cleared");
			assert_eq!(CleanupStage::<Test>::get(), Some(Stage::StateMachineUpdateTime));

			// Second on_idle clears stage 2 and advances.
			run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(count_keys(&p2), 0, "stage 2 must be cleared");
			assert_eq!(CleanupStage::<Test>::get(), Some(Stage::RelayChainStateCommitments));

			// Third on_idle clears stage 3 and finishes.
			run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(count_keys(&p3), 0, "stage 3 must be cleared");

			assert_cleanup_complete();
		});
	}

	#[test]
	fn cleanup_resumes_across_blocks() {
		ExtBuilder::default().build().execute_with(|| {
			let prefix = Stage::StateCommitments.storage_prefix();
			insert_keys(&prefix, 10);

			// Pin to stage 1 so we can observe multi-block progress.
			CleanupStage::<Test>::put(Stage::StateCommitments);

			let mut iterations = 0;
			while CleanupStage::<Test>::get() == Some(Stage::StateCommitments) {
				run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
				iterations += 1;
				assert!(iterations < 100, "cleanup should finish in reasonable iterations");
			}

			assert_eq!(count_keys(&prefix), 0, "all stage-1 keys must be deleted");
		});
	}

	#[test]
	fn cleanup_disables_itself_when_finished() {
		ExtBuilder::default().build().execute_with(|| {
			// DefaultTrue - runs immediately.
			for _ in 0..20 {
				run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
				if !CleanupEnabled::<Test>::get() {
					break;
				}
			}

			assert_cleanup_complete();

			// Subsequent on_idle must be a no-op.
			let used = run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(
				used,
				MockDbWeight::get().reads(1),
				"on_idle must return exactly 1 read after cleanup is done"
			);
		});
	}

	#[test]
	fn pause_preserves_stage_progress() {
		ExtBuilder::default().build().execute_with(|| {
			let p1 = Stage::StateCommitments.storage_prefix();
			insert_keys(&p1, 3);

			// Let stage 1 complete and advance to stage 2.
			run_on_idle(Weight::from_parts(1_000_000_000, 1_000_000));
			assert_eq!(CleanupStage::<Test>::get(), Some(Stage::StateMachineUpdateTime));

			// Pause - stage must be preserved.
			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), true));
			assert_eq!(
				CleanupStage::<Test>::get(),
				Some(Stage::StateMachineUpdateTime),
				"stage must not be reset on pause"
			);

			// Resume - continues from stage 2, not from the beginning.
			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), false));
			assert_eq!(CleanupStage::<Test>::get(), Some(Stage::StateMachineUpdateTime));
		});
	}

	#[test]
	fn resume_is_noop_when_already_running() {
		ExtBuilder::default().build().execute_with(|| {
			// Advance to stage 2 manually.
			CleanupStage::<Test>::put(Stage::StateMachineUpdateTime);

			// Calling resume should NOT reset the stage.
			assert_ok!(Dispatcher::pause_hyperbridge_cleanup(RuntimeOrigin::root(), false));
			assert_eq!(
				CleanupStage::<Test>::get(),
				Some(Stage::StateMachineUpdateTime),
				"resume must not reset an in-progress stage"
			);
		});
	}
}
