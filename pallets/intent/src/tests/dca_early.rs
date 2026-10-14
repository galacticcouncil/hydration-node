//! Early DCA execution: the window gate, the slot stamp and the conditional offer in
//! `solver_intents`. Fixture: ALICE DCA `A→B`, 10_000 per trade, period 20, created and
//! filled at block 100 ⇒ slot 120, window (4 blocks) opens at 116. Raw units, `ED = 1_000`.

use crate::tests::mock::*;
use crate::*;
use frame_support::{assert_noop, assert_ok};
use pretty_assertions::assert_eq;

const A: AssetId = HDX;
const B: AssetId = DOT;
const TRADE: Balance = 10_000;

fn dca_early_intent(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	budget: Option<Balance>,
	period: u32,
) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Dca(DcaParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			slippage: Permill::from_percent(3),
			budget,
			period,
		}),
		deadline: None,
		on_resolved: None,
	}
}

fn swap(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: false,
		}),
		deadline: None,
		on_resolved: None,
	}
}

fn stored_dca(id: IntentId) -> DcaData {
	match IntentPallet::get_intent(id).expect("intent should exist").data {
		IntentData::Dca(dca) => dca,
		_ => panic!("expected DCA intent"),
	}
}

fn swap_resolve(asset_in: AssetId, asset_out: AssetId) -> IntentData {
	IntentData::Swap(SwapData {
		asset_in,
		asset_out,
		amount_in: TRADE,
		amount_out: TRADE,
		partial: Partial::No,
	})
}

/// One DCA trade the way pallet-ice settles it: unlock the per-trade amount, then resolve.
fn fill_dca(who: u64, id: IntentId, asset_in: AssetId, asset_out: AssetId) {
	assert_ok!(IntentPallet::unlock_funds(&who, asset_in, TRADE));
	assert_ok!(IntentPallet::intent_resolved(
		&who,
		&ResolvedIntent {
			id,
			data: swap_resolve(asset_in, asset_out),
		}
	));
}

fn submit(who: u64, input: IntentInput) -> IntentId {
	assert_ok!(IntentPallet::submit_intent(RuntimeOrigin::signed(who), input));
	System::events()
		.iter()
		.rev()
		.find_map(|r| match r.event {
			RuntimeEvent::IntentPallet(Event::IntentSubmitted { id, .. }) => Some(id),
			_ => None,
		})
		.expect("IntentSubmitted event")
}

fn ext() -> ExtBuilder {
	ExtBuilder::default().with_endowed_accounts(vec![
		(ALICE, A, 1_000_000),
		(BOB, B, 1_000_000),
		(BOB, ETH, 1_000_000),
		(CHARLIE, A, 1_000_000),
	])
}

/// Submits and fills `who`'s DCA at block 100, so its next slot is 120.
fn dca_filled_at_100(who: u64, asset_in: AssetId, asset_out: AssetId, budget: Balance) -> IntentId {
	set_block_number(100);
	let id = submit(
		who,
		dca_early_intent(asset_in, asset_out, TRADE, 9_000, Some(budget), 20),
	);
	fill_dca(who, id, asset_in, asset_out);
	assert_eq!(stored_dca(id).last_execution_block, 100);
	id
}

fn alice_dca() -> IntentId {
	set_oracle_price(Some(EmaPrice { n: 1, d: 1 }));
	dca_filled_at_100(ALICE, A, B, 5 * TRADE)
}

fn submit_bob_swap(asset_in: AssetId, amount_in: Balance, amount_out: Balance) -> IntentId {
	submit(BOB, swap(asset_in, A, amount_in, amount_out))
}

fn listed(id: IntentId) -> bool {
	IntentPallet::get_valid_intents().iter().any(|(i, _)| *i == id)
}

// ---- window ----

#[test]
fn dca_early_window_should_cap_at_half_period_when_period_is_short() {
	ext().build().execute_with(|| {
		set_block_number(100);
		let id = submit(ALICE, dca_early_intent(A, B, TRADE, 9_000, Some(5 * TRADE), 5));
		assert_eq!(IntentPallet::dca_early_window(&stored_dca(id)), 2);
	});
}

#[test]
fn dca_early_window_should_use_the_constant_when_half_period_exceeds_it() {
	ext().build().execute_with(|| {
		set_block_number(100);
		let id = submit(ALICE, dca_early_intent(A, B, TRADE, 9_000, Some(5 * TRADE), 20));
		assert_eq!(IntentPallet::dca_early_window(&stored_dca(id)), 4);
	});
}

// ---- resolve-time gate ----

#[test]
fn validate_resolve_should_accept_dca_when_block_is_at_window_open() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(116);
		let intent = IntentPallet::get_intent(id).expect("intent should exist");
		assert_ok!(IntentPallet::validate_resolve(&intent, &swap_resolve(A, B)));
	});
}

#[test]
fn validate_resolve_should_fail_with_intent_active_when_block_is_before_window_open() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(115);
		let intent = IntentPallet::get_intent(id).expect("intent should exist");
		assert_noop!(
			IntentPallet::validate_resolve(&intent, &swap_resolve(A, B)),
			Error::<Test>::IntentActive
		);
	});
}

// ---- stamp ----

#[test]
fn intent_resolved_should_stamp_the_slot_when_fill_is_early() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(116);
		fill_dca(ALICE, id, A, B);
		assert_eq!(stored_dca(id).last_execution_block, 120);
	});
}

#[test]
fn intent_resolved_should_stamp_current_block_when_fill_is_on_time() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(120);
		fill_dca(ALICE, id, A, B);
		assert_eq!(stored_dca(id).last_execution_block, 120);
	});
}

#[test]
fn intent_resolved_should_stamp_current_block_when_fill_is_late() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(125);
		fill_dca(ALICE, id, A, B);
		assert_eq!(stored_dca(id).last_execution_block, 125);
	});
}

#[test]
fn dca_should_keep_its_cadence_when_opposite_flow_is_offered_every_block() {
	ext().build().execute_with(|| {
		set_oracle_price(Some(EmaPrice { n: 1, d: 1 }));
		// Six trades: the fill that drains the budget deletes the intent with its stamp, so
		// five trades would leave the last asserted stamp unobservable.
		let id = dca_filled_at_100(ALICE, A, B, 6 * TRADE);
		submit_bob_swap(B, 100_000, 100_000);

		let mut fills = Vec::new();
		let mut stamps = Vec::new();
		for b in 101..=200 {
			set_block_number(b);
			if listed(id) {
				fills.push(b);
				fill_dca(ALICE, id, A, B);
				stamps.push(IntentPallet::get_intent(id).map(|_| stored_dca(id).last_execution_block));
			}
		}

		assert_eq!(fills, vec![116, 136, 156, 176, 196]);
		assert_eq!(stamps, vec![Some(120), Some(140), Some(160), Some(180), None]);
		assert_eq!(IntentPallet::get_intent(id), None);
		assert_eq!(AccountIntentCount::<Test>::get(ALICE), 0);
		assert_eq!(Currencies::reserved_balance_named(&NAMED_RESERVE_ID, A, &ALICE), 0);
	});
}

// ---- solver_intents ----

#[test]
fn solver_intents_should_withhold_early_dca_when_no_counterparty_exists() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(116);
		assert!(!listed(id));
	});
}

#[test]
fn solver_intents_should_offer_early_dca_when_fillable_reverse_swap_covers_it() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		let bob = submit_bob_swap(B, TRADE, TRADE);
		set_block_number(116);
		let (intents, limits) = IntentPallet::solver_intents();
		let ids: Vec<IntentId> = intents.iter().map(|(i, _)| *i).collect();
		assert_eq!(ids, vec![bob, id]);
		assert_eq!(limits, vec![(id, 9_700)]);
	});
}

#[test]
fn solver_intents_should_withhold_early_dca_when_counterparty_limit_is_above_oracle() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		let bob = submit_bob_swap(B, TRADE, 2 * TRADE);
		set_block_number(116);
		assert!(listed(bob));
		assert!(!listed(id));
	});
}

#[test]
fn solver_intents_should_withhold_early_dca_when_counterparty_is_on_another_pair() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		let bob = submit_bob_swap(ETH, TRADE, TRADE);
		set_block_number(116);
		assert!(listed(bob));
		assert!(!listed(id));
	});
}

#[test]
fn solver_intents_should_withhold_dca_when_block_is_before_window_open_even_with_counterparty() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		let bob = submit_bob_swap(B, TRADE, TRADE);
		set_block_number(115);
		assert!(listed(bob));
		assert!(!listed(id));
	});
}

#[test]
fn solver_intents_should_offer_dca_unconditionally_when_slot_is_due() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		set_block_number(120);
		assert!(listed(id));
	});
}

#[test]
fn solver_intents_should_withhold_early_dca_when_oracle_price_is_missing() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		submit_bob_swap(B, TRADE, TRADE);
		set_oracle_price(None);
		set_block_number(116);
		assert!(!listed(id));
		set_block_number(120);
		assert!(listed(id));
	});
}

#[test]
fn solver_intents_should_offer_both_early_dcas_when_they_are_on_opposite_sides() {
	ext().build().execute_with(|| {
		let alice = alice_dca();
		let bob = dca_filled_at_100(BOB, B, A, 5 * TRADE);
		set_block_number(116);
		let (intents, limits) = IntentPallet::solver_intents();
		let ids: Vec<IntentId> = intents.iter().map(|(i, _)| *i).collect();
		assert_eq!(ids, vec![bob, alice]);
		assert_eq!(limits, vec![(bob, 9_700), (alice, 9_700)]);
	});
}

#[test]
fn solver_intents_should_withhold_early_dca_when_shortfall_reaches_ed() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		submit_bob_swap(B, 9_000, 9_000);
		set_block_number(116);
		assert!(!listed(id));
	});
}

#[test]
fn solver_intents_should_offer_early_dca_when_shortfall_is_below_ed() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		submit_bob_swap(B, 9_500, 9_500);
		set_block_number(116);
		assert!(listed(id));
	});
}

#[test]
fn solver_intents_should_offer_early_dca_when_counterparty_covers_it_at_the_reverse_pair_rate() {
	ext().build().execute_with(|| {
		let id = alice_dca();
		// 1 B sells for 2 A, so 5_000 B of counter-flow covers ALICE's 10_000 A.
		set_pair_oracle_price(B, A, EmaPrice { n: 1, d: 2 });
		submit_bob_swap(B, 5_000, 10_000);
		set_block_number(116);
		assert!(listed(id));
	});
}

#[test]
fn solver_intents_should_offer_only_the_older_early_dca_when_one_counterparty_covers_one_trade() {
	ext().build().execute_with(|| {
		let alice = alice_dca();
		dca_filled_at_100(CHARLIE, A, B, 5 * TRADE);
		let bob = submit_bob_swap(B, TRADE, TRADE);
		set_block_number(116);
		let (intents, _) = IntentPallet::solver_intents();
		let ids: Vec<IntentId> = intents.iter().map(|(i, _)| *i).collect();
		assert_eq!(ids, vec![bob, alice]);
	});
}

#[test]
fn solver_intents_should_withhold_early_dca_when_a_due_same_side_dca_takes_the_counterparty() {
	ext().build().execute_with(|| {
		set_oracle_price(Some(EmaPrice { n: 1, d: 1 }));
		set_block_number(96);
		let charlie = submit(CHARLIE, dca_early_intent(A, B, TRADE, 9_000, Some(5 * TRADE), 20));
		fill_dca(CHARLIE, charlie, A, B);
		let alice = dca_filled_at_100(ALICE, A, B, 5 * TRADE);
		let bob = submit_bob_swap(B, TRADE, TRADE);
		set_block_number(116);
		let (intents, _) = IntentPallet::solver_intents();
		let ids: Vec<IntentId> = intents.iter().map(|(i, _)| *i).collect();
		assert_eq!(ids, vec![bob, charlie]);
		assert!(!listed(alice));
	});
}
