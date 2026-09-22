#![cfg(test)]

//! ICE routing through XYK pools.
//!
//! The Uniswap snapshot is reused for its chain state only. The pool under test
//! is created here from a freshly registered asset paired with HOLLAR, so the
//! new asset has exactly one venue and any route the solver finds for it has to
//! go through XYK.
//!
//! XYK is opt-in, so creating the pool is not enough — `register_pool` has to add
//! it to the routing registry before the solver can see it at all.

use crate::ice::harness::TestSimulator;
use crate::polkadot_test_net::*;
use crate::uniswap_v3_router::{reset_consensus_slots, PATH_TO_SNAPSHOT};
use frame_support::assert_ok;
use frame_support::storage::{with_transaction, TransactionOutcome};
use hydradx_runtime::{AssetRegistry, Currencies, HydrationSimulators, Router, RuntimeOrigin, Treasury, XYK};
use hydradx_traits::amm::{AMMInterface, SimulatorError, SimulatorSet};
use hydradx_traits::registry::{AssetKind, Create};
use hydradx_traits::router::{PoolType, Route, Trade};
use hydradx_traits::AMM;
use ice_support::RoutingState;
use ice_support::RoutingTarget;
use orml_traits::MultiCurrency;
use primitives::{AccountId, AssetId, Balance};

/// HOLLAR. `Erc20`-kind, so it cannot be minted — it comes out of the treasury.
const HOLLAR: AssetId = 222;
/// The gas token on Hydration's EVM, needed before ALICE can touch an ERC20.
const GAS_ASSET: AssetId = 20;
const FUND_GAS: Balance = 1_000_000_000_000_000_000;

/// Both sides are 18 decimals, so the pool prices the new asset at 0.5 HOLLAR.
const POOL_ASSET: Balance = 100_000_000_000_000_000_000;
const POOL_HOLLAR: Balance = 50_000_000_000_000_000_000;
const MINT_ASSET: Balance = 1_000_000_000_000_000_000_000;
/// Inside the treasury's holding in this snapshot, and enough to seed the pool
/// and still trade against it.
const FUND_HOLLAR: Balance = 100_000_000_000_000_000_000;
/// 1 unit of the new asset — 1% of the pool, well under `MaxInRatio`.
const SELL_AMOUNT: Balance = 1_000_000_000_000_000_000;

/// Load the snapshot, register an asset nothing else trades, and pair it with
/// HOLLAR in a new XYK pool.
fn with_xyk_pool(execution: impl FnOnce(AssetId)) {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_consensus_slots();

		let asset = with_transaction(|| {
			TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
				None,
				Some(b"ICE XYK".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1,
				Some(b"IXYK".to_vec().try_into().unwrap()),
				Some(18),
				None,
				None,
			))
		})
		.expect("asset should register");

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(ALICE),
			GAS_ASSET,
			FUND_GAS as i128,
		));
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(ALICE),
			asset,
			MINT_ASSET as i128,
		));

		let treasury = Treasury::account_id();
		let held = Currencies::free_balance(HOLLAR, &treasury);
		assert!(held >= FUND_HOLLAR, "treasury holds {held} HOLLAR, need {FUND_HOLLAR}");
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(treasury),
			AccountId::from(ALICE).into(),
			HOLLAR,
			FUND_HOLLAR,
		));

		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset,
			POOL_ASSET,
			HOLLAR,
			POOL_HOLLAR,
		));
		register_pool(asset);

		execution(asset);
	});
}

/// Opt the pool into the solver's routing. Without this the pool exists on chain
/// but is invisible to ICE — hundreds of permissionless XYK pools never trade, so
/// the solver loads only the ones governance has asked for.
fn register_pool(asset: AssetId) {
	assert_ok!(hydradx_runtime::ICE::update_routing(
		RuntimeOrigin::root(),
		RoutingTarget::XykPool(asset, HOLLAR),
		Some(RoutingState::Included),
	));
}

fn xyk_snapshot() -> amm_simulator::xyk::Snapshot {
	<HydrationSimulators as SimulatorSet>::initial_state().4
}

fn key(asset: AssetId) -> (AssetId, AssetId) {
	if asset < HOLLAR {
		(asset, HOLLAR)
	} else {
		(HOLLAR, asset)
	}
}

fn reserves(asset: AssetId) -> (Balance, Balance) {
	let pair = XYK::get_pair_id(pallet_xyk::types::AssetPair {
		asset_in: asset,
		asset_out: HOLLAR,
	});
	(
		Currencies::free_balance(asset, &pair),
		Currencies::free_balance(HOLLAR, &pair),
	)
}

fn xyk_route(asset_in: AssetId, asset_out: AssetId) -> Vec<Trade<AssetId>> {
	vec![Trade {
		pool: PoolType::XYK,
		asset_in,
		asset_out,
	}]
}

fn simulate_sell(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
) -> Result<hydradx_traits::amm::TradeResult, SimulatorError> {
	<HydrationSimulators as SimulatorSet>::simulate_sell(
		PoolType::XYK,
		asset_in,
		asset_out,
		amount_in,
		0,
		&<HydrationSimulators as SimulatorSet>::initial_state(),
	)
	.map(|(_, result)| result)
}

/// The default. A pool that exists on chain but was never registered is not
/// loaded, does not price, and does not appear as an edge.
#[test]
fn solver_snapshot_should_ignore_a_pool_that_was_never_registered() {
	with_xyk_pool(|asset| {
		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(asset, HOLLAR),
			Some(RoutingState::Excluded),
		));

		assert!(xyk_snapshot().pools.is_empty(), "no pool should be loaded");
		assert!(!pool_edges_contain_xyk());
	});
}

/// Registering is what makes the pool visible, and it is the only thing that does.
#[test]
fn solver_snapshot_should_load_only_registered_pools() {
	with_xyk_pool(|asset| {
		assert_eq!(xyk_snapshot().pools.len(), 1, "only the registered pool");
		assert!(xyk_snapshot().pools.contains_key(&key(asset)));
	});
}

fn pool_edges_contain_xyk() -> bool {
	<HydrationSimulators as SimulatorSet>::pool_edges(&<HydrationSimulators as SimulatorSet>::initial_state())
		.iter()
		.any(|edge| edge.pool_type == PoolType::XYK)
}

#[test]
fn solver_snapshot_should_carry_the_pool_reserves() {
	with_xyk_pool(|asset| {
		let snapshot = xyk_snapshot();
		let (reserve_a, reserve_b) = *snapshot.pools.get(&key(asset)).expect("pool should be sampled");

		let expected = if key(asset).0 == asset {
			(POOL_ASSET, POOL_HOLLAR)
		} else {
			(POOL_HOLLAR, POOL_ASSET)
		};
		assert_eq!((reserve_a, reserve_b), expected);
	});
}

#[test]
fn pool_edges_should_expose_the_pair_to_route_discovery() {
	with_xyk_pool(|asset| {
		let edges =
			<HydrationSimulators as SimulatorSet>::pool_edges(&<HydrationSimulators as SimulatorSet>::initial_state());

		assert!(
			edges
				.iter()
				.any(|edge| edge.pool_type == PoolType::XYK && edge.assets == vec![key(asset).0, key(asset).1]),
			"xyk pair missing from pool edges"
		);
	});
}

/// Unlike the sampled v3 curve, XYK is the same formula on both sides, so the
/// simulated output must equal the executed one exactly.
#[test]
fn simulated_sell_should_equal_the_executed_sell() {
	with_xyk_pool(|asset| {
		let simulated = simulate_sell(asset, HOLLAR, SELL_AMOUNT).expect("simulation should succeed");

		let before = Currencies::free_balance(HOLLAR, &AccountId::from(ALICE));
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			asset,
			HOLLAR,
			SELL_AMOUNT,
			0,
			xyk_route(asset, HOLLAR).try_into().unwrap(),
		));
		let executed = Currencies::free_balance(HOLLAR, &AccountId::from(ALICE)) - before;

		assert_eq!(simulated.amount_out, executed);
	});
}

#[test]
fn simulated_buy_should_equal_the_executed_buy() {
	with_xyk_pool(|asset| {
		let buy_amount = SELL_AMOUNT / 4;
		let (_, simulated) = <HydrationSimulators as SimulatorSet>::simulate_buy(
			PoolType::XYK,
			asset,
			HOLLAR,
			buy_amount,
			Balance::MAX,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("simulation should succeed");

		let before = Currencies::free_balance(asset, &AccountId::from(ALICE));
		assert_ok!(Router::buy(
			RuntimeOrigin::signed(ALICE.into()),
			asset,
			HOLLAR,
			buy_amount,
			Balance::MAX,
			xyk_route(asset, HOLLAR).try_into().unwrap(),
		));
		let executed = before - Currencies::free_balance(asset, &AccountId::from(ALICE));

		assert_eq!(simulated.amount_in, executed);
	});
}

/// The fee never leaves the pool, so a sell leaves more behind than the constant
/// product alone would.
#[test]
fn simulated_sell_should_leave_the_snapshot_matching_the_executed_reserves() {
	with_xyk_pool(|asset| {
		let (state, _) = <HydrationSimulators as SimulatorSet>::simulate_sell(
			PoolType::XYK,
			asset,
			HOLLAR,
			SELL_AMOUNT,
			0,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("simulation should succeed");

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			asset,
			HOLLAR,
			SELL_AMOUNT,
			0,
			xyk_route(asset, HOLLAR).try_into().unwrap(),
		));

		let simulated = *state.4.pools.get(&key(asset)).expect("pool in the simulated state");
		let (on_chain_asset, on_chain_hollar) = reserves(asset);
		let expected = if key(asset).0 == asset {
			(on_chain_asset, on_chain_hollar)
		} else {
			(on_chain_hollar, on_chain_asset)
		};

		assert_eq!(simulated, expected);
	});
}

/// A second leg the same way has to see the first leg's price impact.
#[test]
fn simulated_sell_should_yield_less_when_a_second_leg_runs_the_same_direction() {
	with_xyk_pool(|asset| {
		let (state, first) = <HydrationSimulators as SimulatorSet>::simulate_sell(
			PoolType::XYK,
			asset,
			HOLLAR,
			SELL_AMOUNT,
			0,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("first leg should succeed");

		let (_, second) =
			<HydrationSimulators as SimulatorSet>::simulate_sell(PoolType::XYK, asset, HOLLAR, SELL_AMOUNT, 0, &state)
				.expect("second leg should succeed");

		assert!(
			second.amount_out < first.amount_out,
			"second leg {} should be worse than the first {}",
			second.amount_out,
			first.amount_out
		);
	});
}

#[test]
fn simulated_sell_should_fail_when_amount_exceeds_the_max_in_ratio() {
	with_xyk_pool(|asset| {
		assert_eq!(
			simulate_sell(asset, HOLLAR, POOL_ASSET / 3 + 1),
			Err(SimulatorError::TradeTooLarge)
		);
	});
}

/// Excluding the pair takes the pool out of the snapshot, so route discovery
/// has nothing left to reach the new asset with.
#[test]
fn excluding_an_xyk_pool_should_remove_it_from_the_pool_edges() {
	with_xyk_pool(|asset| {
		assert!(xyk_snapshot().pools.contains_key(&key(asset)));

		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(asset, HOLLAR),
			Some(RoutingState::Excluded),
		));

		assert!(!xyk_snapshot().pools.contains_key(&key(asset)));
		assert_eq!(
			simulate_sell(asset, HOLLAR, SELL_AMOUNT),
			Err(SimulatorError::AssetNotFound)
		);
	});
}

/// Registering the exclusion the other way round must hit the same entry — the
/// pair is normalized on write and on read.
#[test]
fn excluding_an_xyk_pool_should_ignore_the_order_of_the_pair() {
	with_xyk_pool(|asset| {
		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(HOLLAR, asset),
			Some(RoutingState::Excluded),
		));

		assert!(!xyk_snapshot().pools.contains_key(&key(asset)));
	});
}

/// End to end: the new asset has no other venue, so a solved intent on the pair
/// can only have routed through XYK.
#[test]
fn intent_should_resolve_and_settle_when_solution_routes_through_xyk() {
	with_xyk_pool(|asset| {
		let quoted = simulate_sell(asset, HOLLAR, SELL_AMOUNT)
			.expect("simulation should succeed")
			.amount_out;
		let min_out = quoted * 9 / 10;

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: asset,
					asset_out: HOLLAR,
					amount_in: SELL_AMOUNT,
					amount_out: min_out,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let router = Router::router_account();
		let router_before = (
			Currencies::free_balance(asset, &router),
			Currencies::free_balance(HOLLAR, &router),
		);
		let before = Currencies::free_balance(HOLLAR, &AccountId::from(ALICE));

		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"xyk_intent",
		);

		let after = Currencies::free_balance(HOLLAR, &AccountId::from(ALICE));

		assert_eq!(solution.resolved_intents.len(), 1);
		assert!(
			solution
				.trades
				.iter()
				.any(|trade| trade.route.iter().any(|hop| hop.pool == PoolType::XYK)),
			"solution should route through the xyk pool"
		);
		assert!(
			after - before >= min_out,
			"intent owner should have been paid at least the limit: {} < {min_out}",
			after - before
		);
		assert_eq!(
			(
				Currencies::free_balance(asset, &router),
				Currencies::free_balance(HOLLAR, &router),
			),
			router_before,
			"the router holds nothing after settlement"
		);
	});
}

/// With the pair excluded there is no venue left for the new asset, so the same
/// intent cannot be solved at all.
#[test]
fn intent_should_not_resolve_when_the_xyk_pool_is_excluded() {
	with_xyk_pool(|asset| {
		let quoted = simulate_sell(asset, HOLLAR, SELL_AMOUNT)
			.expect("simulation should succeed")
			.amount_out;

		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(asset, HOLLAR),
			Some(RoutingState::Excluded),
		));

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: asset,
					asset_out: HOLLAR,
					amount_in: SELL_AMOUNT,
					amount_out: quoted * 9 / 10,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		crate::ice::harness::set_solver_mode(ice_support::SolverMode::V4);
		assert_eq!(
			crate::ice::harness::solve_as::<crate::ice::harness::V4Solver>(),
			None,
			"an excluded pool leaves the pair unroutable"
		);
	});
}

// ---------------------------------------------------------------------------
// Multi-hop
// ---------------------------------------------------------------------------

/// 100 HDX — small against every venue on the path, so price impact cannot be
/// what decides whether the route is usable.
const HDX_SELL_AMOUNT: Balance = 100_000_000_000_000;

/// The route the solver's own discovery would use for `asset_in -> asset_out`,
/// together with what selling `amount_in` along it is worth.
fn best_route(asset_in: AssetId, asset_out: AssetId, amount_in: Balance) -> (Route<AssetId>, Balance) {
	let state = <HydrationSimulators as SimulatorSet>::initial_state();
	let route = TestSimulator::discover_routes(asset_in, asset_out, &state)
		.expect("a route should exist")
		.into_iter()
		.next()
		.expect("at least one route");
	let (_, execution) =
		TestSimulator::sell(asset_in, asset_out, amount_in, route.clone(), &state).expect("the route should price");
	(route, execution.amount_out)
}

/// Every XYK hop the solution routes through.
fn xyk_hops(solution: &ice_support::Solution) -> Vec<Trade<AssetId>> {
	solution
		.trades
		.iter()
		.flat_map(|trade| trade.route.iter())
		.filter(|hop| hop.pool == PoolType::XYK)
		.cloned()
		.collect()
}

fn longest_route(solution: &ice_support::Solution) -> usize {
	solution.trades.iter().map(|trade| trade.route.len()).max().unwrap_or(0)
}

/// The new asset can only leave its pool through HOLLAR, so anything beyond
/// HOLLAR is a second hop on a trusted venue — XYK as the *first* leg.
#[test]
fn intent_should_resolve_through_a_multi_hop_route_starting_with_an_xyk_leg() {
	with_xyk_pool(|asset| {
		let (route, expected_out) = best_route(asset, HDX, SELL_AMOUNT);
		assert!(route.len() >= 2, "expected a multi-hop route, got {route:?}");
		assert_eq!(route[0].pool, PoolType::XYK, "the first hop should be the xyk pool");

		let min_out = expected_out * 9 / 10;
		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: asset,
					asset_out: HDX,
					amount_in: SELL_AMOUNT,
					amount_out: min_out,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let before = Currencies::free_balance(HDX, &AccountId::from(ALICE));
		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"xyk_multi_hop_sell",
		);
		let after = Currencies::free_balance(HDX, &AccountId::from(ALICE));

		assert_eq!(solution.resolved_intents.len(), 1);
		assert!(
			longest_route(&solution) >= 2,
			"the settled route should be multi-hop: {:?}",
			solution.trades
		);

		// Exactly one XYK hop, and it is the one carrying the new asset. Two would
		// mean the solution went in and back out of the same pool.
		let hops = xyk_hops(&solution);
		assert_eq!(hops.len(), 1, "expected one xyk hop, got {hops:?}");
		assert_eq!((hops[0].asset_in, hops[0].asset_out), (asset, HOLLAR));

		assert!(
			after - before >= min_out,
			"intent owner should have been paid at least the limit: {} < {min_out}",
			after - before
		);
	});
}

/// The mirror: buying the new asset can only end in its pool, so XYK is the
/// *last* leg of a route that starts on a trusted venue.
#[test]
fn intent_should_resolve_through_a_multi_hop_route_ending_with_an_xyk_leg() {
	with_xyk_pool(|asset| {
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			AccountId::from(ALICE),
			HDX,
			(HDX_SELL_AMOUNT * 10) as i128,
		));

		let (route, expected_out) = best_route(HDX, asset, HDX_SELL_AMOUNT);
		assert!(route.len() >= 2, "expected a multi-hop route, got {route:?}");
		assert_eq!(
			route[route.len() - 1].pool,
			PoolType::XYK,
			"the last hop should be the xyk pool"
		);

		let min_out = expected_out * 9 / 10;
		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: HDX,
					asset_out: asset,
					amount_in: HDX_SELL_AMOUNT,
					amount_out: min_out,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let before = Currencies::free_balance(asset, &AccountId::from(ALICE));
		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"xyk_multi_hop_buy",
		);
		let after = Currencies::free_balance(asset, &AccountId::from(ALICE));

		assert_eq!(solution.resolved_intents.len(), 1);
		assert!(
			longest_route(&solution) >= 2,
			"the settled route should be multi-hop: {:?}",
			solution.trades
		);

		let hops = xyk_hops(&solution);
		assert_eq!(hops.len(), 1, "expected one xyk hop, got {hops:?}");
		assert_eq!((hops[0].asset_in, hops[0].asset_out), (HOLLAR, asset));

		assert!(
			after - before >= min_out,
			"intent owner should have been paid at least the limit: {} < {min_out}",
			after - before
		);
	});
}

/// Excluding the pool takes the new asset off the graph entirely, so even the
/// multi-hop route stops existing.
#[test]
fn multi_hop_route_should_disappear_when_the_xyk_pool_is_excluded() {
	with_xyk_pool(|asset| {
		best_route(asset, HDX, SELL_AMOUNT);

		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::XykPool(asset, HOLLAR),
			Some(RoutingState::Excluded),
		));

		let state = <HydrationSimulators as SimulatorSet>::initial_state();
		assert_eq!(
			TestSimulator::discover_routes(asset, HDX, &state),
			Err(SimulatorError::NotSupported)
		);
	});
}

/// What the solver actually loads on a mainnet snapshot, by venue. Run manually
/// when interpreting `ice-solver-bench`'s `simulator_initial_state`.
#[test]
#[ignore]
fn probe_pool_counts() {
	for path in [
		crate::ice::PATH_TO_SNAPSHOT,
		PATH_TO_SNAPSHOT,
		"snapshots/ice/mainnet_sep",
	] {
		TestNet::reset();
		hydra_live_ext(path).execute_with(|| {
			reset_consensus_slots();

			let state = <HydrationSimulators as SimulatorSet>::initial_state();
			println!("--- {path}");
			println!("omnipool assets   {}", state.0.assets.len());
			println!("stableswap pools  {}", state.1.pools.len());
			println!("aave pairs        {}", state.2.pairs.len());
			println!("uniswap v3 pools  {}", state.3.pools.len());
			println!("xyk pools         {}", state.4.pools.len());
			println!(
				"pool edges        {}",
				<HydrationSimulators as SimulatorSet>::pool_edges(&state).len()
			);
			let mut omnipool: Vec<_> = state.0.assets.keys().copied().collect();
			omnipool.sort();
			println!("omnipool ids      {omnipool:?}");
		});
	}
}
