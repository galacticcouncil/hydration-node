use crate::tests::mock::*;
use crate::*;
use frame_support::assert_ok;
use pretty_assertions::assert_eq;

fn swap_with_deadline(deadline: Option<Moment>) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in: HDX,
			asset_out: DOT,
			amount_in: 10 * ONE_HDX,
			amount_out: 100 * ONE_DOT,
			partial: false,
		}),
		deadline,
		on_resolved: None,
	}
}

fn ext_with(deadline: Option<Moment>) -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_with_deadline(deadline))])
		.build()
}

#[test]
fn solver_intents_should_include_swap_when_deadline_is_beyond_the_margin() {
	ext_with(Some(4 * ONE_SECOND)).execute_with(|| {
		// now = 0, cutoff = 2s, deadline = 4s
		assert_eq!(IntentPallet::get_valid_intents().len(), 1);
	});
}

#[test]
fn solver_intents_should_exclude_swap_when_deadline_has_passed() {
	ext_with(Some(4 * ONE_SECOND)).execute_with(|| {
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), 4 * ONE_SECOND + 1));

		assert!(IntentPallet::get_valid_intents().is_empty());
	});
}

/// The batch-poisoning case: still valid now, but gone before the next block.
#[test]
fn solver_intents_should_exclude_swap_when_deadline_falls_within_the_margin() {
	ext_with(Some(4 * ONE_SECOND)).execute_with(|| {
		// now = 3s, cutoff = 5s, deadline = 4s — unexpired, yet withheld.
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), 3 * ONE_SECOND));

		assert!(
			IntentPallet::get_intent(0).is_some(),
			"intent is still live and unexpired"
		);
		assert!(IntentPallet::get_valid_intents().is_empty());
	});
}

#[test]
fn solver_intents_should_exclude_swap_when_deadline_equals_the_margin_boundary() {
	ext_with(Some(SOLVER_DEADLINE_MARGIN)).execute_with(|| {
		// now = 0, cutoff = deadline — the boundary is exclusive of the intent.
		assert!(IntentPallet::get_valid_intents().is_empty());
	});
}

#[test]
fn solver_intents_should_include_swap_when_deadline_is_one_past_the_margin_boundary() {
	ext_with(Some(SOLVER_DEADLINE_MARGIN + 1)).execute_with(|| {
		assert_eq!(IntentPallet::get_valid_intents().len(), 1);
	});
}

#[test]
fn solver_intents_should_include_swap_when_it_has_no_deadline() {
	ext_with(None).execute_with(|| {
		assert_ok!(Timestamp::set(RuntimeOrigin::none(), 4 * ONE_SECOND));

		assert_eq!(IntentPallet::get_valid_intents().len(), 1);
	});
}

/// One doomed intent must not hide a healthy one from the solver.
#[test]
fn solver_intents_should_return_only_the_surviving_swap_when_another_expires_within_the_margin() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 100 * ONE_HDX), (BOB, HDX, 100 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_with_deadline(Some(2 * ONE_SECOND))),
			(BOB, swap_with_deadline(Some(4 * ONE_SECOND + 500))),
		])
		.build()
		.execute_with(|| {
			// now = 1s, cutoff = 3s: alice's 2s deadline is inside it, bob's 4.5s is not.
			assert_ok!(Timestamp::set(RuntimeOrigin::none(), ONE_SECOND));

			let valid = IntentPallet::get_valid_intents();

			assert_eq!(valid.len(), 1);
			assert_eq!(IntentPallet::intent_owner(valid[0].0), Some(BOB));
		});
}
