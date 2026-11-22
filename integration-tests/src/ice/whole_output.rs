use crate::polkadot_test_net::{TestNet, ALICE, BOB};
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, Runtime, RuntimeEvent, RuntimeOrigin, System};
use ice_support::{Balance, IntentId, Solution, SolverMode};
use orml_traits::MultiCurrency;
use primitives::{AccountId, AssetId};
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::harness::{enable_slip_fees, resolved, run_and_submit_as, swap, V4Solver};
use super::PATH_TO_SNAPSHOT;

const HDX: AssetId = 0;
const BNC: AssetId = 14;
const AAVE_UNDERLYING: AssetId = 22;
const ATOKEN: AssetId = 1003;
const UNIT: Balance = 1_000_000_000_000;
const UNDERLYING_UNIT: Balance = 1_000_000;

/// The snapshot's holding pot holds exactly the HDX existential deposit and no BNC or `AAVE_UNDERLYING`.
const POT_HDX: Balance = UNIT;
const POT_SEED_BNC: Balance = 4_321 * UNIT;
const POT_SEED_UNDERLYING: Balance = 4_321 * UNDERLYING_UNIT;

const SWAP_HDX_IN: Balance = 1_000 * UNIT;
const SWAP_BNC_MIN_OUT: Balance = 10 * UNIT;
const PIN_SWAP_CLAIMED: Balance = 67_430_255_289_688;
/// The router's output.
const PIN_SWAP_DELIVERED: Balance = 67_436_998_989_586;

const ATOKEN_UNDERLYING_IN: Balance = 1_000 * UNDERLYING_UNIT;
const ATOKEN_MIN_OUT: Balance = 22_409;
const PIN_ATOKEN_CLAIMED: Balance = 999_999_998;
/// The Aave supply's output.
const PIN_ATOKEN_DELIVERED: Balance = 1_000_000_000;

const DCA_HDX_IN: Balance = 10 * UNIT;
const DCA_BNC_MIN_OUT: Balance = 68_795_189_840;
const DCA_PERIOD: u32 = 15;
const PIN_DCA_CLAIMED: Balance = 674_325_708_682;
/// The router's output.
const PIN_DCA_DELIVERED: Balance = 674_393_147_996;

const OPPOSITE_HDX_IN: Balance = 10_000 * UNIT;
const OPPOSITE_BNC_MIN_OUT: Balance = 100 * UNIT;
const OPPOSITE_BNC_IN: Balance = 1_000 * UNIT;
const OPPOSITE_HDX_MIN_OUT: Balance = 1_000 * UNIT;
/// ALICE's BNC is matched only, so it equals her claim.
const PIN_ALICE_DELIVERED: Balance = 676_286_517_104_114;
const PIN_BOB_CLAIMED: Balance = 14_766_061_328_471_529;
/// Claim plus the HDX residual 2_476_853_818_228 less the swept fee.
const PIN_BOB_DELIVERED: Balance = 14_766_538_182_289_757;
const PIN_FEE_HDX: Balance = 2_000_000_000_000;
const PIN_FEE_BNC: Balance = 135_284_360_292;

fn holding_pot() -> AccountId {
	pallet_ice::Pallet::<Runtime>::get_pallet_account()
}

fn fee_receiver() -> AccountId {
	hydradx_runtime::IceFeeReceiver::get()
}

fn free(asset: AssetId, who: &AccountId) -> Balance {
	Currencies::free_balance(asset, who)
}

/// Kept below the deposit circuit breaker's limit, which would otherwise reserve the excess.
fn seed_pot(asset: AssetId, amount: Balance) {
	if free(asset, &holding_pot()) == 0 {
		assert_ok!(Currencies::deposit(asset, &holding_pot(), amount));
	}
}

fn intent_of(owner: &AccountId) -> IntentId {
	pallet_intent::Intents::<Runtime>::iter_keys()
		.find(|id| pallet_intent::Pallet::<Runtime>::intent_owner(*id).as_ref() == Some(owner))
		.expect("owner has a stored intent")
}

fn intent_events() -> Vec<pallet_intent::Event<Runtime>> {
	System::events()
		.into_iter()
		.filter_map(|r| match r.event {
			RuntimeEvent::Intent(e) => Some(e),
			_ => None,
		})
		.collect()
}

fn resolved_event_out(id: IntentId) -> Balance {
	intent_events()
		.into_iter()
		.find_map(|e| match e {
			pallet_intent::Event::IntentResolved { id: i, amount_out, .. } if i == id => Some(amount_out),
			_ => None,
		})
		.expect("an IntentResolved event")
}

fn dca_event_out(id: IntentId) -> Balance {
	intent_events()
		.into_iter()
		.find_map(|e| match e {
			pallet_intent::Event::DcaTradeExecuted { id: i, amount_out, .. } if i == id => Some(amount_out),
			_ => None,
		})
		.expect("a DcaTradeExecuted event")
}

fn router_out(asset_in: AssetId, asset_out: AssetId) -> Balance {
	System::events()
		.into_iter()
		.filter_map(|r| match r.event {
			RuntimeEvent::Router(pallet_route_executor::Event::Executed {
				asset_in: i,
				asset_out: o,
				amount_out,
				..
			}) if i == asset_in && o == asset_out => Some(amount_out),
			_ => None,
		})
		.sum()
}

fn solve() -> Solution {
	System::reset_events();
	run_and_submit_as::<V4Solver>(SolverMode::V4, "whole_output")
}

fn with_snapshot(endow: &[(AccountId, AssetId, Balance)], execution: impl FnOnce()) {
	TestNet::reset();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	for (who, asset, amount) in endow {
		driver.endow_account(who.clone(), *asset, *amount);
	}
	driver.execute(|| {
		enable_slip_fees();
		execution();
	});
}

fn submit_swap(who: &AccountId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
		pallet_intent::types::IntentInput {
			data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
				asset_in,
				asset_out,
				amount_in,
				amount_out: min_out,
				partial: false,
			}),
			deadline: None,
			on_resolved: None,
		}
	));
}

#[test]
fn single_intent_should_receive_whole_omnipool_output_when_solved_by_v4() {
	let alice: AccountId = ALICE.into();
	with_snapshot(&[(alice.clone(), HDX, 10 * SWAP_HDX_IN)], || {
		submit_swap(&alice, HDX, BNC, SWAP_HDX_IN, SWAP_BNC_MIN_OUT);
		let id = intent_of(&alice);
		seed_pot(HDX, POT_HDX);
		seed_pot(BNC, POT_SEED_BNC);
		let alice_bnc = free(BNC, &alice);

		let sol = solve();

		assert_eq!(swap(resolved(&sol, id)).amount_out, PIN_SWAP_CLAIMED);
		assert_eq!(free(BNC, &alice) - alice_bnc, PIN_SWAP_DELIVERED);
		assert_eq!(router_out(HDX, BNC), PIN_SWAP_DELIVERED);
		assert_eq!(free(BNC, &holding_pot()), POT_SEED_BNC);
		assert_eq!(free(HDX, &holding_pot()), POT_HDX);
		assert_eq!(resolved_event_out(id), PIN_SWAP_DELIVERED);
	});
}

#[test]
fn single_intent_should_receive_whole_output_when_asset_out_is_atoken_and_pot_holds_none() {
	let alice: AccountId = ALICE.into();
	with_snapshot(&[(alice.clone(), AAVE_UNDERLYING, 10 * ATOKEN_UNDERLYING_IN)], || {
		assert_eq!(free(ATOKEN, &holding_pot()), 0);
		submit_swap(&alice, AAVE_UNDERLYING, ATOKEN, ATOKEN_UNDERLYING_IN, ATOKEN_MIN_OUT);
		let id = intent_of(&alice);
		seed_pot(AAVE_UNDERLYING, POT_SEED_UNDERLYING);
		let alice_atoken = free(ATOKEN, &alice);

		let sol = solve();

		assert_eq!(swap(resolved(&sol, id)).amount_out, PIN_ATOKEN_CLAIMED);
		assert_eq!(free(ATOKEN, &alice) - alice_atoken, PIN_ATOKEN_DELIVERED);
		assert_eq!(free(ATOKEN, &holding_pot()), 0);
		assert_eq!(free(AAVE_UNDERLYING, &holding_pot()), POT_SEED_UNDERLYING);
		assert_eq!(resolved_event_out(id), PIN_ATOKEN_DELIVERED);
	});
}

#[test]
fn dca_trade_should_receive_whole_output_when_solved_alone() {
	let alice: AccountId = ALICE.into();
	with_snapshot(&[(alice.clone(), HDX, 100 * DCA_HDX_IN)], || {
		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Dca(ice_support::DcaParams {
					asset_in: HDX,
					asset_out: BNC,
					amount_in: DCA_HDX_IN,
					amount_out: DCA_BNC_MIN_OUT,
					slippage: Permill::from_percent(10),
					budget: Some(5 * DCA_HDX_IN),
					period: DCA_PERIOD,
				}),
				deadline: None,
				on_resolved: None,
			}
		));
		let id = intent_of(&alice);
		seed_pot(HDX, POT_HDX);
		seed_pot(BNC, POT_SEED_BNC);
		let alice_bnc = free(BNC, &alice);

		let sol = solve();

		assert_eq!(swap(resolved(&sol, id)).amount_out, PIN_DCA_CLAIMED);
		assert_eq!(free(BNC, &alice) - alice_bnc, PIN_DCA_DELIVERED);
		assert_eq!(router_out(HDX, BNC), PIN_DCA_DELIVERED);
		assert_eq!(free(BNC, &holding_pot()), POT_SEED_BNC);
		assert_eq!(free(HDX, &holding_pot()), POT_HDX);
		assert_eq!(dca_event_out(id), PIN_DCA_DELIVERED);
	});
}

#[test]
fn opposite_intents_should_receive_whole_net_trade_output_when_solved_by_v4() {
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	with_snapshot(
		&[
			(alice.clone(), HDX, 2 * OPPOSITE_HDX_IN),
			(bob.clone(), BNC, 2 * OPPOSITE_BNC_IN),
		],
		|| {
			submit_swap(&alice, HDX, BNC, OPPOSITE_HDX_IN, OPPOSITE_BNC_MIN_OUT);
			submit_swap(&bob, BNC, HDX, OPPOSITE_BNC_IN, OPPOSITE_HDX_MIN_OUT);
			let alice_id = intent_of(&alice);
			let bob_id = intent_of(&bob);
			seed_pot(HDX, POT_HDX);
			seed_pot(BNC, POT_SEED_BNC);
			let alice_bnc = free(BNC, &alice);
			let bob_hdx = free(HDX, &bob);
			let fee_hdx = free(HDX, &fee_receiver());
			let fee_bnc = free(BNC, &fee_receiver());

			let sol = solve();

			assert_eq!(swap(resolved(&sol, alice_id)).amount_out, PIN_ALICE_DELIVERED);
			assert_eq!(swap(resolved(&sol, bob_id)).amount_out, PIN_BOB_CLAIMED);
			assert_eq!(free(HDX, &bob) - bob_hdx, PIN_BOB_DELIVERED);
			assert_eq!(free(BNC, &alice) - alice_bnc, PIN_ALICE_DELIVERED);
			assert_eq!(free(HDX, &holding_pot()), POT_HDX);
			assert_eq!(free(BNC, &holding_pot()), POT_SEED_BNC);
			assert_eq!(free(HDX, &fee_receiver()) - fee_hdx, PIN_FEE_HDX);
			assert_eq!(free(BNC, &fee_receiver()) - fee_bnc, PIN_FEE_BNC);
			assert_eq!(resolved_event_out(bob_id), PIN_BOB_DELIVERED);
			assert_eq!(resolved_event_out(alice_id), PIN_ALICE_DELIVERED);
		},
	);
}
