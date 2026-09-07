//! Emergency pass-through mode against the real `mainnet_apr` snapshot.
//!
//! Every matched scenario from `netting.rs` and `dca.rs` is replayed under both
//! `SolverMode::V4` and `SolverMode::Passthrough` from the same snapshot, so the
//! two runs can be compared directly: pass-through must settle a subset of what
//! v4 settles, always above each user's own limit, without ever touching the
//! holding pot or the fee receiver. The partial-intent section pins the
//! full-remaining-or-skip policy that only exists in this mode.

use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::pallet_prelude::{
	InvalidTransaction, TransactionSource, TransactionValidityError, ValidateUnsigned,
};
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, Runtime, RuntimeEvent, RuntimeOrigin, Timestamp};
use hydradx_traits::evm::InspectEvmAccounts;
use hydradx_traits::router::{AssetPair, RouteProvider};
use ice_support::{
	Balance, IntentData, IntentId, Partial, PoolTrade, ResolvedIntent, ResolvedIntents, Solution, SolutionTrades,
	SolverMode, SwapData, SwapType,
};
use orml_traits::{MultiCurrency, MultiReservableCurrency};
use primitives::constants::time::MILLISECS_PER_BLOCK;
use primitives::{AccountId, AssetId};
use sp_runtime::Permill;
use std::cell::RefCell;
use xcm_emulator::Network;

use super::harness::{
	amm_trade_count, dump, enable_slip_fees, is_resolved, resolved, run_and_submit_as, run_and_submit_for_mode,
	set_solver_mode, solve_as, solve_for_mode, swap, PassthroughSolver, V4Solver,
};
use super::PATH_TO_SNAPSHOT;

const HDX: AssetId = 0;
const DOT: AssetId = 5;
const BNC: AssetId = 14;
const WETH: AssetId = 20;
const ETH: AssetId = 34;

const HDX_UNIT: Balance = 1_000_000_000_000;
const BNC_UNIT: Balance = 1_000_000_000_000;
const DOT_UNIT: Balance = 10_000_000_000;
const WETH_UNIT: Balance = 1_000_000_000_000_000_000;
const ETH_UNIT: Balance = 1_000_000_000_000_000_000;

// DCA constants shared with `dca.rs` so the scenarios really are the same ones.
const TRADE_AMOUNT: Balance = 10_000_000_000_000;
const MIN_OUT_BNC: Balance = 68_795_189_840;
const PERIOD: u32 = 15;
const DCA_SLIPPAGE: Permill = Permill::from_percent(10);

// ---------------------------------------------------------------------------
// Chain-state helpers
// ---------------------------------------------------------------------------

fn holding_pot() -> AccountId {
	pallet_ice::Pallet::<Runtime>::get_pallet_account()
}

fn fee_receiver() -> AccountId {
	hydradx_runtime::IceFeeReceiver::get()
}

fn balances_of(who: &AccountId, assets: &[AssetId]) -> Vec<Balance> {
	assets.iter().map(|a| Currencies::total_balance(*a, who)).collect()
}

fn deltas(before: &[Balance], after: &[Balance]) -> Vec<i128> {
	before
		.iter()
		.zip(after.iter())
		.map(|(b, a)| *a as i128 - *b as i128)
		.collect()
}

/// Intent ids in ascending order — submission order, since every intent in a
/// scenario is created in the same block (`id = now << 64 | seq`).
fn intent_ids_ascending() -> Vec<IntentId> {
	let mut ids: Vec<IntentId> = pallet_intent::Intents::<Runtime>::iter_keys().collect();
	ids.sort_unstable();
	ids
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

fn ice_events() -> Vec<pallet_ice::Event<Runtime>> {
	hydradx_runtime::System::events()
		.into_iter()
		.filter_map(|r| match r.event {
			RuntimeEvent::ICE(e) => Some(e),
			_ => None,
		})
		.collect()
}

fn last_solution_executed() -> pallet_ice::Event<Runtime> {
	ice_events()
		.into_iter()
		.filter(|e| matches!(e, pallet_ice::Event::SolutionExecuted { .. }))
		.next_back()
		.expect("a SolutionExecuted event")
}

fn any_partial_resolution_event() -> bool {
	intent_events()
		.iter()
		.any(|e| matches!(e, pallet_intent::Event::IntentResovedPartially { .. }))
}

fn stored_swap(id: IntentId) -> SwapData {
	let intent = pallet_intent::Pallet::<Runtime>::get_intent(id).expect("intent to exist");
	match intent.data {
		IntentData::Swap(s) => s,
		_ => panic!("expected a swap intent"),
	}
}

fn stored_dca(id: IntentId) -> ice_support::DcaData {
	let intent = pallet_intent::Pallet::<Runtime>::get_intent(id).expect("intent to exist");
	match intent.data {
		IntentData::Dca(d) => d,
		_ => panic!("expected a DCA intent"),
	}
}

fn submit_swap(who: &AccountId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) {
	let deadline = MILLISECS_PER_BLOCK * 10 + Timestamp::now();
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
			deadline: Some(deadline),
			on_resolved: None,
		}
	));
}

fn submit_dca(who: &AccountId, budget: Option<Balance>, slippage: Permill) {
	assert_ok!(hydradx_runtime::Intent::submit_intent(
		RuntimeOrigin::signed(who.clone()),
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

/// Recreates the state a v4-era partial fill leaves behind: the intent tracks
/// `filled`, and that part of the reserve is already gone from the account.
/// `submit_intent` refuses to create partial intents, so it is injected here by
/// mutating storage directly — same recipe as the pallet's own unit tests.
fn make_partial(id: IntentId, owner: &AccountId, filled: Balance) {
	pallet_intent::Intents::<Runtime>::mutate(id, |maybe_intent| {
		let intent = maybe_intent.as_mut().expect("intent to exist");
		let IntentData::Swap(ref mut swap) = intent.data else {
			panic!("expected a swap intent");
		};
		swap.partial = Partial::Yes(filled);
	});

	if filled > 0 {
		let asset_in = stored_swap(id).asset_in;
		assert_ok!(pallet_intent::Pallet::<Runtime>::unlock_funds(owner, asset_in, filled));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			owner.clone(),
			asset_in,
			-(filled as i128),
		));
	}
}

/// What the on-chain default route pays for `amount_in`. `None` when the pool
/// refuses the size outright, which reads the same as "cannot clear a limit".
fn router_quote(asset_in: AssetId, asset_out: AssetId, amount_in: Balance) -> Option<Balance> {
	let route = hydradx_runtime::Router::get_route(AssetPair::new(asset_in, asset_out));
	hydradx_runtime::Router::calculate_expected_amount_out(&route, amount_in).ok()
}

fn single_sell(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> PoolTrade {
	PoolTrade {
		direction: SwapType::ExactIn,
		amount_in,
		amount_out: min_out,
		route: hydradx_runtime::Router::get_route(AssetPair::new(asset_in, asset_out)),
	}
}

fn solution_of(resolved_intents: Vec<ResolvedIntent>, trades: Vec<PoolTrade>, score: Balance) -> Solution {
	Solution::new(
		ResolvedIntents::truncate_from(resolved_intents),
		SolutionTrades::truncate_from(trades),
		score,
	)
}

/// What the transaction pool (and block import) makes of this solution under the
/// mode currently stored on chain.
fn validate_in_pool(solution: Solution) -> Result<(), TransactionValidityError> {
	let call = pallet_ice::Call::<Runtime>::submit_solution { solution };
	pallet_ice::Pallet::<Runtime>::validate_unsigned(TransactionSource::Local, &call).map(|_| ())
}

fn rejected_by_pool(solution: Solution) {
	assert_eq!(
		validate_in_pool(solution),
		Err(TransactionValidityError::Invalid(InvalidTransaction::Call))
	);
}

// ---------------------------------------------------------------------------
// Comparative swap-scenario harness
// ---------------------------------------------------------------------------

struct SwapSpec {
	who: AccountId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
}

/// What one (mode, solver) run of a scenario actually did on chain.
struct Outcome {
	resolved: Vec<bool>,
	resolved_count: usize,
	trades: usize,
	score: Balance,
	/// `asset_out` credited to each spec's owner (0 when the intent was skipped).
	received: Vec<Balance>,
	/// Sum over settled intents of what each owner got above their own limit.
	surplus: Balance,
	pot_delta: Vec<i128>,
	fee_delta: Vec<i128>,
}

fn dump_outcome(label: &str, o: &Outcome) {
	let received = o.received.iter().map(|r| format!("{r}")).collect::<Vec<_>>().join(", ");
	println!(
		"// {label}: assert_outcome(&x, {}, {}, {}, {}, &[{received}]);",
		o.resolved_count, o.trades, o.score, o.surplus
	);
	println!(
		"// {label}: resolved={:?} pot_delta={:?} fee_delta={:?}",
		o.resolved, o.pot_delta, o.fee_delta
	);
}

/// Pin one run in full: what settled, how many AMM legs it took, the reported
/// score, the surplus users actually received and each owner's exact output.
fn assert_outcome(
	o: &Outcome,
	resolved_count: usize,
	trades: usize,
	score: Balance,
	surplus: Balance,
	received: &[Balance],
) {
	assert_eq!(o.resolved_count, resolved_count, "resolved count");
	assert_eq!(o.trades, trades, "AMM trade count");
	assert_eq!(o.score, score, "reported score");
	assert_eq!(o.surplus, surplus, "surplus received by owners");
	assert_eq!(o.received.as_slice(), received, "per-owner output");
}

fn touched_assets(specs: &[SwapSpec]) -> Vec<AssetId> {
	let mut assets: Vec<AssetId> = specs.iter().flat_map(|s| [s.asset_in, s.asset_out]).collect();
	assets.sort_unstable();
	assets.dedup();
	assets
}

/// Run one swap scenario end to end under `mode` on a fresh snapshot.
fn run_swap_scenario(mode: SolverMode, label: &str, specs: Vec<SwapSpec>) -> Outcome {
	TestNet::reset();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	for s in &specs {
		driver.endow_account(s.who.clone(), s.asset_in, s.amount_in.saturating_mul(10));
	}

	let captured: RefCell<Option<Outcome>> = RefCell::new(None);
	driver.execute(|| {
		enable_slip_fees();
		for s in &specs {
			submit_swap(&s.who, s.asset_in, s.asset_out, s.amount_in, s.min_out);
		}
		let ids = intent_ids_ascending();
		assert_eq!(ids.len(), specs.len(), "every intent must be stored");
		assert_eq!(
			pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
			specs.len(),
			"every intent must be solver-valid"
		);

		let assets = touched_assets(&specs);
		let before: Vec<Balance> = specs
			.iter()
			.map(|s| Currencies::total_balance(s.asset_out, &s.who))
			.collect();
		let pot_before = balances_of(&holding_pot(), &assets);
		let fee_before = balances_of(&fee_receiver(), &assets);

		let sol = run_and_submit_for_mode(mode, label);

		let after: Vec<Balance> = specs
			.iter()
			.map(|s| Currencies::total_balance(s.asset_out, &s.who))
			.collect();
		let pot_after = balances_of(&holding_pot(), &assets);
		let fee_after = balances_of(&fee_receiver(), &assets);

		let resolved_flags: Vec<bool> = ids.iter().map(|id| is_resolved(&sol, *id)).collect();
		let received: Vec<Balance> = before.iter().zip(after.iter()).map(|(b, a)| a - b).collect();
		let mut surplus: Balance = 0;
		for (i, spec) in specs.iter().enumerate() {
			if resolved_flags[i] {
				assert!(
					received[i] >= spec.min_out,
					"{label}: intent {i} settled below its own limit: {} < {}",
					received[i],
					spec.min_out
				);
				surplus += received[i] - spec.min_out;
			} else {
				assert_eq!(received[i], 0, "{label}: skipped intent {i} must receive nothing");
				assert_eq!(
					Currencies::reserved_balance(spec.asset_in, &spec.who),
					spec.amount_in,
					"{label}: skipped intent {i} must keep its reserve"
				);
			}
		}

		if mode == SolverMode::Passthrough {
			assert_eq!(sol.trades.len(), sol.resolved_intents.len(), "{label}: 1:1 shape");
			assert_eq!(pot_after, pot_before, "{label}: holding pot must stay untouched");
			assert_eq!(fee_after, fee_before, "{label}: fee receiver must stay untouched");
			assert!(!any_partial_resolution_event(), "{label}: never a partial resolution");
		}

		let outcome = Outcome {
			resolved: resolved_flags,
			resolved_count: sol.resolved_intents.len(),
			trades: sol.trades.len(),
			score: sol.score,
			received,
			surplus,
			pot_delta: deltas(&pot_before, &pot_after),
			fee_delta: deltas(&fee_before, &fee_after),
		};
		dump_outcome(label, &outcome);
		*captured.borrow_mut() = Some(outcome);
	});

	captured.into_inner().expect("the scenario must have run")
}

/// The relations the design pins for every comparative re-run: pass-through
/// settles a subset of v4's set, one AMM trade per settled intent.
fn assert_subset_and_shape(v4: &Outcome, pt: &Outcome) {
	for (i, settled) in pt.resolved.iter().enumerate() {
		if *settled {
			assert!(v4.resolved[i], "pass-through settled intent {i} that v4 skipped");
		}
	}
	assert!(pt.resolved_count <= v4.resolved_count);
	assert_eq!(pt.trades, pt.resolved_count, "pass-through routes 1 trade per intent");
	assert!(
		v4.trades <= v4.resolved_count,
		"matching never routes more than 1 trade per intent"
	);
	if pt.resolved_count == v4.resolved_count {
		assert!(
			v4.trades <= pt.trades,
			"matching internalizes legs pass-through must route"
		);
	}
}

// ---------------------------------------------------------------------------
// 1. Comparative re-runs — netting suite
// ---------------------------------------------------------------------------

fn chain_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: BNC,
			asset_out: HDX,
			amount_in: 1_000 * BNC_UNIT,
			min_out: 1_000 * HDX_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in: 14_000 * HDX_UNIT,
			min_out: DOT_UNIT,
		},
	]
}

#[test]
fn chain_should_settle_every_intent_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "chain/v4", chain_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "chain/passthrough", chain_specs());

	// v4 nets the HDX leg away; pass-through routes both legs through the pool.
	assert_outcome(
		&v4,
		2,
		2,
		13778637061199802,
		13778637061199802,
		&[14778425464087880, 221597111922],
	);
	assert_outcome(
		&pt,
		2,
		2,
		13731994797173544,
		13731994797207844,
		&[14731783072285759, 221724922085],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "matching must not pay worse than the AMM");

	// Strict routes value through the pot and sweeps the matched fee; pass-through
	// touches neither account.
	assert_eq!(v4.pot_delta, vec![0, 22161927, 1]);
	assert_eq!(v4.fee_delta, vec![2800000000000, 0, 0]);
	assert_eq!(pt.pot_delta, vec![0, 0, 0]);
	assert_eq!(pt.fee_delta, vec![0, 0, 0]);
}

fn three_asset_cycle_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 1_000 * HDX_UNIT,
			min_out: BNC_UNIT / 2,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: BNC,
			asset_out: DOT,
			amount_in: 5 * BNC_UNIT,
			min_out: DOT_UNIT / 10,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in: 10 * DOT_UNIT,
			min_out: 500 * HDX_UNIT,
		},
	]
}

#[test]
fn three_asset_cycle_should_settle_every_intent_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "cycle3/v4", three_asset_cycle_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "cycle3/passthrough", three_asset_cycle_specs());

	assert_outcome(
		&v4,
		3,
		2,
		5848644064753465,
		5848644064753465,
		&[67433819967149, 1173241108, 6281710071545208],
	);
	assert_outcome(
		&pt,
		3,
		3,
		5846120277400754,
		5846120266188516,
		&[67436998989586, 1170670963, 6279183096527967],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "internalizing the cycle must beat the AMM");
}

fn partial_coincidence_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: BNC,
			asset_out: HDX,
			amount_in: 1_000 * BNC_UNIT,
			min_out: 1_000 * HDX_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in: 5_000 * HDX_UNIT,
			min_out: DOT_UNIT,
		},
	]
}

#[test]
fn partial_coincidence_should_settle_every_intent_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "coincidence/v4", partial_coincidence_specs());
	let pt = run_swap_scenario(
		SolverMode::Passthrough,
		"coincidence/passthrough",
		partial_coincidence_specs(),
	);

	assert_outcome(
		&v4,
		2,
		2,
		13749119045233406,
		13749119045233406,
		&[14749049912240615, 79132992791],
	);
	assert_outcome(
		&pt,
		2,
		2,
		13731852260978758,
		13731852260991009,
		&[14731783072285759, 79188705250],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "netting the overlap must beat the AMM");
}

fn four_asset_cycle_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 10_000 * HDX_UNIT,
			min_out: BNC_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: BNC,
			asset_out: DOT,
			amount_in: 680 * BNC_UNIT,
			min_out: DOT_UNIT,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: DOT,
			asset_out: WETH,
			amount_in: 15 * DOT_UNIT,
			min_out: WETH_UNIT / 1000,
		},
		SwapSpec {
			who: DAVE.into(),
			asset_in: WETH,
			asset_out: HDX,
			amount_in: WETH_UNIT / 30,
			min_out: 100 * HDX_UNIT,
		},
	]
}

#[test]
fn four_asset_cycle_should_settle_every_intent_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "cycle4/v4", four_asset_cycle_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "cycle4/passthrough", four_asset_cycle_specs());

	assert_outcome(
		&v4,
		4,
		3,
		43712571451562280,
		43712571451562280,
		&[676286517104114, 159538036540, 8919692554988036, 35217442841433590],
	);
	assert_outcome(
		&pt,
		4,
		4,
		43648282646246644,
		43648281302325024,
		&[674159536400221, 159228461232, 8883625605429048, 35191346932034523],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "internalizing the cycle must beat the AMM");
}

fn five_asset_cycle_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 10_000 * HDX_UNIT,
			min_out: BNC_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: BNC,
			asset_out: DOT,
			amount_in: 680 * BNC_UNIT,
			min_out: DOT_UNIT,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: DOT,
			asset_out: WETH,
			amount_in: 15 * DOT_UNIT,
			min_out: WETH_UNIT / 1000,
		},
		SwapSpec {
			who: DAVE.into(),
			asset_in: WETH,
			asset_out: ETH,
			amount_in: WETH_UNIT / 30,
			min_out: ETH_UNIT / 1000,
		},
		SwapSpec {
			who: EVE.into(),
			asset_in: ETH,
			asset_out: HDX,
			amount_in: ETH_UNIT / 30,
			min_out: 100 * HDX_UNIT,
		},
	]
}

#[test]
fn five_asset_cycle_should_settle_every_intent_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "cycle5/v4", five_asset_cycle_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "cycle5/passthrough", five_asset_cycle_specs());

	assert_outcome(
		&v4,
		5,
		4,
		76040401743062421,
		76040401743062421,
		&[
			676286517104114,
			159538039639,
			8919692554988035,
			33305976737841383,
			35239296395089250,
		],
	);
	assert_outcome(
		&pt,
		5,
		5,
		75982782668032656,
		75982781323278257,
		&[
			674159536400221,
			159228461232,
			8883625605429048,
			33312537313045166,
			35213309639942590,
		],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "internalizing the cycle must beat the AMM");
}

fn direct_match_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: BNC,
			asset_out: HDX,
			amount_in: 1_000 * BNC_UNIT,
			min_out: 1_000 * HDX_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 10_000 * HDX_UNIT,
			min_out: BNC_UNIT / 2,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: HDX,
			asset_out: DOT,
			amount_in: 5_000 * HDX_UNIT,
			min_out: DOT_UNIT,
		},
	]
}

#[test]
fn same_pair_direct_match_should_settle_both_sides_through_the_amm_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "direct_match/v4", direct_match_specs());
	let pt = run_swap_scenario(
		SolverMode::Passthrough,
		"direct_match/passthrough",
		direct_match_specs(),
	);

	assert_outcome(
		&v4,
		3,
		2,
		14456573836907606,
		14456573836907606,
		&[14780718153014936, 676286517104114, 79166788556],
	);
	assert_outcome(
		&pt,
		3,
		3,
		14405918509662016,
		14405918614028742,
		&[14731783072285759, 674566355699449, 79186043534],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(v4.surplus >= pt.surplus, "the direct match must beat two AMM legs");
}

fn tight_edge_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 1_000 * HDX_UNIT,
			min_out: 68 * BNC_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: BNC,
			asset_out: DOT,
			amount_in: 5 * BNC_UNIT,
			min_out: DOT_UNIT / 10,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: DOT,
			asset_out: HDX,
			amount_in: 10 * DOT_UNIT,
			min_out: 500 * HDX_UNIT,
		},
		SwapSpec {
			who: DAVE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 1_000 * HDX_UNIT,
			min_out: 60 * BNC_UNIT,
		},
	]
}

#[test]
fn tight_limit_edge_intent_should_skip_without_failing_the_batch_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "tight_edge/v4", tight_edge_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "tight_edge/passthrough", tight_edge_specs());

	assert_outcome(
		&v4,
		3,
		2,
		5789144064753465,
		5789144064753465,
		&[0, 1173241108, 6281710071545208, 67433819967149],
	);
	assert_outcome(
		&pt,
		3,
		3,
		5786561706387558,
		5786561709861513,
		&[0, 1170651988, 6279122529632992, 67439009576533],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(!v4.resolved[0], "the tight-limit intent stays out under v4");
	assert!(!pt.resolved[0], "and stays out under pass-through");
	assert!(v4.surplus >= pt.surplus, "the ring still beats three AMM legs");
}

fn tight_cycle_specs() -> Vec<SwapSpec> {
	vec![
		SwapSpec {
			who: ALICE.into(),
			asset_in: HDX,
			asset_out: BNC,
			amount_in: 10_000 * HDX_UNIT,
			min_out: BNC_UNIT,
		},
		SwapSpec {
			who: BOB.into(),
			asset_in: BNC,
			asset_out: DOT,
			amount_in: 680 * BNC_UNIT,
			min_out: DOT_UNIT,
		},
		SwapSpec {
			who: CHARLIE.into(),
			asset_in: DOT,
			asset_out: WETH,
			amount_in: 15 * DOT_UNIT,
			min_out: WETH_UNIT / 1000,
		},
		SwapSpec {
			who: DAVE.into(),
			asset_in: WETH,
			asset_out: HDX,
			amount_in: WETH_UNIT / 30,
			min_out: 35_500 * HDX_UNIT,
		},
	]
}

#[test]
fn tight_limit_cycle_intent_should_skip_without_failing_the_batch_when_mode_is_passthrough() {
	let v4 = run_swap_scenario(SolverMode::V4, "tight_cycle/v4", tight_cycle_specs());
	let pt = run_swap_scenario(SolverMode::Passthrough, "tight_cycle/passthrough", tight_cycle_specs());

	assert_outcome(
		&v4,
		3,
		3,
		8559350182822069,
		8559350182822069,
		&[676286517104114, 159538642564, 8883914127075391, 0],
	);
	assert_outcome(
		&pt,
		3,
		3,
		8556934370234291,
		8556934370290501,
		&[674159536400221, 159228461232, 8883625605429048, 0],
	);

	assert_subset_and_shape(&v4, &pt);
	assert!(!v4.resolved[3], "the tight-limit intent stays out under v4");
	assert!(!pt.resolved[3], "and stays out under pass-through");
	assert!(v4.surplus >= pt.surplus, "the netted chain still beats three AMM legs");
}

// ---------------------------------------------------------------------------
// 2. Comparative re-runs — DCA suite
// ---------------------------------------------------------------------------

/// One DCA slot under `mode`: advance to eligibility, solve, submit on the next
/// block (as the node worker does), and report the BNC the owner received.
fn dca_slot(mode: SolverMode, label: &str, blocks: u32) -> (Solution, Balance) {
	for _ in 0..blocks {
		hydradx_run_to_next_block();
	}
	set_solver_mode(mode);
	let sol = solve_for_mode(mode).expect("solver must produce a solution");
	dump(label, &sol);
	hydradx_run_to_next_block();
	assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
		RuntimeOrigin::none(),
		sol.clone(),
	));
	let executed = match last_solution_executed() {
		pallet_ice::Event::SolutionExecuted { score, .. } => score,
		_ => unreachable!(),
	};
	(sol, executed)
}

struct DcaRun {
	/// Per slot: (resolved count, trade count, BNC credited to the owner).
	slots: Vec<(usize, usize, Balance)>,
	remaining_intents: usize,
	remaining_budget: Balance,
}

fn dump_dca_run(label: &str, run: &DcaRun) {
	println!("// === DCA RUN BEGIN: {label} ===");
	for (i, (resolved_count, trades, out)) in run.slots.iter().enumerate() {
		println!("assert_eq!(run.slots[{i}], ({resolved_count}, {trades}, {out}u128));");
	}
	println!("assert_eq!(run.remaining_intents, {});", run.remaining_intents);
	println!("assert_eq!(run.remaining_budget, {}u128);", run.remaining_budget);
	println!("// === DCA RUN END: {label} ===");
}

/// Run a single-owner DCA for `slots` periods under `mode` on a fresh snapshot.
fn run_dca_scenario(mode: SolverMode, label: &str, budget: Balance, slippage: Permill, slots: u32) -> DcaRun {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, budget * 10);

	let captured: RefCell<Option<DcaRun>> = RefCell::new(None);
	driver.execute(|| {
		enable_slip_fees();
		submit_dca(&alice, Some(budget), slippage);
		let id = intent_ids_ascending()[0];

		let mut recorded = Vec::new();
		for slot in 0..slots {
			let before = Currencies::total_balance(BNC, &alice);
			let (sol, _) = dca_slot(mode, &format!("{label}/slot{slot}"), PERIOD);
			let received = Currencies::total_balance(BNC, &alice) - before;
			recorded.push((sol.resolved_intents.len(), sol.trades.len(), received));
			assert!(received >= MIN_OUT_BNC, "{label}: tranche below the hard limit");
		}

		let remaining_intents = pallet_intent::Intents::<Runtime>::iter().count();
		let remaining_budget = if remaining_intents > 0 {
			stored_dca(id).remaining_budget
		} else {
			0
		};

		if mode == SolverMode::Passthrough {
			assert_eq!(
				Currencies::total_balance(BNC, &holding_pot()),
				0,
				"{label}: holding pot must stay untouched"
			);
			assert_eq!(
				Currencies::total_balance(BNC, &fee_receiver()),
				0,
				"{label}: fee receiver must stay untouched"
			);
		}

		let run = DcaRun {
			slots: recorded,
			remaining_intents,
			remaining_budget,
		};
		dump_dca_run(label, &run);
		*captured.borrow_mut() = Some(run);
	});

	captured.into_inner().expect("the DCA scenario must have run")
}

#[test]
fn dca_single_trade_should_execute_the_tranche_when_mode_is_passthrough() {
	let budget = 5 * TRADE_AMOUNT;
	let v4 = run_dca_scenario(SolverMode::V4, "dca_single/v4", budget, Permill::from_percent(3), 1);
	let pt = run_dca_scenario(
		SolverMode::Passthrough,
		"dca_single/passthrough",
		budget,
		Permill::from_percent(3),
		1,
	);

	assert_eq!(v4.slots, vec![(1, 1, 674325708682u128)]);
	assert_eq!(pt.slots, vec![(1, 1, 674393147996u128)]);
	assert_eq!(
		v4.remaining_budget, pt.remaining_budget,
		"one tranche is drawn in either mode"
	);
	assert_eq!(v4.remaining_budget, 4 * TRADE_AMOUNT);
	assert_eq!(v4.remaining_intents, 1);
	assert_eq!(pt.remaining_intents, 1);
}

#[test]
fn dca_multi_period_should_complete_identically_when_mode_is_passthrough() {
	let budget = 3 * TRADE_AMOUNT;
	let v4 = run_dca_scenario(SolverMode::V4, "dca_multi/v4", budget, DCA_SLIPPAGE, 3);
	let pt = run_dca_scenario(
		SolverMode::Passthrough,
		"dca_multi/passthrough",
		budget,
		DCA_SLIPPAGE,
		3,
	);

	assert_eq!(v4.remaining_intents, 0, "budget exhausted under v4");
	assert_eq!(pt.remaining_intents, 0, "budget exhausted under pass-through too");
	assert_eq!(v4.remaining_budget, 0);
	assert_eq!(pt.remaining_budget, 0);

	// One tranche per slot in both modes; the same three periods complete.
	assert_eq!(
		v4.slots,
		vec![
			(1, 1, 674325708682u128),
			(1, 1, 674325474839u128),
			(1, 1, 674325240754u128),
		]
	);
	assert_eq!(
		pt.slots,
		vec![
			(1, 1, 674393147996u128),
			(1, 1, 674392914130u128),
			(1, 1, 674392680022u128),
		]
	);
}

#[test]
fn dca_with_3_percent_slippage_should_complete_when_mode_is_passthrough() {
	let budget = 3 * TRADE_AMOUNT;
	let slippage = Permill::from_percent(3);
	let v4 = run_dca_scenario(SolverMode::V4, "dca_3pct/v4", budget, slippage, 3);
	let pt = run_dca_scenario(SolverMode::Passthrough, "dca_3pct/passthrough", budget, slippage, 3);

	assert_eq!(v4.remaining_intents, 0);
	assert_eq!(pt.remaining_intents, 0, "the tighter floor still clears at AMM prices");
	assert_eq!(
		v4.slots,
		vec![
			(1, 1, 674325708682u128),
			(1, 1, 674325474839u128),
			(1, 1, 674325240754u128),
		]
	);
	assert_eq!(
		pt.slots,
		vec![
			(1, 1, 674393147996u128),
			(1, 1, 674392914130u128),
			(1, 1, 674392680022u128),
		]
	);
}

/// Two owners, one slot each — pass-through gives each their own AMM trade where
/// v4 batches both into one.
fn run_dca_multiple_users(mode: SolverMode, label: &str) -> (usize, usize, Balance, Balance) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100);
	driver.endow_account(bob.clone(), HDX, TRADE_AMOUNT * 100);

	let captured: RefCell<Option<(usize, usize, Balance, Balance)>> = RefCell::new(None);
	driver.execute(|| {
		enable_slip_fees();
		submit_dca(&alice, Some(3 * TRADE_AMOUNT), DCA_SLIPPAGE);
		submit_dca(&bob, Some(3 * TRADE_AMOUNT), DCA_SLIPPAGE);

		let alice_before = Currencies::total_balance(BNC, &alice);
		let bob_before = Currencies::total_balance(BNC, &bob);
		let (sol, _) = dca_slot(mode, label, PERIOD);
		let alice_out = Currencies::total_balance(BNC, &alice) - alice_before;
		let bob_out = Currencies::total_balance(BNC, &bob) - bob_before;

		println!(
			"// {label}: resolved={} trades={}",
			sol.resolved_intents.len(),
			sol.trades.len()
		);
		println!("// {label}: alice_out={alice_out} bob_out={bob_out}");
		*captured.borrow_mut() = Some((sol.resolved_intents.len(), sol.trades.len(), alice_out, bob_out));
	});

	captured.into_inner().expect("the scenario must have run")
}

#[test]
fn dca_multiple_users_should_each_get_a_tranche_when_mode_is_passthrough() {
	let (v4_resolved, v4_trades, v4_alice, v4_bob) = run_dca_multiple_users(SolverMode::V4, "dca_users/v4");
	let (pt_resolved, pt_trades, pt_alice, pt_bob) =
		run_dca_multiple_users(SolverMode::Passthrough, "dca_users/passthrough");

	assert_eq!(v4_resolved, 2);
	assert_eq!(v4_trades, 1, "v4 batches both tranches into one AMM leg");
	assert_eq!(pt_resolved, 2, "both owners settle under pass-through");
	assert_eq!(pt_trades, 2, "one AMM trade each — no batching");
	assert!(v4_alice >= MIN_OUT_BNC && v4_bob >= MIN_OUT_BNC);
	assert!(pt_alice >= MIN_OUT_BNC && pt_bob >= MIN_OUT_BNC);

	assert_eq!(v4_alice, 674325474547u128);
	assert_eq!(v4_bob, 674325474547u128);
	assert_eq!(pt_alice, 674393147996u128);
	assert_eq!(pt_bob, 674392797146u128);

	// Two same-direction tranches have no coincidence of wants to net, so there
	// is no matching surplus to win — v4 only batches them into one AMM leg and
	// keeps a ~1bp margin in the pot to guarantee its claimed amounts, while
	// pass-through hands the router's actual output straight to each owner. The
	// "v4 pays at least as well" relation therefore holds only where matching
	// really happens (see `dca_should_settle_alongside_an_opposing_swap`).
	assert!(pt_alice + pt_bob > v4_alice + v4_bob);
	assert_eq!(pt_alice + pt_bob - (v4_alice + v4_bob), 134996048u128);
}

/// A DCA tranche and an opposing swap in the same batch: v4 matches them, and
/// pass-through routes both independently.
fn run_dca_with_opposing_swap(mode: SolverMode, label: &str) -> (usize, usize, Balance, Balance) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100);
	driver.endow_account(bob.clone(), BNC, TRADE_AMOUNT * 100);

	let captured: RefCell<Option<(usize, usize, Balance, Balance)>> = RefCell::new(None);
	driver.execute(|| {
		enable_slip_fees();
		submit_dca(&alice, Some(5 * TRADE_AMOUNT), DCA_SLIPPAGE);
		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}
		submit_swap(&bob, BNC, HDX, TRADE_AMOUNT, 1_000_000_000_000);

		assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);
		let alice_before = Currencies::total_balance(BNC, &alice);
		let bob_before = Currencies::total_balance(HDX, &bob);
		let (sol, _) = dca_slot(mode, label, 0);
		let alice_out = Currencies::total_balance(BNC, &alice) - alice_before;
		let bob_out = Currencies::total_balance(HDX, &bob) - bob_before;

		println!(
			"// {label}: resolved={} trades={}",
			sol.resolved_intents.len(),
			sol.trades.len()
		);
		println!("// {label}: alice_out={alice_out} bob_out={bob_out}");
		*captured.borrow_mut() = Some((sol.resolved_intents.len(), sol.trades.len(), alice_out, bob_out));
	});

	captured.into_inner().expect("the scenario must have run")
}

#[test]
fn dca_should_settle_alongside_an_opposing_swap_when_mode_is_passthrough() {
	let (v4_resolved, v4_trades, v4_alice, v4_bob) = run_dca_with_opposing_swap(SolverMode::V4, "dca_opposing/v4");
	let (pt_resolved, pt_trades, pt_alice, pt_bob) =
		run_dca_with_opposing_swap(SolverMode::Passthrough, "dca_opposing/passthrough");

	assert_eq!(v4_resolved, 2);
	assert_eq!(v4_trades, 1, "v4 matches the opposing pair down to one AMM leg");
	assert_eq!(pt_resolved, 2, "both settle under pass-through");
	assert_eq!(pt_trades, 2, "pass-through routes both legs");
	assert!(pt_alice >= MIN_OUT_BNC);
	assert!(pt_bob >= 1_000_000_000_000);
	// The two sides receive different assets, so the comparison is per owner.
	assert!(v4_alice >= pt_alice, "the matched DCA tranche must not pay worse");
	assert!(v4_bob >= pt_bob, "the matched swap must not pay worse");
}

// ---------------------------------------------------------------------------
// 3. Partial intents — full-remaining-or-skip
// ---------------------------------------------------------------------------

/// Fixture for the partial-intent tests: one BNC->HDX intent for `amount_in`
/// with limit `min_out`, injected as partial with `filled` already gone.
fn with_partial_intent(amount_in: Balance, min_out: Balance, filled: Balance, body: impl FnOnce(&AccountId, IntentId)) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), BNC, amount_in.saturating_mul(10));
	driver.execute(|| {
		enable_slip_fees();
		submit_swap(&alice, BNC, HDX, amount_in, min_out);
		let id = intent_ids_ascending()[0];
		make_partial(id, &alice, filled);
		body(&alice, id);
	});
}

#[test]
fn passthrough_should_fill_the_entire_amount_when_intent_is_freshly_partial() {
	let amount_in = 1_000 * BNC_UNIT;
	let min_out = 1_000 * HDX_UNIT;

	with_partial_intent(amount_in, min_out, 0, |alice, id| {
		assert_eq!(stored_swap(id).partial, Partial::Yes(0));
		let before = Currencies::total_balance(HDX, alice);

		let sol = run_and_submit_as::<PassthroughSolver>(SolverMode::Passthrough, "partial/fresh");

		assert_eq!(sol.resolved_intents.len(), 1);
		assert_eq!(sol.trades.len(), 1);
		assert_eq!(
			swap(resolved(&sol, id)).amount_in,
			amount_in,
			"the entire amount in one trade"
		);
		assert_eq!(swap(resolved(&sol, id)).partial, Partial::Yes(0));

		let received = Currencies::total_balance(HDX, alice) - before;
		assert!(received >= min_out);
		assert_eq!(received, 14731783072285759u128);

		assert_eq!(
			pallet_intent::Intents::<Runtime>::get(id),
			None,
			"intent leaves storage"
		);
		assert_eq!(Currencies::reserved_balance(BNC, alice), 0);
		assert!(!any_partial_resolution_event());
	});
}

#[test]
fn passthrough_should_settle_the_exact_remainder_when_intent_was_half_filled_under_v4() {
	let amount_in = 2_000 * BNC_UNIT;
	let min_out = 2_000 * HDX_UNIT;
	let filled = 1_000 * BNC_UNIT;

	with_partial_intent(amount_in, min_out, filled, |alice, id| {
		assert_eq!(Currencies::reserved_balance(BNC, alice), amount_in - filled);
		let before = Currencies::total_balance(HDX, alice);

		let sol = run_and_submit_as::<PassthroughSolver>(SolverMode::Passthrough, "partial/remainder");

		assert_eq!(sol.resolved_intents.len(), 1);
		assert_eq!(
			swap(resolved(&sol, id)).amount_in,
			amount_in - filled,
			"exactly the remainder, never less"
		);

		let received = Currencies::total_balance(HDX, alice) - before;
		// Pro-rata limit over the remainder = filled/amount_in of the original limit.
		assert!(received >= min_out / 2);
		// The same output as a fresh 1_000 BNC intent — the remainder is a full fill.
		assert_eq!(received, 14731783072285759u128);

		assert_eq!(
			pallet_intent::Intents::<Runtime>::get(id),
			None,
			"intent leaves storage"
		);
		assert_eq!(Currencies::reserved_balance(BNC, alice), 0);
		assert!(!any_partial_resolution_event());
	});
}

#[test]
fn passthrough_should_fill_the_remainder_when_the_solution_claims_a_smaller_fill() {
	let amount_in = 2_000 * BNC_UNIT;
	let min_out = 2_000 * HDX_UNIT;
	let filled = 1_000 * BNC_UNIT;

	with_partial_intent(amount_in, min_out, filled, |alice, id| {
		set_solver_mode(SolverMode::Passthrough);
		let built = solve_as::<PassthroughSolver>().expect("a solution");

		// Claim a tenth of the remainder — execution derives the amount from storage.
		let sliver = (amount_in - filled) / 10;
		let quoted = swap(&built.resolved_intents[0]).amount_out;
		let tampered = solution_of(
			vec![ResolvedIntent {
				id,
				data: IntentData::Swap(SwapData {
					asset_in: BNC,
					asset_out: HDX,
					amount_in: sliver,
					amount_out: quoted / 10,
					partial: Partial::Yes(filled),
				}),
			}],
			vec![PoolTrade {
				direction: SwapType::ExactIn,
				amount_in: sliver,
				amount_out: quoted / 10,
				route: built.trades[0].route.clone(),
			}],
			0,
		);
		// Shape validation is amount-blind, so the tampered claim still enters the pool.
		assert_ok!(validate_in_pool(tampered.clone()));

		let before = Currencies::total_balance(HDX, alice);
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			tampered,
		));

		let received = Currencies::total_balance(HDX, alice) - before;
		assert!(
			received >= min_out / 2,
			"the whole remainder was filled, not the sliver"
		);
		// Identical to the untampered run: the claim never reached execution.
		assert_eq!(received, 14731783072285759u128);
		assert_eq!(pallet_intent::Intents::<Runtime>::get(id), None);
		assert_eq!(Currencies::reserved_balance(BNC, alice), 0);
		assert!(!any_partial_resolution_event());
	});
}

/// A partial intent whose remaining amount cannot clear its own pro-rata limit,
/// alongside a loose intent that can. The rate is calibrated from a small probe
/// quote, so the remainder is unfillable purely because of AMM slippage — which
/// is exactly what a pro-rata fill of a smaller slice would escape.
struct TightPartial {
	alice: AccountId,
	bob: AccountId,
	id: IntentId,
	bob_id: IntentId,
	total: Balance,
	filled: Balance,
	remaining: Balance,
	probe: Balance,
	/// Stored `amount_out` of the injected intent (the limit over `total`).
	limit: Balance,
}

const TIGHT_TOTAL: Balance = 200_000 * BNC_UNIT;
const TIGHT_FILLED: Balance = 100_000 * BNC_UNIT;
const TIGHT_PROBE: Balance = 100 * BNC_UNIT;
const BOB_LOOSE_IN: Balance = 100 * BNC_UNIT;
const BOB_LOOSE_OUT: Balance = 100 * HDX_UNIT;

fn with_tight_partial(body: impl FnOnce(&TightPartial)) {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	// Endowments stay inside the circuit breaker's per-block issuance limit for
	// BNC — anything beyond it is locked down on deposit instead of credited.
	driver.endow_account(alice.clone(), BNC, TIGHT_TOTAL * 2);
	driver.endow_account(bob.clone(), BNC, BOB_LOOSE_IN * 2);

	driver.execute(|| {
		enable_slip_fees();
		let remaining = TIGHT_TOTAL - TIGHT_FILLED;

		// Limit rate = the small-probe rate less 1%, so a slice of `probe` clears
		// comfortably while the 1000x-larger remainder cannot.
		let probe_quote = router_quote(BNC, HDX, TIGHT_PROBE).expect("the probe size must quote");
		let limit = probe_quote * (TIGHT_TOTAL / TIGHT_PROBE) / 100 * 99;
		let remaining_quote = router_quote(BNC, HDX, remaining).unwrap_or(0);
		let pro_rata_remaining = remaining * limit / TIGHT_TOTAL;
		let pro_rata_probe = TIGHT_PROBE * limit / TIGHT_TOTAL;
		println!(
			"// tight fixture: probe_quote={probe_quote} limit={limit} remaining_quote={remaining_quote} \
			 pro_rata_remaining={pro_rata_remaining} pro_rata_probe={pro_rata_probe}"
		);
		assert!(
			remaining_quote < pro_rata_remaining,
			"fixture: the remainder must be unfillable ({remaining_quote} >= {pro_rata_remaining})"
		);
		assert!(
			probe_quote >= pro_rata_probe,
			"fixture: a probe-sized slice must be fillable ({probe_quote} < {pro_rata_probe})"
		);

		submit_swap(&alice, BNC, HDX, TIGHT_TOTAL, limit);
		submit_swap(&bob, BNC, HDX, BOB_LOOSE_IN, BOB_LOOSE_OUT);
		let ids = intent_ids_ascending();
		make_partial(ids[0], &alice, TIGHT_FILLED);

		body(&TightPartial {
			alice: alice.clone(),
			bob: bob.clone(),
			id: ids[0],
			bob_id: ids[1],
			total: TIGHT_TOTAL,
			filled: TIGHT_FILLED,
			remaining,
			probe: TIGHT_PROBE,
			limit,
		});
	});
}

#[test]
fn passthrough_should_skip_partial_intent_when_the_remainder_cannot_clear_its_pro_rata_limit() {
	with_tight_partial(|f| {
		let alice_before = Currencies::total_balance(HDX, &f.alice);
		let bob_before = Currencies::total_balance(HDX, &f.bob);

		let sol = run_and_submit_as::<PassthroughSolver>(SolverMode::Passthrough, "partial/skip");

		assert!(!is_resolved(&sol, f.id), "the unfillable remainder is not proposed");
		assert!(is_resolved(&sol, f.bob_id), "the rest of the batch still settles");
		assert_eq!(sol.resolved_intents.len(), 1);
		assert_eq!(amm_trade_count(&sol), 1);
		assert_eq!(sol.score, 1373858618274903u128);

		// State and reserve untouched — never a further partial fill.
		assert_eq!(Currencies::total_balance(HDX, &f.alice), alice_before);
		assert_eq!(Currencies::reserved_balance(BNC, &f.alice), f.remaining);
		assert_eq!(stored_swap(f.id).partial, Partial::Yes(f.filled));
		assert_eq!(stored_swap(f.id).amount_in, f.total);
		assert!(!any_partial_resolution_event());

		let bob_out = Currencies::total_balance(HDX, &f.bob) - bob_before;
		assert!(bob_out >= BOB_LOOSE_OUT);
		assert_eq!(bob_out, 1473858618274903u128);
		assert_eq!(pallet_intent::Intents::<Runtime>::get(f.bob_id), None);
	});
}

#[test]
fn v4_should_resolve_the_same_partial_intent_pro_rata_when_mode_is_strict() {
	with_tight_partial(|f| {
		// The very slice pass-through refuses to consider: a pro-rata fill that
		// clears the intent's own rate. Strict accepts it — full-remaining-or-skip
		// is a Passthrough policy, not a property of the fixture.
		let claimed_out = f.probe * f.limit / f.total;
		let sol = solution_of(
			vec![ResolvedIntent {
				id: f.id,
				data: IntentData::Swap(SwapData {
					asset_in: BNC,
					asset_out: HDX,
					amount_in: f.probe,
					amount_out: claimed_out,
					partial: Partial::Yes(f.filled),
				}),
			}],
			vec![single_sell(BNC, HDX, f.probe, claimed_out)],
			0,
		);

		let before = Currencies::total_balance(HDX, &f.alice);
		assert_eq!(pallet_ice::CurrentSolverMode::<Runtime>::get(), SolverMode::V4);
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			sol
		));

		assert_eq!(Currencies::total_balance(HDX, &f.alice) - before, claimed_out);
		assert_eq!(
			stored_swap(f.id).partial,
			Partial::Yes(f.filled + f.probe),
			"strict advanced the fill pro rata"
		);
		assert_eq!(Currencies::reserved_balance(BNC, &f.alice), f.remaining - f.probe);
		assert!(
			intent_events()
				.iter()
				.any(|e| matches!(e, pallet_intent::Event::IntentResovedPartially { .. })),
			"strict emits the partial-resolution event pass-through never can"
		);
	});
}

// ---------------------------------------------------------------------------
// 4. Mode lifecycle, forward callbacks and the validation asymmetry
// ---------------------------------------------------------------------------

#[test]
fn forward_should_fire_when_intent_resolves_in_passthrough() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100);

	driver.execute(|| {
		enable_slip_fees();
		let receiver = crate::utils::contracts::deploy_contract(
			"IntentResolutionReceiver",
			hydradx_runtime::EVMAccounts::evm_address(&alice),
		);
		let target_evm = sp_core::H160::repeat_byte(0xAA);
		let target_account = hydradx_runtime::EVMAccounts::account_id(target_evm);
		let receiver_account = hydradx_runtime::EVMAccounts::account_id(receiver);

		let mut word = [0u8; 32];
		word[12..32].copy_from_slice(target_evm.as_bytes());
		let deadline = MILLISECS_PER_BLOCK * 100 + Timestamp::now();
		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(alice.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: HDX,
					asset_out: BNC,
					amount_in: TRADE_AMOUNT,
					amount_out: MIN_OUT_BNC,
					partial: false,
				}),
				deadline: Some(deadline),
				on_resolved: Some(pallet_intent::types::OnResolved::Forward {
					contract: receiver,
					data: frame_support::BoundedVec::truncate_from(word.to_vec()),
				}),
			}
		));

		let alice_bnc_before = Currencies::total_balance(BNC, &alice);
		let target_bnc_before = Currencies::total_balance(BNC, &target_account);

		let sol = run_and_submit_as::<PassthroughSolver>(SolverMode::Passthrough, "forward/passthrough");
		assert_eq!(sol.resolved_intents.len(), 1);

		// The owner was credited the router's actual output, and the forward carries it.
		let amount_out = Currencies::total_balance(BNC, &alice) - alice_bnc_before;
		assert!(amount_out >= MIN_OUT_BNC);
		assert_eq!(amount_out, 674393147996u128);

		let stored = hydradx_runtime::LazyExecutor::call_queue(0).expect("forward queued");
		assert_eq!(stored.owner, alice);
		assert_eq!(stored.action.contract, receiver);
		assert_eq!(stored.action.asset_out, BNC);
		assert_eq!(stored.action.amount_out, amount_out);

		assert_ok!(hydradx_runtime::LazyExecutor::dispatch_top(RuntimeOrigin::none(), 0));

		assert_eq!(
			Currencies::total_balance(BNC, &target_account),
			target_bnc_before + amount_out
		);
		assert_eq!(Currencies::total_balance(BNC, &receiver_account), 0);
		assert_eq!(Currencies::total_balance(BNC, &alice), alice_bnc_before);
	});
}

#[test]
fn passthrough_should_skip_only_the_dca_tranche_when_its_oracle_floor_rises_after_the_build() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), HDX, TRADE_AMOUNT * 100);
	driver.endow_account(bob.clone(), BNC, 1_000 * BNC_UNIT);

	driver.execute(|| {
		enable_slip_fees();
		submit_dca(&alice, Some(2 * TRADE_AMOUNT), DCA_SLIPPAGE);
		submit_swap(&bob, BNC, HDX, 100 * BNC_UNIT, 100 * HDX_UNIT);
		let ids = intent_ids_ascending();
		let dca_id = ids[0];
		let swap_id = ids[1];

		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}
		assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

		set_solver_mode(SolverMode::Passthrough);
		let sol = solve_as::<PassthroughSolver>().expect("a pass-through solution");
		dump("oracle_floor/build", &sol);
		assert_eq!(sol.resolved_intents.len(), 2, "both admitted at build time");

		// Between build and execution the enforced floor rises: the oracle-derived
		// limit is `estimated_out * (1 - slippage)`, so dropping the tolerance to
		// zero lifts it to the full oracle price — above anything the AMM pays.
		pallet_intent::Intents::<Runtime>::mutate(dca_id, |maybe_intent| {
			let intent = maybe_intent.as_mut().expect("DCA intent to exist");
			let IntentData::Dca(ref mut dca) = intent.data else {
				panic!("expected a DCA intent");
			};
			dca.slippage = Permill::zero();
		});

		let alice_bnc_before = Currencies::total_balance(BNC, &alice);
		let bob_hdx_before = Currencies::total_balance(HDX, &bob);
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			sol
		));

		// The DCA tranche alone was skipped: budget, reserve and slot untouched.
		assert_eq!(Currencies::total_balance(BNC, &alice), alice_bnc_before);
		assert_eq!(Currencies::reserved_balance(HDX, &alice), 2 * TRADE_AMOUNT);
		assert_eq!(stored_dca(dca_id).remaining_budget, 2 * TRADE_AMOUNT);

		// The swap in the same solution settled.
		let bob_out = Currencies::total_balance(HDX, &bob) - bob_hdx_before;
		assert!(bob_out >= 100 * HDX_UNIT);
		assert_eq!(bob_out, 1473858618274903u128);
		assert_eq!(pallet_intent::Intents::<Runtime>::get(swap_id), None);

		match last_solution_executed() {
			pallet_ice::Event::SolutionExecuted {
				intents_executed,
				trades_executed,
				..
			} => {
				assert_eq!(intents_executed, 1);
				assert_eq!(trades_executed, 1);
			}
			_ => unreachable!(),
		}
	});
}

#[test]
fn passthrough_solution_should_fail_strict_validation_when_mode_is_v4() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), BNC, 10_000 * BNC_UNIT);
	driver.endow_account(bob.clone(), BNC, 10_000 * BNC_UNIT);

	driver.execute(|| {
		enable_slip_fees();
		// Two intents on the same pair: pass-through quotes them sequentially, so
		// they clear at two different prices — exactly what Strict forbids. Sizes
		// stay well inside the circuit breaker so the trades reach that check.
		submit_swap(&alice, BNC, HDX, 200 * BNC_UNIT, 100 * HDX_UNIT);
		submit_swap(&bob, BNC, HDX, 200 * BNC_UNIT, 100 * HDX_UNIT);

		set_solver_mode(SolverMode::Passthrough);
		let sol = solve_as::<PassthroughSolver>().expect("a pass-through solution");
		assert_eq!(sol.resolved_intents.len(), 2);
		assert_eq!(sol.trades.len(), 2);

		set_solver_mode(SolverMode::V4);
		rejected_by_pool(sol.clone());

		// Forced past the pool, execution refuses it for the second reason the
		// mode differs on: pass-through resolves at the raw quote, so the router
		// sell carries no slippage margin and misses its own `min_amount_out`.
		let err = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), sol).unwrap_err();
		assert_eq!(
			err.error,
			sp_runtime::DispatchError::from(pallet_route_executor::Error::<Runtime>::TradingLimitReached)
		);
	});
}

#[test]
fn v4_matched_solution_should_fail_shape_validation_when_mode_is_passthrough() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), BNC, 10_000 * BNC_UNIT);
	driver.endow_account(bob.clone(), HDX, 100_000 * HDX_UNIT);

	driver.execute(|| {
		enable_slip_fees();
		submit_swap(&alice, BNC, HDX, 1_000 * BNC_UNIT, 1_000 * HDX_UNIT);
		submit_swap(&bob, HDX, BNC, 10_000 * HDX_UNIT, BNC_UNIT / 2);

		let sol = solve_as::<V4Solver>().expect("a matched v4 solution");
		assert_eq!(sol.resolved_intents.len(), 2);
		assert!(
			sol.trades.len() < sol.resolved_intents.len(),
			"matching nets a leg away"
		);

		// Accepted while the mode still matches the shape it was built for.
		assert_ok!(validate_in_pool(sol.clone()));

		set_solver_mode(SolverMode::Passthrough);
		rejected_by_pool(sol);
	});
}

#[test]
fn solver_mode_lifecycle_should_disable_then_drain_then_resume_matching() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice.clone(), BNC, 10_000 * BNC_UNIT);
	driver.endow_account(bob.clone(), HDX, 100_000 * HDX_UNIT);
	driver.endow_account(charlie.clone(), HDX, 100_000 * HDX_UNIT);

	driver.execute(|| {
		enable_slip_fees();
		assert_eq!(pallet_ice::CurrentSolverMode::<Runtime>::get(), SolverMode::V4);
		// Trade on a block after the snapshot's own, so the EMA oracle has a
		// strictly older entry to integrate this block's volume into.
		hydradx_run_to_next_block();

		// --- V4: matching works ---
		submit_swap(&alice, BNC, HDX, 100 * BNC_UNIT, 100 * HDX_UNIT);
		submit_swap(&bob, HDX, BNC, 1_000 * HDX_UNIT, BNC_UNIT / 2);
		let matched = run_and_submit_as::<V4Solver>(SolverMode::V4, "lifecycle/v4");
		assert_eq!(matched.resolved_intents.len(), 2);
		assert_eq!(matched.trades.len(), 1, "the opposing pair nets down to one leg");
		assert_eq!(matched.score, 1443806026106479u128);
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 0);
		hydradx_run_to_next_block();

		// --- Disabled: nothing is accepted ---
		submit_swap(&alice, BNC, HDX, 100 * BNC_UNIT, 100 * HDX_UNIT);
		submit_swap(&bob, HDX, BNC, 1_000 * HDX_UNIT, BNC_UNIT / 2);
		let v4_shaped = solve_as::<V4Solver>().expect("v4 still builds a solution");
		set_solver_mode(SolverMode::Disabled);
		assert_eq!(pallet_ice::CurrentSolverMode::<Runtime>::get(), SolverMode::Disabled);
		rejected_by_pool(v4_shaped.clone());
		let err = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), v4_shaped.clone()).unwrap_err();
		assert_eq!(
			err.error,
			sp_runtime::DispatchError::from(pallet_ice::Error::<Runtime>::InvalidSolution)
		);
		assert!(solve_as::<V4Solver>().is_none(), "run() refuses to emit a call");

		// --- Passthrough: v4 shapes still rejected, mixed set drains ---
		set_solver_mode(SolverMode::Passthrough);
		rejected_by_pool(v4_shaped);
		submit_dca(&charlie, Some(2 * TRADE_AMOUNT), DCA_SLIPPAGE);
		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}
		assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);

		let pot_before = Currencies::total_balance(BNC, &holding_pot());
		let drained = run_and_submit_as::<PassthroughSolver>(SolverMode::Passthrough, "lifecycle/passthrough");
		assert_eq!(drained.resolved_intents.len(), 3, "the mixed swap/DCA set drains");
		assert_eq!(drained.trades.len(), 3, "one AMM trade per intent");
		assert_eq!(drained.score, 1441381957091516u128);
		assert_eq!(Currencies::total_balance(BNC, &holding_pot()), pot_before);
		// The two swaps are gone; the DCA keeps its second tranche.
		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1);
		hydradx_run_to_next_block();

		// --- Back to V4: matching resumes ---
		set_solver_mode(SolverMode::V4);
		assert_eq!(pallet_ice::CurrentSolverMode::<Runtime>::get(), SolverMode::V4);
		submit_swap(&alice, BNC, HDX, 100 * BNC_UNIT, 100 * HDX_UNIT);
		submit_swap(&bob, HDX, BNC, 1_000 * HDX_UNIT, BNC_UNIT / 2);
		let again = solve_as::<V4Solver>().expect("matching resumes");
		assert!(again.trades.len() < again.resolved_intents.len(), "netting is back");
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			again
		));
	});
}

/// Disabling the solver must also stop new intents being created.
///
/// An intent reserves the caller's funds and only settlement, expiry or cancellation
/// releases them. A DCA carries no deadline, so it never expires: accepting one while
/// nothing can settle it strands the budget until the owner cancels by hand. The
/// runtime therefore reads the ICE solver mode when accepting a submission.
#[test]
fn submit_intent_should_be_refused_while_the_solver_is_disabled() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let hollar = 222u32;
	let amount_in = 10_000 * 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount_in * 10)
		.execute(|| {
			let ts = hydradx_runtime::Timestamp::now();
			let intent = || pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: hdx,
					asset_out: hollar,
					amount_in,
					amount_out: 1_000_000_000_000_000_000u128,
					partial: false,
				}),
				deadline: Some(ts + primitives::constants::time::MILLISECS_PER_BLOCK * 100),
				on_resolved: None,
			};

			// Default mode settles, so submission is accepted.
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				intent()
			));
			let after_first = pallet_intent::Intents::<Runtime>::iter().count();

			assert_ok!(hydradx_runtime::ICE::set_solver_mode(
				RuntimeOrigin::root(),
				ice_support::SolverMode::Disabled
			));

			assert_noop!(
				hydradx_runtime::Intent::submit_intent(RuntimeOrigin::signed(alice.clone()), intent()),
				pallet_intent::Error::<Runtime>::SettlementDisabled
			);
			assert_eq!(
				pallet_intent::Intents::<Runtime>::iter().count(),
				after_first,
				"no intent may be stored while the solver is disabled"
			);

			// Re-enabling restores submission.
			assert_ok!(hydradx_runtime::ICE::set_solver_mode(
				RuntimeOrigin::root(),
				ice_support::SolverMode::V4
			));
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				intent()
			));
			assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), after_first + 1);
		});
}
