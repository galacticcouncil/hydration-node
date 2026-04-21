use crate::polkadot_test_net::{hydradx_run_to_next_block, last_hydra_events, TestNet, ALICE, BOB};
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, Runtime, RuntimeEvent, RuntimeOrigin};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use ice_solver::v2::Solver as IceSolver;
use ice_support::Solution;
use orml_traits::{MultiCurrency, MultiReservableCurrency};
use pallet_omnipool::types::SlipFeeConfig;
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

// Asset IDs proven to work in existing solver tests
const HDX: u32 = 0;
const BNC: u32 = 14;

// Amounts from solver_execute_solution1 — known to work
const TRADE_AMOUNT: u128 = 10_000_000_000_000;
const MIN_OUT_BNC: u128 = 68_795_189_840;

const PERIOD: u32 = 5;

// 10% slippage — realistic user setting for recurring DCA trades.
// Oracle limit = estimated_out * 0.90, giving the solver enough room across periods
// as the oracle adjusts between blocks.
const DCA_SLIPPAGE: Permill = Permill::from_percent(10);

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;
type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;

fn enable_slip_fees() {
	assert_ok!(hydradx_runtime::Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

/// Prints `assert_eq!(...)` lines that pin down every field of a Solution.
#[allow(dead_code)]
fn dump_solution(label: &str, solution: &ice_support::Solution) {
	println!("// === DUMP_SOLUTION BEGIN: {} ===", label);
	println!(
		"assert_eq!(solution.resolved_intents.len(), {}, \"resolved count\");",
		solution.resolved_intents.len()
	);
	println!("assert_eq!(solution.score, {}, \"score\");", solution.score);
	println!(
		"assert_eq!(solution.trades.len(), {}, \"trades count\");",
		solution.trades.len()
	);
	for (i, ri) in solution.resolved_intents.iter().enumerate() {
		match &ri.data {
			ice_support::IntentData::Swap(sw) => {
				let partial_str = match sw.partial {
					ice_support::Partial::No => "ice_support::Partial::No".to_string(),
					ice_support::Partial::Yes(b) => format!("ice_support::Partial::Yes({}u128)", b),
				};
				println!(
					"{{ let r = &solution.resolved_intents[{i}]; assert_eq!(r.id, {id}); \
					 let ice_support::IntentData::Swap(ref s) = r.data else {{ panic!(\"expected Swap\"); }}; \
					 assert_eq!(s.asset_in, {ain}); assert_eq!(s.asset_out, {aout}); \
					 assert_eq!(s.amount_in, {amin}u128); assert_eq!(s.amount_out, {amout}u128); \
					 assert_eq!(s.partial, {pstr}); }}",
					i = i,
					id = ri.id,
					ain = sw.asset_in,
					aout = sw.asset_out,
					amin = sw.amount_in,
					amout = sw.amount_out,
					pstr = partial_str,
				);
			}
			ice_support::IntentData::Dca(d) => {
				println!(
					"{{ let r = &solution.resolved_intents[{i}]; assert_eq!(r.id, {id}); \
					 let ice_support::IntentData::Dca(ref d) = r.data else {{ panic!(\"expected Dca\"); }}; \
					 assert_eq!(d.asset_in, {ain}); assert_eq!(d.asset_out, {aout}); \
					 assert_eq!(d.amount_in, {amin}u128); assert_eq!(d.amount_out, {amout}u128); \
					 assert_eq!(d.remaining_budget, {rb}u128); }}",
					i = i,
					id = ri.id,
					ain = d.asset_in,
					aout = d.asset_out,
					amin = d.amount_in,
					amout = d.amount_out,
					rb = d.remaining_budget,
				);
			}
		}
	}
	println!("// === DUMP_SOLUTION END: {} ===", label);
}

/// Prints `assert_eq!(<var>, <value>u128);` for each named variable.
#[allow(unused_macros)]
macro_rules! dump_exact {
	($($var:ident),+ $(,)?) => {
		$(
			println!("assert_eq!({}, {}u128);", stringify!($var), $var);
		)+
	};
}

fn run_solver_and_submit() -> Solution {
	let block = hydradx_runtime::System::block_number();
	let call = pallet_ice::Pallet::<Runtime>::run(
		block,
		|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
	)
	.expect("Solver should produce a solution");

	let pallet_ice::Call::submit_solution { solution, .. } = call else {
		panic!("Expected submit_solution call");
	};
	let solution_clone = solution.clone();

	hydradx_run_to_next_block();
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		solution,
	));

	solution_clone
}

fn advance_and_solve(n: u32) -> Solution {
	for _ in 0..n {
		hydradx_run_to_next_block();
	}
	run_solver_and_submit()
}

fn submit_dca_hdx_bnc(who: AccountId, budget: Option<u128>) {
	submit_dca_hdx_bnc_with_slippage(who, budget, DCA_SLIPPAGE);
}

fn submit_dca_hdx_bnc_with_slippage(who: AccountId, budget: Option<u128>, slippage: Permill) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who),
		pallet_intent::types::IntentInput {
			data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
				asset_in: HDX,
				asset_out: BNC,
				amount_in: TRADE_AMOUNT,
				amount_out: MIN_OUT_BNC,
				slippage,
				budget,
				period: PERIOD,
			}),
			deadline: None,
			on_resolved: None,
		}
	));
}

// === A. Basic Lifecycle ===

#[test]
fn dca_single_trade_execution() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			// 3% slippage — realistic user setting
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			let hdx_before = Currencies::total_balance(HDX, &alice);
			let bnc_before = Currencies::total_balance(BNC, &alice);

			assert_eq!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
				0,
				"Not yet eligible"
			);

			let _s = advance_and_solve(PERIOD);
			{
				let solution = &_s;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}

			assert!(Currencies::total_balance(HDX, &alice) < hdx_before, "HDX decreased");
			assert!(Currencies::total_balance(BNC, &alice) > bnc_before, "BNC increased");

			let remaining: Vec<_> = pallet_intent::Intents::<Runtime>::iter().collect();
			assert_eq!(remaining.len(), 1, "DCA still active");
			match &remaining[0].1.data {
				ice_support::IntentData::Dca(dca) => {
					assert_eq!(dca.remaining_budget, budget - TRADE_AMOUNT);
				}
				_ => panic!("Expected DCA"),
			}

			// Account index still tracks the active DCA
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
		});
}

#[test]
fn dca_multi_period_completes() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(budget));

			let _s1 = advance_and_solve(PERIOD);
			{
				let solution = &_s1;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");

			let _s2 = advance_and_solve(PERIOD);
			{
				let solution = &_s2;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597724290, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392914130u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");

			let _s3 = advance_and_solve(PERIOD);
			{
				let solution = &_s3;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597490182, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392680022u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");

			// Account index cleaned up after DCA completion
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

// Period eligibility is tested in unit tests (dca_intent::should_not_include_dca_before_period_elapsed).
// The snapshot-based integration tests use RelayChainBlockNumberProvider which behaves differently
// from the mock, making period timing assertions unreliable here.

// === B. Rolling Budget ===

#[test]
fn dca_rolling_budget_continues() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), None); // rolling

			for i in 1..=3 {
				let _s = advance_and_solve(PERIOD);
				{
					let solution = &_s;
					let (expected_score, expected_amount_out): (u128, u128) = match i {
						1 => (605597958156, 674393147996),
						2 => (605597724290, 674392914130),
						3 => (605597490182, 674392680022),
						_ => unreachable!(),
					};
					assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
					assert_eq!(solution.score, expected_score, "score iter {}", i);
					assert_eq!(solution.trades.len(), 1, "trades count");
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, expected_amount_out, "amount_out iter {}", i);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				assert_eq!(
					pallet_intent::Intents::<Runtime>::iter().count(),
					1,
					"Rolling after trade {i}"
				);
			}
		});
}

// === C. Direct Matching ===

#[test]
fn dca_matched_with_opposing_swap() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.endow_account(bob.clone(), BNC, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(5 * TRADE_AMOUNT));

			for _ in 0..PERIOD {
				hydradx_run_to_next_block();
			}

			// Bob opposing swap: BNC → HDX
			let ts = hydradx_runtime::Timestamp::now();
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(bob.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: BNC,
						asset_out: HDX,
						amount_in: TRADE_AMOUNT,
						amount_out: 1_000_000_000_000u128,
						partial: false,
					}),
					deadline: Some(MILLISECS_PER_BLOCK * 100u64 + ts),
					on_resolved: None,
				}
			));

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
			let solution = run_solver_and_submit();
			{
				let solution = &solution;
				assert_eq!(solution.resolved_intents.len(), 2, "resolved count");
				assert_eq!(solution.score, 147016637925436, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960001);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 14);
					assert_eq!(s.asset_out, 0);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 147409011313812u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				{
					let r = &solution.resolved_intents[1];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 676421801464u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(solution.resolved_intents.len(), 2);
			assert!(solution.score > 0, "Surplus from direct matching");
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "DCA stays");

			// Alice's DCA still tracked, Bob's swap resolved and cleaned up
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&bob), 0);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&bob).count(), 0);
		});
}

// === D. Cancellation ===

#[test]
fn dca_cancel_mid_execution() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(5 * TRADE_AMOUNT));

			let _s1 = advance_and_solve(PERIOD);
			{
				let solution = &_s1;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);

			let (id, _) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
			assert_ok!(hydradx_runtime::Intent::remove_intent(
				RuntimeOrigin::signed(alice.clone()),
				id
			));
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

			// Account index cleaned up after cancellation
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

// === E. Multiple Users ===

#[test]
fn dca_multiple_users() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.endow_account(bob.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(3 * TRADE_AMOUNT));
			submit_dca_hdx_bnc(bob.clone(), Some(3 * TRADE_AMOUNT));

			let solution = advance_and_solve(PERIOD);
			{
				let solution = &solution;
				assert_eq!(solution.resolved_intents.len(), 2, "resolved count");
				assert_eq!(solution.score, 1211060569414, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960001);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674325474547u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				{
					let r = &solution.resolved_intents[1];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674325474547u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(solution.resolved_intents.len(), 2);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 2);

			// Each user has exactly 1 intent tracked
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&bob), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&bob).count(), 1);
		});
}

// === F. Slippage Levels ===

#[test]
fn dca_with_3_percent_slippage() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			// Execute all 3 trades with tight slippage
			let _s1 = advance_and_solve(PERIOD);
			{
				let solution = &_s1;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");

			let _s2 = advance_and_solve(PERIOD);
			{
				let solution = &_s2;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597724290, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392914130u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");

			let _s3 = advance_and_solve(PERIOD);
			{
				let solution = &_s3;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597490182, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392680022u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");

			// Account index cleaned up
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

#[test]
fn dca_with_1_percent_slippage() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();
			// Very tight 1% slippage — single trade
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(5 * TRADE_AMOUNT), Permill::from_percent(1));

			let _s = advance_and_solve(PERIOD);
			{
				let solution = &_s;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}

			// Should still work for a single trade on fresh snapshot state
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "DCA still active");
		});
}

#[test]
fn ice_dca_driving() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 3 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.enable_slip_fees(Permill::from_percent(5))
		.new_block()
		.submit_dca_intent(
			alice.clone(),
			HDX,
			BNC,
			TRADE_AMOUNT,
			MIN_OUT_BNC,
			Permill::from_percent(3),
			Some(budget),
			PERIOD,
		)
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 1");
		})
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "After trade 2");
		})
		.advance(PERIOD)
		.run_solver()
		.execute(|| {
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0, "Completed");
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

#[test]
fn dca_create_schedule_should_work() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			let hdx_free_before = Currencies::free_balance(HDX, &alice);

			submit_dca_hdx_bnc(alice.clone(), Some(budget));

			let stored: Vec<_> = pallet_intent::Intents::<Runtime>::iter().collect();
			assert_eq!(stored.len(), 1);
			match &stored[0].1.data {
				ice_support::IntentData::Dca(dca) => {
					assert_eq!(dca.asset_in, HDX);
					assert_eq!(dca.asset_out, BNC);
					assert_eq!(dca.amount_in, TRADE_AMOUNT);
					assert_eq!(dca.period, PERIOD);
					assert_eq!(dca.budget, Some(budget));
					assert_eq!(dca.remaining_budget, budget);
				}
				_ => panic!("Expected DCA intent data"),
			}

			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 1);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 1);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), budget);
			assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before - budget);

			let events = last_hydra_events(10);
			assert!(events.iter().any(|e| matches!(
				e,
				RuntimeEvent::Intent(pallet_intent::Event::IntentSubmitted { owner, .. })
					if owner == &alice
			)));

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 0);
		});
}

#[test]
fn dca_rolling_terminates_gracefully_on_funds_exhaustion() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	// Rolling DCA reserves 2*amount_in up front and tries to top up by 1*amount_in
	// after each trade. With 5x initial balance we expect termination within ~5 trades.
	let alice_initial_hdx = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, alice_initial_hdx)
		.execute(|| {
			enable_slip_fees();
			let hdx_free_at_start = Currencies::free_balance(HDX, &alice);

			submit_dca_hdx_bnc(alice.clone(), None);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), 2 * TRADE_AMOUNT);

			let bnc_before = Currencies::total_balance(BNC, &alice);
			let mut trades: usize = 0;
			let expected: [(u128, u128); 5] = [
				(605597958156, 674393147996),
				(605597724290, 674392914130),
				(605597490182, 674392680022),
				(605597256317, 674392446157),
				(605597022451, 674392212291),
			];
			for _ in 0..20 {
				let solution = advance_and_solve(PERIOD);
				if trades < expected.len() {
					let (expected_score, expected_amount_out) = expected[trades];
					assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
					assert_eq!(solution.score, expected_score, "score iter {}", trades);
					assert_eq!(solution.trades.len(), 1, "trades count");
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, expected_amount_out, "amount_out iter {}", trades);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				trades += 1;
				if pallet_intent::Intents::<Runtime>::iter().count() == 0 {
					break;
				}
			}

			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
			assert!(trades >= 1);
			assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
			assert!(Currencies::free_balance(HDX, &alice) <= hdx_free_at_start);
		});
}

#[test]
fn dca_terminate_freshly_created_returns_reserved_budget() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			let hdx_free_before = Currencies::free_balance(HDX, &alice);

			submit_dca_hdx_bnc(alice.clone(), Some(budget));
			assert_eq!(Currencies::reserved_balance(HDX, &alice), budget);
			assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before - budget);

			let (id, _) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
			assert_ok!(hydradx_runtime::Intent::remove_intent(
				RuntimeOrigin::signed(alice.clone()),
				id
			));

			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
			assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before);
		});
}

#[test]
fn dca_multiple_schedules_same_user_complete_independently() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let b1 = TRADE_AMOUNT;
	let b2 = 2 * TRADE_AMOUNT;
	let b3 = 3 * TRADE_AMOUNT;
	let total = b1 + b2 + b3;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, total * 10)
		.execute(|| {
			enable_slip_fees();

			submit_dca_hdx_bnc(alice.clone(), Some(b1));
			submit_dca_hdx_bnc(alice.clone(), Some(b2));
			submit_dca_hdx_bnc(alice.clone(), Some(b3));

			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 3);
			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 3);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 3);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), total);

			let bnc_before = Currencies::total_balance(BNC, &alice);

			let sol1 = advance_and_solve(PERIOD);
			{
				let solution = &sol1;
				assert_eq!(solution.resolved_intents.len(), 3, "resolved count");
				assert_eq!(solution.score, 1816590152442, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960002);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674325240654u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				{
					let r = &solution.resolved_intents[1];
					assert_eq!(r.id, 32752052247409382067756072960001);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674325240654u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				{
					let r = &solution.resolved_intents[2];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674325240654u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(sol1.resolved_intents.len(), 3);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 2);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), total - 3 * TRADE_AMOUNT);
			let bnc_after_1 = Currencies::total_balance(BNC, &alice);
			assert!(bnc_after_1 > bnc_before);

			let sol2 = advance_and_solve(PERIOD);
			{
				let solution = &sol2;
				assert_eq!(solution.resolved_intents.len(), 2, "resolved count");
				assert_eq!(solution.score, 1211059166118, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960002);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674324772899u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				{
					let r = &solution.resolved_intents[1];
					assert_eq!(r.id, 32752052247409382067756072960001);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674324772899u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(sol2.resolved_intents.len(), 2);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), total - 5 * TRADE_AMOUNT);
			let bnc_after_2 = Currencies::total_balance(BNC, &alice);
			assert!(bnc_after_2 > bnc_after_1);

			let sol3 = advance_and_solve(PERIOD);
			{
				let solution = &sol3;
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605596788586, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960002);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674391978426u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
			}
			assert_eq!(sol3.resolved_intents.len(), 1);
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
			assert!(Currencies::total_balance(BNC, &alice) > bnc_after_2);

			assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
			assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		});
}

#[test]
fn dca_emits_trade_executed_and_completed_events() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 2 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc(alice.clone(), Some(budget));

			let intent_id = pallet_intent::Intents::<Runtime>::iter()
				.next()
				.map(|(id, _)| id)
				.unwrap();

			{
				let solution = advance_and_solve(PERIOD);
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				solution
			};

			let events1 = last_hydra_events(20);
			let trade1 = events1.iter().find_map(|e| match e {
				RuntimeEvent::Intent(pallet_intent::Event::DcaTradeExecuted {
					id,
					amount_in,
					remaining_budget,
					..
				}) if *id == intent_id => Some((*amount_in, *remaining_budget)),
				_ => None,
			});
			let (amount_in, remaining_after_1) = trade1.expect("DcaTradeExecuted");
			assert_eq!(amount_in, TRADE_AMOUNT);
			assert_eq!(remaining_after_1, TRADE_AMOUNT);
			assert!(!events1.iter().any(|e| matches!(
				e,
				RuntimeEvent::Intent(pallet_intent::Event::DcaCompleted { id }) if *id == intent_id
			)));

			{
				let solution = advance_and_solve(PERIOD);
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597724290, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392914130u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				solution
			};

			let events2 = last_hydra_events(20);
			assert!(events2.iter().any(|e| matches!(
				e,
				RuntimeEvent::Intent(pallet_intent::Event::DcaCompleted { id }) if *id == intent_id
			)));
			assert!(!events2.iter().any(|e| matches!(
				e,
				RuntimeEvent::Intent(pallet_intent::Event::DcaTradeExecuted { id, .. }) if *id == intent_id
			)));
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
		});
}

#[test]
fn dca_through_stableswap_single_hop() {
	use amm_simulator::stableswap::Simulator as StableswapSimulator;
	use hydradx_runtime::{ice_simulator_provider, AssetRegistry, Router};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use hydradx_traits::BoundErc20;

	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		// Pick a stableswap pool whose first two assets are non-contract and both routable to HDX.
		let snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();
		let selected = snapshot.pools.iter().find_map(|(_, pool)| {
			if pool.assets.len() < 2 {
				return None;
			}
			let (a, b) = (pool.assets[0], pool.assets[1]);
			if AssetRegistry::contract_address(a).is_some() || AssetRegistry::contract_address(b).is_some() {
				return None;
			}
			if Router::get_onchain_route(AssetPair::new(a, HDX)).is_some()
				&& Router::get_onchain_route(AssetPair::new(b, HDX)).is_some()
			{
				Some((a, b, pool.reserves[0].decimals, pool.reserves[1].decimals))
			} else {
				None
			}
		});
		let (asset_in, asset_out, decimals_in, decimals_out) =
			selected.expect("no suitable stableswap pool in snapshot");

		let per_trade_in = 100 * 10u128.pow(decimals_in as u32);
		let per_trade_out_min = 10u128.pow(decimals_out as u32);
		let budget = 2 * per_trade_in;

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			asset_in,
			(budget * 10) as i128,
		));

		let in_before = Currencies::total_balance(asset_in, &alice);
		let out_before = Currencies::total_balance(asset_out, &alice);

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in,
					asset_out,
					amount_in: per_trade_in,
					amount_out: per_trade_out_min,
					slippage: DCA_SLIPPAGE,
					budget: Some(budget),
					period: PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));
		assert_eq!(Currencies::reserved_balance(asset_in, &alice), budget);

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 99045134304444271642, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 10);
				assert_eq!(s.asset_out, 18);
				assert_eq!(s.amount_in, 100000000u128);
				assert_eq!(s.amount_out, 100045134304444271642u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 98383307180234467371, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 10);
				assert_eq!(s.asset_out, 18);
				assert_eq!(s.amount_in, 100000000u128);
				assert_eq!(s.amount_out, 99383307180234467371u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

		assert!(Currencies::total_balance(asset_in, &alice) < in_before);
		assert!(Currencies::total_balance(asset_out, &alice) > out_before);
		assert_eq!(Currencies::reserved_balance(asset_in, &alice), 0);
	});
}

#[test]
fn dca_through_omnipool_and_stableswap_multi_hop() {
	use amm_simulator::stableswap::Simulator as StableswapSimulator;
	use hydradx_runtime::{ice_simulator_provider, AssetRegistry, Router};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use hydradx_traits::BoundErc20;

	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		// HDX lives in Omnipool, stable leg in Stableswap — any route the solver picks is multi-pool.
		let snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();
		let stable_asset_id = snapshot
			.pools
			.iter()
			.find_map(|(_, pool)| {
				pool.assets
					.iter()
					.find(|&&a| {
						AssetRegistry::contract_address(a).is_none()
							&& Router::get_onchain_route(AssetPair::new(HDX, a)).is_some()
							&& Router::get_onchain_route(AssetPair::new(a, HDX)).is_some()
					})
					.copied()
			})
			.expect("no stableswap asset with HDX route in snapshot");

		let per_trade_in = TRADE_AMOUNT;
		let per_trade_out_min =
			<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(
				stable_asset_id,
			)
			.expect("stable asset has ED");
		let budget = 2 * per_trade_in;

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			HDX,
			(budget * 10) as i128,
		));

		let hdx_before = Currencies::total_balance(HDX, &alice);
		let stable_before = Currencies::total_balance(stable_asset_id, &alice);

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in: HDX,
					asset_out: stable_asset_id,
					amount_in: per_trade_in,
					amount_out: per_trade_out_min,
					slippage: DCA_SLIPPAGE,
					budget: Some(budget),
					period: PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 10324, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 0);
				assert_eq!(s.asset_out, 10);
				assert_eq!(s.amount_in, 10000000000000u128);
				assert_eq!(s.amount_out, 20324u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);
		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 10324, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 0);
				assert_eq!(s.asset_out, 10);
				assert_eq!(s.amount_in, 10000000000000u128);
				assert_eq!(s.amount_out, 20324u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

		assert!(Currencies::total_balance(HDX, &alice) < hdx_before);
		assert!(Currencies::total_balance(stable_asset_id, &alice) > stable_before);
		assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
	});
}

#[test]
fn dca_through_aave_pair() {
	use amm_simulator::aave::Simulator as AaveSimulator;
	use hydradx_runtime::ice_simulator_provider;
	use hydradx_traits::amm::AmmSimulator;

	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let aave_snapshot = AaveSimulator::<ice_simulator_provider::Aave<Runtime>>::snapshot();
		let picked = aave_snapshot.pairs.iter().find_map(|(a, b)| {
			let ed_in =
				<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(*a)?;
			let ed_out =
				<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(*b)?;
			Some((*a, *b, ed_in, ed_out))
		});

		let Some((asset_in, asset_out, ed_in, ed_out)) = picked else {
			// Snapshot has no Aave pairs — nothing to exercise, not a failure.
			return;
		};

		let per_trade_in = ed_in.saturating_mul(100);
		let per_trade_out_min = ed_out;
		let budget = 2 * per_trade_in;

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			asset_in,
			(budget * 10) as i128,
		));

		let in_before = Currencies::total_balance(asset_in, &alice);
		let out_before = Currencies::total_balance(asset_out, &alice);

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in,
					asset_out,
					amount_in: per_trade_in,
					amount_out: per_trade_out_min,
					slippage: DCA_SLIPPAGE,
					budget: Some(budget),
					period: PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 977591, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 22);
				assert_eq!(s.asset_out, 1003);
				assert_eq!(s.amount_in, 1000000u128);
				assert_eq!(s.amount_out, 1000000u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);
		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 977591, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 22);
				assert_eq!(s.asset_out, 1003);
				assert_eq!(s.amount_in, 1000000u128);
				assert_eq!(s.amount_out, 1000000u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

		assert!(Currencies::total_balance(asset_in, &alice) < in_before);
		assert!(Currencies::total_balance(asset_out, &alice) > out_before);
		assert_eq!(Currencies::reserved_balance(asset_in, &alice), 0);
	});
}

#[test]
fn dca_stays_alive_when_trade_fails_until_lockdown_is_lifted() {
	use amm_simulator::stableswap::Simulator as StableswapSimulator;
	use hydradx_runtime::{ice_simulator_provider, AssetRegistry, Router};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use hydradx_traits::BoundErc20;

	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();
		let selected = snapshot.pools.iter().find_map(|(pid, pool)| {
			if pool.assets.is_empty() {
				return None;
			}
			let a = pool.assets[0];
			if AssetRegistry::contract_address(a).is_some() {
				return None;
			}
			Router::get_onchain_route(AssetPair::new(a, HDX)).map(|_| (*pid, a, pool.reserves[0].decimals))
		});
		let (pool_id, stable_asset_1, decimals_a) = selected.expect("no suitable stableswap pool");

		let per_trade_in = 10u128 * 10u128.pow(decimals_a as u32);
		let budget = 2 * per_trade_in;
		let ed_pool =
			<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(pool_id)
				.expect("pool_id has ED");

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			stable_asset_1,
			(budget * 10) as i128,
		));

		// Trigger circuit-breaker lockdown on pool_id by pinning and exceeding its deposit limit.
		crate::deposit_limiter::update_deposit_limit(pool_id, ed_pool).unwrap();
		assert_ok!(Currencies::deposit(pool_id, &bob, ed_pool * 2));

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in: stable_asset_1,
					asset_out: pool_id,
					amount_in: per_trade_in,
					amount_out: ed_pool,
					slippage: DCA_SLIPPAGE,
					budget: Some(budget),
					period: PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));

		let (intent_id, snap_before) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
		let remaining_before = match &snap_before.data {
			ice_support::IntentData::Dca(dca) => dca.remaining_budget,
			_ => panic!("expected Dca intent"),
		};

		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}

		// Solver operates off-chain (no circuit breaker there) so it produces a solution;
		// on-chain dispatch rejects it because of the lockdown. Intent must stay untouched.
		let call = pallet_ice::Pallet::<Runtime>::run(hydradx_runtime::System::block_number(), |intents, state| {
			Solver::solve(intents, state).ok()
		});
		if let Some(pallet_ice::Call::submit_solution { solution, .. }) = call {
			hydradx_run_to_next_block();
			let res = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
			assert!(res.is_err(), "submit must fail during lockdown; got {:?}", res);
		}

		let intent_after_failed = pallet_intent::Intents::<Runtime>::get(intent_id).unwrap();
		match intent_after_failed.data {
			ice_support::IntentData::Dca(dca) => {
				assert_eq!(dca.remaining_budget, remaining_before);
			}
			_ => panic!("expected Dca intent"),
		}

		assert_ok!(hydradx_runtime::CircuitBreaker::force_lift_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
		));
		crate::deposit_limiter::update_deposit_limit(pool_id, u128::MAX / 2).unwrap();

		let alice_pool_before = Currencies::total_balance(pool_id, &alice);
		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 9555398534085278591, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 10);
				assert_eq!(s.asset_out, 100);
				assert_eq!(s.amount_in, 10000000u128);
				assert_eq!(s.amount_out, 9555398534085279591u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};

		assert!(Currencies::total_balance(pool_id, &alice) > alice_pool_before);
		if let Some(intent_after_success) = pallet_intent::Intents::<Runtime>::get(intent_id) {
			match intent_after_success.data {
				ice_support::IntentData::Dca(dca) => {
					assert!(dca.remaining_budget < remaining_before);
				}
				_ => panic!("expected Dca intent"),
			}
		}
	});
}

#[test]
fn dca_works_when_free_balance_is_exactly_ed_after_reserve() {
	use hydradx_runtime::Balances;

	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 2 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let hdx_ed =
			<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(HDX)
				.expect("HDX has ED");

		// force_set_balance (not endow) because we need the exact value `budget + ED`.
		assert_ok!(Balances::force_set_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			budget + hdx_ed,
		));
		assert_eq!(Currencies::free_balance(HDX, &alice), budget + hdx_ed);

		let bnc_before = Currencies::total_balance(BNC, &alice);

		submit_dca_hdx_bnc(alice.clone(), Some(budget));
		assert_eq!(Currencies::reserved_balance(HDX, &alice), budget);
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_ed);

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 605597958156, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 0);
				assert_eq!(s.asset_out, 14);
				assert_eq!(s.amount_in, 10000000000000u128);
				assert_eq!(s.amount_out, 674393147996u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_ed);
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);

		{
			let solution = advance_and_solve(PERIOD);
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 605597724290, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 0);
				assert_eq!(s.asset_out, 14);
				assert_eq!(s.amount_in, 10000000000000u128);
				assert_eq!(s.amount_out, 674392914130u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
		assert_eq!(pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(), 0);
		assert_eq!(pallet_intent::Pallet::<Runtime>::account_intent_count(&alice), 0);
		assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_ed);
		assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
	});
}

#[test]
fn dca_retries_every_block_until_success() {
	use amm_simulator::stableswap::Simulator as StableswapSimulator;
	use hydradx_runtime::{ice_simulator_provider, AssetRegistry, Router};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use hydradx_traits::BoundErc20;

	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();
		let selected = snapshot.pools.iter().find_map(|(pid, pool)| {
			if pool.assets.is_empty() {
				return None;
			}
			let a = pool.assets[0];
			if AssetRegistry::contract_address(a).is_some() {
				return None;
			}
			Router::get_onchain_route(AssetPair::new(a, HDX)).map(|_| (*pid, a, pool.reserves[0].decimals))
		});
		let (pool_id, stable_asset, decimals) = selected.expect("no suitable stableswap pool");

		let per_trade_in = 10u128 * 10u128.pow(decimals as u32);
		let budget = 2 * per_trade_in;
		let ed_pool =
			<pallet_asset_registry::Pallet<Runtime> as hydradx_traits::registry::Inspect>::existential_deposit(pool_id)
				.expect("pool_id has ED");

		assert_ok!(Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			alice.clone(),
			stable_asset,
			(budget * 10) as i128,
		));

		crate::deposit_limiter::update_deposit_limit(pool_id, ed_pool).unwrap();
		assert_ok!(Currencies::deposit(pool_id, &bob, ed_pool * 2));

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in: stable_asset,
					asset_out: pool_id,
					amount_in: per_trade_in,
					amount_out: ed_pool,
					slippage: DCA_SLIPPAGE,
					budget: Some(budget),
					period: PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));

		let (intent_id, _) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
		let leb_after_submit = match pallet_intent::Intents::<Runtime>::get(intent_id).unwrap().data {
			ice_support::IntentData::Dca(ref dca) => dca.last_execution_block,
			_ => panic!("expected DCA"),
		};

		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}

		let call = pallet_ice::Pallet::<Runtime>::run(hydradx_runtime::System::block_number(), |intents, state| {
			Solver::solve(intents, state).ok()
		});
		if let Some(pallet_ice::Call::submit_solution { solution, .. }) = call {
			hydradx_run_to_next_block();
			let res = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
			assert!(res.is_err());
		}

		// Failure must not advance last_execution_block or consume budget.
		let dca_after_fail = match pallet_intent::Intents::<Runtime>::get(intent_id).unwrap().data {
			ice_support::IntentData::Dca(ref dca) => dca.clone(),
			_ => panic!("expected DCA"),
		};
		assert_eq!(dca_after_fail.last_execution_block, leb_after_submit);
		assert_eq!(dca_after_fail.remaining_budget, budget);

		// Intent must be eligible on the very next block (not after another full period).
		hydradx_run_to_next_block();
		let valid = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		assert!(valid.iter().any(|(id, _)| *id == intent_id));

		assert_ok!(hydradx_runtime::CircuitBreaker::force_lift_lockdown(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
		));
		crate::deposit_limiter::update_deposit_limit(pool_id, u128::MAX / 2).unwrap();

		{
			let solution = run_solver_and_submit();
			assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
			assert_eq!(solution.score, 9555398534085278591, "score");
			assert_eq!(solution.trades.len(), 1, "trades count");
			{
				let r = &solution.resolved_intents[0];
				assert_eq!(r.id, 32752052247409382067756072960000);
				let ice_support::IntentData::Swap(ref s) = r.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.asset_in, 10);
				assert_eq!(s.asset_out, 100);
				assert_eq!(s.amount_in, 10000000u128);
				assert_eq!(s.amount_out, 9555398534085279591u128);
				assert_eq!(s.partial, ice_support::Partial::No);
			}
			solution
		};

		if let Some(intent) = pallet_intent::Intents::<Runtime>::get(intent_id) {
			match intent.data {
				ice_support::IntentData::Dca(ref dca) => {
					assert!(dca.last_execution_block > leb_after_submit);
					assert_eq!(dca.remaining_budget, budget - per_trade_in);
				}
				_ => panic!("expected DCA"),
			}
		}

		assert!(Currencies::total_balance(pool_id, &alice) > 0);
	});
}

#[test]
fn dca_residual_budget_returned_without_partial_trade() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	// 2.5 × amount_in: after two full trades, 0.5 × amount_in remains and is
	// returned to free balance (new DCA does not execute a partial last trade).
	let budget = 2 * TRADE_AMOUNT + TRADE_AMOUNT / 2;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();

			let hdx_free_before = Currencies::free_balance(HDX, &alice);
			let bnc_before = Currencies::total_balance(BNC, &alice);

			submit_dca_hdx_bnc(alice.clone(), Some(budget));
			assert_eq!(Currencies::reserved_balance(HDX, &alice), budget);

			{
				let solution = advance_and_solve(PERIOD);
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597958156, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674393147996u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				solution
			};
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);

			{
				let solution = advance_and_solve(PERIOD);
				assert_eq!(solution.resolved_intents.len(), 1, "resolved count");
				assert_eq!(solution.score, 605597724290, "score");
				assert_eq!(solution.trades.len(), 1, "trades count");
				{
					let r = &solution.resolved_intents[0];
					assert_eq!(r.id, 32752052247409382067756072960000);
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					assert_eq!(s.asset_in, 0);
					assert_eq!(s.asset_out, 14);
					assert_eq!(s.amount_in, 10000000000000u128);
					assert_eq!(s.amount_out, 674392914130u128);
					assert_eq!(s.partial, ice_support::Partial::No);
				}
				solution
			};
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);

			let residual = TRADE_AMOUNT / 2;
			assert_eq!(
				Currencies::free_balance(HDX, &alice),
				hdx_free_before - budget + residual
			);
			assert_eq!(Currencies::reserved_balance(HDX, &alice), 0);
			assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
		});
}

// DCA period must be enforced at resolve time, not only in get_valid_intents.
// Fails today: a crafted intent list that skips the period filter gets its
// solution accepted via submit_solution. Fix: add period check to
// validate_dca_intent_resolve in pallets/intent/src/lib.rs.
#[test]
fn dca_period_can_be_bypassed_at_resolve_time() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(3));

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 0);

			hydradx_run_to_next_block();
			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 0);

			let (intent_id, dca_before) = pallet_intent::Intents::<Runtime>::iter()
				.next()
				.map(|(id, intent)| match intent.data {
					ice_support::IntentData::Dca(dca) => (id, dca),
					_ => panic!("expected DCA"),
				})
				.unwrap();

			// Simulates a block author bypassing get_valid_intents: transform DCA to
			// Swap directly and hand it to the solver without the period filter.
			let crafted: Vec<ice_support::Intent> = pallet_intent::Intents::<Runtime>::iter()
				.map(|(id, intent)| {
					let data = match intent.data {
						ice_support::IntentData::Dca(ref dca) => ice_support::IntentData::Swap(dca.to_swap_data()),
						other => other,
					};
					ice_support::Intent { id, data }
				})
				.collect();

			let state = <<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::initial_state();
			let solution = Solver::solve(crafted, state).expect("solver fills crafted intent");

			let hdx_before = Currencies::total_balance(HDX, &alice);
			let bnc_before = Currencies::total_balance(BNC, &alice);

			hydradx_run_to_next_block();
			let result = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);

			assert!(
				result.is_err(),
				"out-of-period trade must be rejected; got {:?}",
				result
			);
			assert_eq!(Currencies::total_balance(HDX, &alice), hdx_before);
			assert_eq!(Currencies::total_balance(BNC, &alice), bnc_before);

			let intent_after = pallet_intent::Intents::<Runtime>::get(intent_id).unwrap();
			match intent_after.data {
				ice_support::IntentData::Dca(dca) => {
					assert_eq!(dca.remaining_budget, dca_before.remaining_budget);
					assert_eq!(dca.last_execution_block, dca_before.last_execution_block);
				}
				_ => panic!("expected DCA"),
			}
		});
}

// Dynamic slippage must be enforced at resolve time, not only as a pre-filter
// in get_valid_intents. Acrafted solution with resolve.amount_out
// at the hard limit (below the oracle-derived slippage floor) is accepted.
// Fix: enforce resolve.amount_out >= compute_dca_effective_limit(dca) in
// validate_dca_intent_resolve.
#[test]
fn dca_slippage_not_enforced_at_resolve_time() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let budget = 5 * TRADE_AMOUNT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, budget * 10)
		.execute(|| {
			enable_slip_fees();
			// Tight 1% slippage but loose hard limit — the gap S2 exposes.
			submit_dca_hdx_bnc_with_slippage(alice.clone(), Some(budget), Permill::from_percent(1));

			let (intent_id, _) = pallet_intent::Intents::<Runtime>::iter().next().unwrap();
			let dca = match pallet_intent::Intents::<Runtime>::get(intent_id).unwrap().data {
				ice_support::IntentData::Dca(dca) => dca,
				_ => panic!("expected DCA"),
			};

			// Oracle floor should be well above the hard limit for HDX→BNC.
			let effective_limit = pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(&dca);
			assert!(
				effective_limit > dca.amount_out,
				"test requires oracle floor ({}) above hard limit ({})",
				effective_limit,
				dca.amount_out,
			);

			// Run the honest solver to get a valid solution shape (routes + trades).
			let honest_solution = advance_and_solve(PERIOD);
			let honest_out = honest_solution.resolved_intents[0].data.amount_out();
			assert!(honest_out >= effective_limit);

			// DCA still active after the first honest trade.
			let dca_after_1 = match pallet_intent::Intents::<Runtime>::get(intent_id).unwrap().data {
				ice_support::IntentData::Dca(dca) => dca,
				_ => panic!("expected DCA"),
			};

			// Advance another period and get a second honest solution for the shape.
			for _ in 0..PERIOD {
				hydradx_run_to_next_block();
			}

			let block = hydradx_runtime::System::block_number();
			let call = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents, state| Solver::solve(intents, state).ok(),
			)
			.expect("solver should produce a solution");
			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			// Craft a worse solution: set resolve.amount_out to the hard limit
			// (below oracle floor). A malicious collator keeps the surplus.
			let mut crafted = solution;
			crafted.resolved_intents[0] = ice_support::Intent {
				id: crafted.resolved_intents[0].id,
				data: ice_support::IntentData::Swap(ice_support::SwapData {
					asset_in: HDX,
					asset_out: BNC,
					amount_in: TRADE_AMOUNT,
					amount_out: dca.amount_out, // hard limit, below oracle floor
					partial: ice_support::Partial::No,
				}),
			};
			// surplus = resolve.amount_out - dca.amount_out = 0
			crafted.score = 0;

			let hdx_before = Currencies::total_balance(HDX, &alice);
			let bnc_before = Currencies::total_balance(BNC, &alice);

			hydradx_run_to_next_block();
			let result = pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				crafted,
			);

			// Today: accepts (only hard limit is checked).
			assert!(
				result.is_err(),
				"resolve at hard limit ({}) below oracle floor ({}) must be rejected; got {:?}",
				dca.amount_out,
				effective_limit,
				result,
			);
			assert_eq!(Currencies::total_balance(HDX, &alice), hdx_before);
			assert_eq!(Currencies::total_balance(BNC, &alice), bnc_before);

			let dca_after = match pallet_intent::Intents::<Runtime>::get(intent_id).unwrap().data {
				ice_support::IntentData::Dca(dca) => dca,
				_ => panic!("expected DCA"),
			};
			assert_eq!(dca_after.remaining_budget, dca_after_1.remaining_budget);
			assert_eq!(dca_after.last_execution_block, dca_after_1.last_execution_block);
		});
}
