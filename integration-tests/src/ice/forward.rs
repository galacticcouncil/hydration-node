use crate::polkadot_test_net::{hydradx_run_to_next_block, last_hydra_events, TestNet, ALICE};
use crate::utils::contracts::deploy_contract;
use amm_simulator::HydrationSimulator;
use frame_support::traits::Time;
use frame_support::{assert_ok, BoundedVec};
use hydradx_runtime::{Currencies, EVMAccounts, Intent, LazyExecutor, Runtime, RuntimeEvent, RuntimeOrigin, Timestamp};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::evm::InspectEvmAccounts;
use ice_solver::v4::Solver as IceSolver;
use ice_support::Solution;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::SlipFeeConfig;
use primitives::{AccountId, EvmAddress};
use sp_core::H160;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

const HDX: u32 = 0;
const BNC: u32 = 14;
const TRADE_AMOUNT: u128 = 10_000_000_000_000;
const MIN_OUT_BNC: u128 = 68_795_189_840;
const PERIOD: u32 = 5;

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

fn run_solver_and_submit() -> Solution {
	let block = hydradx_runtime::System::block_number();
	let call = pallet_ice::Pallet::<Runtime>::run(
		block,
		|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
			Solver::solve(intents, state, pallet_ice::ProtocolFee::<Runtime>::get()).ok()
		},
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

/// ABI-encodes a single `address` argument (right-aligned in a 32-byte word).
fn encode_address(addr: H160) -> Vec<u8> {
	let mut word = [0u8; 32];
	word[12..32].copy_from_slice(addr.as_bytes());
	word.to_vec()
}

fn advance_and_solve(n: u32) -> Solution {
	for _ in 0..n {
		hydradx_run_to_next_block();
	}
	run_solver_and_submit()
}

fn resolved_swap_amount_out(solution: &Solution) -> u128 {
	match &solution.resolved_intents[0].data {
		ice_support::IntentData::Swap(s) => s.amount_out,
		_ => panic!("expected resolved Swap"),
	}
}

/// The fee the most recent `Queued` event charged `who` (the forward execution pre-charge).
fn last_queued_fee(who: &AccountId) -> u128 {
	last_hydra_events(50)
		.into_iter()
		.rev()
		.find_map(|e| match e {
			RuntimeEvent::LazyExecutor(pallet_lazy_executor::Event::Queued { who: w, fees, .. }) if &w == who => {
				Some(fees)
			}
			_ => None,
		})
		.expect("a Queued event for the owner")
}

fn submit_dca_with_forward(who: AccountId, budget: u128, contract: EvmAddress, data: Vec<u8>) {
	assert_ok!(Intent::submit_intent(
		RuntimeOrigin::signed(who),
		pallet_intent::types::IntentInput {
			data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
				asset_in: HDX,
				asset_out: BNC,
				amount_in: TRADE_AMOUNT,
				amount_out: MIN_OUT_BNC,
				slippage: Permill::from_percent(10),
				budget: Some(budget),
				period: PERIOD,
			}),
			deadline: None,
			on_resolved: Some(pallet_intent::types::OnResolved::Forward {
				contract,
				data: BoundedVec::truncate_from(data),
			}),
		}
	));
}

fn submit_swap_with_forward(who: AccountId, contract: EvmAddress, data: Vec<u8>) {
	let ts = Timestamp::now();
	assert_ok!(Intent::submit_intent(
		RuntimeOrigin::signed(who),
		pallet_intent::types::IntentInput {
			data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
				asset_in: HDX,
				asset_out: BNC,
				amount_in: TRADE_AMOUNT,
				amount_out: MIN_OUT_BNC,
				partial: false,
			}),
			deadline: Some(primitives::constants::time::MILLISECS_PER_BLOCK * 100u64 + ts),
			on_resolved: Some(pallet_intent::types::OnResolved::Forward {
				contract,
				data: BoundedVec::truncate_from(data),
			}),
		}
	));
}

#[test]
fn forward_should_push_resolved_output_to_receiver_then_to_target() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();

			let receiver = deploy_contract("IntentResolutionReceiver", EVMAccounts::evm_address(&alice));

			// The receiver forwards everything it gets to this target (decoded from `data`).
			let target_evm = H160::repeat_byte(0xAA);
			let target_account = EVMAccounts::account_id(target_evm);
			let receiver_account = EVMAccounts::account_id(receiver);

			submit_swap_with_forward(alice.clone(), receiver, encode_address(target_evm));

			let alice_bnc_before = Currencies::total_balance(BNC, &alice);
			let target_bnc_before = Currencies::total_balance(BNC, &target_account);
			// Native (HDX) free balance after the input was reserved — only the forward fee touches it.
			let alice_hdx_free_before = Currencies::free_balance(HDX, &alice);

			let solution = run_solver_and_submit();
			assert_eq!(solution.resolved_intents.len(), 1, "swap resolved");
			let amount_out = resolved_swap_amount_out(&solution);

			// Resolution credited the owner the full output.
			assert_eq!(Currencies::total_balance(BNC, &alice), alice_bnc_before + amount_out);

			// The owner paid the forward-execution fee at queue time.
			let fee = last_queued_fee(&alice);
			assert_eq!(Currencies::free_balance(HDX, &alice), alice_hdx_free_before - fee);

			// The forward is queued with this trade's resolved amounts.
			let stored = LazyExecutor::call_queue(0).expect("forward queued");
			assert_eq!(stored.owner, alice);
			assert_eq!(stored.action.contract, receiver);
			assert_eq!(stored.action.asset_out, BNC);
			assert_eq!(stored.action.amount_out, amount_out);

			// Execute the forward (the OCW would normally submit this unsigned extrinsic).
			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

			// Push committed: the contract received `amount_out` and forwarded it all to the target.
			assert_eq!(
				Currencies::total_balance(BNC, &target_account),
				target_bnc_before + amount_out
			);
			assert_eq!(Currencies::total_balance(BNC, &receiver_account), 0);
			// Owner is net-flat on BNC: received `amount_out`, then it was pushed out.
			assert_eq!(Currencies::total_balance(BNC, &alice), alice_bnc_before);

			let events = last_hydra_events(20);
			assert!(events.iter().any(|e| matches!(
				e,
				RuntimeEvent::LazyExecutor(pallet_lazy_executor::Event::Executed { result: Ok(()), .. })
			)));
		});
}

#[test]
fn forward_should_roll_back_and_leave_owner_whole_when_receiver_reverts() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();

			let receiver = deploy_contract("RevertingReceiver", EVMAccounts::evm_address(&alice));
			let receiver_account = EVMAccounts::account_id(receiver);

			let alice_bnc_before = Currencies::total_balance(BNC, &alice);

			submit_swap_with_forward(alice.clone(), receiver, encode_address(H160::repeat_byte(0xAA)));

			let solution = run_solver_and_submit();
			let amount_out = resolved_swap_amount_out(&solution);
			let alice_bnc_after_resolve = Currencies::total_balance(BNC, &alice);
			// Resolution credited the owner; the rollback assertion below is therefore meaningful.
			assert_eq!(alice_bnc_after_resolve, alice_bnc_before + amount_out);

			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

			// Forward failed: push rolled back, owner keeps the output, contract got nothing.
			assert_eq!(Currencies::total_balance(BNC, &alice), alice_bnc_after_resolve);
			assert_eq!(Currencies::total_balance(BNC, &receiver_account), 0);

			let events = last_hydra_events(20);
			assert!(events.iter().any(|e| matches!(
				e,
				RuntimeEvent::LazyExecutor(pallet_lazy_executor::Event::Executed { result: Err(_), .. })
			)));
		});
}

#[test]
fn forward_should_fire_and_charge_fee_on_each_dca_trade() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100)
		.execute(|| {
			enable_slip_fees();

			let receiver = deploy_contract("IntentResolutionReceiver", EVMAccounts::evm_address(&alice));
			let target_evm = H160::repeat_byte(0xAA);
			let target_account = EVMAccounts::account_id(target_evm);
			let receiver_account = EVMAccounts::account_id(receiver);

			// 2-trade DCA (budget = 2 × per-trade input), each resolution forwards that trade's output.
			submit_dca_with_forward(alice.clone(), 2 * TRADE_AMOUNT, receiver, encode_address(target_evm));
			let target_before = Currencies::total_balance(BNC, &target_account);

			// ---- Trade 1: intermediate (intent stays) ----
			let hdx_free_before_1 = Currencies::free_balance(HDX, &alice);
			let out1 = resolved_swap_amount_out(&advance_and_solve(PERIOD));
			assert_eq!(
				pallet_intent::Intents::<Runtime>::iter().count(),
				1,
				"DCA still active after trade 1"
			);

			// Exactly one forward queued so far, carrying trade 1's output; owner charged the fee.
			assert_eq!(
				LazyExecutor::call_queue(1),
				None,
				"only one forward queued after trade 1"
			);
			let queued1 = LazyExecutor::call_queue(0).expect("forward 0 queued");
			assert_eq!(queued1.action.amount_out, out1);
			let fee1 = last_queued_fee(&alice);
			assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before_1 - fee1);

			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));
			assert_eq!(Currencies::total_balance(BNC, &target_account), target_before + out1);

			// ---- Trade 2: final (budget exhausted, intent removed) ----
			let hdx_free_before_2 = Currencies::free_balance(HDX, &alice);
			let out2 = resolved_swap_amount_out(&advance_and_solve(PERIOD));
			assert_eq!(
				pallet_intent::Intents::<Runtime>::iter().count(),
				0,
				"DCA complete after trade 2"
			);

			// A second forward fired for trade 2; owner charged again.
			let queued2 = LazyExecutor::call_queue(1).expect("forward 1 queued");
			assert_eq!(queued2.action.amount_out, out2);
			let fee2 = last_queued_fee(&alice);
			assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before_2 - fee2);

			assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none(), 1));

			// Target accrued both trades' outputs; the receiver is left empty.
			assert_eq!(
				Currencies::total_balance(BNC, &target_account),
				target_before + out1 + out2
			);
			assert_eq!(Currencies::total_balance(BNC, &receiver_account), 0);
		});
}
