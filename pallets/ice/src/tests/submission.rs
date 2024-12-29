use super::*;
use crate::pallet::SolutionScore;
use crate::tests::{ExtBuilder, ICE};
use frame_support::traits::Hooks;
use frame_support::{assert_noop, assert_ok};

#[test]
fn validate_submission_should_return_ok_and_set_current_score_when_submission_is_valid() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		assert_eq!(SolutionScore::<Test>::get(), Some((who, score)));
	});
}

#[test]
fn validate_submission_should_fail_when_block_is_not_correct() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::default();
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(!crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number + 1
		),);
	});
}

#[test]
fn validate_submission_should_fail_when_resolved_intents_is_empty() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::default();
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(!crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		),);
	});
}

#[test]
fn validate_submission_should_fail_when_score_is_not_higher_then_current_score() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 10;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));

		let who = BOB;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 5;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(!crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
	});
}

#[test]
fn validate_submission_should_return_ok_when_submission_is_validated_again() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
	});
}

#[test]
fn validate_submission_should_return_false_when_submission_is_by_same_account_but_lesser_score() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 10;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 5;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(!crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
	});
}

#[test]
fn validate_submission_should_replace_solution_when_submitted_by_different_account() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));

		let who = BOB;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 10;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		assert_eq!(SolutionScore::<Test>::get(), Some((BOB, 10)));
	});
}

#[test]
fn validate_submission_should_replace_solution_when_submitted_by_same_account() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));

		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 20;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		assert_eq!(SolutionScore::<Test>::get(), Some((ALICE, 20)));
	});
}

#[test]
fn on_finalize_should_clear_solution_score() {
	ExtBuilder::default().build().execute_with(|| {
		let who = ALICE;
		let resolved_intents = BoundedResolvedIntents::truncate_from(vec![ResolvedIntent {
			intent_id: 0,
			amount_in: 100,
			amount_out: 200,
		}]);
		let score = 1;
		let block_number = <Test as pallet_ice::Config>::BlockNumberProvider::current_block_number();
		assert!(crate::Pallet::<Test>::validate_submission(
			&who,
			&resolved_intents,
			score,
			block_number
		));
		assert_eq!(SolutionScore::<Test>::get(), Some((ALICE, 1)));
		<crate::Pallet<Test>>::on_finalize(1);
		assert_eq!(SolutionScore::<Test>::get(), None);
	});
}
