use crate::{mock::*, Event};
use frame_system::Pallet as System;
use pallet_session::SessionManager;

fn last_bench_event() -> Option<(AccountId, sp_staking::SessionIndex)> {
	System::<Test>::events().into_iter().rev().find_map(|r| match r.event {
		RuntimeEvent::CollatorRotation(Event::CollatorBenched { who, session_index }) => Some((who, session_index)),
		_ => None,
	})
}

fn bench_events() -> Vec<(AccountId, sp_staking::SessionIndex)> {
	System::<Test>::events()
		.into_iter()
		.filter_map(|r| match r.event {
			RuntimeEvent::CollatorRotation(Event::CollatorBenched { who, session_index }) => Some((who, session_index)),
			_ => None,
		})
		.collect()
}

#[test]
fn even_sessions_run_full_set_with_no_event() {
	ExtBuilder.build().execute_with(|| {
		set_inner(Some(vec![1, 2, 3, 4, 5]));
		for idx in [0u32, 2, 4, 6, 100] {
			System::<Test>::reset_events();
			let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(idx).unwrap();
			assert_eq!(out, vec![1, 2, 3, 4, 5], "session {idx} should keep full set");
			assert!(bench_events().is_empty(), "session {idx} must not emit");
		}
	});
}

#[test]
fn benches_first_collator_in_session_one() {
	ExtBuilder.build().execute_with(|| {
		set_inner(Some(vec![1, 2, 3, 4, 5]));
		let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(1).unwrap();
		assert_eq!(out, vec![2, 3, 4, 5]);
		assert_eq!(last_bench_event(), Some((1, 1)));
	});
}

#[test]
fn bench_index_advances_every_other_session() {
	// Session 1 -> bench[0], session 3 -> bench[1], session 5 -> bench[2], etc.
	ExtBuilder.build().execute_with(|| {
		let set = vec![10, 20, 30, 40, 50];
		set_inner(Some(set.clone()));
		for (i, &expected_who) in set.iter().enumerate() {
			let session = (i as u32) * 2 + 1;
			System::<Test>::reset_events();
			let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(session).unwrap();
			assert_eq!(out.len(), set.len() - 1);
			assert_eq!(last_bench_event(), Some((expected_who, session)));
		}
	});
}

#[test]
fn bench_index_wraps_modulo_set_length() {
	// session 7 -> 7/2 = 3 -> bench[3 % 3 = 0] = 10
	ExtBuilder.build().execute_with(|| {
		set_inner(Some(vec![10, 20, 30]));
		let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(7).unwrap();
		assert_eq!(out, vec![20, 30]);
		assert_eq!(last_bench_event(), Some((10, 7)));
	});
}

#[test]
fn does_not_bench_single_collator_set() {
	ExtBuilder.build().execute_with(|| {
		set_inner(Some(vec![42]));
		let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(1).unwrap();
		assert_eq!(out, vec![42]);
		assert!(bench_events().is_empty());
	});
}

#[test]
fn propagates_none_from_inner() {
	ExtBuilder.build().execute_with(|| {
		set_inner(None);
		let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(1);
		assert!(out.is_none());
		assert!(bench_events().is_empty());
	});
}

#[test]
fn empty_set_from_inner_passes_through_without_bench() {
	ExtBuilder.build().execute_with(|| {
		set_inner(Some(vec![]));
		let out = <crate::Pallet<Test> as SessionManager<AccountId>>::new_session(1).unwrap();
		assert!(out.is_empty());
		assert!(bench_events().is_empty());
	});
}

#[test]
fn end_and_start_session_pass_through() {
	ExtBuilder.build().execute_with(|| {
		<crate::Pallet<Test> as SessionManager<AccountId>>::end_session(3);
		<crate::Pallet<Test> as SessionManager<AccountId>>::end_session(4);
		<crate::Pallet<Test> as SessionManager<AccountId>>::start_session(7);
		assert_eq!(end_calls(), vec![3, 4]);
		assert_eq!(start_calls(), vec![7]);
	});
}
