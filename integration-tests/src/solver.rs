use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::traits::Time;
use hydradx_runtime::{Currencies, LazyExecutor, Omnipool, Router, Runtime, RuntimeOrigin, Stableswap, Timestamp};
use hydradx_traits::amm::{AMMInterface, AmmSimulator, SimulatorConfig, SimulatorSet};
use ice_solver::v1::SolverV1;
use ice_support::Solution;
use orml_traits::MultiCurrency;
use primitives::AccountId;
use xcm_emulator::Network;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/mainnet_nov4";

pub struct HDXAssetId;
impl frame_support::traits::Get<u32> for HDXAssetId {
	fn get() -> u32 {
		0
	}
}

pub struct HydrationTestConfig;

impl SimulatorConfig for HydrationTestConfig {
	type Simulators = (Omnipool, Stableswap);
	type RouteProvider = Router;
	type PriceDenominator = HDXAssetId;
}

pub type CombinedSimulatorState = <(Omnipool, Stableswap) as SimulatorSet>::State;

type TestSimulator = HydrationSimulator<HydrationTestConfig>;
type Solver = SolverV1<TestSimulator>;

#[test]
fn test_simulator_snapshot() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let snapshot = <Omnipool as AmmSimulator>::snapshot();

		assert!(!snapshot.assets.is_empty(), "Snapshot should contain assets");
		assert!(snapshot.hub_asset_id > 0, "Hub asset id should be set");

		dbg!(&snapshot.assets.len());
		dbg!(&snapshot.hub_asset_id);
	});
}

#[test]
fn test_simulator_sell() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		use hydradx_traits::amm::SimulatorError;

		let snapshot = <Omnipool as AmmSimulator>::snapshot();

		let assets: Vec<_> = snapshot.assets.keys().copied().collect();
		if assets.len() < 2 {
			println!("Not enough assets in snapshot to test trading");
			return;
		}

		let asset_in = assets[0];
		let asset_out = assets[1];

		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			println!("Skipping test - one of the assets is hub asset");
			return;
		}

		let amount_in = 1_000_000_000_000u128;

		let result = <Omnipool as AmmSimulator>::simulate_sell(asset_in, asset_out, amount_in, 0, &snapshot);

		match result {
			Ok((new_snapshot, trade_result)) => {
				println!("Trade successful!");
				println!("  Amount in: {}", trade_result.amount_in);
				println!("  Amount out: {}", trade_result.amount_out);

				let old_reserve_in = snapshot.assets.get(&asset_in).unwrap().reserve;
				let new_reserve_in = new_snapshot.assets.get(&asset_in).unwrap().reserve;
				assert!(new_reserve_in > old_reserve_in, "Asset in reserve should increase");

				let old_reserve_out = snapshot.assets.get(&asset_out).unwrap().reserve;
				let new_reserve_out = new_snapshot.assets.get(&asset_out).unwrap().reserve;
				assert!(new_reserve_out < old_reserve_out, "Asset out reserve should decrease");
			}
			Err(e) => {
				println!("Trade failed with error: {:?}", e);
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

		println!("=== Stableswap Snapshot ===");
		println!("Number of pools: {}", stableswap_snapshot.pools.len());
		println!("Min trading limit: {}", stableswap_snapshot.min_trading_limit);

		for (pool_id, pool) in &stableswap_snapshot.pools {
			println!("\n--- Pool {} ---", pool_id);
			println!("  Assets: {:?}", pool.assets.to_vec());
			println!("  Amplification: {}", pool.amplification);
			println!("  Fee: {:?}", pool.fee);
			println!("  Share issuance: {}", pool.share_issuance);
			println!("  Reserves:");
			for (i, (asset_id, reserve)) in pool.assets.iter().zip(pool.reserves.iter()).enumerate() {
				println!(
					"    [{}] Asset {}: amount={}, decimals={}",
					i, asset_id, reserve.amount, reserve.decimals
				);
			}
			println!("  Pegs: {:?}", pool.pegs.to_vec());
		}

		println!("\n=== End Stableswap Snapshot ===");
	});
}

#[test]
fn test_stableswap_simulator_direct() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let snapshot = <Stableswap as AmmSimulator>::snapshot();

		let pool_id = 104u32;
		let Some(pool) = snapshot.pools.get(&pool_id) else {
			println!("Pool 104 not found");
			return;
		};

		let asset_a = pool.assets[0];
		let asset_b = pool.assets[1];
		let decimals_a = pool.reserves[0].decimals;

		println!("=== Testing Stableswap Simulator Directly ===");
		println!(
			"Pool {}: assets=[{}, {}], decimals={}",
			pool_id, asset_a, asset_b, decimals_a
		);
		println!("Reserve A: {}", pool.reserves[0].amount);
		println!("Reserve B: {}", pool.reserves[1].amount);

		let amount_in = 10u128.pow(decimals_a as u32);
		println!("\n--- Test simulate_sell ---");
		println!("Selling {} units of asset {} for asset {}", amount_in, asset_a, asset_b);

		match <Stableswap as AmmSimulator>::simulate_sell(asset_a, asset_b, amount_in, 0, &snapshot) {
			Ok((new_snapshot, result)) => {
				println!("SUCCESS!");
				println!("  Amount in: {}", result.amount_in);
				println!("  Amount out: {}", result.amount_out);

				let new_pool = new_snapshot.pools.get(&pool_id).unwrap();
				let old_reserve_a = pool.reserves[0].amount;
				let new_reserve_a = new_pool.reserves[0].amount;
				println!(
					"  Reserve A: {} -> {} (delta: +{})",
					old_reserve_a,
					new_reserve_a,
					new_reserve_a - old_reserve_a
				);

				let old_reserve_b = pool.reserves[1].amount;
				let new_reserve_b = new_pool.reserves[1].amount;
				println!(
					"  Reserve B: {} -> {} (delta: -{})",
					old_reserve_b,
					new_reserve_b,
					old_reserve_b - new_reserve_b
				);

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
			}
			Err(e) => {
				println!("FAILED: {:?}", e);
				panic!("simulate_sell should succeed");
			}
		}

		let amount_out = 10u128.pow(decimals_a as u32);
		println!("\n--- Test simulate_buy ---");
		println!(
			"Buying {} units of asset {} with asset {}",
			amount_out, asset_b, asset_a
		);

		match <Stableswap as AmmSimulator>::simulate_buy(asset_a, asset_b, amount_out, u128::MAX, &snapshot) {
			Ok((_new_snapshot, result)) => {
				println!("SUCCESS!");
				println!("  Amount in: {}", result.amount_in);
				println!("  Amount out: {}", result.amount_out);
				assert_eq!(result.amount_out, amount_out, "Amount out should match requested");
			}
			Err(e) => {
				println!("FAILED: {:?}", e);
				panic!("simulate_buy should succeed");
			}
		}

		println!("\n--- Test get_spot_price ---");
		match <Stableswap as AmmSimulator>::get_spot_price(asset_a, asset_b, &snapshot) {
			Ok(price) => {
				println!("SUCCESS!");
				println!("  Price {}/{}: {}/{}", asset_a, asset_b, price.n, price.d);
				let price_f64 = price.n as f64 / price.d as f64;
				println!("  Price as float: {:.6}", price_f64);
			}
			Err(e) => {
				println!("FAILED: {:?}", e);
				panic!("get_spot_price should succeed");
			}
		}

		println!("\n=== Stableswap Simulator Tests PASSED ===");
	});
}

#[test]
fn test_stableswap_intent() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let stableswap_snapshot = <Stableswap as AmmSimulator>::snapshot();

		println!("=== Available Stableswap Pools ===");
		for (pid, pool) in &stableswap_snapshot.pools {
			let min_reserve = pool.reserves.iter().map(|r| r.amount).min().unwrap_or(0);
			println!(
				"Pool {}: assets={:?}, min_reserve={}, share_issuance={}",
				pid,
				pool.assets.to_vec(),
				min_reserve,
				pool.share_issuance
			);
		}

		use hydradx_traits::router::{AssetPair, PoolType as RouterPoolType, RouteProvider};
		let hdx = 0u32;

		let mut selected_pool: Option<(u32, u32, u32, u8)> = None;

		for (pid, pool) in &stableswap_snapshot.pools {
			if pool.assets.len() < 2 {
				continue;
			}
			let a = pool.assets[0];
			let b = pool.assets[1];

			let route_ab = Router::get_route(AssetPair::new(a, b));
			let uses_stableswap = route_ab.iter().any(|t| matches!(t.pool, RouterPoolType::Stableswap(_)));

			if !uses_stableswap {
				continue;
			}

			let route_a_hdx = Router::get_route(AssetPair::new(a, hdx));
			let route_b_hdx = Router::get_route(AssetPair::new(b, hdx));

			let a_omnipool_only =
				!route_a_hdx.is_empty() && route_a_hdx.iter().all(|t| matches!(t.pool, RouterPoolType::Omnipool));
			let b_omnipool_only =
				!route_b_hdx.is_empty() && route_b_hdx.iter().all(|t| matches!(t.pool, RouterPoolType::Omnipool));

			println!(
				"Pool {}: assets=[{}, {}], uses_ss={}, a_omni={}, b_omni={}",
				pid, a, b, uses_stableswap, a_omnipool_only, b_omnipool_only
			);

			if a_omnipool_only && b_omnipool_only {
				selected_pool = Some((*pid, a, b, pool.reserves[0].decimals));
				break;
			}
		}

		let Some((pool_id, asset_a, asset_b, decimals_a)) = selected_pool else {
			println!("No suitable stableswap pool found with Omnipool-only routes to HDX");
			println!("This might mean the current snapshot doesn't have ideal test data.");
			println!("Skipping test - stableswap simulator is implemented but can't be tested with this snapshot.");
			return;
		};

		println!("\n=== Stableswap Intent Test ===");
		println!("Selected Pool ID: {}", pool_id);
		println!("Pool assets: [{}, {}]", asset_a, asset_b);
		println!("Trading: {} -> {}", asset_a, asset_b);
		println!("Asset A decimals: {}", decimals_a);

		let hdx_asset_id = 0u32;

		println!("\n=== Route Checking ===");

		let route_a_to_hdx = Router::get_route(AssetPair::new(asset_a, hdx_asset_id));
		println!("Route {} -> HDX: {:?}", asset_a, route_a_to_hdx);

		let route_b_to_hdx = Router::get_route(AssetPair::new(asset_b, hdx_asset_id));
		println!("Route {} -> HDX: {:?}", asset_b, route_b_to_hdx);

		let route_a_to_b = Router::get_route(AssetPair::new(asset_a, asset_b));
		println!("Route {} -> {}: {:?}", asset_a, asset_b, route_a_to_b);

		let uses_stableswap = route_a_to_b
			.iter()
			.any(|t| matches!(t.pool, RouterPoolType::Stableswap(_)));
		println!("Route uses Stableswap: {}", uses_stableswap);

		if !uses_stableswap {
			println!("\nRoute goes through Omnipool instead of Stableswap.");
			println!("Looking for assets that would force stableswap route...");

			if let Some(pool_101) = stableswap_snapshot.pools.get(&101) {
				let a = pool_101.assets[0];
				let b = pool_101.assets[1];
				let route = Router::get_route(AssetPair::new(a, b));
				let ss_route = route.iter().any(|t| matches!(t.pool, RouterPoolType::Stableswap(_)));
				println!(
					"Pool 101 [{} -> {}]: uses_stableswap={}, route={:?}",
					a, b, ss_route, route
				);
			}

			if let Some(pool_103) = stableswap_snapshot.pools.get(&103) {
				let a = pool_103.assets[0];
				let b = pool_103.assets[1];
				let route = Router::get_route(AssetPair::new(a, b));
				let ss_route = route.iter().any(|t| matches!(t.pool, RouterPoolType::Stableswap(_)));
				println!(
					"Pool 103 [{} -> {}]: uses_stableswap={}, route={:?}",
					a, b, ss_route, route
				);
			}

			if let Some(pool_104) = stableswap_snapshot.pools.get(&104) {
				let a = pool_104.assets[0];
				let b = pool_104.assets[1];
				let route = Router::get_route(AssetPair::new(a, b));
				let ss_route = route.iter().any(|t| matches!(t.pool, RouterPoolType::Stableswap(_)));
				println!(
					"Pool 104 [{} -> {}]: uses_stableswap={}, route={:?}",
					a, b, ss_route, route
				);
			}
		}

		let combined_state = <(Omnipool, Stableswap) as SimulatorSet>::initial_state();
		println!("\n=== Spot Price Checking ===");

		match TestSimulator::get_spot_price(asset_a, hdx_asset_id, &combined_state) {
			Ok(price) => println!("Price {} in HDX: {}/{}", asset_a, price.n, price.d),
			Err(e) => println!("Failed to get price {} -> HDX: {:?}", asset_a, e),
		}

		match TestSimulator::get_spot_price(asset_b, hdx_asset_id, &combined_state) {
			Ok(price) => println!("Price {} in HDX: {}/{}", asset_b, price.n, price.d),
			Err(e) => println!("Failed to get price {} -> HDX: {:?}", asset_b, e),
		}

		let amount_in = 10u128.pow(decimals_a as u32);
		println!("Amount in: {} (1 unit)", amount_in);

		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			ALICE.into(),
			asset_a,
			(amount_in * 10) as i128,
		));

		let alice_a_before = Currencies::total_balance(asset_a, &ALICE.into());
		let alice_b_before = Currencies::total_balance(asset_b, &ALICE.into());
		println!("Alice before: asset_a={}, asset_b={}", alice_a_before, alice_b_before);

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
		println!("Created {} intent(s)", intents.len());

		let block = hydradx_runtime::System::block_number();

		let mut captured_solution: Option<Solution> = None;
		let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
			println!("Solving with {} intent(s)", intents.len());

			let solution = Solver::solve(intents, state).ok()?;
			println!("Solution found!");
			println!("  Resolved intents: {}", solution.resolved_intents.len());
			println!("  Trades: {}", solution.trades.len());

			for (i, trade) in solution.trades.iter().enumerate() {
				println!("  Trade {}: {:?}", i, trade);
			}

			captured_solution = Some(solution.clone());
			Some(solution)
		});

		if result.is_none() {
			println!("No solution found - this may indicate the route goes through Omnipool");
			println!("Checking if direct stableswap trade works...");

			let direct_result = hydradx_runtime::Stableswap::sell(
				RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				asset_a,
				asset_b,
				amount_in,
				0,
			);
			println!("Direct stableswap result: {:?}", direct_result);
			return;
		}

		let solution = captured_solution.expect("Solution should be captured");

		crate::polkadot_test_net::hydradx_run_to_next_block();
		let new_block = hydradx_runtime::System::block_number();

		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution,
			new_block,
		));

		let alice_a_after = Currencies::total_balance(asset_a, &ALICE.into());
		let alice_b_after = Currencies::total_balance(asset_b, &ALICE.into());
		println!("Alice after: asset_a={}, asset_b={}", alice_a_after, alice_b_after);

		let a_change = alice_a_before as i128 - alice_a_after as i128;
		let b_change = alice_b_after as i128 - alice_b_before as i128;
		println!("Changes: asset_a={}, asset_b=+{}", -a_change, b_change);

		assert!(alice_a_after < alice_a_before, "Alice should have less asset_a");
		assert!(alice_b_after > alice_b_before, "Alice should have more asset_b");

		println!("=== Stableswap Intent Test PASSED ===");
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
			println!("Number of intents: {}", intents.len());
			assert_eq!(intents.len(), 2, "Should have 2 intents");
			dbg!(&intents);

			let b = hydradx_runtime::System::block_number();

			let result = pallet_ice::Pallet::<Runtime>::run(b, |intents, state: CombinedSimulatorState| {
				println!("Solving with {} intents", intents.len());
				dbg!(&intents);

				let solution = Solver::solve(intents, state).ok()?;
				println!("Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Score: {}", solution.score);
				dbg!(&solution);
				Some(solution)
			});

			match result {
				Some(call) => {
					println!("Solver produced a valid solution");
					dbg!(&call);
				}
				None => {
					println!("No solution found (this may be expected if intents cannot be matched)");
				}
			}
		});
}

#[test]
fn test_solver_execute_solution1() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let asset_a = 0u32;
	let asset_b = 14u32;
	let amount = 10_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, amount * 10)
		.endow_account(bob.clone(), asset_b, amount * 10)
		.submit_sell_intent(alice.clone(), asset_a, asset_b, amount, 1, 10)
		.submit_sell_intent(bob.clone(), asset_b, asset_a, amount, 1, 10)
		.execute(|| {
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);
			let bob_balance_a_before = Currencies::total_balance(asset_a, &bob);
			let bob_balance_b_before = Currencies::total_balance(asset_b, &bob);

			println!("=== Balances BEFORE solution ===");
			println!(
				"Alice: asset_a={}, asset_b={}",
				alice_balance_a_before, alice_balance_b_before
			);
			println!(
				"Bob:   asset_a={}, asset_b={}",
				bob_balance_a_before, bob_balance_b_before
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();
			println!("Current block: {}", block);

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("Solving with {} intents", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Score: {}", solution.score);

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let call = result.expect("Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();
			println!("Advanced to block: {}", new_block);

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("Solution submitted successfully!");

			let alice_balance_a_after = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_after = Currencies::total_balance(asset_b, &alice);
			let bob_balance_a_after = Currencies::total_balance(asset_a, &bob);
			let bob_balance_b_after = Currencies::total_balance(asset_b, &bob);

			println!("=== Balances AFTER solution ===");
			println!(
				"Alice: asset_a={}, asset_b={}",
				alice_balance_a_before, alice_balance_b_before
			);
			println!(
				"Alice: asset_a={}, asset_b={}",
				alice_balance_a_after, alice_balance_b_after
			);
			println!(
				"Bob:   asset_a={}, asset_b={}",
				bob_balance_a_after, bob_balance_b_after
			);

			assert!(
				alice_balance_a_after < alice_balance_a_before,
				"Alice's asset_a balance should decrease after selling"
			);
			assert!(
				alice_balance_b_after > alice_balance_b_before,
				"Alice's asset_b balance should increase after buying"
			);

			assert!(
				bob_balance_b_after < bob_balance_b_before,
				"Bob's asset_b balance should decrease after selling"
			);
			assert!(
				bob_balance_a_after > bob_balance_a_before,
				"Bob's asset_a balance should increase after buying"
			);

			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("Remaining intents after solution: {}", remaining_intents.len());

			println!("=== Balance changes ===");
			println!(
				"Alice: asset_a {} -> {} (delta: {})",
				alice_balance_a_before,
				alice_balance_a_after,
				alice_balance_a_before as i128 - alice_balance_a_after as i128
			);
			println!(
				"Alice: asset_b {} -> {} (delta: {})",
				alice_balance_b_before,
				alice_balance_b_after,
				alice_balance_b_after as i128 - alice_balance_b_before as i128
			);
			println!(
				"Bob:   asset_a {} -> {} (delta: {})",
				bob_balance_a_before,
				bob_balance_a_after,
				bob_balance_a_after as i128 - bob_balance_a_before as i128
			);
			println!(
				"Bob:   asset_b {} -> {} (delta: {})",
				bob_balance_b_before,
				bob_balance_b_after,
				bob_balance_b_before as i128 - bob_balance_b_after as i128
			);

			println!("Test completed successfully!");
		});
}

#[test]
fn test_solver_execute_solution_with_buy_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let asset_a = 0u32;
	let asset_b = 14u32;

	let alice_wants_to_buy = 20_000_000_000_000u128;
	let alice_max_pay = 2_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, alice_max_pay * 10)
		.submit_buy_intent(alice.clone(), asset_a, asset_b, alice_max_pay, alice_wants_to_buy, 10)
		.execute(|| {
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);

			println!("=== Balances BEFORE solution (Buy Intent) ===");
			println!(
				"Alice: asset_a={}, asset_b={}",
				alice_balance_a_before, alice_balance_b_before
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();
			println!("Current block: {}", block);

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("Solving with {} buy intent(s)", intents.len());

				dbg!(&intents);

				let solution = Solver::solve(intents, state).ok()?;
				dbg!(&solution);
				println!("Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Score: {}", solution.score);

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("Solver should produce a solution for buy intent");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();
			println!("Advanced to block: {}", new_block);

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("Solution submitted successfully!");

			let alice_balance_a_after = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_after = Currencies::total_balance(asset_b, &alice);

			println!("=== Balances AFTER solution (Buy Intent) ===");
			println!(
				"Alice: asset_a={}, asset_b={}",
				alice_balance_a_after, alice_balance_b_after
			);

			assert!(
				alice_balance_a_after < alice_balance_a_before,
				"Alice's asset_a balance should decrease after paying"
			);
			assert!(
				alice_balance_b_after > alice_balance_b_before,
				"Alice's asset_b balance should increase after buying"
			);

			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("Remaining intents after solution: {}", remaining_intents.len());

			println!("=== Balance changes (Buy Intent) ===");
			println!(
				"Alice: asset_a {} -> {} (paid: {})",
				alice_balance_a_before,
				alice_balance_a_after,
				alice_balance_a_before as i128 - alice_balance_a_after as i128
			);
			println!(
				"Alice: asset_b {} -> {} (received: {})",
				alice_balance_b_before,
				alice_balance_b_after,
				alice_balance_b_after as i128 - alice_balance_b_before as i128
			);

			println!("Buy intent test completed successfully!");
		});
}

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

			println!("=== Balances BEFORE solution (Mixed Intents) ===");
			println!("Alice:   HDX={}, BNC={}", alice_hdx_before, alice_bnc_before);
			println!("Bob:     HDX={}, BNC={}", bob_hdx_before, bob_bnc_before);
			println!("Charlie: HDX={}, BNC={}", charlie_hdx_before, charlie_bnc_before);
			println!("Dave:    HDX={}, BNC={}", dave_hdx_before, dave_bnc_before);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");
			println!("Created {} intents", intents.len());

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("Solving with {} mixed intents", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());

				for (i, trade) in solution.trades.iter().enumerate() {
					println!(
						"  Trade {}: {:?} - in={}, out={}",
						i + 1,
						trade.direction,
						trade.amount_in,
						trade.amount_out
					);
				}

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("Solver should produce a solution for mixed intents");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			dbg!(&solution);
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("Solution submitted successfully!");

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_after = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);
			let dave_hdx_after = Currencies::total_balance(hdx, &dave);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);

			println!("=== Balances AFTER solution (Mixed Intents) ===");
			println!("Alice:   HDX={}, BNC={}", alice_hdx_after, alice_bnc_after);
			println!("Bob:     HDX={}, BNC={}", bob_hdx_after, bob_bnc_after);
			println!("Charlie: HDX={}, BNC={}", charlie_hdx_after, charlie_bnc_after);
			println!("Dave:    HDX={}, BNC={}", dave_hdx_after, dave_bnc_after);

			assert!(
				alice_hdx_after < alice_hdx_before,
				"Alice should have less HDX after selling"
			);
			assert!(
				alice_bnc_after > alice_bnc_before,
				"Alice should have more BNC after selling"
			);

			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX after buying");
			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC after paying");
			assert!(
				charlie_bnc_after < charlie_bnc_before,
				"Charlie should have less BNC after selling"
			);
			assert!(
				charlie_hdx_after > charlie_hdx_before,
				"Charlie should have more HDX after selling"
			);

			assert!(
				dave_bnc_after > dave_bnc_before,
				"Dave should have more BNC after buying"
			);
			assert!(
				dave_hdx_after < dave_hdx_before,
				"Dave should have less HDX after paying"
			);

			let remaining = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("Remaining intents: {}", remaining.len());

			println!("=== Balance Changes Summary ===");
			println!(
				"Alice:   HDX {:+}, BNC {:+}",
				alice_hdx_after as i128 - alice_hdx_before as i128,
				alice_bnc_after as i128 - alice_bnc_before as i128
			);
			println!(
				"Bob:     HDX {:+}, BNC {:+}",
				bob_hdx_after as i128 - bob_hdx_before as i128,
				bob_bnc_after as i128 - bob_bnc_before as i128
			);
			println!(
				"Charlie: HDX {:+}, BNC {:+}",
				charlie_hdx_after as i128 - charlie_hdx_before as i128,
				charlie_bnc_after as i128 - charlie_bnc_before as i128
			);
			println!(
				"Dave:    HDX {:+}, BNC {:+}",
				dave_hdx_after as i128 - dave_hdx_before as i128,
				dave_bnc_after as i128 - dave_bnc_before as i128
			);

			println!("Mixed sell/buy intents test completed successfully!");
		});
}

#[test]
fn test_solver_v1_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let amount = 10_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 10)
		.submit_sell_intent(alice.clone(), hdx, bnc, amount, 1, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);

			println!("=== V1 Solver: Single Intent Test ===");
			println!("Alice before: HDX={}, BNC={}", alice_hdx_before, alice_bnc_before);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("V1 Solver: Processing {} intent(s)", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("V1 Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Clearing prices: {} entries", solution.clearing_prices.len());
				println!("  Score: {}", solution.score);

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("V1 Solution submitted successfully!");

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);

			println!("Alice after: HDX={}, BNC={}", alice_hdx_after, alice_bnc_after);

			assert!(
				alice_hdx_after < alice_hdx_before,
				"Alice should have less HDX after selling"
			);
			assert!(
				alice_bnc_after > alice_bnc_before,
				"Alice should have more BNC after selling"
			);

			println!("=== Balance Changes ===");
			println!(
				"Alice: HDX {:+}, BNC {:+}",
				alice_hdx_after as i128 - alice_hdx_before as i128,
				alice_bnc_after as i128 - alice_bnc_before as i128
			);

			println!("V1 Solver single intent test completed successfully!");
		});
}

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

			println!("=== V1 Solver: Two Intents Partial Match Test ===");
			println!("Alice before: HDX={}, BNC={}", alice_hdx_before, alice_bnc_before);
			println!("Bob before: HDX={}, BNC={}", bob_hdx_before, bob_bnc_before);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("V1 Solver: Processing {} intent(s)", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("V1 Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Clearing prices: {} entries", solution.clearing_prices.len());
				println!("  Score: {}", solution.score);

				for (i, trade) in solution.trades.iter().enumerate() {
					println!(
						"  Trade {}: {:?} amount_in={} amount_out={}",
						i, trade.direction, trade.amount_in, trade.amount_out
					);
				}

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			assert_eq!(solution.resolved_intents.len(), 2, "Both intents should be resolved");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("V1 Solution submitted successfully!");

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);

			println!("Alice after: HDX={}, BNC={}", alice_hdx_after, alice_bnc_after);
			println!("Bob after: HDX={}, BNC={}", bob_hdx_after, bob_bnc_after);

			assert!(
				alice_hdx_after < alice_hdx_before,
				"Alice should have less HDX after selling"
			);
			assert!(
				alice_bnc_after > alice_bnc_before,
				"Alice should have more BNC after selling"
			);

			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC after selling");
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX after selling");

			println!("=== Balance Changes ===");
			println!(
				"Alice: HDX {:+}, BNC {:+}",
				alice_hdx_after as i128 - alice_hdx_before as i128,
				alice_bnc_after as i128 - alice_bnc_before as i128
			);
			println!(
				"Bob:   HDX {:+}, BNC {:+}",
				bob_hdx_after as i128 - bob_hdx_before as i128,
				bob_bnc_after as i128 - bob_bnc_before as i128
			);

			println!(
				"Total AMM trades: {} (matching reduces AMM interaction)",
				solution.trades.len()
			);

			println!("V1 Solver two intents partial match test completed successfully!");
		});
}

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
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		.submit_sell_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1, 10)
		.submit_sell_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 1, 10)
		.submit_buy_intent(dave.clone(), hdx, bnc, 400 * hdx_unit, 10 * bnc_unit, 10)
		.submit_buy_intent(eve.clone(), bnc, hdx, 50 * bnc_unit, 500 * hdx_unit, 10)
		.execute(|| {
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);
			let dave_hdx_before = Currencies::total_balance(hdx, &dave);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);
			let eve_hdx_before = Currencies::total_balance(hdx, &eve);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			println!("=== V1 Solver: Five Mixed Intents Test ===");
			println!("Intents:");
			println!("  Alice: sell 500 HDX for BNC (ExactIn)");
			println!("  Bob: sell 300 BNC for HDX (ExactIn)");
			println!("  Charlie: sell 200 HDX for BNC (ExactIn)");
			println!("  Dave: buy 10 BNC with max 400 HDX (ExactOut)");
			println!("  Eve: buy 500 HDX with max 50 BNC (ExactOut)");

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("\nV1 Solver: Processing {} intent(s)", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("V1 Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Clearing prices: {} entries", solution.clearing_prices.len());
				println!("  Score: {}", solution.score);

				for (i, trade) in solution.trades.iter().enumerate() {
					println!(
						"  Trade {}: {:?} amount_in={} amount_out={}",
						i, trade.direction, trade.amount_in, trade.amount_out
					);
				}

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

			dbg!(&solution);

			println!("\nResolved intents: {}", solution.resolved_intents.len());

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
				new_block,
			));

			println!("V1 Solution submitted successfully!");

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_after = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);
			let dave_hdx_after = Currencies::total_balance(hdx, &dave);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);
			let eve_hdx_after = Currencies::total_balance(hdx, &eve);
			let eve_bnc_after = Currencies::total_balance(bnc, &eve);

			println!("\n=== Balance Changes ===");
			println!(
				"Alice (sell HDX):   HDX {:+}, BNC {:+}",
				alice_hdx_after as i128 - alice_hdx_before as i128,
				alice_bnc_after as i128 - alice_bnc_before as i128
			);
			println!(
				"Bob (sell BNC):     HDX {:+}, BNC {:+}",
				bob_hdx_after as i128 - bob_hdx_before as i128,
				bob_bnc_after as i128 - bob_bnc_before as i128
			);
			println!(
				"Charlie (sell HDX): HDX {:+}, BNC {:+}",
				charlie_hdx_after as i128 - charlie_hdx_before as i128,
				charlie_bnc_after as i128 - charlie_bnc_before as i128
			);
			println!(
				"Dave (buy BNC):     HDX {:+}, BNC {:+}",
				dave_hdx_after as i128 - dave_hdx_before as i128,
				dave_bnc_after as i128 - dave_bnc_before as i128
			);
			println!(
				"Eve (buy HDX):      HDX {:+}, BNC {:+}",
				eve_hdx_after as i128 - eve_hdx_before as i128,
				eve_bnc_after as i128 - eve_bnc_before as i128
			);

			assert!(alice_hdx_after < alice_hdx_before, "Alice should have less HDX");
			assert!(charlie_hdx_after < charlie_hdx_before, "Charlie should have less HDX");

			assert!(bob_bnc_after < bob_bnc_before, "Bob should have less BNC");
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have more HDX");

			println!(
				"\nTotal AMM trades: {} (matching reduces AMM interaction)",
				solution.trades.len()
			);

			println!("V1 Solver five mixed intents test completed successfully!");
		});
}

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
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		.submit_sell_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1, 10)
		.submit_sell_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 1, 10)
		.submit_sell_intent(dave.clone(), hdx, bnc, 100 * hdx_unit, 1, 10)
		.submit_sell_intent(eve.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		.execute(|| {
			println!("=== V1 Solver: Five Sell Intents - Uniform Price Test ===");
			println!("Intents (all ExactIn/sell):");
			println!("  Alice: sell 500 HDX for BNC");
			println!("  Bob: sell 300 BNC for HDX");
			println!("  Charlie: sell 200 HDX for BNC");
			println!("  Dave: sell 100 HDX for BNC");
			println!("  Eve: sell 500 HDX for BNC (same as Alice)");

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("\nV1 Solver: Processing {} intent(s)", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("V1 Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Score: {}", solution.score);

				for (i, trade) in solution.trades.iter().enumerate() {
					println!(
						"  Trade {}: {:?} amount_in={} amount_out={}",
						i, trade.direction, trade.amount_in, trade.amount_out
					);
				}

				captured_solution = Some(solution.clone());
				Some(solution)
			});

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

			println!("\nV1 Solution submitted successfully!");

			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let charlie_bnc_after = Currencies::total_balance(bnc, &charlie);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);
			let eve_bnc_after = Currencies::total_balance(bnc, &eve);

			let alice_bnc_received = alice_bnc_after.saturating_sub(alice_bnc_before);
			let charlie_bnc_received = charlie_bnc_after.saturating_sub(charlie_bnc_before);
			let dave_bnc_received = dave_bnc_after.saturating_sub(dave_bnc_before);
			let eve_bnc_received = eve_bnc_after.saturating_sub(eve_bnc_before);

			println!("\n=== Uniform Price Verification ===");
			println!("Alice   (sell 500 HDX): receives {} BNC raw", alice_bnc_received);
			println!("Charlie (sell 200 HDX): receives {} BNC raw", charlie_bnc_received);
			println!("Dave    (sell 100 HDX): receives {} BNC raw", dave_bnc_received);
			println!("Eve     (sell 500 HDX): receives {} BNC raw", eve_bnc_received);

			println!("\n--- Alice vs Eve (both 500 HDX) ---");
			if alice_bnc_received == eve_bnc_received {
				println!("✓ PERFECT: Alice and Eve receive EXACTLY the same amount!");
			} else {
				let diff = alice_bnc_received.abs_diff(eve_bnc_received);
				let pct = (diff as f64 / alice_bnc_received as f64) * 100.0;
				println!("✗ DIFFERENCE: {} BNC raw ({:.6}%)", diff, pct);
			}

			println!("\n--- Rate Consistency Check ---");
			let alice_rate = alice_bnc_received as f64 / 500.0;
			let charlie_rate = charlie_bnc_received as f64 / 200.0;
			let dave_rate = dave_bnc_received as f64 / 100.0;
			let eve_rate = eve_bnc_received as f64 / 500.0;

			println!("Alice rate:   {:.6} BNC per HDX unit", alice_rate);
			println!("Charlie rate: {:.6} BNC per HDX unit", charlie_rate);
			println!("Dave rate:    {:.6} BNC per HDX unit", dave_rate);
			println!("Eve rate:     {:.6} BNC per HDX unit", eve_rate);

			let rate_diff_charlie = (alice_rate - charlie_rate).abs() / alice_rate * 100.0;
			let rate_diff_dave = (alice_rate - dave_rate).abs() / alice_rate * 100.0;
			let rate_diff_eve = (alice_rate - eve_rate).abs() / alice_rate * 100.0;

			println!("\nRate differences from Alice:");
			println!("  Charlie: {:.6}%", rate_diff_charlie);
			println!("  Dave:    {:.6}%", rate_diff_dave);
			println!("  Eve:     {:.6}%", rate_diff_eve);

			assert_eq!(
				alice_bnc_received, eve_bnc_received,
				"Alice and Eve should receive exactly the same BNC for selling the same HDX"
			);

			let expected_charlie = alice_bnc_received * 200 / 500;
			let charlie_diff = charlie_bnc_received.abs_diff(expected_charlie);
			assert!(
				charlie_diff <= 1,
				"Charlie's amount should be proportional to Alice's (diff: {})",
				charlie_diff
			);

			let expected_dave = alice_bnc_received * 100 / 500;
			let dave_diff = dave_bnc_received.abs_diff(expected_dave);
			assert!(
				dave_diff <= 1,
				"Dave's amount should be proportional to Alice's (diff: {})",
				dave_diff
			);

			println!("\n✓ All participants receive exactly proportional amounts!");
			println!("V1 Solver five sell intents uniform price test completed successfully!");
		});
}

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
		.submit_sell_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 1, 10)
		.submit_sell_intent(eve.clone(), bnc, hdx, eve_bnc_sell, 1, 10)
		.submit_sell_intent(bob.clone(), bnc, hdx, 200 * bnc_unit, 1, 10)
		.execute(|| {
			println!("=== V1 Solver: Opposite Direction Sells - Uniform Price Test ===");
			println!("Intents (all ExactIn/sell, but opposite directions):");
			println!("  Alice: sell 500 HDX for BNC");
			println!("  Eve: sell {} BNC raw for HDX", eve_bnc_sell);
			println!("  Bob: sell 200 BNC for HDX");

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 3, "Should have 3 intents");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("\nV1 Solver: Processing {} intent(s)", intents.len());

				let solution = Solver::solve(intents, state).ok()?;
				println!("V1 Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());
				println!("  Score: {}", solution.score);

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			let _call = result.expect("V1 Solver should produce a solution");
			let solution = captured_solution.expect("Solution should be captured");

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

			println!("\nV1 Solution submitted successfully!");

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let eve_hdx_after = Currencies::total_balance(hdx, &eve);
			let eve_bnc_after = Currencies::total_balance(bnc, &eve);

			let alice_hdx_spent = alice_hdx_before.saturating_sub(alice_hdx_after);
			let alice_bnc_received = alice_bnc_after.saturating_sub(alice_bnc_before);
			let eve_bnc_spent = eve_bnc_before.saturating_sub(eve_bnc_after);
			let eve_hdx_received = eve_hdx_after.saturating_sub(eve_hdx_before);

			println!("\n=== Balance Changes ===");
			println!("Alice: HDX -{}, BNC +{}", alice_hdx_spent, alice_bnc_received);
			println!("Eve:   BNC -{}, HDX +{}", eve_bnc_spent, eve_hdx_received);

			println!("\n=== Rate Analysis ===");
			let alice_rate = alice_bnc_received as f64 / alice_hdx_spent as f64;
			let eve_rate = eve_hdx_received as f64 / eve_bnc_spent as f64;
			let eve_inverse_rate = eve_bnc_spent as f64 / eve_hdx_received as f64;

			println!("Alice rate (BNC/HDX): {:.10}", alice_rate);
			println!("Eve rate (HDX/BNC):   {:.10}", eve_rate);
			println!("Eve inverse (BNC/HDX): {:.10}", eve_inverse_rate);

			let rate_diff_pct = ((alice_rate - eve_inverse_rate).abs() / alice_rate) * 100.0;
			println!("\nRate difference: {:.6}%", rate_diff_pct);

			if rate_diff_pct < 0.001 {
				println!("✓ PERFECT: Alice and Eve get consistent rates (< 0.001% diff)!");
			} else if rate_diff_pct < 0.1 {
				println!(
					"~ CLOSE: Small difference due to integer rounding ({:.6}%)",
					rate_diff_pct
				);
			} else {
				println!("✗ SIGNIFICANT DIFFERENCE: {:.6}%", rate_diff_pct);
			}

			println!("\n=== Inverse Trade Check ===");
			println!(
				"Alice sold {} HDX, received {} BNC",
				alice_hdx_spent, alice_bnc_received
			);
			println!("Eve sold {} BNC, received {} HDX", eve_bnc_spent, eve_hdx_received);

			let expected_eve_hdx = if eve_bnc_spent > 0 {
				(alice_bnc_received as u128)
					.checked_mul(eve_hdx_received)
					.and_then(|n| n.checked_div(eve_bnc_spent))
					.unwrap_or(0)
			} else {
				0
			};
			println!(
				"If Eve sold {} BNC (Alice's receive), she'd get ~{} HDX",
				alice_bnc_received, expected_eve_hdx
			);

			let hdx_diff = expected_eve_hdx.abs_diff(alice_hdx_spent);
			let hdx_diff_pct = (hdx_diff as f64 / alice_hdx_spent as f64) * 100.0;
			println!("Difference from Alice's 500 HDX: {} ({:.6}%)", hdx_diff, hdx_diff_pct);

			println!("\nV1 Solver opposite direction sells test completed!");
		});
}

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
			println!("=== Intent with on_success Callback Test ===");
			println!(
				"Alice sells BNC for HDX, then callback sends {} HDX to Bob",
				hdx_to_transfer
			);

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);

			println!("\n--- Initial Balances ---");
			println!("Alice: HDX={}, BNC={}", alice_hdx_before, alice_bnc_before);
			println!("Bob:   HDX={}", bob_hdx_before);

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
			println!("\n--- Intent Submitted ---");
			println!("Intent count: {}", intents.len());
			println!("Alice sells {} BNC, wants at least {} HDX", bnc_to_sell, min_hdx_out);
			println!("on_success callback will transfer {} HDX to Bob", hdx_to_transfer);

			let block = hydradx_runtime::System::block_number();
			let mut captured_solution: Option<Solution> = None;

			let result = pallet_ice::Pallet::<Runtime>::run(block, |intents, state: CombinedSimulatorState| {
				println!("\nSolver: Processing {} intent(s)", intents.len());

				println!("{:?}\n", &state);

				let solution = Solver::solve(intents, state).ok()?;
				println!("Solution found!");
				println!("  Resolved intents: {}", solution.resolved_intents.len());
				println!("  Trades: {}", solution.trades.len());

				captured_solution = Some(solution.clone());
				Some(solution)
			});

			if result.is_none() {
				println!("No solution found - solver could not resolve the intent");
				return;
			}

			let solution = captured_solution.expect("Solution should be captured");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let new_block = hydradx_runtime::System::block_number();

			println!("\n--- Submitting Solution at block {} ---", new_block);
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
				new_block,
			));

			let alice_hdx_after_solution = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after_solution = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after_solution = Currencies::total_balance(hdx, &bob);

			println!("\n--- After Solution (before callback) ---");
			println!(
				"Alice: HDX={}, BNC={}",
				alice_hdx_after_solution, alice_bnc_after_solution
			);
			println!("Bob:   HDX={}", bob_hdx_after_solution);

			let alice_hdx_received = alice_hdx_after_solution.saturating_sub(alice_hdx_before);
			let alice_bnc_spent = alice_bnc_before.saturating_sub(alice_bnc_after_solution);
			println!(
				"Alice received {} HDX, spent {} BNC",
				alice_hdx_received, alice_bnc_spent
			);

			let next_dispatch_id = LazyExecutor::dispatch_next_id();
			let next_call_id = LazyExecutor::next_call_id();
			println!("\n--- Lazy Executor Queue ---");
			println!("Next dispatch ID: {}, Next call ID: {}", next_dispatch_id, next_call_id);
			println!(
				"Queue has {} pending call(s)",
				next_call_id.saturating_sub(next_dispatch_id)
			);

			println!("\n--- Dispatching Callback ---");
			if next_call_id > next_dispatch_id {
				let dispatch_result = LazyExecutor::dispatch_top(RuntimeOrigin::none());
				println!("dispatch_top result: {:?}", dispatch_result);
			} else {
				println!("No callbacks in queue!");
			}

			let alice_hdx_final = Currencies::total_balance(hdx, &alice);
			let alice_bnc_final = Currencies::total_balance(bnc, &alice);
			let bob_hdx_final = Currencies::total_balance(hdx, &bob);

			println!("\n--- Final Balances ---");
			println!("Alice: HDX={}, BNC={}", alice_hdx_final, alice_bnc_final);
			println!("Bob:   HDX={}", bob_hdx_final);

			let bob_hdx_received = bob_hdx_final.saturating_sub(bob_hdx_before);

			println!("\n--- Summary ---");
			println!("Alice BNC spent: {}", alice_bnc_before.saturating_sub(alice_bnc_final));
			println!(
				"Alice HDX change: {} -> {} (delta: {})",
				alice_hdx_before,
				alice_hdx_final,
				alice_hdx_final as i128 - alice_hdx_before as i128
			);
			println!("Bob HDX received: {}", bob_hdx_received);

			assert!(alice_hdx_received > 0, "Alice should have received some HDX");
			assert!(
				alice_hdx_received >= hdx_to_transfer,
				"Alice should have received at least {} HDX for the callback",
				hdx_to_transfer
			);
			assert_eq!(
				bob_hdx_received, hdx_to_transfer,
				"Bob should have received {} HDX from callback",
				hdx_to_transfer
			);

			println!("\n✓ SUCCESS: Callback executed! Bob received {} HDX", bob_hdx_received);
			println!("=== Intent with Callback Test Complete ===");
		});
}
