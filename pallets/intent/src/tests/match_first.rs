//! Match-first swaps: submission rules, `match_until` lifecycle and the conditional-offer pass
//! in `solver_intents`. `W` is the waiting intent under test, `O` a plain counterparty, `P` a
//! match-first partial on the reverse pair. Amounts are raw units with `ED = 1_000`.

use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

const A: AssetId = HDX;
const B: AssetId = DOT;
const WINDOW: Moment = 3 * ONE_SECOND;

fn swap(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance, partial: bool) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial,
		}),
		deadline: None,
		on_resolved: None,
	}
}

fn match_first(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	partial: bool,
	match_window: Moment,
	deadline: Option<Moment>,
) -> IntentInput {
	IntentInput {
		data: IntentDataInput::MatchFirstSwap {
			swap: SwapParams {
				asset_in,
				asset_out,
				amount_in,
				amount_out,
				partial,
			},
			match_window,
		},
		deadline,
		on_resolved: None,
	}
}

fn waiting(amount_in: Balance, partial: bool) -> IntentInput {
	match_first(A, B, amount_in, amount_in, partial, WINDOW, None)
}

fn dca(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Dca(DcaParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			slippage: Permill::from_percent(3),
			budget: Some(5 * amount_in),
			period: MIN_DCA_PERIOD,
		}),
		deadline: None,
		on_resolved: None,
	}
}

fn ids(list: &[(IntentId, Intent)]) -> Vec<IntentId> {
	list.iter().map(|(id, _)| *id).collect()
}

fn view(list: &[(IntentId, Intent)], id: IntentId) -> SwapData {
	let (_, intent) = list.iter().find(|(i, _)| *i == id).expect("intent in list");
	let IntentData::Swap(s) = &intent.data else {
		panic!("expected a swap view")
	};
	s.clone()
}

fn stored_swap(id: IntentId) -> SwapData {
	let data = IntentPallet::get_intent(id).expect("intent stored").data;
	data.swap().expect("expected a stored swap").clone()
}

/// Seeds a partial fill of `amount` at the stored 1:1 rate (`validate_resolve` must clear).
fn fill(id: IntentId, amount: Balance) {
	let stored = stored_swap(id);
	let who = IntentPallet::intent_owner(id).expect("owner");
	assert_ok!(IntentPallet::intent_resolved(
		&who,
		&ResolvedIntent {
			id,
			data: IntentData::Swap(SwapData {
				asset_in: stored.asset_in,
				asset_out: stored.asset_out,
				amount_in: amount,
				amount_out: amount,
				partial: Partial::Yes(0),
			}),
		}
	));
}

fn match_until(id: IntentId) -> Option<Moment> {
	match IntentPallet::get_intent(id)?.data {
		IntentData::MatchFirstSwap { match_until, .. } => Some(match_until),
		_ => None,
	}
}

fn set_now(t: Moment) {
	pallet_timestamp::Now::<Test>::put(t);
}

fn one_to_one() {
	set_oracle_price(Some(EmaPrice { n: 1, d: 1 }));
}

fn ext() -> ExtBuilder {
	ExtBuilder::default().with_endowed_accounts(vec![
		(ALICE, A, 1_000_000),
		(BOB, B, 1_000_000),
		(BOB, ETH, 1_000_000),
		(CHARLIE, A, 1_000_000),
	])
}

// ---------------------------------------------------------------------------
// Submission and `match_until` lifecycle
// ---------------------------------------------------------------------------

#[test]
fn submit_intent_should_store_swap_with_match_until_when_match_window_is_valid() {
	ext().build().execute_with(|| {
		assert_ok!(IntentPallet::submit_intent(
			RuntimeOrigin::signed(ALICE),
			match_first(A, B, 10_000, 10_000, false, WINDOW, None)
		));

		let expected = Intent {
			data: IntentData::MatchFirstSwap {
				swap: SwapData {
					asset_in: A,
					asset_out: B,
					amount_in: 10_000,
					amount_out: 10_000,
					partial: Partial::No,
				},
				match_until: WINDOW,
			},
			deadline: None,
			on_resolved: None,
		};
		assert_eq!(IntentPallet::get_intent(0), Some(expected.clone()));
		assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, A, &ALICE), 10_000);
		System::assert_has_event(RuntimeEvent::IntentPallet(Event::IntentSubmitted {
			id: 0,
			owner: ALICE,
			intent: expected,
		}));
	});
}

#[test]
fn submit_intent_should_fail_when_match_window_is_zero() {
	ext().build().execute_with(|| {
		assert_noop!(
			IntentPallet::submit_intent(
				RuntimeOrigin::signed(ALICE),
				match_first(A, B, 10_000, 10_000, false, 0, None)
			),
			Error::<Test>::InvalidMatchWindow
		);
	});
}

#[test]
fn submit_intent_should_fail_when_match_window_ends_at_deadline() {
	ext().build().execute_with(|| {
		assert_noop!(
			IntentPallet::submit_intent(
				RuntimeOrigin::signed(ALICE),
				match_first(A, B, 10_000, 10_000, false, WINDOW, Some(WINDOW))
			),
			Error::<Test>::InvalidMatchWindow
		);
	});
}

#[test]
fn submit_intent_should_fail_when_match_window_ends_after_deadline() {
	ext().build().execute_with(|| {
		assert_noop!(
			IntentPallet::submit_intent(
				RuntimeOrigin::signed(ALICE),
				match_first(A, B, 10_000, 10_000, false, 4 * ONE_SECOND, Some(WINDOW))
			),
			Error::<Test>::InvalidMatchWindow
		);
	});
}

#[test]
fn submit_intent_should_accept_match_window_beyond_max_intent_duration_when_no_deadline() {
	ext().build().execute_with(|| {
		assert_ok!(IntentPallet::submit_intent(
			RuntimeOrigin::signed(ALICE),
			match_first(A, B, 10_000, 10_000, false, u64::MAX, None)
		));

		assert_eq!(match_until(0), Some(Moment::MAX));
	});
}

#[test]
fn intent_resolved_should_keep_match_until_when_partially_filled() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, true))])
		.build()
		.execute_with(|| {
			fill(0, 4_000);

			assert_eq!(stored_swap(0).partial, Partial::Yes(4_000));
			assert_eq!(match_until(0), Some(WINDOW));
		});
}

#[test]
fn intent_resolved_should_remove_intent_when_fully_filled_by_plain_swap() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false))])
		.build()
		.execute_with(|| {
			assert_ok!(IntentPallet::intent_resolved(
				&ALICE,
				&ResolvedIntent {
					id: 0,
					data: IntentData::Swap(stored_swap(0)),
				}
			));

			assert_eq!(IntentPallet::get_intent(0), None);
		});
}

#[test]
fn intent_resolved_should_fail_when_resolve_carries_match_first_variant() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false))])
		.build()
		.execute_with(|| {
			let stored = IntentPallet::get_intent(0).expect("intent stored");

			assert_noop!(
				IntentPallet::intent_resolved(
					&ALICE,
					&ResolvedIntent {
						id: 0,
						data: stored.data
					}
				),
				Error::<Test>::ResolveMismatch
			);
		});
}

// ---------------------------------------------------------------------------
// The list rule
// ---------------------------------------------------------------------------

#[test]
fn solver_intents_should_withhold_match_first_swap_when_no_counterparty_exists() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false))])
		.build()
		.execute_with(|| {
			one_to_one();

			assert_eq!(IntentPallet::get_valid_intents(), vec![]);
		});
}

#[test]
fn solver_intents_should_offer_match_first_swap_when_reverse_pair_counterparty_covers_it() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
			assert_eq!(view(&list, 1), stored_swap(1));
		});
}

#[test]
fn solver_intents_should_withhold_match_first_swap_when_counterparty_limit_is_above_market() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000, 20_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_withhold_match_first_swap_when_counterparty_is_on_another_pair() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(ETH, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_withhold_non_partial_match_first_swap_when_counter_flow_is_short_by_at_least_ed() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000 - ED, 10_000 - ED, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_offer_non_partial_match_first_swap_when_counter_flow_is_short_by_less_than_ed() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000 - ED + 1, 10_000 - ED + 1, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
		});
}

#[test]
fn solver_intents_should_offer_non_partial_match_first_swap_when_counter_flow_equals_it() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0).partial, Partial::No);
			assert_eq!(view(&list, 0).remaining(), 10_000);
		});
}

#[test]
fn solver_intents_should_cap_partial_match_first_swap_when_counter_flow_covers_less_than_remaining() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, swap(B, A, 4_000, 4_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(
				view(&list, 0),
				SwapData {
					asset_in: A,
					asset_out: B,
					amount_in: 10_000,
					amount_out: 10_000,
					partial: Partial::Yes(6_000),
				}
			);
			assert_eq!(view(&list, 0).remaining(), 4_000);
			assert_eq!(view(&list, 1), stored_swap(1));
			assert_eq!(stored_swap(0).partial, Partial::Yes(0));
		});
}

#[test]
fn solver_intents_should_withhold_partial_match_first_swap_when_counter_flow_is_below_ed() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, swap(B, A, 10_000, 10_000, true)),
		])
		.build()
		.execute_with(|| {
			one_to_one();
			fill(1, 10_000 - ED / 2);
			assert_eq!(stored_swap(1).remaining(), ED / 2);

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_offer_whole_remaining_when_capping_would_leave_less_than_ed() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, swap(B, A, 10_000 - ED / 2, 10_000 - ED / 2, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0).partial, Partial::Yes(0));
			assert_eq!(view(&list, 0).remaining(), 10_000);
		});
}

#[test]
fn solver_intents_should_cap_from_stored_remaining_when_match_first_swap_is_already_partly_filled() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, swap(B, A, 4_000, 4_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();
			fill(0, 3_000);

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0).partial, Partial::Yes(6_000));
			assert_eq!(view(&list, 0).remaining(), 4_000);
			assert_eq!(stored_swap(0).partial, Partial::Yes(3_000));
		});
}

#[test]
fn solver_intents_should_offer_stored_remaining_when_counter_flow_exceeds_it_after_a_partial_fill() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, swap(B, A, 8_000, 8_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();
			fill(0, 3_000);

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0).partial, Partial::Yes(3_000));
			assert_eq!(view(&list, 0).remaining(), 7_000);
		});
}

#[test]
fn solver_intents_should_offer_both_when_two_match_first_partials_face_each_other() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, true)),
			(BOB, match_first(B, A, 30_000, 30_000, true, WINDOW, None)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0).partial, Partial::Yes(0));
			assert_eq!(view(&list, 0).remaining(), 10_000);
			assert_eq!(view(&list, 1).partial, Partial::Yes(20_000));
			assert_eq!(view(&list, 1).remaining(), 10_000);
		});
}

/// The rounding case: a capped partial can never cover a non-partial exactly under integer
/// floors (`⌊3/7·⌊7/3·X⌋⌋ < X`); the sub-ED tolerance is what keeps both offered.
#[test]
fn solver_intents_should_offer_non_partial_match_first_swap_when_covered_by_a_capped_match_first_partial_at_a_rounding_rate(
) {
	ext()
		.with_intents(vec![
			(ALICE, match_first(A, B, 10_000, 23_000, false, WINDOW, None)),
			(BOB, match_first(B, A, 100_000, 40_000, true, WINDOW, None)),
		])
		.build()
		.execute_with(|| {
			set_pair_oracle_price(A, B, EmaPrice { n: 3, d: 7 });
			set_pair_oracle_price(B, A, EmaPrice { n: 7, d: 3 });

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
			assert_eq!(view(&list, 1).partial, Partial::Yes(76_667));
			assert_eq!(view(&list, 1).remaining(), 23_333);
		});
}

#[test]
fn solver_intents_should_withhold_both_when_the_only_coverage_is_a_withheld_match_first_swap() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, match_first(B, A, 20_000, 20_000, false, WINDOW, None)),
			(CHARLIE, swap(A, B, 5_000, 50_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![2]);
		});
}

#[test]
fn solver_intents_should_withhold_match_first_swap_when_no_oracle_price_exists() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			set_oracle_price(None);

			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_offer_match_first_swap_as_ordinary_swap_when_window_has_closed() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false))])
		.build()
		.execute_with(|| {
			one_to_one();
			set_now(WINDOW);

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![0]);
			assert_eq!(view(&list, 0), stored_swap(0));
			assert_eq!(match_until(0), Some(WINDOW));
		});
}

#[test]
fn solver_intents_should_withhold_match_first_swap_when_one_ms_before_window_closes() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false))])
		.build()
		.execute_with(|| {
			one_to_one();
			set_now(WINDOW - 1);

			assert_eq!(IntentPallet::get_valid_intents(), vec![]);
		});
}

#[test]
fn solver_intents_should_exclude_match_first_swap_when_deadline_falls_within_the_margin() {
	ext()
		.with_intents(vec![
			(
				ALICE,
				match_first(A, B, 10_000, 10_000, false, WINDOW, Some(4 * ONE_SECOND)),
			),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();
			// now = 0, cutoff = 2s < 4s deadline
			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1, 0]);

			// now = 2s, cutoff = 4s == deadline — withheld by the margin, window still open
			set_now(SOLVER_DEADLINE_MARGIN);
			assert_eq!(ids(&IntentPallet::get_valid_intents()), vec![1]);
		});
}

#[test]
fn solver_intents_should_count_due_dca_as_counterparty_when_its_hard_limit_clears_the_oracle() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false)), (BOB, dca(B, A, 10_000, 10_000))])
		.build()
		.execute_with(|| {
			one_to_one();
			set_block_number(10);

			let (list, limits) = IntentPallet::solver_intents();

			assert_eq!(ids(&list), vec![1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
			assert_eq!(view(&list, 1).amount_out, 10_000);
			assert_eq!(limits, vec![(1, 10_000)]);
		});
}

#[test]
fn solver_intents_should_not_count_due_dca_as_counterparty_when_its_hard_limit_is_above_the_oracle() {
	ext()
		.with_intents(vec![(ALICE, waiting(10_000, false)), (BOB, dca(B, A, 10_000, 20_000))])
		.build()
		.execute_with(|| {
			one_to_one();
			set_block_number(10);

			let (list, limits) = IntentPallet::solver_intents();

			assert_eq!(ids(&list), vec![1]);
			assert_eq!(limits, vec![(1, 20_000)]);
		});
}

#[test]
fn solver_intents_should_withhold_newer_match_first_swap_when_older_one_takes_the_only_counter_flow() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(CHARLIE, waiting(10_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![2, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
		});
}

#[test]
fn solver_intents_should_cap_newer_partial_to_counter_flow_left_when_older_partial_is_offered_first() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(6_000, true)),
			(CHARLIE, waiting(10_000, true)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![2, 1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
			assert_eq!(view(&list, 1).partial, Partial::Yes(6_000));
			assert_eq!(view(&list, 1).remaining(), 4_000);
		});
}

#[test]
fn solver_intents_should_withhold_match_first_swap_when_plain_same_side_swap_takes_the_counter_flow() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(CHARLIE, swap(A, B, 10_000, 10_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![2, 1]);
		});
}

#[test]
fn solver_intents_should_offer_match_first_swap_when_plain_same_side_swap_limit_is_above_market() {
	ext()
		.with_intents(vec![
			(ALICE, waiting(10_000, false)),
			(CHARLIE, swap(A, B, 10_000, 20_000, false)),
			(BOB, swap(B, A, 10_000, 10_000, false)),
		])
		.build()
		.execute_with(|| {
			one_to_one();

			let list = IntentPallet::get_valid_intents();

			assert_eq!(ids(&list), vec![2, 1, 0]);
			assert_eq!(view(&list, 0), stored_swap(0));
		});
}
