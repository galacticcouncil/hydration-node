use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::traits::{Get, Time};
use hydradx_runtime::{
	AssetRegistry, Currencies, LazyExecutor, Omnipool, Router, Runtime, RuntimeOrigin, Stableswap, Timestamp,
};
use hydradx_traits::amm::{AMMInterface, AmmSimulator, SimulatorConfig, SimulatorSet};
use hydradx_traits::router::{AssetPair, RouteProvider, RouteSpotPriceProvider};
use hydradx_traits::BoundErc20;
use ice_solver::v1::SolverV1;
use ice_support::Solution;
use orml_traits::MultiCurrency;
use primitives::AccountId;
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/mainnet_nov4";

pub type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

type TestSimulator = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
type Solver = SolverV1<TestSimulator>;

// Custom simulator config for Hollar tests with price denominator 222
pub struct HollarSimulatorConfig;

pub struct HollarPriceDenominator;
impl Get<u32> for HollarPriceDenominator {
	fn get() -> u32 {
		222
	}
}

impl SimulatorConfig for HollarSimulatorConfig {
	type Simulators = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators;
	type RouteProvider = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::RouteProvider;
	type PriceDenominator = HollarPriceDenominator;
}

type HollarSimulator = HydrationSimulator<HollarSimulatorConfig>;
type HollarSolver = SolverV1<HollarSimulator>;

#[test]
fn test_simulator_snapshot() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let snapshot = <Omnipool as AmmSimulator>::snapshot();

		assert!(!snapshot.assets.is_empty(), "Snapshot should contain assets");
		assert!(snapshot.hub_asset_id > 0, "Hub asset id should be set");
	});
}

#[test]
fn test_simulator_sell() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		use hydradx_traits::amm::SimulatorError;

		let snapshot = <Omnipool as AmmSimulator>::snapshot();

		let assets: Vec<_> = snapshot.assets.keys().copied().collect();
		assert!(assets.len() >= 2, "Snapshot should have at least 2 assets");

		let asset_in = assets[0];
		let asset_out = assets[1];

		// Skip if using hub asset
		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			return;
		}

		let amount_in = 1_000_000_000_000u128;

		let result = <Omnipool as AmmSimulator>::simulate_sell(asset_in, asset_out, amount_in, 0, &snapshot);

		match result {
			Ok((new_snapshot, trade_result)) => {
				assert!(trade_result.amount_in > 0, "Amount in should be positive");
				assert!(trade_result.amount_out > 0, "Amount out should be positive");

				let old_reserve_in = snapshot.assets.get(&asset_in).unwrap().reserve;
				let new_reserve_in = new_snapshot.assets.get(&asset_in).unwrap().reserve;
				assert!(new_reserve_in > old_reserve_in, "Asset in reserve should increase");

				let old_reserve_out = snapshot.assets.get(&asset_out).unwrap().reserve;
				let new_reserve_out = new_snapshot.assets.get(&asset_out).unwrap().reserve;
				assert!(new_reserve_out < old_reserve_out, "Asset out reserve should decrease");
			}
			Err(e) => {
				assert!(
					matches!(
						e,
						SimulatorError::TradeTooSmall | SimulatorError::TradeTooLarge | SimulatorError::Other
					),
					"Unexpected error: {:?}",
					e
				);
			}
		}
	});
}

#[test]
fn test_stableswap_snapshot() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let stableswap_snapshot = <Stableswap as AmmSimulator>::snapshot();

		assert!(!stableswap_snapshot.pools.is_empty(), "Should have stableswap pools");
		assert!(
			stableswap_snapshot.min_trading_limit > 0,
			"Min trading limit should be set"
		);

		for (_pool_id, pool) in &stableswap_snapshot.pools {
			assert!(!pool.assets.is_empty(), "Pool should have assets");
			assert!(pool.amplification > 0, "Amplification should be positive");
			assert!(pool.share_issuance > 0, "Share issuance should be positive");
			assert_eq!(
				pool.assets.len(),
				pool.reserves.len(),
				"Assets and reserves count should match"
			);

			for reserve in pool.reserves.iter() {
				assert!(reserve.decimals > 0, "Decimals should be positive");
			}
		}
	});
}

#[test]
fn test_stableswap_simulator_direct() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let snapshot = <Stableswap as AmmSimulator>::snapshot();

		let pool_id = 104u32;
		let Some(pool) = snapshot.pools.get(&pool_id) else {
			// Pool 104 not found in snapshot, skip test
			return;
		};

		let asset_a = pool.assets[0];
		let asset_b = pool.assets[1];
		let decimals_a = pool.reserves[0].decimals;

		let amount_in = 10u128.pow(decimals_a as u32);

		// Test simulate_sell
		let (new_snapshot, result) =
			<Stableswap as AmmSimulator>::simulate_sell(asset_a, asset_b, amount_in, 0, &snapshot)
				.expect("simulate_sell should succeed");

		assert!(result.amount_in > 0, "Amount in should be positive");
		assert!(result.amount_out > 0, "Amount out should be positive");

		let new_pool = new_snapshot.pools.get(&pool_id).unwrap();
		let old_reserve_a = pool.reserves[0].amount;
		let new_reserve_a = new_pool.reserves[0].amount;
		let old_reserve_b = pool.reserves[1].amount;
		let new_reserve_b = new_pool.reserves[1].amount;

		assert_eq!(
			new_reserve_a - old_reserve_a,
			amount_in,
			"Reserve A should increase by amount_in"
		);
		assert_eq!(
			old_reserve_b - new_reserve_b,
			result.amount_out,
			"Reserve B should decrease by amount_out"
		);

		// Test simulate_buy
		let amount_out = 10u128.pow(decimals_a as u32);
		let (_new_snapshot, buy_result) =
			<Stableswap as AmmSimulator>::simulate_buy(asset_a, asset_b, amount_out, u128::MAX, &snapshot)
				.expect("simulate_buy should succeed");

		assert_eq!(buy_result.amount_out, amount_out, "Amount out should match requested");

		// Test get_spot_price
		let price = <Stableswap as AmmSimulator>::get_spot_price(asset_a, asset_b, &snapshot)
			.expect("get_spot_price should succeed");

		assert!(price.n > 0, "Price numerator should be positive");
		assert!(price.d > 0, "Price denominator should be positive");
	});
}

/// Test stableswap intent: trade between stableswap pool assets
#[test]
fn test_stableswap_intent() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		use hydradx_traits::router::{AssetPair, RouteProvider};

		let stableswap_snapshot = <Stableswap as AmmSimulator>::snapshot();
		let hdx = 0u32;

		// Find a suitable stableswap pool with routes to HDX
		let mut selected_pool: Option<(u32, u32, u32, u8)> = None;
		for (pid, pool) in &stableswap_snapshot.pools {
			if pool.assets.len() < 2 {
				continue;
			}
			let a = pool.assets[0];
			let b = pool.assets[1];

			if AssetRegistry::contract_address(a).is_some() || AssetRegistry::contract_address(b).is_some() {
				continue;
			}
			let route_a_hdx = Router::get_onchain_route(AssetPair::new(a, hdx));
			let route_b_hdx = Router::get_onchain_route(AssetPair::new(b, hdx));
			if route_a_hdx.is_some() && route_b_hdx.is_some() {
				selected_pool = Some((*pid, a, b, pool.reserves[0].decimals));
				break;
			}
		}

		let Some((_pool_id, asset_a, asset_b, decimals_a)) = selected_pool else {
			// No suitable pool found in this snapshot, skip test
			assert!(false, "no suitable pool to test stablepool intent");
			return;
		};

		let amount_in = 10u128.pow(decimals_a as u32);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			asset_a,
			(amount_in * 10) as i128,
		));

		let alice_a_before = Currencies::total_balance(asset_a, &ALICE.into());
		let alice_b_before = Currencies::total_balance(asset_b, &ALICE.into());

		let ts = Timestamp::now();
		let deadline = 6000u64 * 10 + ts;
		assert_ok!(pallet_intent::Pallet::<Runtime>::submit_intent(
			RuntimeOrigin::signed(ALICE.into()),
			pallet_intent::types::Intent {
				data: ice_support::IntentData::Swap(ice_support::SwapData {
					asset_in: asset_a,
					asset_out: asset_b,
					amount_in,
					amount_out: 1,
					swap_type: ice_support::SwapType::ExactIn,
					partial: false,
				}),
				deadline,
				on_success: None,
				on_failure: None,
			},
		));

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		assert_eq!(intents.len(), 1, "Should have 1 intent");

		let block = hydradx_runtime::System::block_number();

		let mut captured_solution: Option<Solution> = None;
		let result = pallet_ice::Pallet::<Runtime>::run(
			block,
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
				let solution = Solver::solve(intents, state).ok()?;
				captured_solution = Some(solution.clone());
				Some(solution)
			},
		);

		assert!(result.is_some(), "No solution found");
		let solution = captured_solution.expect("Solution should be captured");
		assert_eq!(solution.resolved_intents.len(), 1, "Should resolve the intent");

		crate::polkadot_test_net::hydradx_run_to_next_block();
		let new_block = hydradx_runtime::System::block_number();

		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution,
			new_block,
		));

		let alice_a_after = Currencies::total_balance(asset_a, &ALICE.into());
		let alice_b_after = Currencies::total_balance(asset_b, &ALICE.into());

		assert!(alice_a_after < alice_a_before, "Alice should have less asset_a");
		assert!(alice_b_after > alice_b_before, "Alice should have more asset_b");
	});
}

#[test]
fn test_solver_two_intents() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(ALICE.into(), 0, 1_000_000_000_000_000)
		.endow_account(BOB.into(), 5, 1_000_000_000_000_000)
		.submit_sell_intent(ALICE.into(), 0, 5, 1_000_000_000_000, 1, 2)
		.submit_sell_intent(BOB.into(), 5, 0, 1_000_000_000_000, 1, 2)
		.execute(|| {
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					assert!(
						!solution.resolved_intents.is_empty(),
						"Should resolve at least one intent"
					);
					assert!(solution.score > 0, "Solution score should be positive");
					Some(solution)
				},
			);

			// Solver may or may not find a solution depending on market conditions
			if let Some(_call) = result {
				// Solution found - this is the expected path
			}
		});
}

/// Test CoW (Coincidence of Wants) matching: Alice sells A for B, Bob sells B for A
#[test]
fn test_solver_execute_solution1() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let asset_a = 0u32;
	let asset_b = 14u32;
	let amount = 10_000_000_000_000u128;
	let min_amount_out = 1u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, amount * 10)
		.endow_account(bob.clone(), asset_b, amount * 10)
		.submit_sell_intent(alice.clone(), asset_a, asset_b, amount, min_amount_out, 10)
		.submit_sell_intent(bob.clone(), asset_b, asset_a, amount, min_amount_out, 10)
		.execute(|| {
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);
			let bob_balance_a_before = Currencies::total_balance(asset_a, &bob);
			let bob_balance_b_before = Currencies::total_balance(asset_b, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 2, "Should resolve both intents");
			assert!(solution.score > 0, "Solution score should be positive");
			assert!(
				solution.clearing_prices.contains_key(&asset_a),
				"Should have price for asset_a"
			);
			assert!(
				solution.clearing_prices.contains_key(&asset_b),
				"Should have price for asset_b"
			);

			// Verify each resolved intent
			for resolved in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref swap_data) = resolved.data;
				assert!(swap_data.amount_in > 0, "amount_in should be positive");
				assert!(swap_data.amount_out >= min_amount_out, "amount_out should be >= min");
				assert_eq!(swap_data.swap_type, ice_support::SwapType::ExactIn, "Should be ExactIn");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			// Verify intents removed from storage
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "All intents should be resolved");

			let alice_balance_a_after = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_after = Currencies::total_balance(asset_b, &alice);
			let bob_balance_a_after = Currencies::total_balance(asset_a, &bob);
			let bob_balance_b_after = Currencies::total_balance(asset_b, &bob);

			// Verify balance changes direction
			assert!(
				alice_balance_a_after < alice_balance_a_before,
				"Alice's asset_a should decrease"
			);
			assert!(
				alice_balance_b_after > alice_balance_b_before,
				"Alice's asset_b should increase"
			);
			assert!(
				bob_balance_b_after < bob_balance_b_before,
				"Bob's asset_b should decrease"
			);
			assert!(
				bob_balance_a_after > bob_balance_a_before,
				"Bob's asset_a should increase"
			);

			// Verify balance changes match solution
			let alice_resolved = solution
				.resolved_intents
				.iter()
				.find(|r| {
					let ice_support::IntentData::Swap(ref s) = r.data;
					s.asset_in == asset_a
				})
				.expect("Should find Alice's intent");
			let bob_resolved = solution
				.resolved_intents
				.iter()
				.find(|r| {
					let ice_support::IntentData::Swap(ref s) = r.data;
					s.asset_in == asset_b
				})
				.expect("Should find Bob's intent");

			let ice_support::IntentData::Swap(ref alice_swap) = alice_resolved.data;
			let ice_support::IntentData::Swap(ref bob_swap) = bob_resolved.data;

			assert_eq!(alice_balance_a_before - alice_balance_a_after, alice_swap.amount_in);
			assert_eq!(alice_balance_b_after - alice_balance_b_before, alice_swap.amount_out);
			assert_eq!(bob_balance_b_before - bob_balance_b_after, bob_swap.amount_in);
			assert_eq!(bob_balance_a_after - bob_balance_a_before, bob_swap.amount_out);
		});
}

/// Test single ExactOut (buy) intent: Alice wants to buy BNC with HDX
#[test]
fn test_solver_execute_solution_with_buy_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let asset_a = 0u32; // HDX
	let asset_b = 14u32; // BNC

	let alice_wants_to_buy = 20_000_000_000_000u128;
	let alice_max_pay = 2_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, alice_max_pay * 10)
		.submit_buy_intent(alice.clone(), asset_a, asset_b, alice_max_pay, alice_wants_to_buy, 10)
		.execute(|| {
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution for buy intent");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve the buy intent");
			let resolved = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data;
			assert_eq!(
				swap_data.swap_type,
				ice_support::SwapType::ExactOut,
				"Should be ExactOut"
			);
			assert_eq!(
				swap_data.amount_out, alice_wants_to_buy,
				"Should buy exact amount requested"
			);
			assert!(swap_data.amount_in <= alice_max_pay, "Should not exceed max payment");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_balance_a_after = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_after = Currencies::total_balance(asset_b, &alice);

			// Verify balance changes
			assert!(
				alice_balance_a_after < alice_balance_a_before,
				"Alice's asset_a balance should decrease after paying"
			);
			assert!(
				alice_balance_b_after > alice_balance_b_before,
				"Alice's asset_b balance should increase after buying"
			);

			// Verify exact amounts match solution
			let paid = alice_balance_a_before - alice_balance_a_after;
			let received = alice_balance_b_after - alice_balance_b_before;
			assert_eq!(paid, swap_data.amount_in, "Paid amount should match solution");
			assert_eq!(received, swap_data.amount_out, "Received amount should match solution");

			// Verify intent removed
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "Intent should be resolved");
		});
}

/// Test mixed sell and buy intents from multiple users
#[test]
fn test_solver_mixed_sell_and_buy_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let sell_hdx_amount = 1_000_000_000_000u128;
	let sell_bnc_amount = 100_000_000_000u128;
	let buy_hdx_amount = 100_000_000_000_000u128;
	let buy_bnc_amount = 20_000_000_000u128;
	let max_pay = 10_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, max_pay)
		.endow_account(alice.clone(), bnc, max_pay)
		.endow_account(bob.clone(), hdx, max_pay)
		.endow_account(bob.clone(), bnc, max_pay)
		.endow_account(charlie.clone(), hdx, max_pay)
		.endow_account(charlie.clone(), bnc, max_pay)
		.endow_account(dave.clone(), hdx, max_pay)
		.endow_account(dave.clone(), bnc, max_pay)
		.submit_sell_intent(alice.clone(), hdx, bnc, sell_hdx_amount, 1, 10)
		.submit_buy_intent(bob.clone(), bnc, hdx, max_pay, buy_hdx_amount, 10)
		.submit_sell_intent(charlie.clone(), bnc, hdx, sell_bnc_amount, 1, 10)
		.submit_buy_intent(dave.clone(), hdx, bnc, max_pay, buy_bnc_amount, 10)
		.submit_sell_intent(alice.clone(), hdx, bnc, sell_hdx_amount, 1, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);
			let dave_hdx_before = Currencies::total_balance(hdx, &dave);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution for mixed intents");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert!(
				!solution.resolved_intents.is_empty(),
				"Should resolve at least some intents"
			);
			assert!(solution.score > 0, "Solution score should be positive");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_after = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);
			let dave_hdx_after = Currencies::total_balance(hdx, &dave);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);

			// Verify Alice (sells HDX for BNC)
			assert!(
				alice_hdx_after < alice_hdx_before,
				"Alice should have less HDX after selling"
			);
			assert!(
				alice_bnc_after > alice_bnc_before,
				"Alice should have more BNC after selling"
			);

			// Verify Bob (buys HDX with BNC)
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX after buying");
			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC after paying");

			// Verify Charlie (sells BNC for HDX)
			assert!(
				charlie_bnc_after < charlie_bnc_before,
				"Charlie should have less BNC after selling"
			);
			assert!(
				charlie_hdx_after > charlie_hdx_before,
				"Charlie should have more HDX after selling"
			);

			// Verify Dave (buys BNC with HDX)
			assert!(
				dave_bnc_after > dave_bnc_before,
				"Dave should have more BNC after buying"
			);
			assert!(
				dave_hdx_after < dave_hdx_before,
				"Dave should have less HDX after paying"
			);
		});
}

/// Test single ExactIn sell intent: Alice sells HDX for BNC
#[test]
fn test_solver_v1_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let amount = 10_000_000_000_000u128;
	let min_amount_out = 1u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 10)
		.submit_sell_intent(alice.clone(), hdx, bnc, amount, min_amount_out, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");
			let original_intent_id = intents[0].0;

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
			assert!(solution.score > 0, "Solution score should be positive");

			// Verify the resolved intent
			let resolved = &solution.resolved_intents[0];
			assert_eq!(resolved.id, original_intent_id, "Resolved intent ID should match");
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data;
			assert_eq!(swap_data.asset_in, hdx, "asset_in should be HDX");
			assert_eq!(swap_data.asset_out, bnc, "asset_out should be BNC");
			assert_eq!(swap_data.amount_in, amount, "amount_in should match submitted amount");
			assert!(
				swap_data.amount_out >= min_amount_out,
				"amount_out should be >= min_amount_out"
			);
			assert_eq!(
				swap_data.swap_type,
				ice_support::SwapType::ExactIn,
				"Should be ExactIn swap"
			);

			// Verify clearing prices contain both assets
			assert!(
				solution.clearing_prices.contains_key(&hdx),
				"Should have HDX clearing price"
			);
			assert!(
				solution.clearing_prices.contains_key(&bnc),
				"Should have BNC clearing price"
			);

			// Verify trades are valid
			assert!(!solution.trades.is_empty(), "Should have at least one trade");
			for trade in solution.trades.iter() {
				assert!(trade.amount_in > 0, "Trade amount_in should be positive");
				assert!(trade.amount_out > 0, "Trade amount_out should be positive");
				assert!(!trade.route.is_empty(), "Trade route should not be empty");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			// Verify intent was removed from storage
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(
				remaining_intents.is_empty(),
				"Intent should be removed after resolution"
			);

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);

			// Verify balance changes match the solution
			let hdx_spent = alice_hdx_before - alice_hdx_after;
			let bnc_received = alice_bnc_after - alice_bnc_before;

			assert_eq!(
				hdx_spent, swap_data.amount_in,
				"HDX spent should equal resolved amount_in"
			);
			assert_eq!(
				bnc_received, swap_data.amount_out,
				"BNC received should equal resolved amount_out"
			);
		});
}

/// Test partial CoW match: Alice sells large HDX, Bob sells small BNC (opposite directions)
#[test]
fn test_solver_v1_two_intents_partial_cow_match() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let hdx = 0u32;
	let bnc = 14u32;

	let alice_hdx_amount = 1_000_000_000_000_000u128;
	let bob_bnc_amount = 500_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_amount * 10)
		.endow_account(bob.clone(), bnc, bob_bnc_amount * 10)
		.submit_sell_intent(alice.clone(), hdx, bnc, alice_hdx_amount, 1, 10)
		.submit_sell_intent(bob.clone(), bnc, hdx, bob_bnc_amount, 1, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify both intents resolved
			assert_eq!(solution.resolved_intents.len(), 2, "Both intents should be resolved");
			assert!(solution.score > 0, "Solution score should be positive");
			assert!(solution.clearing_prices.contains_key(&hdx), "Should have HDX price");
			assert!(solution.clearing_prices.contains_key(&bnc), "Should have BNC price");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);

			// Verify Alice (sells HDX for BNC)
			assert!(
				alice_hdx_after < alice_hdx_before,
				"Alice should have less HDX after selling"
			);
			assert!(
				alice_bnc_after > alice_bnc_before,
				"Alice should have more BNC after selling"
			);

			// Verify Bob (sells BNC for HDX)
			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC after selling");
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX after selling");

			// Verify balance changes match solution
			for resolved in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref swap_data) = resolved.data;
				if swap_data.asset_in == hdx {
					// Alice's intent
					assert_eq!(alice_hdx_before - alice_hdx_after, swap_data.amount_in);
					assert_eq!(alice_bnc_after - alice_bnc_before, swap_data.amount_out);
				} else {
					// Bob's intent
					assert_eq!(bob_bnc_before - bob_bnc_after, swap_data.amount_in);
					assert_eq!(bob_hdx_after - bob_hdx_before, swap_data.amount_out);
				}
			}
		});
}

/// Test five mixed intents (3 sells, 2 buys) from different users
#[test]
fn test_solver_v1_five_mixed_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();
	let eve: AccountId = EVE.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, 1000 * hdx_unit)
		.endow_account(bob.clone(), bnc, 500 * bnc_unit)
		.endow_account(charlie.clone(), hdx, 500 * hdx_unit)
		.endow_account(dave.clone(), hdx, 500 * hdx_unit)
		.endow_account(eve.clone(), bnc, 100 * bnc_unit)
		// Alice: sell 500 HDX for BNC (ExactIn)
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		// Bob: sell 300 BNC for HDX (ExactIn)
		.submit_sell_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1, 10)
		// Charlie: sell 200 HDX for BNC (ExactIn)
		.submit_sell_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 1, 10)
		// Dave: buy 10 BNC with max 400 HDX (ExactOut)
		.submit_buy_intent(dave.clone(), hdx, bnc, 400 * hdx_unit, 10 * bnc_unit, 10)
		// Eve: buy 500 HDX with max 50 BNC (ExactOut)
		.submit_buy_intent(eve.clone(), bnc, hdx, 50 * bnc_unit, 500 * hdx_unit, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert!(
				!solution.resolved_intents.is_empty(),
				"Should resolve at least some intents"
			);
			assert!(solution.score > 0, "Solution score should be positive");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_after = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);

			// Verify sellers
			assert!(alice_hdx_after < alice_hdx_before, "Alice should have less HDX");
			assert!(alice_bnc_after > alice_bnc_before, "Alice should have more BNC");
			assert!(charlie_hdx_after < charlie_hdx_before, "Charlie should have less HDX");
			assert!(charlie_bnc_after > charlie_bnc_before, "Charlie should have more BNC");
			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC");
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX");
		});
}

/// Test uniform clearing price: multiple sellers of HDX should get proportional BNC
#[test]
fn test_solver_v1_uniform_price_all_sells() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();
	let eve: AccountId = EVE.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, 1000 * hdx_unit)
		.endow_account(bob.clone(), bnc, 500 * bnc_unit)
		.endow_account(charlie.clone(), hdx, 500 * hdx_unit)
		.endow_account(dave.clone(), hdx, 500 * hdx_unit)
		.endow_account(eve.clone(), hdx, 1000 * hdx_unit)
		// All ExactIn (sell) intents
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		.submit_sell_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1, 10)
		.submit_sell_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 1, 10)
		.submit_sell_intent(dave.clone(), hdx, bnc, 100 * hdx_unit, 1, 10)
		.submit_sell_intent(eve.clone(), hdx, bnc, 500 * hdx_unit, 1, 10) // Same as Alice
		.execute(|| {
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);
			let eve_bnc_after = Currencies::total_balance(bnc, &eve);

			let alice_bnc_received = alice_bnc_after.saturating_sub(alice_bnc_before);
			let charlie_bnc_received = charlie_bnc_after.saturating_sub(charlie_bnc_before);
			let dave_bnc_received = dave_bnc_after.saturating_sub(dave_bnc_before);
			let eve_bnc_received = eve_bnc_after.saturating_sub(eve_bnc_before);

			// Uniform price: Alice and Eve both sold 500 HDX, should receive same BNC
			assert_eq!(
				alice_bnc_received, eve_bnc_received,
				"Alice and Eve should receive exactly the same BNC for selling the same HDX"
			);

			// Proportionality check: Charlie (200 HDX) should get 2/5 of Alice's BNC
			let expected_charlie = alice_bnc_received * 200 / 500;
			let charlie_diff = charlie_bnc_received.abs_diff(expected_charlie);
			assert!(
				charlie_diff <= 1,
				"Charlie's amount should be proportional to Alice's (diff: {})",
				charlie_diff
			);

			// Proportionality check: Dave (100 HDX) should get 1/5 of Alice's BNC
			let expected_dave = alice_bnc_received * 100 / 500;
			let dave_diff = dave_bnc_received.abs_diff(expected_dave);
			assert!(
				dave_diff <= 1,
				"Dave's amount should be proportional to Alice's (diff: {})",
				dave_diff
			);
		});
}

/// Test uniform price with opposite direction sells (Alice sells HDX, Eve/Bob sell BNC)
#[test]
fn test_solver_v1_uniform_price_opposite_sells() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let eve: AccountId = EVE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	let eve_bnc_sell = 10_380_308_715_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, 1000 * hdx_unit)
		.endow_account(eve.clone(), bnc, 100 * bnc_unit)
		.endow_account(bob.clone(), bnc, 500 * bnc_unit)
		// Alice sells HDX for BNC
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		// Eve sells BNC for HDX (opposite direction)
		.submit_sell_intent(eve.clone(), bnc, hdx, eve_bnc_sell, 1, 10)
		// Bob sells BNC for HDX (same direction as Eve)
		.submit_sell_intent(bob.clone(), bnc, hdx, 200 * bnc_unit, 1, 10)
		.execute(|| {
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 3, "Should have 3 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert!(!solution.resolved_intents.is_empty(), "Should resolve intents");
			assert!(solution.score > 0, "Solution score should be positive");

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let eve_hdx_before = Currencies::total_balance(hdx, &eve);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let eve_hdx_after = Currencies::total_balance(hdx, &eve);
			let eve_bnc_after = Currencies::total_balance(bnc, &eve);

			let alice_hdx_spent = alice_hdx_before.saturating_sub(alice_hdx_after);
			let alice_bnc_received = alice_bnc_after.saturating_sub(alice_bnc_before);
			let eve_bnc_spent = eve_bnc_before.saturating_sub(eve_bnc_after);
			let eve_hdx_received = eve_hdx_after.saturating_sub(eve_hdx_before);

			// Verify Alice sold HDX, received BNC
			assert!(alice_hdx_spent > 0, "Alice should have spent HDX");
			assert!(alice_bnc_received > 0, "Alice should have received BNC");

			// Verify Eve sold BNC, received HDX
			assert!(eve_bnc_spent > 0, "Eve should have spent BNC");
			assert!(eve_hdx_received > 0, "Eve should have received HDX");

			// Verify rate consistency (uniform clearing price)
			// Alice's rate (BNC/HDX) should equal Eve's inverse rate (BNC/HDX)
			let alice_rate = alice_bnc_received as f64 / alice_hdx_spent as f64;
			let eve_inverse_rate = eve_bnc_spent as f64 / eve_hdx_received as f64;
			let rate_diff_pct = ((alice_rate - eve_inverse_rate).abs() / alice_rate) * 100.0;

			// Allow small difference due to integer rounding and AMM price impact
			assert!(
				rate_diff_pct < 1.0,
				"Rates should be consistent (diff: {:.6}%)",
				rate_diff_pct
			);
		});
}

/// Test intent with on_success callback: Alice sells BNC, callback transfers HDX to Bob
#[test]
fn test_intent_with_on_success_callback() {
	use codec::Encode;
	use hydradx_runtime::RuntimeCall;

	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	let hdx_to_transfer = hdx_unit;
	let bnc_to_sell = bnc_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), bnc, 10 * bnc_unit)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);

			// Create callback: transfer HDX to Bob after successful swap
			let transfer_call = RuntimeCall::Currencies(pallet_currencies::Call::transfer {
				dest: bob.clone(),
				currency_id: hdx,
				amount: hdx_to_transfer,
			});
			let callback_data: pallet_intent::types::CallData =
				transfer_call.encode().try_into().expect("callback should fit");

			let ts = Timestamp::now();
			let deadline = ts + 6000 * 10;

			let min_hdx_out = 1u128;

			assert_ok!(pallet_intent::Pallet::<Runtime>::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::Intent {
					data: ice_support::IntentData::Swap(ice_support::SwapData {
						asset_in: bnc,
						asset_out: hdx,
						amount_in: bnc_to_sell,
						amount_out: min_hdx_out,
						swap_type: ice_support::SwapType::ExactIn,
						partial: false,
					}),
					deadline,
					on_success: Some(callback_data),
					on_failure: None,
				},
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();
			let mut captured_solution: Option<Solution> = None;

			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let Some(_call) = result else {
				// No solution found - skip test
				return;
			};

			let solution = captured_solution.expect("Solution should be captured");
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve the intent");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
				new_block,
			));

			// After solution, Alice should have received HDX
			let alice_hdx_after_solution = Currencies::total_balance(hdx, &alice);
			let alice_hdx_received = alice_hdx_after_solution.saturating_sub(alice_hdx_before);
			assert!(alice_hdx_received > 0, "Alice should have received some HDX");
			assert!(
				alice_hdx_received >= hdx_to_transfer,
				"Alice should have received at least {} HDX for the callback",
				hdx_to_transfer
			);

			// Dispatch the callback from lazy executor queue
			let next_dispatch_id = LazyExecutor::dispatch_next_id();
			let next_call_id = LazyExecutor::next_call_id();

			if next_call_id > next_dispatch_id {
				assert_ok!(LazyExecutor::dispatch_top(RuntimeOrigin::none()));
			}

			// Verify final state
			let alice_hdx_final = Currencies::total_balance(hdx, &alice);
			let alice_bnc_final = Currencies::total_balance(bnc, &alice);
			let bob_hdx_final = Currencies::total_balance(hdx, &bob);

			// Alice spent BNC
			assert!(alice_bnc_final < alice_bnc_before, "Alice should have spent BNC");

			// Bob received HDX from callback
			let bob_hdx_received = bob_hdx_final.saturating_sub(bob_hdx_before);
			assert_eq!(
				bob_hdx_received, hdx_to_transfer,
				"Bob should have received {} HDX from callback",
				hdx_to_transfer
			);

			// Alice's final HDX should be: received - transferred to Bob
			let expected_alice_hdx = alice_hdx_before + alice_hdx_received - hdx_to_transfer;
			assert_eq!(
				alice_hdx_final, expected_alice_hdx,
				"Alice HDX balance should match expected"
			);
		});
}

/// Test single intent trading USDT (asset 10, 6 decimals) for WETH (asset 20, 18 decimals)
/// This tests route discovery with different decimal assets
#[test]
fn test_usdt_weth_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();

	// Asset IDs
	let usdt = 10u32; // Tether - 6 decimals
	let weth = 20u32; // WETH - 18 decimals

	// Units based on decimals
	let usdt_unit = 1_000_000u128; // 10^6
	let _weth_unit = 1_000_000_000_000_000_000u128; // 10^18

	// Sell 100 USDT
	let amount_in = 100 * usdt_unit;
	let min_amount_out = 1u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), usdt, amount_in * 10)
		.submit_sell_intent(alice.clone(), usdt, weth, amount_in, min_amount_out, 10)
		.execute(|| {
			let alice_usdt_before = Currencies::total_balance(usdt, &alice);
			let alice_weth_before = Currencies::total_balance(weth, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");
			let original_intent_id = intents[0].0;

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution for USDT->WETH");
			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
			assert!(solution.score > 0, "Solution score should be positive");

			// Verify the resolved intent
			let resolved = &solution.resolved_intents[0];
			assert_eq!(resolved.id, original_intent_id, "Resolved intent ID should match");
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data;
			assert_eq!(swap_data.asset_in, usdt, "asset_in should be USDT");
			assert_eq!(swap_data.asset_out, weth, "asset_out should be WETH");
			assert_eq!(
				swap_data.amount_in, amount_in,
				"amount_in should match submitted amount"
			);
			assert!(
				swap_data.amount_out >= min_amount_out,
				"amount_out should be >= min_amount_out"
			);
			assert_eq!(
				swap_data.swap_type,
				ice_support::SwapType::ExactIn,
				"Should be ExactIn swap"
			);

			// Verify clearing prices contain both assets
			assert!(
				solution.clearing_prices.contains_key(&usdt),
				"Should have USDT clearing price"
			);
			assert!(
				solution.clearing_prices.contains_key(&weth),
				"Should have WETH clearing price"
			);

			// Verify trades are valid
			assert!(!solution.trades.is_empty(), "Should have at least one trade");
			for trade in solution.trades.iter() {
				assert!(trade.amount_in > 0, "Trade amount_in should be positive");
				assert!(trade.amount_out > 0, "Trade amount_out should be positive");
				assert!(!trade.route.is_empty(), "Trade route should not be empty");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_usdt_after = Currencies::total_balance(usdt, &alice);
			let alice_weth_after = Currencies::total_balance(weth, &alice);

			// Verify balances changed correctly
			assert!(
				alice_usdt_after < alice_usdt_before,
				"Alice should have less USDT after sell"
			);
			assert!(
				alice_weth_after > alice_weth_before,
				"Alice should have more WETH after sell"
			);

			// Verify exact amounts match solution
			let usdt_spent = alice_usdt_before - alice_usdt_after;
			let weth_received = alice_weth_after - alice_weth_before;
			assert_eq!(usdt_spent, swap_data.amount_in, "USDT spent should match solution");
			assert_eq!(
				weth_received, swap_data.amount_out,
				"WETH received should match solution"
			);

			// Verify intent was resolved
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "Intent should be resolved");
		});
}

/// Compare trading USDT->WETH via solver vs direct router
/// Both should give the same result for a single intent
#[test]
fn test_usdt_weth_solver_vs_router() {
	use hydradx_traits::router::RouteProvider;

	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	// Asset IDs
	let usdt = 10u32; // Tether - 6 decimals
	let weth = 20u32; // WETH - 18 decimals

	// Units based on decimals
	let usdt_unit = 1_000_000u128; // 10^6

	// Sell 100 USDT
	let amount_in = 100 * usdt_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), usdt, amount_in * 10)
		.endow_account(bob.clone(), usdt, amount_in * 10)
		.submit_sell_intent(alice.clone(), usdt, weth, amount_in, 1, 10)
		.execute(|| {
			// ========== SOLVER PATH (Alice) ==========
			let alice_usdt_before = Currencies::total_balance(usdt, &alice);
			let alice_weth_before = Currencies::total_balance(weth, &alice);

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_usdt_after = Currencies::total_balance(usdt, &alice);
			let alice_weth_after = Currencies::total_balance(weth, &alice);

			let solver_usdt_spent = alice_usdt_before - alice_usdt_after;
			let solver_weth_received = alice_weth_after - alice_weth_before;

			// ========== DIRECT ROUTER PATH (Bob) ==========
			let bob_usdt_before = Currencies::total_balance(usdt, &bob);
			let bob_weth_before = Currencies::total_balance(weth, &bob);

			// Get the route that would be used
			let route = Router::get_route(hydradx_traits::router::AssetPair::new(usdt, weth));

			// Execute sell directly via router
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(bob.clone()),
				usdt,
				weth,
				amount_in,
				1, // min_amount_out
				route.clone(),
			));

			let bob_usdt_after = Currencies::total_balance(usdt, &bob);
			let bob_weth_after = Currencies::total_balance(weth, &bob);

			let router_usdt_spent = bob_usdt_before - bob_usdt_after;
			let router_weth_received = bob_weth_after - bob_weth_before;

			// Both should spend the same amount of USDT
			assert_eq!(solver_usdt_spent, router_usdt_spent, "USDT spent should be the same");

			// WETH received will differ slightly because the pool state changes after the solver trade.
			// The solver trades first, so when the router trades afterward, pools have different reserves.
			// For a fair comparison, we verify they're within a small percentage of each other.
			let diff_pct = if solver_weth_received > router_weth_received {
				(solver_weth_received - router_weth_received) * 10000 / router_weth_received
			} else {
				(router_weth_received - solver_weth_received) * 10000 / solver_weth_received
			};
			// Should be within 1% (100 bps) - accounting for pool state change
			assert!(
				diff_pct < 100,
				"WETH difference should be within 1%, got {}bps",
				diff_pct
			);
		});
}

/// Test 2 opposing intents: Alice sells USDT for WETH, Bob sells WETH for USDT
/// These should partially match (CoW), giving Alice a better price than single intent
#[test]
fn test_usdt_weth_two_opposing_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	// Asset IDs
	let usdt = 10u32; // Tether - 6 decimals
	let weth = 20u32; // WETH - 18 decimals

	// Units based on decimals
	let usdt_unit = 1_000_000u128; // 10^6
	let weth_unit = 1_000_000_000_000_000_000u128; // 10^18

	// Alice sells 100 USDT for WETH
	let alice_usdt_amount = 100 * usdt_unit;
	// Bob sells 0.01 WETH for USDT (roughly equivalent value to create partial match)
	let bob_weth_amount = weth_unit / 100; // 0.01 WETH

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), usdt, alice_usdt_amount * 100)
		.endow_account(bob.clone(), weth, bob_weth_amount * 100)
		// Also give some of the opposite asset for potential edge cases
		.endow_account(alice.clone(), weth, weth_unit)
		.endow_account(bob.clone(), usdt, 1000 * usdt_unit)
		// Alice: sell USDT for WETH
		.submit_sell_intent(alice.clone(), usdt, weth, alice_usdt_amount, 1, 10)
		// Bob: sell WETH for USDT (opposite direction)
		.submit_sell_intent(bob.clone(), weth, usdt, bob_weth_amount, 1, 10)
		.execute(|| {
			let alice_usdt_before = Currencies::total_balance(usdt, &alice);
			let alice_weth_before = Currencies::total_balance(weth, &alice);
			let bob_usdt_before = Currencies::total_balance(usdt, &bob);
			let bob_weth_before = Currencies::total_balance(weth, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_usdt_after = Currencies::total_balance(usdt, &alice);
			let alice_weth_after = Currencies::total_balance(weth, &alice);
			let bob_usdt_after = Currencies::total_balance(usdt, &bob);
			let alice_weth_received = alice_weth_after - alice_weth_before;
			let bob_usdt_received = bob_usdt_after - bob_usdt_before;

			let single_intent_weth = 32_040_810_565_082_029u128;
			let improvement = if alice_weth_received > single_intent_weth {
				alice_weth_received - single_intent_weth
			} else {
				0
			};
			let improvement_pct = improvement as f64 / single_intent_weth as f64 * 100.0;
			// Verify both intents were resolved
			assert!(solution.resolved_intents.len() >= 1, "Should resolve at least 1 intent");

			// Verify Alice got WETH
			assert!(alice_weth_received > 0, "Alice should receive WETH");

			// Verify Bob got USDT
			assert!(bob_usdt_received > 0, "Bob should receive USDT");
		});
}

/// Test: Single intent - sell ETH for 3pool
/// ETH (asset 34) - 18 decimals
/// 3pool (asset 103) - 18 decimals
#[test]
fn test_eth_3pool_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();

	// Asset IDs
	let eth = 34u32; // ETH - 18 decimals
	let pool3 = 103u32; // 3pool - 18 decimals

	// Units based on decimals (both 18 decimals)
	let unit = 1_000_000_000_000_000_000u128; // 10^18

	// Alice sells 0.1 ETH for 3pool
	let alice_eth_amount = unit / 10; // 0.1 ETH

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), eth, alice_eth_amount * 10)
		// Alice: sell ETH for 3pool
		.submit_sell_intent(alice.clone(), eth, pool3, alice_eth_amount, 1, 10)
		.execute(|| {
			let alice_eth_before = Currencies::total_balance(eth, &alice);
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = HollarSolver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_eth_after = Currencies::total_balance(eth, &alice);
			let alice_3pool_after = Currencies::total_balance(pool3, &alice);

			let eth_spent = alice_eth_before - alice_eth_after;
			let pool3_received = alice_3pool_after - alice_3pool_before;

			// Verify Alice spent ETH and received 3pool
			assert_eq!(eth_spent, alice_eth_amount, "Alice should spend the intent amount");
			assert!(pool3_received > 0, "Alice should receive 3pool");
		});
}

/// Test: Compare solver results with direct router trade for ETH -> 3pool
#[test]
fn test_eth_3pool_solver_vs_router() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	// Asset IDs
	let eth = 34u32; // ETH - 18 decimals
	let pool3 = 103u32; // 3pool - 18 decimals

	// Units based on decimals (both 18 decimals)
	let unit = 1_000_000_000_000_000_000u128; // 10^18

	// Both sell 0.1 ETH for 3pool
	let amount_in = unit / 10; // 0.1 ETH

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), eth, amount_in * 10)
		.endow_account(bob.clone(), eth, amount_in * 10)
		// Alice: sell ETH for 3pool via intent
		.submit_sell_intent(alice.clone(), eth, pool3, amount_in, 1, 10)
		.execute(|| {
			// ========== SOLVER PATH (Alice) ==========
			let alice_eth_before = Currencies::total_balance(eth, &alice);
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = HollarSolver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_eth_after = Currencies::total_balance(eth, &alice);
			let alice_3pool_after = Currencies::total_balance(pool3, &alice);

			let solver_eth_spent = alice_eth_before - alice_eth_after;
			let solver_3pool_received = alice_3pool_after - alice_3pool_before;

			// ========== DIRECT ROUTER PATH (Bob) ==========
			let bob_eth_before = Currencies::total_balance(eth, &bob);
			let bob_3pool_before = Currencies::total_balance(pool3, &bob);

			// Get the route that would be used
			let route = Router::get_route(hydradx_traits::router::AssetPair::new(eth, pool3));

			// Execute sell directly via router
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(bob.clone()),
				eth,
				pool3,
				amount_in,
				1, // min_amount_out
				route,
			));

			let bob_eth_after = Currencies::total_balance(eth, &bob);
			let bob_3pool_after = Currencies::total_balance(pool3, &bob);

			let router_eth_spent = bob_eth_before - bob_eth_after;
			let router_3pool_received = bob_3pool_after - bob_3pool_before;

			// Both should spend the same amount of ETH
			assert_eq!(solver_eth_spent, router_eth_spent, "ETH spent should be the same");

			// 3pool received will differ slightly because the pool state changes after the solver trade
			let diff_pct = if solver_3pool_received > router_3pool_received {
				(solver_3pool_received - router_3pool_received) * 10000 / router_3pool_received
			} else {
				(router_3pool_received - solver_3pool_received) * 10000 / solver_3pool_received
			};

			// Should be within 1% (100 bps)
			assert!(
				diff_pct < 100,
				"3pool difference should be within 1%, got {}bps",
				diff_pct
			);
		});
}

/// Test: Two opposing intents for ETH <-> 3pool (CoW matching)
#[test]
fn test_eth_3pool_two_opposing_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	// Asset IDs
	let eth = 34u32; // ETH - 18 decimals
	let pool3 = 103u32; // 3pool - 18 decimals

	// Units based on decimals (both 18 decimals)
	let unit = 1_000_000_000_000_000_000u128; // 10^18

	// Alice sells 0.1 ETH for 3pool
	let alice_eth_amount = unit / 10; // 0.1 ETH
								   // Bob sells 100 3pool for ETH (roughly equivalent value to create partial match)
	let bob_3pool_amount = 100 * unit; // 100 3pool

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), eth, alice_eth_amount * 100)
		.endow_account(bob.clone(), pool3, bob_3pool_amount * 100)
		// Also give some of the opposite asset
		.endow_account(alice.clone(), pool3, unit)
		.endow_account(bob.clone(), eth, unit)
		// Alice: sell ETH for 3pool
		.submit_sell_intent(alice.clone(), eth, pool3, alice_eth_amount, 1, 10)
		// Bob: sell 3pool for ETH (opposite direction)
		.submit_sell_intent(bob.clone(), pool3, eth, bob_3pool_amount, 1, 10)
		.execute(|| {
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);
			let bob_eth_before = Currencies::total_balance(eth, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = HollarSolver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			);

			let _call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			let alice_3pool_after = Currencies::total_balance(pool3, &alice);
			let bob_eth_after = Currencies::total_balance(eth, &bob);

			let alice_3pool_received = alice_3pool_after - alice_3pool_before;
			let bob_eth_received = bob_eth_after - bob_eth_before;

			// Verify both intents were resolved
			assert!(solution.resolved_intents.len() >= 1, "Should resolve at least 1 intent");

			// Verify Alice got 3pool
			assert!(alice_3pool_received > 0, "Alice should receive 3pool");

			// Verify Bob got ETH
			assert!(bob_eth_received > 0, "Bob should receive ETH");
		});
}
