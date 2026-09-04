//! Integration coverage for the DCA -> DCA-intent migration (`ICE_DCA_MIGRATION_PLAN.md` §7.2).
//!
//! Runs against the `mainnet_apr` snapshot with the real v4 solver. Old-style schedules are created
//! through `DCA::schedule` before the flag flip; buy schedules are planted directly because the
//! extrinsic no longer accepts them.

use crate::polkadot_test_net::{hydradx_run_to_next_block, TestNet, ALICE, BOB};
use amm_simulator::HydrationSimulator;
use frame_support::traits::{Get, Time};
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::{Currencies, Runtime, RuntimeEvent, RuntimeOrigin, DCA};
use hydradx_traits::amm::{SimulatorConfig, SimulatorSet};
use hydradx_traits::registry::Inspect;
use hydradx_traits::router::{PoolType, Trade};
use ice_solver::v4::Solver as IceSolver;
use ice_support::Solution;
use orml_traits::{MultiCurrency, NamedMultiReservableCurrency};
use pallet_dca::types::{CancelReason, Order, Schedule};
use pallet_omnipool::types::SlipFeeConfig;
use primitives::constants::currency::UNITS;
use primitives::AccountId;
use sp_runtime::traits::ConstU32;
use sp_runtime::{BoundedVec, Permill};
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

const HDX: u32 = 0;
const BNC: u32 = 14;

/// 20x the snapshot's DCA transaction fee (~0.99 HDX) clears `MinTradeAmountNotReached`.
const AMOUNT_IN: u128 = 200 * UNITS;
/// `MinBudgetInNativeCurrency` is 1000 HDX, so a fixed budget has to clear that.
const BUDGET: u128 = 4_000 * UNITS;
/// A rolling schedule reserves `(amount_in + fee) * 2`, which has to clear the 1000 HDX
/// `MinBudgetInNativeCurrency`.
const ROLLING_AMOUNT_IN: u128 = 600 * UNITS;
/// Existential deposit of BNC — the floor `add_intent` enforces on `amount_out`.
const MIN_OUT_BNC: u128 = 68_795_189_840;
const PERIOD: u32 = 15;
const DCA_SLIPPAGE: Permill = Permill::from_percent(10);

const DCA_RESERVE_ID: [u8; 8] = *b"dcaorder";
const INTENT_RESERVE_ID: [u8; 8] = *b"ICE_int#";

type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;
type Solver = IceSolver<HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>>;
type DcaSchedule = Schedule<AccountId, u32, u32>;

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
		|intents: Vec<ice_support::Intent>,
		 limits: Vec<(ice_support::IntentId, ice_support::Balance)>,
		 state: CombinedSimulatorState| {
			Solver::solve_with_limits(
				intents,
				limits.into_iter().collect(),
				state,
				pallet_ice::ProtocolFee::<Runtime>::get(),
			)
			.ok()
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

/// `hydradx_run_to_block` trips the Aura consensus hook on a snapshot; only
/// `hydradx_run_to_next_block` clears `CurrentSlot` between blocks.
fn run_to(target: u32) {
	while hydradx_runtime::System::block_number() < target {
		hydradx_run_to_next_block();
	}
}

fn advance_and_solve(n: u32) -> Solution {
	for _ in 0..n {
		hydradx_run_to_next_block();
	}
	run_solver_and_submit()
}

fn route(asset_in: u32, asset_out: u32) -> BoundedVec<Trade<u32>, ConstU32<9>> {
	BoundedVec::truncate_from(vec![Trade {
		pool: PoolType::Omnipool,
		asset_in,
		asset_out,
	}])
}

fn sell_schedule(owner: AccountId, total_amount: u128, amount_in: u128, min_amount_out: u128) -> DcaSchedule {
	Schedule {
		owner,
		period: PERIOD,
		total_amount,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(DCA_SLIPPAGE),
		order: Order::Sell {
			asset_in: HDX,
			asset_out: BNC,
			amount_in,
			min_amount_out,
			route: route(HDX, BNC),
		},
	}
}

fn default_sell_schedule(owner: AccountId) -> DcaSchedule {
	sell_schedule(owner, BUDGET, AMOUNT_IN, MIN_OUT_BNC)
}

fn buy_schedule(owner: AccountId) -> DcaSchedule {
	Schedule {
		owner,
		period: PERIOD,
		total_amount: BUDGET,
		max_retries: None,
		stability_threshold: None,
		slippage: Some(DCA_SLIPPAGE),
		order: Order::Buy {
			asset_in: HDX,
			asset_out: BNC,
			amount_out: MIN_OUT_BNC * 10,
			max_amount_in: AMOUNT_IN,
			route: route(HDX, BNC),
		},
	}
}

/// Creates an old-style schedule through the extrinsic; returns its id and planned block.
fn schedule_dca(who: AccountId, schedule: DcaSchedule) -> (u32, u32) {
	let id = DCA::next_schedule_id();
	assert_ok!(DCA::schedule(RuntimeOrigin::signed(who), schedule, None));
	let block = pallet_dca::ScheduleExecutionBlock::<Runtime>::get(id).expect("schedule must be planned");
	(id, block)
}

/// Writes the state the `schedule` extrinsic would write. Needed for buy orders, which the
/// extrinsic rejects (`NoLongerSupported`) but which still exist on chain from before that change.
fn plant_schedule(schedule: DcaSchedule, execution_block: u32) -> u32 {
	let owner = schedule.owner.clone();
	let asset_in = schedule.order.get_asset_in();
	let reserve = if schedule.is_rolling() {
		let amount_in = match schedule.order {
			Order::Sell { amount_in, .. } => amount_in,
			Order::Buy { max_amount_in, .. } => max_amount_in,
		};
		let fee = DCA::get_transaction_fee(&schedule.order, None).unwrap();
		(amount_in + fee) * 2
	} else {
		schedule.total_amount
	};

	let id = pallet_dca::ScheduleIdSequencer::<Runtime>::mutate(|current| {
		let id = *current;
		*current += 1;
		id
	});

	pallet_dca::Schedules::<Runtime>::insert(id, &schedule);
	pallet_dca::ScheduleOwnership::<Runtime>::insert(&owner, id, ());
	pallet_dca::RemainingAmounts::<Runtime>::insert(id, reserve);
	pallet_dca::RetriesOnError::<Runtime>::insert(id, 0);
	pallet_dca::ScheduleExecutionBlock::<Runtime>::insert(id, execution_block);
	pallet_dca::ScheduleIdsPerBlock::<Runtime>::mutate(execution_block, |ids| {
		ids.try_push(id).expect("execution block is full");
	});

	assert_ok!(Currencies::reserve_named(&DCA_RESERVE_ID, asset_in, &owner, reserve));

	id
}

fn enable_migration() {
	assert_ok!(DCA::set_migration_enabled(RuntimeOrigin::root(), true));
}

/// The snapshot carries the real chain's events, and the test harness's `on_initialize` does not
/// clear them, so every event assertion has to be scoped to what happened after this mark.
fn event_mark() -> usize {
	frame_system::Pallet::<Runtime>::events().len()
}

fn dca_events_since(mark: usize) -> Vec<pallet_dca::Event<Runtime>> {
	frame_system::Pallet::<Runtime>::events()
		.into_iter()
		.skip(mark)
		.map(|record| record.event)
		.filter_map(|e| match e {
			RuntimeEvent::DCA(inner) => Some(inner),
			_ => None,
		})
		.collect()
}

/// The intent a schedule became, taken from its `Migrated` event - the only record kept of the
/// id change.
fn migrated_intent_id(schedule_id: u32) -> u128 {
	find_migrated_intent_id(schedule_id).expect("schedule should have been migrated")
}

fn find_migrated_intent_id(schedule_id: u32) -> Option<u128> {
	dca_events_since(0).into_iter().find_map(|e| match e {
		pallet_dca::Event::Migrated { id, intent_id, .. } if id == schedule_id => Some(intent_id),
		_ => None,
	})
}

fn dca_data(intent_id: u128) -> ice_support::DcaData {
	match pallet_intent::Intents::<Runtime>::get(intent_id)
		.expect("intent should exist")
		.data
	{
		ice_support::IntentData::Dca(dca) => dca,
		_ => panic!("expected a DCA intent"),
	}
}

fn assert_schedule_gone(owner: &AccountId, id: u32) {
	assert!(pallet_dca::Schedules::<Runtime>::get(id).is_none(), "Schedules");
	assert!(
		pallet_dca::ScheduleOwnership::<Runtime>::get(owner, id).is_none(),
		"ScheduleOwnership"
	);
	assert!(
		pallet_dca::RemainingAmounts::<Runtime>::get(id).is_none(),
		"RemainingAmounts"
	);
	assert!(
		pallet_dca::ScheduleExecutionBlock::<Runtime>::get(id).is_none(),
		"ScheduleExecutionBlock"
	);
	assert_eq!(pallet_dca::RetriesOnError::<Runtime>::get(id), 0, "RetriesOnError");
	assert_eq!(pallet_dca::ScheduleExtraGas::<Runtime>::get(id), 0, "ScheduleExtraGas");
}

fn driver_with_funded_alice() -> crate::driver::HydrationTestDriver {
	let alice: AccountId = ALICE.into();
	let driver = crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT);
	driver.endow_account(alice, HDX, 10 * BUDGET);
	driver
}

// === A. Conversion across pool topologies ===

#[test]
fn migration_should_convert_and_fill_when_pair_routes_through_omnipool() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();

		run_to(block);
		let intent_id = migrated_intent_id(id);
		println!("TEMP protocol fee {:?}", pallet_ice::ProtocolFee::<Runtime>::get());

		let bnc_before = Currencies::total_balance(BNC, &alice);
		let solution = advance_and_solve(PERIOD);

		assert_eq!(solution.resolved_intents.len(), 1);
		assert_eq!(solution.resolved_intents[0].id, intent_id);
		assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
		assert_eq!(dca_data(intent_id).remaining_budget, BUDGET - AMOUNT_IN);
	});
}

#[test]
fn migration_should_drop_stored_route_when_schedule_carries_one() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		// A route through a pool that is not the one the solver would pick still converts: the
		// intent carries no route at all, the solver rediscovers it.
		let mut schedule = default_sell_schedule(alice.clone());
		if let Order::Sell { ref mut route, .. } = schedule.order {
			*route = BoundedVec::truncate_from(vec![]);
		}

		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		let bnc_before = Currencies::total_balance(BNC, &alice);
		let solution = advance_and_solve(PERIOD);
		assert_eq!(solution.resolved_intents[0].id, intent_id);
		assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
	});
}

#[test]
fn migration_should_convert_and_fill_when_pair_routes_through_stableswap() {
	use amm_simulator::stableswap::Simulator as StableswapSimulator;
	use hydradx_runtime::{ice_simulator_provider, AssetRegistry, Router};
	use hydradx_traits::amm::AmmSimulator;
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use hydradx_traits::BoundErc20;

	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		// Same selection as `ice::dca::dca_through_stableswap_single_hop`: a pool whose first two
		// assets are non-contract and both routable to HDX.
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
		let Some((asset_in, asset_out, decimals_in, decimals_out)) = selected else {
			println!("SKIPPED: no suitable stableswap pool in snapshot");
			return;
		};

		let amount_in = 100 * 10u128.pow(decimals_in as u32);
		let min_out = 10u128.pow(decimals_out as u32);
		let budget = 100 * amount_in;

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			alice.clone(),
			asset_in,
			(budget * 10) as i128,
		));

		let schedule = Schedule {
			owner: alice.clone(),
			period: PERIOD,
			total_amount: budget,
			max_retries: None,
			stability_threshold: None,
			slippage: Some(DCA_SLIPPAGE),
			order: Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_amount_out: min_out,
				route: BoundedVec::truncate_from(vec![]),
			},
		};

		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		assert_eq!(dca_data(intent_id).asset_out, asset_out);

		let out_before = Currencies::total_balance(asset_out, &alice);
		advance_and_solve(PERIOD);
		assert!(
			Currencies::total_balance(asset_out, &alice) > out_before,
			"migrated stableswap DCA should fill"
		);
	});
}

#[test]
fn migration_should_convert_and_fill_when_pair_is_aave() {
	use amm_simulator::aave::Simulator as AaveSimulator;
	use hydradx_runtime::ice_simulator_provider;
	use hydradx_traits::amm::AmmSimulator;

	TestNet::reset();
	let alice: AccountId = ALICE.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let aave_snapshot = AaveSimulator::<ice_simulator_provider::Aave<Runtime>>::snapshot();
		let picked = aave_snapshot.pairs.iter().find_map(|(a, b)| {
			let ed_in = <pallet_asset_registry::Pallet<Runtime> as Inspect>::existential_deposit(*a)?;
			let ed_out = <pallet_asset_registry::Pallet<Runtime> as Inspect>::existential_deposit(*b)?;
			Some((*a, *b, ed_in, ed_out))
		});
		let Some((asset_in, asset_out, ed_in, ed_out)) = picked else {
			println!("SKIPPED: snapshot has no aave pairs");
			return;
		};

		let amount_in = ed_in.saturating_mul(1_000);
		let budget = 100 * amount_in;

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			alice.clone(),
			asset_in,
			(budget * 10) as i128,
		));

		let schedule = Schedule {
			owner: alice.clone(),
			period: PERIOD,
			total_amount: budget,
			max_retries: None,
			stability_threshold: None,
			slippage: Some(DCA_SLIPPAGE),
			order: Order::Sell {
				asset_in,
				asset_out,
				amount_in,
				min_amount_out: ed_out,
				route: BoundedVec::truncate_from(vec![]),
			},
		};

		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		assert_eq!(dca_data(intent_id).asset_in, asset_in);
		assert_eq!(dca_data(intent_id).asset_out, asset_out);

		let out_before = Currencies::total_balance(asset_out, &alice);
		advance_and_solve(PERIOD);
		assert!(
			Currencies::total_balance(asset_out, &alice) > out_before,
			"migrated aave DCA should fill"
		);
	});
}

// === B. Timing and cadence ===

#[test]
fn migration_should_not_execute_trade_when_conversion_happens() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();

		let hdx_free_before = Currencies::free_balance(HDX, &alice);
		let bnc_before = Currencies::total_balance(BNC, &alice);

		let mark = event_mark();
		run_to(block);

		// No trade, no transaction fee, no execution events.
		assert_eq!(Currencies::total_balance(BNC, &alice), bnc_before);
		assert_eq!(Currencies::free_balance(HDX, &alice), hdx_free_before);
		let events = dca_events_since(mark);
		assert!(
			!events.iter().any(|e| matches!(
				e,
				pallet_dca::Event::TradeExecuted { .. }
					| pallet_dca::Event::TradeFailed { .. }
					| pallet_dca::Event::ExecutionStarted { .. }
					| pallet_dca::Event::ExecutionPlanned { .. }
			)),
			"no execution events at the conversion block, got {events:?}"
		);
		assert!(events.iter().any(|e| matches!(e, pallet_dca::Event::Migrated { .. })));
		assert!(find_migrated_intent_id(id).is_some());
	});
}

#[test]
fn migrated_intent_should_first_fill_one_period_after_conversion() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		assert_eq!(dca_data(intent_id).last_execution_block, block);

		// Not eligible until a full period has passed.
		for _ in 0..PERIOD - 1 {
			assert!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().is_empty(),
				"eligible too early at block {}",
				hydradx_runtime::System::block_number()
			);
			hydradx_run_to_next_block();
		}

		hydradx_run_to_next_block();
		assert_eq!(
			pallet_intent::Pallet::<Runtime>::get_valid_intents().len(),
			1,
			"eligible exactly one period after conversion"
		);
	});
}

#[test]
fn migrated_intent_should_keep_period_cadence_over_three_fills() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		run_to(block);
		let intent_id = migrated_intent_id(id);

		let mut expected_remaining = BUDGET;
		for _ in 0..3 {
			advance_and_solve(PERIOD);
			expected_remaining -= AMOUNT_IN;
			assert_eq!(dca_data(intent_id).remaining_budget, expected_remaining);
		}
	});
}

#[test]
fn schedule_should_not_trade_between_flag_flip_and_its_planned_block() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let mut schedule = default_sell_schedule(alice.clone());
		schedule.period = 100;
		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();

		let bnc_before = Currencies::total_balance(BNC, &alice);
		run_to(block - 1);

		// Still on the old pallet, still untouched.
		assert!(pallet_dca::Schedules::<Runtime>::get(id).is_some());
		assert_eq!(Currencies::total_balance(BNC, &alice), bnc_before);

		run_to(block);
		assert_schedule_gone(&alice, id);
		assert_eq!(Currencies::total_balance(BNC, &alice), bnc_before);
	});
}

#[test]
fn migration_should_convert_when_price_stability_check_would_have_failed() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		// A zero threshold makes `prepare_schedule` reject any price movement. The migration
		// branch runs before that check, so conversion must not care.
		let mut schedule = default_sell_schedule(alice.clone());
		schedule.stability_threshold = Some(Permill::zero());

		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();
		run_to(block);

		assert!(find_migrated_intent_id(id).is_some());
	});
}

#[test]
fn migration_should_convert_when_schedule_has_pending_retries() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		pallet_dca::RetriesOnError::<Runtime>::insert(id, 2);
		enable_migration();
		run_to(block);

		assert!(find_migrated_intent_id(id).is_some());
		assert_schedule_gone(&alice, id);
	});
}

// === C. Cancellation ===

#[test]
fn migration_should_cancel_and_refund_when_order_is_buy() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let block = hydradx_runtime::System::block_number() + 2;
		let id = plant_schedule(buy_schedule(alice.clone()), block);
		let free_before = Currencies::free_balance(HDX, &alice);
		enable_migration();

		let mark = event_mark();
		run_to(block);

		assert_eq!(Currencies::free_balance(HDX, &alice), free_before + BUDGET);
		assert_eq!(Currencies::reserved_balance_named(&DCA_RESERVE_ID, HDX, &alice), 0);
		assert_schedule_gone(&alice, id);
		assert!(find_migrated_intent_id(id).is_none());
		assert!(dca_events_since(mark).iter().any(|e| matches!(
			e,
			pallet_dca::Event::MigrationCancelled {
				reason: CancelReason::BuyOrder,
				..
			}
		)));
	});
}

#[test]
fn migration_should_cancel_when_remaining_budget_is_below_one_trade() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		// Simulate a nearly-spent schedule: less than one trade left.
		let leftover = AMOUNT_IN / 2;
		pallet_dca::RemainingAmounts::<Runtime>::insert(id, leftover);
		let free_before = Currencies::free_balance(HDX, &alice);
		enable_migration();

		let mark = event_mark();
		run_to(block);

		assert_eq!(Currencies::free_balance(HDX, &alice), free_before + leftover);
		assert_schedule_gone(&alice, id);
		assert!(dca_events_since(mark).iter().any(|e| matches!(
			e,
			pallet_dca::Event::MigrationCancelled {
				reason: CancelReason::BudgetBelowTrade,
				..
			}
		)));
	});
}

// === D. Reserve conservation ===

#[test]
fn migration_should_move_reserve_exactly_when_budget_is_fixed() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		let free_before = Currencies::free_balance(HDX, &alice);
		enable_migration();

		run_to(block);

		assert_eq!(Currencies::reserved_balance_named(&DCA_RESERVE_ID, HDX, &alice), 0);
		assert_eq!(
			Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice),
			BUDGET
		);
		assert_eq!(Currencies::free_balance(HDX, &alice), free_before);
		assert_eq!(dca_data(migrated_intent_id(id)).remaining_budget, BUDGET);
	});
}

#[test]
fn migration_should_release_excess_reserve_when_schedule_is_rolling() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(
			alice.clone(),
			sell_schedule(alice.clone(), 0, ROLLING_AMOUNT_IN, MIN_OUT_BNC),
		);
		let dca_reserve = pallet_dca::RemainingAmounts::<Runtime>::get(id).unwrap();
		let free_before = Currencies::free_balance(HDX, &alice);
		enable_migration();

		run_to(block);

		// The intent pallet reserves 2x the trade amount for a rolling DCA; the old pallet also
		// reserved the transaction fee, and that difference goes back to the owner.
		let intent_reserve = ROLLING_AMOUNT_IN * 2;
		assert_eq!(
			Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice),
			intent_reserve
		);
		assert_eq!(Currencies::reserved_balance_named(&DCA_RESERVE_ID, HDX, &alice), 0);
		assert_eq!(
			Currencies::free_balance(HDX, &alice),
			free_before + dca_reserve - intent_reserve
		);
		assert_eq!(dca_data(migrated_intent_id(id)).budget, None);
	});
}

#[test]
fn conservation_should_hold_when_mixed_batch_migrates_and_cancels() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let block = hydradx_runtime::System::block_number() + 2;
		let sell_id = plant_schedule(default_sell_schedule(alice.clone()), block);
		let buy_id = plant_schedule(buy_schedule(alice.clone()), block);

		let free_before = Currencies::free_balance(HDX, &alice);
		let dca_before = Currencies::reserved_balance_named(&DCA_RESERVE_ID, HDX, &alice);
		let intent_before = Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice);

		enable_migration();
		run_to(block);

		let free_after = Currencies::free_balance(HDX, &alice);
		let dca_after = Currencies::reserved_balance_named(&DCA_RESERVE_ID, HDX, &alice);
		let intent_after = Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice);

		assert_eq!(
			free_before + dca_before + intent_before,
			free_after + dca_after + intent_after,
			"nothing created or destroyed"
		);
		assert!(find_migrated_intent_id(sell_id).is_some());
		assert!(find_migrated_intent_id(buy_id).is_none());
	});
}

// === E. Interaction with the rest of the runtime ===

#[test]
fn solution_should_include_migrated_dca_and_organic_intent_together() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let driver = driver_with_funded_alice();
	driver.endow_account(bob.clone(), HDX, 10 * BUDGET);
	driver.execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		run_to(block);
		let migrated = migrated_intent_id(id);

		// Bob submits a plain swap intent that becomes eligible immediately.
		for _ in 0..PERIOD {
			hydradx_run_to_next_block();
		}
		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(bob.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: HDX,
					asset_out: BNC,
					amount_in: AMOUNT_IN,
					amount_out: MIN_OUT_BNC,
					partial: false,
				}),
				deadline: Some(
					<hydradx_runtime::Runtime as pallet_intent::Config>::TimestampProvider::now() + 1_000_000,
				),
				on_resolved: None,
			}
		));

		let solution = run_solver_and_submit();
		assert_eq!(solution.resolved_intents.len(), 2, "both intents in one solution");
		assert!(solution.resolved_intents.iter().any(|r| r.id == migrated));
	});
}

#[test]
fn migration_should_handle_a_full_block_of_schedules() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let block = hydradx_runtime::System::block_number() + 2;
		let max = <Runtime as pallet_dca::Config>::MaxSchedulePerBlock::get();

		let mut ids = vec![];
		for i in 0..max {
			// Mix converts and cancels in the same block.
			let schedule = if i % 2 == 0 {
				default_sell_schedule(alice.clone())
			} else {
				buy_schedule(alice.clone())
			};
			ids.push((i % 2 == 0, plant_schedule(schedule, block)));
		}

		enable_migration();
		run_to(block);

		for (should_migrate, id) in ids {
			assert_schedule_gone(&alice, id);
			assert_eq!(find_migrated_intent_id(id).is_some(), should_migrate, "schedule {id}");
		}
		assert!(pallet_dca::ScheduleIdsPerBlock::<Runtime>::get(block).is_empty());
	});
}

#[test]
fn migration_should_bypass_cap_when_owner_is_at_max_intents_per_account() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));

		// Pretend the owner already holds the maximum number of intents.
		let cap: u32 = <Runtime as pallet_intent::Config>::MaxIntentsPerAccount::get();
		pallet_intent::AccountIntentCount::<Runtime>::insert(&alice, cap);

		enable_migration();
		run_to(block);

		assert!(find_migrated_intent_id(id).is_some(), "cap must not block a migration");
	});
}

#[test]
fn schedule_should_fail_when_migration_is_enabled() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();
		enable_migration();

		assert_noop!(
			DCA::schedule(
				RuntimeOrigin::signed(alice.clone()),
				default_sell_schedule(alice.clone()),
				None
			),
			pallet_dca::Error::<Runtime>::MigrationInProgress
		);
	});
}

#[test]
fn terminate_should_refund_normally_when_called_after_flag_flip() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, _) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		let free_before = Currencies::free_balance(HDX, &alice);
		enable_migration();

		assert_ok!(DCA::terminate(RuntimeOrigin::signed(alice.clone()), id, None));

		assert_eq!(Currencies::free_balance(HDX, &alice), free_before + BUDGET);
		assert_schedule_gone(&alice, id);
	});
}

// === F. Termination and drain ===

#[test]
fn all_schedules_should_be_gone_when_the_longest_period_has_elapsed() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let start = hydradx_runtime::System::block_number();
		let longest_period = 40u32;

		let mut sells = vec![];
		for period in [PERIOD, 20, longest_period] {
			let mut schedule = default_sell_schedule(alice.clone());
			schedule.period = period;
			let (id, _) = schedule_dca(alice.clone(), schedule);
			sells.push(id);
		}
		let buy_id = plant_schedule(buy_schedule(alice.clone()), start + longest_period);

		enable_migration();
		run_to(start + longest_period + 1);

		assert_eq!(
			pallet_dca::Schedules::<Runtime>::iter().count(),
			0,
			"every schedule resolves within one longest-period window"
		);
		for id in sells {
			assert!(find_migrated_intent_id(id).is_some());
		}
		assert!(find_migrated_intent_id(buy_id).is_none());
	});
}

#[test]
fn schedule_should_be_migrated_exactly_once_when_slot_is_visited() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();

		let mark = event_mark();
		run_to(block + 3 * PERIOD);

		assert_eq!(pallet_intent::Intents::<Runtime>::iter().count(), 1, "one intent only");
		assert_eq!(
			dca_events_since(mark)
				.iter()
				.filter(|e| matches!(e, pallet_dca::Event::Migrated { .. }))
				.count(),
			1
		);
		assert!(find_migrated_intent_id(id).is_some());
	});
}

#[test]
fn force_cancel_should_remove_schedule_when_it_has_no_planned_block() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		// Orphan it: reachable in `Schedules` but never visited by `on_initialize`.
		pallet_dca::ScheduleIdsPerBlock::<Runtime>::remove(block);
		pallet_dca::ScheduleExecutionBlock::<Runtime>::remove(id);

		enable_migration();

		let mark = event_mark();
		run_to(block + 1);
		assert!(
			pallet_dca::Schedules::<Runtime>::get(id).is_some(),
			"orphan survives the drain"
		);

		let free_before = Currencies::free_balance(HDX, &alice);
		assert_ok!(DCA::force_cancel_schedules(RuntimeOrigin::root(), vec![id]));

		assert_eq!(Currencies::free_balance(HDX, &alice), free_before + BUDGET);
		assert_schedule_gone(&alice, id);
		assert!(dca_events_since(mark).iter().any(|e| matches!(
			e,
			pallet_dca::Event::MigrationCancelled {
				reason: CancelReason::ForceCancelled,
				..
			}
		)));
	});
}

#[test]
fn old_execution_should_resume_when_flag_is_disabled_again() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		assert_ok!(DCA::set_migration_enabled(RuntimeOrigin::root(), false));

		let bnc_before = Currencies::total_balance(BNC, &alice);
		run_to(block);

		// Flag off: the schedule traded on the old path and replanned.
		assert!(find_migrated_intent_id(id).is_none());
		assert!(Currencies::total_balance(BNC, &alice) > bnc_before);
		let next_block = pallet_dca::ScheduleExecutionBlock::<Runtime>::get(id).expect("replanned");

		// Flag back on: the next slot converts it.
		enable_migration();
		run_to(next_block);
		assert!(find_migrated_intent_id(id).is_some());
	});
}

// === G. Events and the indexer contract ===

#[test]
fn migrated_event_should_carry_schedule_and_intent_id() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();

		let mark = event_mark();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		let migrated = dca_events_since(mark)
			.into_iter()
			.find_map(|e| match e {
				pallet_dca::Event::Migrated {
					id: schedule_id,
					who,
					intent_id,
				} => Some((schedule_id, who, intent_id)),
				_ => None,
			})
			.expect("Migrated event");

		assert_eq!(migrated, (id, alice.clone(), intent_id));
		assert!(pallet_intent::Intents::<Runtime>::get(intent_id).is_some());
	});
}

#[test]
fn cancelled_event_should_carry_reason_and_refund_amount() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let block = hydradx_runtime::System::block_number() + 2;
		let id = plant_schedule(buy_schedule(alice.clone()), block);
		enable_migration();

		let mark = event_mark();
		run_to(block);

		let cancelled = dca_events_since(mark)
			.into_iter()
			.find_map(|e| match e {
				pallet_dca::Event::MigrationCancelled {
					id,
					who,
					asset,
					refunded,
					reason,
				} => Some((id, who, asset, refunded, reason)),
				_ => None,
			})
			.expect("MigrationCancelled event");

		assert_eq!(cancelled, (id, alice.clone(), HDX, BUDGET, CancelReason::BuyOrder));
	});
}

// === H. Post-migration intent semantics ===

#[test]
fn migrated_intent_should_carry_original_slippage_and_limits() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let slippage = Permill::from_percent(7);
		let mut schedule = default_sell_schedule(alice.clone());
		schedule.slippage = Some(slippage);

		let (id, block) = schedule_dca(alice.clone(), schedule);
		enable_migration();
		run_to(block);

		let dca = dca_data(migrated_intent_id(id));
		assert_eq!(dca.slippage, slippage);
		assert_eq!(dca.asset_in, HDX);
		assert_eq!(dca.asset_out, BNC);
		assert_eq!(dca.amount_in, AMOUNT_IN);
		assert_eq!(dca.amount_out, MIN_OUT_BNC);
		assert_eq!(dca.period, PERIOD);
		assert_eq!(dca.budget, Some(BUDGET));
		assert_eq!(dca.remaining_budget, BUDGET);
		assert_eq!(dca.last_execution_block, block);
	});
}

#[test]
fn migrated_intent_should_clamp_amount_out_to_existential_deposit_when_schedule_limit_is_dust() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		// Old-DCA users routinely leave `min_amount_out` at dust; the conversion lifts it to the ED
		// rather than cancelling the schedule.
		let ed_out = <pallet_asset_registry::Pallet<Runtime> as Inspect>::existential_deposit(BNC).unwrap();
		let (id, block) = schedule_dca(alice.clone(), sell_schedule(alice.clone(), BUDGET, AMOUNT_IN, 1));
		enable_migration();
		run_to(block);

		let intent_id = migrated_intent_id(id);
		assert_eq!(dca_data(intent_id).amount_out, ed_out);

		let bnc_before = Currencies::total_balance(BNC, &alice);
		let solution = advance_and_solve(PERIOD);
		assert_eq!(solution.resolved_intents.len(), 1);
		assert_eq!(solution.resolved_intents[0].id, intent_id);
		assert_eq!(dca_data(intent_id).remaining_budget, BUDGET - AMOUNT_IN);

		// The clamped hard limit is not what bound the fill - the oracle floor is far above it, and
		// the trade cleared that floor.
		let floor = pallet_intent::Pallet::<Runtime>::compute_dca_effective_limit(&dca_data(intent_id));
		let received = Currencies::total_balance(BNC, &alice) - bnc_before;
		assert!(
			floor > ed_out,
			"oracle floor {floor} should dominate the ED clamp {ed_out}"
		);
		assert!(
			received >= floor,
			"fill {received} paid less than the oracle floor {floor}"
		);
	});
}

#[test]
fn migrated_intent_should_complete_when_budget_is_below_one_trade() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		// Two trades' worth of budget, so the intent closes after the second fill.
		let budget = 2 * AMOUNT_IN;
		let (id, block) = schedule_dca(
			alice.clone(),
			sell_schedule(alice.clone(), budget.max(1_000 * UNITS), AMOUNT_IN, MIN_OUT_BNC),
		);
		enable_migration();
		run_to(block);
		let intent_id = migrated_intent_id(id);

		let budget = dca_data(intent_id).remaining_budget;
		let fills = (budget / AMOUNT_IN) as u32;
		for _ in 0..fills {
			advance_and_solve(PERIOD);
		}

		let leftover = budget - u128::from(fills) * AMOUNT_IN;
		assert!(leftover < AMOUNT_IN);
		assert!(
			pallet_intent::Intents::<Runtime>::get(intent_id).is_none(),
			"intent closes once the budget cannot fund another trade"
		);
		assert_eq!(
			Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice),
			0,
			"reserve fully released"
		);
	});
}

#[test]
fn dormant_intent_should_refund_when_owner_removes_it() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		run_to(block);
		let intent_id = migrated_intent_id(id);

		let free_before = Currencies::free_balance(HDX, &alice);
		assert_ok!(hydradx_runtime::Intent::remove_intent(
			RuntimeOrigin::signed(alice.clone()),
			intent_id
		));

		assert_eq!(Currencies::free_balance(HDX, &alice), free_before + BUDGET);
		assert_eq!(Currencies::reserved_balance_named(&INTENT_RESERVE_ID, HDX, &alice), 0);
	});
}

// === I. Old path vs. migrated path ===

/// The outcome of running one schedule to `n` fills, measured on whichever path produced it.
#[derive(Default, Debug, Clone, Copy)]
struct Fills {
	first_block: u32,
	last_block: u32,
	amount_out: u128,
	/// What the fills took out of the schedule's budget.
	budget_spent: u128,
	/// What they cost the owner overall - budget plus anything the path charged elsewhere.
	hdx_spent: u128,
}

impl Fills {
	/// BNC received per HDX the owner actually parted with, scaled to survive integer division.
	fn price(&self) -> u128 {
		self.amount_out.saturating_mul(UNITS) / self.hdx_spent
	}
}

/// Runs the schedule on the old DCA path until it has executed `count` times, first at `first_block`.
///
/// The schedule is planted rather than scheduled so the caller can line the first execution up with
/// the migrated path's first fill, putting both against the same pool state.
fn old_dca_fills(first_block: u32, count: u32) -> Fills {
	let alice: AccountId = ALICE.into();
	let mut fills = Fills::default();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let id = plant_schedule(default_sell_schedule(alice.clone()), first_block);
		let bnc_before = Currencies::total_balance(BNC, &alice);
		let hdx_before = Currencies::total_balance(HDX, &alice);

		run_to(first_block + (count - 1) * PERIOD);
		assert_eq!(
			pallet_dca::ScheduleExecutionBlock::<Runtime>::get(id),
			Some(first_block + count * PERIOD),
			"schedule should have executed exactly {count} times"
		);

		fills = Fills {
			first_block,
			last_block: hydradx_runtime::System::block_number(),
			amount_out: Currencies::total_balance(BNC, &alice) - bnc_before,
			budget_spent: BUDGET - pallet_dca::RemainingAmounts::<Runtime>::get(id).unwrap_or_default(),
			hdx_spent: hdx_before - Currencies::total_balance(HDX, &alice),
		};
	});

	fills
}

/// Migrates the same schedule and lets the solver fill it `count` times.
fn migrated_intent_fills(count: u32) -> Fills {
	let alice: AccountId = ALICE.into();
	let mut fills = Fills::default();

	driver_with_funded_alice().execute(|| {
		enable_slip_fees();

		let (id, block) = schedule_dca(alice.clone(), default_sell_schedule(alice.clone()));
		enable_migration();
		run_to(block);
		let intent_id = migrated_intent_id(id);

		let bnc_before = Currencies::total_balance(BNC, &alice);
		let hdx_before = Currencies::total_balance(HDX, &alice);

		let mut first_block = 0;
		for fill in 0..count {
			advance_and_solve(PERIOD);
			if fill == 0 {
				first_block = hydradx_runtime::System::block_number();
			}
		}

		fills = Fills {
			first_block,
			last_block: hydradx_runtime::System::block_number(),
			amount_out: Currencies::total_balance(BNC, &alice) - bnc_before,
			budget_spent: BUDGET - dca_data(intent_id).remaining_budget,
			hdx_spent: hdx_before - Currencies::total_balance(HDX, &alice),
		};
	});

	fills
}

/// Asserts the two paths bought within `tolerance_bps` of each other and that the migrated one is
/// not the worse deal per HDX spent.
fn assert_comparable(old: Fills, new: Fills, tolerance_bps: u128) {
	println!("old {old:?} price {}", old.price());
	println!("new {new:?} price {}", new.price());

	let gap = old.amount_out.abs_diff(new.amount_out);
	assert!(
		gap.saturating_mul(10_000) <= old.amount_out.saturating_mul(tolerance_bps),
		"amount out differs by more than {tolerance_bps} bps: old {} vs new {}",
		old.amount_out,
		new.amount_out
	);

	assert!(
		new.price() >= old.price(),
		"migrated fill priced worse: old {} vs new {}",
		old.price(),
		new.price()
	);
}

/// The same sell, executed at the same height against the same pool state, on both paths.
#[test]
fn migrated_intent_should_fill_like_old_dca_on_a_single_execution() {
	TestNet::reset();

	let new = migrated_intent_fills(1);
	let old = old_dca_fills(new.first_block, 1);
	assert_eq!(
		old.last_block, new.last_block,
		"comparison is only fair at equal height"
	);

	// The solver executes marginally better than the router, then hands back 0.02% as the ICE
	// protocol fee, so the output lands about a basis point under the old path's.
	assert_comparable(old, new, 10);

	// The old path takes a per-execution transaction fee out of the budget; the intent path
	// charges nothing beyond the trade itself.
	assert_eq!(new.budget_spent, AMOUNT_IN);
	assert_eq!(new.hdx_spent, AMOUNT_IN);
	assert!(old.hdx_spent > AMOUNT_IN, "old path takes an execution fee");
}

/// Three fills on each path. Only the first is height-aligned - the solver needs a block of its
/// own per fill - so this checks that the gap stays flat rather than compounding.
#[test]
fn migrated_intent_should_fill_like_old_dca_over_three_executions() {
	TestNet::reset();

	let new = migrated_intent_fills(3);
	let old = old_dca_fills(new.first_block, 3);

	assert_comparable(old, new, 10);
	assert_eq!(new.budget_spent, 3 * AMOUNT_IN);
	assert!(old.budget_spent > 3 * AMOUNT_IN);
}
