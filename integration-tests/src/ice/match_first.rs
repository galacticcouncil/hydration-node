//! Match-first swaps against the real `mainnet_apr` snapshot: an intent that the AMM could
//! fill right now waits for reverse-pair flow while its window is open, then behaves like
//! any swap. `go_to_block` neither moves `Timestamp` nor resets `System::events`, so `step()`
//! does both by hand.

use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE, BOB, CHARLIE};
use frame_support::assert_ok;
use frame_support::storage::with_transaction;
use frame_support::traits::Time;
use hydradx_runtime::{AssetRegistry, Currencies, Runtime, RuntimeEvent, RuntimeOrigin, Timestamp};
use hydradx_traits::registry::{AssetKind, Create, Inspect};
use ice_support::{Balance, IntentData, IntentDataInput, IntentId, Partial, SolverMode, SwapData, SwapParams};
use orml_traits::{MultiCurrency, NamedMultiReservableCurrency};
use pallet_intent::NAMED_RESERVE_ID;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::{AccountId, AssetId};
use sp_runtime::{DispatchError, TransactionOutcome};
use xcm_emulator::Network;

use super::harness::{
	amm_in_for, amm_trade_count, enable_slip_fees, is_resolved, resolved, run_and_submit_as, solve_as, swap, V4Solver,
};
use super::PATH_TO_SNAPSHOT;

const HDX: AssetId = 0;
const BNC: AssetId = 14;
const HDX_UNIT: Balance = 1_000_000_000_000;
const BNC_UNIT: Balance = 1_000_000_000_000;

/// The `solver_execute_solution1` pair: 10 HDX ⇄ 10 BNC, both limits loose.
const AMOUNT: Balance = 10 * HDX_UNIT;
const MIN_OUT_BNC: Balance = 68_795_189_840;
const MIN_OUT_HDX: Balance = HDX_UNIT;
const WINDOW_BLOCKS: u32 = 10;
const WINDOW: u64 = MILLISECS_PER_BLOCK * WINDOW_BLOCKS as u64;

/// Block-1 AMM fill of 10 HDX → BNC alone (`solver_v1_single_intent`).
const CONTROL_OUT: Balance = 674_325_708_682;
/// Matched against BOB after nine empty blocks.
const PIN_ALICE_MATCHED_OUT: Balance = 676_286_517_105;
const PIN_BOB_AMM_IN: Balance = 9_323_578_198_535;
const PIN_BOB_OUT: Balance = 147_407_011_313_812;
/// AMM fill of 10 HDX → BNC alone, one block after the window closed.
const PIN_OUT_AFTER_WINDOW: Balance = 674_325_708_682;
const PIN_OUT_AFTER_WINDOW_WITH_RESTING_ORDER: Balance = 674_325_708_682;
const PIN_FEE_HDX: Balance = 0;
const PIN_FEE_BNC: Balance = 0;
const PIN_POT_HDX: Balance = 15_742_275_358;
const PIN_POT_BNC: Balance = 135_284_360;

const PARTIAL_AMOUNT: Balance = 1_000 * HDX_UNIT;
const PARTIAL_MIN_OUT: Balance = 50 * BNC_UNIT;
const COUNTER_AMOUNT: Balance = 7 * BNC_UNIT;
const PIN_CAP: Balance = 103_485_832_957_624;
const PIN_FILL_1: Balance = 103_485_832_957_624;
const PIN_FILL_2: Balance = 103_485_832_957_624;
const PIN_REST_OUT: Balance = 53_474_486_908_530;

const PIN_BOB_CAP: Balance = 676_421_090_688;
const PIN_MUTUAL_ALICE_OUT: Balance = 676_285_806_470;
const PIN_MUTUAL_BOB_FILL: Balance = 676_421_090_688;
const PIN_MUTUAL_AMM_TRADES: usize = 0;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn step() {
	hydradx_run_to_next_block();
	hydradx_runtime::System::reset_events();
	Timestamp::set_timestamp(Timestamp::now() + MILLISECS_PER_BLOCK);
}

fn intent_of(owner: &AccountId) -> IntentId {
	pallet_intent::Intents::<Runtime>::iter_keys()
		.find(|id| pallet_intent::Pallet::<Runtime>::intent_owner(*id).as_ref() == Some(owner))
		.expect("owner has a stored intent")
}

fn match_until(id: IntentId) -> Option<u64> {
	match pallet_intent::Pallet::<Runtime>::get_intent(id)?.data {
		IntentData::MatchFirstSwap { match_until, .. } => Some(match_until),
		_ => None,
	}
}

fn in_list(id: IntentId) -> bool {
	pallet_intent::Pallet::<Runtime>::get_valid_intents()
		.iter()
		.any(|(i, _)| *i == id)
}

fn view(id: IntentId) -> SwapData {
	let list = pallet_intent::Pallet::<Runtime>::get_valid_intents();
	let (_, intent) = list.iter().find(|(i, _)| *i == id).expect("intent offered");
	match &intent.data {
		IntentData::Swap(s) => s.clone(),
		_ => panic!("expected a swap view"),
	}
}

fn stored_swap(id: IntentId) -> SwapData {
	let data = pallet_intent::Pallet::<Runtime>::get_intent(id)
		.expect("intent stored")
		.data;
	data.swap().expect("expected a stored swap").clone()
}

fn reserved(who: &AccountId, asset: AssetId) -> Balance {
	Currencies::reserved_balance_named(&NAMED_RESERVE_ID, asset, who)
}

fn intent_events() -> Vec<pallet_intent::Event<Runtime>> {
	hydradx_runtime::System::events()
		.into_iter()
		.filter_map(|r| match r.event {
			RuntimeEvent::Intent(e) => Some(e),
			_ => None,
		})
		.collect()
}

fn resolved_ids() -> Vec<IntentId> {
	intent_events()
		.into_iter()
		.filter_map(|e| match e {
			pallet_intent::Event::IntentResolved { id, .. } => Some(id),
			_ => None,
		})
		.collect()
}

fn partial_fill_of(id: IntentId) -> Balance {
	intent_events()
		.into_iter()
		.find_map(|e| match e {
			pallet_intent::Event::IntentResovedPartially { id: i, amount_in, .. } if i == id => Some(amount_in),
			_ => None,
		})
		.expect("a partial resolution event")
}

fn fee_receiver() -> AccountId {
	hydradx_runtime::IceFeeReceiver::get()
}

fn holding_pot() -> AccountId {
	pallet_ice::Pallet::<Runtime>::get_pallet_account()
}

fn balances(who: &AccountId) -> (Balance, Balance) {
	(Currencies::total_balance(HDX, who), Currencies::total_balance(BNC, who))
}

fn submit(who: &AccountId, data: IntentDataInput) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
		pallet_intent::types::IntentInput {
			data,
			deadline: None,
			on_resolved: None,
		}
	));
}

fn plain(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> IntentDataInput {
	IntentDataInput::Swap(SwapParams {
		asset_in,
		asset_out,
		amount_in,
		amount_out,
		partial: false,
	})
}

fn match_first(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	partial: bool,
) -> IntentDataInput {
	IntentDataInput::MatchFirstSwap {
		swap: SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial,
		},
		match_window: WINDOW,
	}
}

/// The waiting intent is absent from the solver view, nothing resolves it, its reserve is intact.
fn assert_withheld(id: IntentId, who: &AccountId, asset_in: AssetId, reserve: Balance) {
	assert!(!in_list(id), "waiting intent must not be offered");
	assert!(
		solve_as::<V4Solver>().is_none_or(|s| !is_resolved(&s, id)),
		"nothing may resolve a withheld intent"
	);
	assert_eq!(reserved(who, asset_in), reserve);
	assert!(!resolved_ids().contains(&id));
	assert!(match_until(id).is_some());
}

/// ALICE's match-first 10 HDX → BNC, submitted at `t0`; returns `(id, until)`.
fn submit_alice_match_first(alice: &AccountId) -> (IntentId, u64) {
	submit(alice, match_first(HDX, BNC, AMOUNT, MIN_OUT_BNC, false));
	let id = intent_of(alice);
	let until = Timestamp::now() + WINDOW;
	assert_eq!(match_until(id), Some(until));
	assert_eq!(reserved(alice, HDX), AMOUNT);
	(id, until)
}

fn with_pair_endowed(execution: impl FnOnce(AccountId, AccountId)) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, 2_000 * HDX_UNIT);
	driver.endow_account(bob.clone(), BNC, 100 * BNC_UNIT);
	driver.endow_account(bob.clone(), HDX, 100 * HDX_UNIT);
	driver.execute(|| {
		enable_slip_fees();
		execution(alice, bob);
	});
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn match_first_intent_should_wait_for_a_counterparty_although_the_amm_could_fill_it() {
	with_pair_endowed(|alice, bob| {
		// Control, rolled back: the identical plain swap fills through the AMM right now.
		let control_out = with_transaction(|| {
			submit(&alice, plain(HDX, BNC, AMOUNT, MIN_OUT_BNC));
			let sol = solve_as::<V4Solver>().expect("plain swap fills via the AMM");
			assert_eq!(sol.resolved_intents.len(), 1);
			assert_eq!(amm_trade_count(&sol), 1);
			assert_eq!(amm_in_for(&sol, HDX, BNC), AMOUNT);
			let out = swap(&sol.resolved_intents[0]).amount_out;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				sol
			));
			TransactionOutcome::Rollback(Ok::<_, DispatchError>(out))
		})
		.expect("control runs");
		assert_eq!(control_out, CONTROL_OUT);
		assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);

		let (id, until) = submit_alice_match_first(&alice);

		for _ in 0..9 {
			step();
			assert!(Timestamp::now() < until);
			assert_withheld(id, &alice, HDX, AMOUNT);
		}

		// Same block: BOB arrives, the window is still open, the pair is matched.
		submit(&bob, plain(BNC, HDX, AMOUNT, MIN_OUT_HDX));
		let bob_id = intent_of(&bob);
		assert!(Timestamp::now() < match_until(id).expect("window set"));
		assert!(in_list(id));
		assert_eq!(view(id), stored_swap(id));

		let alice_before = balances(&alice);
		let fee_before = balances(&fee_receiver());
		let pot_before = balances(&holding_pot());

		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_headline");

		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(is_resolved(&sol, id));
		assert!(is_resolved(&sol, bob_id));
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0, "ALICE's HDX never touches a pool");
		assert_eq!(amm_trade_count(&sol), 1);
		let alice_out = swap(resolved(&sol, id)).amount_out;
		let bob_out = swap(resolved(&sol, bob_id)).amount_out;
		let alice_after = balances(&alice);
		let fee_after = balances(&fee_receiver());
		let pot_after = balances(&holding_pot());
		println!(
			"PIN headline: alice_out={alice_out} bob_amm_in={} bob_out={bob_out} fee=({},{}) pot=({},{})",
			amm_in_for(&sol, BNC, HDX),
			fee_after.0 - fee_before.0,
			fee_after.1 - fee_before.1,
			pot_after.0 - pot_before.0,
			pot_after.1 - pot_before.1,
		);
		assert_eq!(
			amm_in_for(&sol, BNC, HDX),
			PIN_BOB_AMM_IN,
			"the only AMM trade is BOB's excess BNC"
		);
		assert_eq!(alice_out, PIN_ALICE_MATCHED_OUT);
		assert!(alice_out > control_out, "matched at spot beats the AMM fill");
		assert_eq!(bob_out, PIN_BOB_OUT);
		assert_eq!(alice_after.1 - alice_before.1, PIN_ALICE_MATCHED_OUT);
		assert_eq!(alice_before.0 - alice_after.0, AMOUNT);
		assert_eq!(reserved(&alice, HDX), 0);
		assert_eq!(match_until(id), None);
		assert_eq!(resolved_ids(), vec![id, bob_id]);
		assert_eq!(fee_after.0 - fee_before.0, PIN_FEE_HDX);
		assert_eq!(fee_after.1 - fee_before.1, PIN_FEE_BNC);
		assert_eq!(pot_after.0 - pot_before.0, PIN_POT_HDX);
		assert_eq!(pot_after.1 - pot_before.1, PIN_POT_BNC);
	});
}

#[test]
fn match_first_intent_should_fill_through_the_amm_once_the_window_closes() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, 2_000 * HDX_UNIT)
		.submit_match_first_swap_intent(alice.clone(), HDX, BNC, AMOUNT, MIN_OUT_BNC, false, WINDOW_BLOCKS, None)
		.execute(|| {
			enable_slip_fees();
			let id = intent_of(&alice);
			let until = Timestamp::now() + WINDOW;
			assert_eq!(match_until(id), Some(until));
			assert_eq!(reserved(&alice, HDX), AMOUNT);

			for _ in 0..9 {
				step();
				assert!(Timestamp::now() < until);
				assert_withheld(id, &alice, HDX, AMOUNT);
			}

			step();
			assert_eq!(Timestamp::now(), until);
			assert!(in_list(id));
			assert_eq!(view(id), stored_swap(id));

			let alice_before = balances(&alice);
			let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_window_closed");

			assert_eq!(sol.resolved_intents.len(), 1);
			assert!(is_resolved(&sol, id));
			assert_eq!(amm_trade_count(&sol), 1);
			assert_eq!(amm_in_for(&sol, HDX, BNC), AMOUNT);
			let out = swap(resolved(&sol, id)).amount_out;
			println!("PIN window_closed: out={out}");
			assert_eq!(out, PIN_OUT_AFTER_WINDOW);
			assert_eq!(balances(&alice).1 - alice_before.1, out);
			assert_eq!(reserved(&alice, HDX), 0);
			assert_eq!(match_until(id), None);
			assert_eq!(resolved_ids(), vec![id]);
		});
}

#[test]
fn match_first_partial_intent_should_fill_only_what_the_counterparty_covers() {
	with_pair_endowed(|alice, bob| {
		submit(&alice, match_first(HDX, BNC, PARTIAL_AMOUNT, PARTIAL_MIN_OUT, true));
		let id = intent_of(&alice);
		let until = Timestamp::now() + WINDOW;
		assert_eq!(match_until(id), Some(until));
		assert!(!in_list(id));

		submit(&bob, plain(BNC, HDX, COUNTER_AMOUNT, MIN_OUT_HDX));
		let bob_id = intent_of(&bob);

		let offered = view(id);
		let ed_hdx = AssetRegistry::existential_deposit(HDX).expect("HDX ED");
		println!("PIN partial: cap={} ed_hdx={ed_hdx}", offered.remaining());
		assert!(offered.remaining() >= ed_hdx);
		assert_eq!(offered.remaining(), PIN_CAP);
		assert_eq!(offered.partial, Partial::Yes(PARTIAL_AMOUNT - PIN_CAP));
		assert_eq!(offered.amount_in, PARTIAL_AMOUNT);
		assert_eq!(offered.amount_out, PARTIAL_MIN_OUT);
		assert_eq!(stored_swap(id).partial, Partial::Yes(0));

		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_partial_1");
		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(is_resolved(&sol, bob_id));
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0, "ALICE's HDX never touches a pool");
		let fill_1 = partial_fill_of(id);
		println!("PIN partial: fill_1={fill_1} amm_trades={}", amm_trade_count(&sol));
		assert_eq!(fill_1, PIN_FILL_1);
		assert!(fill_1 <= PIN_CAP);
		assert_eq!(stored_swap(id).partial, Partial::Yes(fill_1));
		assert_eq!(reserved(&alice, HDX), PARTIAL_AMOUNT - fill_1);
		assert_eq!(match_until(id), Some(until));

		// A second counterparty fills more; the window is still open.
		submit(&bob, plain(BNC, HDX, COUNTER_AMOUNT, MIN_OUT_HDX));
		let bob_id_2 = intent_of(&bob);
		step();
		assert!(Timestamp::now() < until);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_partial_2");
		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(is_resolved(&sol, bob_id_2));
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0);
		let fill_2 = partial_fill_of(id);
		println!("PIN partial: fill_2={fill_2}");
		assert_eq!(fill_2, PIN_FILL_2);
		assert_eq!(stored_swap(id).partial, Partial::Yes(fill_1 + fill_2));
		assert_eq!(match_until(id), Some(until));

		// Window closes: the remainder goes through the AMM.
		while Timestamp::now() < until {
			step();
		}
		assert!(in_list(id));
		assert_eq!(view(id), stored_swap(id));
		let rest = PARTIAL_AMOUNT - fill_1 - fill_2;
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_partial_rest");
		assert_eq!(sol.resolved_intents.len(), 1);
		assert!(is_resolved(&sol, id));
		assert_eq!(amm_in_for(&sol, HDX, BNC), rest);
		let rest_out = swap(resolved(&sol, id)).amount_out;
		println!("PIN partial: rest={rest} rest_out={rest_out}");
		assert_eq!(rest_out, PIN_REST_OUT);
		assert_eq!(resolved_ids(), vec![id]);
		assert_eq!(reserved(&alice, HDX), 0);
		assert_eq!(match_until(id), None);
	});
}

#[test]
fn match_first_intent_should_stay_withheld_when_the_only_counterparty_is_above_market() {
	with_pair_endowed(|alice, bob| {
		let (id, until) = submit_alice_match_first(&alice);
		// Like the resting HDX→HOLLAR orders on mainnet: a limit 3× the market.
		submit(&bob, plain(BNC, HDX, AMOUNT, 3 * 147_407_011_313_812));
		let bob_id = intent_of(&bob);

		for _ in 0..9 {
			step();
			assert!(Timestamp::now() < until);
			assert_withheld(id, &alice, HDX, AMOUNT);
			assert!(in_list(bob_id));
		}

		step();
		assert_eq!(Timestamp::now(), until);
		assert!(in_list(id));

		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_resting_order");
		assert_eq!(sol.resolved_intents.len(), 1);
		assert!(is_resolved(&sol, id));
		assert_eq!(amm_trade_count(&sol), 1);
		assert_eq!(amm_in_for(&sol, HDX, BNC), AMOUNT);
		let out = swap(resolved(&sol, id)).amount_out;
		println!("PIN resting_order: out={out}");
		assert_eq!(out, PIN_OUT_AFTER_WINDOW_WITH_RESTING_ORDER);
		assert_eq!(match_until(id), None);
		assert!(pallet_intent::Pallet::<Runtime>::get_intent(bob_id).is_some());
		assert_eq!(reserved(&bob, BNC), AMOUNT);
	});
}

#[test]
fn match_first_intents_should_match_each_other() {
	with_pair_endowed(|alice, bob| {
		let (alice_id, alice_until) = submit_alice_match_first(&alice);
		submit(&bob, match_first(BNC, HDX, AMOUNT, MIN_OUT_HDX, true));
		let bob_id = intent_of(&bob);
		let bob_until = Timestamp::now() + WINDOW;
		assert_eq!(match_until(bob_id), Some(bob_until));

		assert!(in_list(alice_id));
		assert!(in_list(bob_id));
		assert_eq!(view(alice_id), stored_swap(alice_id));
		let bob_view = view(bob_id);
		let ed_bnc = AssetRegistry::existential_deposit(BNC).expect("BNC ED");
		println!("PIN mutual: bob_cap={} ed_bnc={ed_bnc}", bob_view.remaining());
		assert!(bob_view.remaining() >= ed_bnc);
		assert_eq!(bob_view.remaining(), PIN_BOB_CAP);
		assert_eq!(bob_view.partial, Partial::Yes(AMOUNT - PIN_BOB_CAP));

		let alice_before = balances(&alice);
		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_mutual");

		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(is_resolved(&sol, alice_id));
		assert!(is_resolved(&sol, bob_id));
		assert_eq!(
			swap(resolved(&sol, alice_id)).amount_in,
			AMOUNT,
			"ALICE is fully matched"
		);
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0, "ALICE's HDX never touches a pool");
		let alice_out = swap(resolved(&sol, alice_id)).amount_out;
		let bob_fill = partial_fill_of(bob_id);
		println!(
			"PIN mutual: alice_out={alice_out} bob_fill={bob_fill} amm_trades={}",
			amm_trade_count(&sol)
		);
		assert_eq!(alice_out, PIN_MUTUAL_ALICE_OUT);
		assert_eq!(balances(&alice).1 - alice_before.1, alice_out);
		assert_eq!(bob_fill, PIN_MUTUAL_BOB_FILL);
		assert!(bob_fill <= PIN_BOB_CAP);
		assert_eq!(amm_trade_count(&sol), PIN_MUTUAL_AMM_TRADES);
		assert_eq!(stored_swap(bob_id).partial, Partial::Yes(bob_fill));
		assert_eq!(match_until(alice_id), None);
		assert_eq!(match_until(bob_id), Some(bob_until));
		assert!(Timestamp::now() < alice_until);
	});
}

/// Fail-closed: an asset on no venue has no reference price, so the pair cannot be matched
/// inside the window and the intent is simply withheld until the window closes.
#[test]
fn match_first_intents_should_take_the_counter_flow_in_submission_order() {
	with_pair_endowed(|alice, bob| {
		let charlie: AccountId = CHARLIE.into();
		assert_ok!(Currencies::deposit(HDX, &charlie, 2_000 * HDX_UNIT));
		let (alice_id, _) = submit_alice_match_first(&alice);
		submit(&charlie, match_first(HDX, BNC, AMOUNT, MIN_OUT_BNC, false));
		let charlie_id = intent_of(&charlie);
		// ~14.8 HDX of counter-flow: covers one waiting intent, not both.
		submit(&bob, plain(BNC, HDX, BNC_UNIT, MIN_OUT_HDX));
		let bob_id = intent_of(&bob);

		assert!(alice_id < charlie_id);
		assert!(in_list(alice_id));
		assert!(!in_list(charlie_id));

		let sol = run_and_submit_as::<V4Solver>(SolverMode::V4, "match_first_queue");

		assert_eq!(resolved_ids(), vec![alice_id, bob_id]);
		assert_eq!(amm_in_for(&sol, HDX, BNC), 0, "no waiting HDX touches a pool");
		assert_eq!(reserved(&charlie, HDX), AMOUNT);
		assert_eq!(stored_swap(charlie_id).partial, Partial::No);
		assert!(match_until(charlie_id).is_some());
	});
}

#[test]
fn match_first_intent_should_not_be_offered_when_the_pair_has_no_oracle_route() {
	with_pair_endowed(|alice, bob| {
		let fresh = with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
				None,
				Some(b"ICE MF".to_vec().try_into().expect("name fits")),
				AssetKind::Token,
				1,
				Some(b"IMF".to_vec().try_into().expect("symbol fits")),
				Some(12),
				None,
				None,
			))
		})
		.expect("asset registers");
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			alice.clone(),
			fresh,
			(100 * HDX_UNIT) as i128,
		));

		submit(&alice, match_first(fresh, HDX, 10 * HDX_UNIT, HDX_UNIT, false));
		let id = intent_of(&alice);
		let until = Timestamp::now() + WINDOW;
		submit(&bob, plain(HDX, fresh, 10 * HDX_UNIT, 1));
		let bob_id = intent_of(&bob);

		for _ in 0..9 {
			step();
			assert!(Timestamp::now() < until);
			assert_withheld(id, &alice, fresh, 10 * HDX_UNIT);
			assert!(in_list(bob_id));
		}

		step();
		assert_eq!(Timestamp::now(), until);
		assert!(in_list(id));
		assert_eq!(view(id), stored_swap(id));
	});
}
