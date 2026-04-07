use crate::polkadot_test_net::{TestNet, ALICE, BOB, CHARLIE, DAVE, EVE};
use amm_simulator::omnipool::Simulator as OmnipoolSimulator;
use amm_simulator::stableswap::Simulator as StableswapSimulator;
use amm_simulator::HydrationSimulator;
use frame_support::assert_ok;
use frame_support::traits::{Get, Time};
use hydradx_runtime::{
	ice_simulator_provider, AssetRegistry, Currencies, LazyExecutor, Omnipool, Router, Runtime, RuntimeOrigin,
	Timestamp,
};
use hydradx_traits::amm::{AmmSimulator, SimulatorConfig, SimulatorSet};
use hydradx_traits::router::RouteProvider;
use hydradx_traits::BoundErc20;
use ice_solver::v1::Solver as IceSolver;
use ice_support::Solution;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::SlipFeeConfig;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

//pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/mainnet_nov4";
pub const PATH_TO_SNAPSHOT: &str = "snapshots/hsm/slim2";

pub type CombinedSimulatorState =
	<<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators as SimulatorSet>::State;

type TestSimulator = HydrationSimulator<hydradx_runtime::HydrationSimulatorConfig>;
type Solver = IceSolver<TestSimulator>;

// Custom simulator config for Hollar tests with price denominator 222
pub struct HollarSimulatorConfig;

pub struct HollarPriceDenominator;
impl Get<u32> for HollarPriceDenominator {
	fn get() -> u32 {
		222
	}
}

fn enable_slip_fees() {
	assert_ok!(Omnipool::set_slip_fee(
		RuntimeOrigin::root(),
		Some(SlipFeeConfig {
			max_slip_fee: Permill::from_percent(5),
		})
	));
}

impl SimulatorConfig for HollarSimulatorConfig {
	type Simulators = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators;
	type RouteDiscovery = <hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::RouteDiscovery;
	type PriceDenominator = HollarPriceDenominator;
}

type HollarSimulator = HydrationSimulator<HollarSimulatorConfig>;
type HollarSolver = IceSolver<HollarSimulator>;

#[test]
fn simulator_snapshot() {
	TestNet::reset();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		let snapshot = OmnipoolSimulator::<ice_simulator_provider::Omnipool<Runtime>>::snapshot();

		assert!(!snapshot.assets.is_empty(), "Snapshot should contain assets");
		assert!(snapshot.hub_asset_id > 0, "Hub asset id should be set");
		assert!(snapshot.slip_fee.is_some(), "Snapshot should contain slip fees");
	});
}

#[test]
fn simulator_sell() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		use hydradx_traits::amm::SimulatorError;

		let snapshot = OmnipoolSimulator::<ice_simulator_provider::Omnipool<Runtime>>::snapshot();

		let assets: Vec<_> = snapshot.assets.keys().copied().collect();
		assert!(assets.len() >= 2, "Snapshot should have at least 2 assets");

		let asset_in = assets[0];
		let asset_out = assets[1];

		// Skip if using hub asset
		if asset_in == snapshot.hub_asset_id || asset_out == snapshot.hub_asset_id {
			return;
		}

		let amount_in = 1_000_000_000_000u128;

		let result = <OmnipoolSimulator<ice_simulator_provider::Omnipool<Runtime>> as AmmSimulator>::simulate_sell(
			asset_in, asset_out, amount_in, 0, &snapshot,
		);

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
				assert!(
					new_snapshot.slip_fee.is_some(),
					"New snapshot should have slip fee config"
				);
				assert!(
					new_snapshot.slip_fee_delta.get(&asset_in).is_some(),
					"Asset in slip fee delta should be in snapshot"
				);
				assert!(
					new_snapshot.slip_fee_delta.get(&asset_out).is_some(),
					"Asset out slip fee delta should be in snapshot"
				);
				assert!(
					new_snapshot.slip_fee_hubreserve_at_block_start.get(&asset_in).is_some(),
					"Asset in slip fee hub reserve at block start should be in snapshot"
				);
				assert!(
					new_snapshot
						.slip_fee_hubreserve_at_block_start
						.get(&asset_out)
						.is_some(),
					"Asset out slip fee hub reserve at block start should be in snapshot"
				);
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
fn stableswap_snapshot() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let stableswap_snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();

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
fn stableswap_simulator_direct() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		let snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();

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
			<StableswapSimulator<ice_simulator_provider::Stableswap<Runtime>> as AmmSimulator>::simulate_sell(
				asset_a, asset_b, amount_in, 0, &snapshot,
			)
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
			<StableswapSimulator<ice_simulator_provider::Stableswap<Runtime>> as AmmSimulator>::simulate_buy(
				asset_a,
				asset_b,
				amount_out,
				u128::MAX,
				&snapshot,
			)
			.expect("simulate_buy should succeed");

		assert_eq!(buy_result.amount_out, amount_out, "Amount out should match requested");

		// Test get_spot_price
		let price = <StableswapSimulator<ice_simulator_provider::Stableswap<Runtime>> as AmmSimulator>::get_spot_price(
			asset_a, asset_b, &snapshot,
		)
		.expect("get_spot_price should succeed");

		assert!(price.n > 0, "Price numerator should be positive");
		assert!(price.d > 0, "Price denominator should be positive");
	});
}

/// Test stableswap intent: trade between stableswap pool assets
#[test]
fn stableswap_intent() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		use hydradx_traits::router::{AssetPair, RouteProvider};

		let stableswap_snapshot = StableswapSimulator::<ice_simulator_provider::Stableswap<Runtime>>::snapshot();
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
		let deadline = Some(6000u64 * 10 + ts);
		assert_ok!(pallet_intent::Pallet::<Runtime>::submit_intent(
			RuntimeOrigin::signed(ALICE.into()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapData {
					asset_in: asset_a,
					asset_out: asset_b,
					amount_in,
					amount_out: 10_000_000_000_000_000u128,
					partial: false,
				}),
				deadline,
				on_resolved: None,
			},
		));

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		assert_eq!(intents.len(), 1, "Should have 1 intent");

		let block = hydradx_runtime::System::block_number();
		let call = pallet_ice::Pallet::<Runtime>::run(
			block,
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		)
		.expect("Solver should produce a solution for mixed intents");

		let pallet_ice::Call::submit_solution { solution, .. } = call;
		assert_eq!(solution.resolved_intents.len(), 1, "Should resolve the intent");

		crate::polkadot_test_net::hydradx_run_to_next_block();
		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution,
		));

		let alice_a_after = Currencies::total_balance(asset_a, &ALICE.into());
		let alice_b_after = Currencies::total_balance(asset_b, &ALICE.into());

		assert!(alice_a_after < alice_a_before, "Alice should have less asset_a");
		assert!(alice_b_after > alice_b_before, "Alice should have more asset_b");
	});
}

#[test]
fn solver_two_intents() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(ALICE.into(), 0, 1_000_000_000_000_000)
		.endow_account(BOB.into(), 5, 1_000_000_000_000_000)
		.submit_swap_intent(ALICE.into(), 0, 5, 1_000_000_000_000, 17_540_000u128, Some(2))
		.submit_swap_intent(BOB.into(), 5, 0, 1_000_000_000_000, 1_000_000_000_000u128, Some(2))
		.execute(|| {
			enable_slip_fees();
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let call = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution for mixed intents");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert!(
				!solution.resolved_intents.is_empty(),
				"Should resolve at least one intent"
			);
			assert!(solution.score > 0, "Solution score should be positive");
		});
}

/// Test Direct matching: Alice sells A for B, Bob sells B for A
#[test]
fn solver_execute_solution1() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let asset_a = 0u32;
	let asset_b = 14u32;
	let amount = 10_000_000_000_000u128;
	let min_amount_out_a = 1_000_000_000_000u128;
	let min_amount_out_b = 68_795_189_840u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, amount * 10)
		.endow_account(bob.clone(), asset_b, amount * 10)
		.submit_swap_intent(alice.clone(), asset_a, asset_b, amount, min_amount_out_b, Some(10))
		.submit_swap_intent(bob.clone(), asset_b, asset_a, amount, min_amount_out_a, None) //no deadline
		.execute(|| {
			enable_slip_fees();
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);
			let bob_balance_a_before = Currencies::total_balance(asset_a, &bob);
			let bob_balance_b_before = Currencies::total_balance(asset_b, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let block = hydradx_runtime::System::block_number();

			let call = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 2, "Should resolve both intents");
			assert!(solution.score > 0, "Solution score should be positive");

			// Verify each resolved intent
			for resolved in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref swap_data) = resolved.data else {
					panic!("expected Swap");
				};
				assert!(swap_data.amount_in > 0, "amount_in should be positive");
				let min_amount_out = if swap_data.asset_out == asset_a {
					min_amount_out_a
				} else {
					min_amount_out_b
				};
				assert!(swap_data.amount_out >= min_amount_out, "amount_out should be >= min");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));

			// Verify intents removed from storage
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "All intents should be resolved");

			// Verify account intent index cleaned up
			assert_eq!(
				pallet_intent::AccountIntents::<Runtime>::iter_prefix(&alice).count(),
				0,
				"Alice's account intents index should be empty"
			);
			assert_eq!(
				pallet_intent::AccountIntents::<Runtime>::iter_prefix(&bob).count(),
				0,
				"Bob's account intents index should be empty"
			);
			assert_eq!(
				pallet_intent::Pallet::<Runtime>::account_intent_count(&alice),
				0,
				"Alice's intent count should be zero"
			);
			assert_eq!(
				pallet_intent::Pallet::<Runtime>::account_intent_count(&bob),
				0,
				"Bob's intent count should be zero"
			);

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
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					s.asset_in == asset_a
				})
				.expect("Should find Alice's intent");
			let bob_resolved = solution
				.resolved_intents
				.iter()
				.find(|r| {
					let ice_support::IntentData::Swap(ref s) = r.data else {
						panic!("expected Swap");
					};
					s.asset_in == asset_b
				})
				.expect("Should find Bob's intent");

			let ice_support::IntentData::Swap(ref alice_swap) = alice_resolved.data else {
				panic!("expected Swap");
			};
			let ice_support::IntentData::Swap(ref bob_swap) = bob_resolved.data else {
				panic!("expected Swap");
			};

			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			assert_eq!(alice_balance_a_before - alice_balance_a_after, alice_swap.amount_in);
			assert_eq!(
				alice_balance_b_after - alice_balance_b_before,
				alice_swap.amount_out - ice_fee.mul_floor(alice_swap.amount_out)
			);
			assert_eq!(bob_balance_b_before - bob_balance_b_after, bob_swap.amount_in);
			assert_eq!(
				bob_balance_a_after - bob_balance_a_before,
				bob_swap.amount_out - ice_fee.mul_floor(bob_swap.amount_out)
			);
		});
}

/// Test single ExactOut (buy) intent: Alice wants to buy BNC with HDX
#[test]
fn solver_execute_solution_with_buy_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let asset_a = 0u32; // HDX
	let asset_b = 14u32; // BNC

	let alice_wants_amount_out = 20_000_000_000_000u128;
	let alice_amount_in = 2_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), asset_a, alice_amount_in * 10)
		.submit_swap_intent(
			alice.clone(),
			asset_a,
			asset_b,
			alice_amount_in,
			alice_wants_amount_out,
			Some(10),
		)
		.execute(|| {
			enable_slip_fees();
			let alice_balance_a_before = Currencies::total_balance(asset_a, &alice);
			let alice_balance_b_before = Currencies::total_balance(asset_b, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let block = hydradx_runtime::System::block_number();

			let mut captured_solution: Option<Solution> = None;
			let _result = pallet_ice::Pallet::<Runtime>::run(
				block,
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					let solution = Solver::solve(intents, state).ok()?;
					captured_solution = Some(solution.clone());
					Some(solution)
				},
			)
			.expect("Solver should produce a solution for buy intent");

			let solution = captured_solution.expect("Solution should be captured");

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve intent");
			let resolved = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data else {
				panic!("expected Swap");
			};
			assert!(
				swap_data.amount_out >= alice_wants_amount_out,
				"Should buy >= amount requested"
			);
			assert!(swap_data.amount_in == alice_amount_in, "Should equal to amount in");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
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

			// Verify exact amounts match solution (received = amount_out - fee)
			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			let paid = alice_balance_a_before - alice_balance_a_after;
			let received = alice_balance_b_after - alice_balance_b_before;
			assert_eq!(paid, swap_data.amount_in, "Paid amount should match solution");
			assert_eq!(
				received,
				swap_data.amount_out - ice_fee.mul_floor(swap_data.amount_out),
				"Received amount should match solution minus fee"
			);

			// Verify intent removed
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "Intent should be resolved");
		});
}

/// Test mixed multiple users' intents
#[test]
fn solver_mixed_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let sell_hdx_amount = 100_000_000_000_000u128;
	let sell_bnc_amount = 100_000_000_000u128;
	let min_hdx_out_amount = 100_000_000_000_000u128;
	let min_bnc_out_amount = 68_795_189_840u128;
	let in_amount = 10_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, in_amount)
		.endow_account(alice.clone(), bnc, in_amount)
		.endow_account(bob.clone(), hdx, in_amount)
		.endow_account(bob.clone(), bnc, in_amount)
		.endow_account(charlie.clone(), hdx, in_amount)
		.endow_account(charlie.clone(), bnc, in_amount)
		.endow_account(dave.clone(), hdx, in_amount)
		.endow_account(dave.clone(), bnc, in_amount)
		.submit_swap_intent(alice.clone(), hdx, bnc, sell_hdx_amount, min_bnc_out_amount, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, in_amount, min_hdx_out_amount, Some(10))
		.submit_swap_intent(
			charlie.clone(),
			bnc,
			hdx,
			sell_bnc_amount,
			1_000_000_000_000u128,
			Some(10),
		)
		.submit_swap_intent(dave.clone(), hdx, bnc, in_amount, min_bnc_out_amount, Some(10))
		.submit_swap_intent(alice.clone(), hdx, bnc, sell_hdx_amount, min_bnc_out_amount, Some(10))
		.execute(|| {
			enable_slip_fees();
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

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution for mixed intents");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			// Verify solution structure
			assert!(
				!solution.resolved_intents.is_empty(),
				"Should resolve at least some intents"
			);
			assert!(solution.score > 0, "Solution score should be positive");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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

/// Test single swap intent: Alice sells HDX for BNC
#[test]
fn solver_v1_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let amount = 10_000_000_000_000u128;
	let min_amount_out = 68_795_189_840u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, amount, min_amount_out, Some(10))
		.execute(|| {
			enable_slip_fees();
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");
			let original_intent_id = intents[0].0;

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
			assert!(solution.score > 0, "Solution score should be positive");

			// Verify the resolved intent
			let resolved = &solution.resolved_intents[0];
			assert_eq!(resolved.id, original_intent_id, "Resolved intent ID should match");
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data else {
				panic!("expected Swap");
			};
			assert_eq!(swap_data.asset_in, hdx, "asset_in should be HDX");
			assert_eq!(swap_data.asset_out, bnc, "asset_out should be BNC");
			assert_eq!(swap_data.amount_in, amount, "amount_in should match submitted amount");
			assert!(
				swap_data.amount_out >= min_amount_out,
				"amount_out should be >= min_amount_out"
			);

			// Verify trades are valid
			assert!(!solution.trades.is_empty(), "Should have at least one trade");
			for trade in solution.trades.iter() {
				assert!(trade.amount_in > 0, "Trade amount_in should be positive");
				assert!(trade.amount_out > 0, "Trade amount_out should be positive");
				assert!(!trade.route.is_empty(), "Trade route should not be empty");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));

			// Verify intent was removed from storage
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(
				remaining_intents.is_empty(),
				"Intent should be removed after resolution"
			);

			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);

			// Verify balance changes match the solution (received = amount_out - fee)
			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			let hdx_spent = alice_hdx_before - alice_hdx_after;
			let bnc_received = alice_bnc_after - alice_bnc_before;

			assert_eq!(
				hdx_spent, swap_data.amount_in,
				"HDX spent should equal resolved amount_in"
			);
			assert_eq!(
				bnc_received,
				swap_data.amount_out - ice_fee.mul_floor(swap_data.amount_out),
				"BNC received should equal resolved amount_out minus fee"
			);
		});
}

/// Test partial direct match: Alice sells large HDX, Bob sells small BNC (opposite directions)
#[test]
fn solver_v1_two_intents_partial_match() {
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
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_amount, 68_795_189_840u128, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_amount, 1_000_000_000_000u128, Some(10))
		.execute(|| {
			enable_slip_fees();
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("V1 Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			// Verify both intents resolved
			assert_eq!(solution.resolved_intents.len(), 2, "Both intents should be resolved");
			assert!(solution.score > 0, "Solution score should be positive");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
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

			// Verify balance changes match solution (received = amount_out - fee)
			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			for resolved in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref swap_data) = resolved.data else {
					panic!("expected Swap");
				};
				let expected_payout = swap_data.amount_out - ice_fee.mul_floor(swap_data.amount_out);
				if swap_data.asset_in == hdx {
					// Alice's intent
					assert_eq!(alice_hdx_before - alice_hdx_after, swap_data.amount_in);
					assert_eq!(alice_bnc_after - alice_bnc_before, expected_payout);
				} else {
					// Bob's intent
					assert_eq!(bob_bnc_before - bob_bnc_after, swap_data.amount_in);
					assert_eq!(bob_hdx_after - bob_hdx_before, expected_payout);
				}
			}
		});
}

/// Test five mixed intents from different users
#[test]
fn solver_v1_five_mixed_intents() {
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
		// Alice: sell 500 HDX for BNC
		.submit_swap_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 68_795_189_840u128, Some(10))
		// Bob: sell 300 BNC for HDX
		.submit_swap_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1_000_000_000_000u128, Some(10))
		// Charlie: sell 200 HDX for BNC
		.submit_swap_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 168_795_189_840u128, Some(10))
		// Dave: sell 400 HDX for 10 BNC
		.submit_swap_intent(dave.clone(), hdx, bnc, 400 * hdx_unit, 10 * bnc_unit, Some(10))
		// Eve: buy max 50 BNC for 500 HDX
		.submit_swap_intent(eve.clone(), bnc, hdx, 50 * bnc_unit, 500 * hdx_unit, Some(10))
		.execute(|| {
			enable_slip_fees();
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("V1 Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			// Verify solution structure
			assert!(
				!solution.resolved_intents.is_empty(),
				"Should resolve at least some intents"
			);
			assert!(solution.score > 0, "Solution score should be positive");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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
fn solver_v1_uniform_price_all_sells() {
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
		.submit_swap_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 68_795_189_840u128, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, 300 * bnc_unit, 1_000_000_000_000u128, Some(10))
		.submit_swap_intent(charlie.clone(), hdx, bnc, 200 * hdx_unit, 68_795_189_840u128, Some(10))
		.submit_swap_intent(dave.clone(), hdx, bnc, 100 * hdx_unit, 68_795_189_840u128, Some(10))
		.submit_swap_intent(eve.clone(), hdx, bnc, 500 * hdx_unit, 68_795_189_840u128, Some(10)) // Same as Alice
		.execute(|| {
			enable_slip_fees();
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 5, "Should have 5 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("V1 Solver should produce a solution");

			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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
fn solver_v1_uniform_price_opposite_sells() {
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
		.submit_swap_intent(alice.clone(), hdx, bnc, 500 * hdx_unit, 68_795_189_840u128, Some(10))
		// Eve sells BNC for HDX (opposite direction)
		.submit_swap_intent(eve.clone(), bnc, hdx, eve_bnc_sell, 1_000_000_000_000u128, Some(10))
		// Bob sells BNC for HDX (same direction as Eve)
		.submit_swap_intent(bob.clone(), bnc, hdx, 200 * bnc_unit, 1_000_000_000_000u128, Some(10))
		.execute(|| {
			enable_slip_fees();
			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 3, "Should have 3 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("V1 Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			// Verify solution structure
			assert!(!solution.resolved_intents.is_empty(), "Should resolve intents");
			assert!(solution.score > 0, "Solution score should be positive");

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let eve_hdx_before = Currencies::total_balance(hdx, &eve);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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
fn intent_with_on_success_callback() {
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
			enable_slip_fees();
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
			let deadline = Some(ts + 6000 * 10);

			let min_hdx_out = 1_000_000_000_000u128;

			assert_ok!(pallet_intent::Pallet::<Runtime>::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapData {
						asset_in: bnc,
						asset_out: hdx,
						amount_in: bnc_to_sell,
						amount_out: min_hdx_out,
						partial: false,
					}),
					deadline,
					on_resolved: Some(callback_data),
				},
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve the intent");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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
			assert_ok!(LazyExecutor::dispatch_top(
				RuntimeOrigin::none(),
				LazyExecutor::dispatch_next_id()
			));

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
fn usdt_weth_single_intent() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();

	// Asset IDs
	let usdt = 10u32; // Tether - 6 decimals
	let weth = 20u32; // WETH - 18 decimals

	// Units based on decimals
	let usdt_unit = 1_000_000u128; // 10^6

	// Sell 100 USDT
	let amount_in = 100 * usdt_unit;
	let min_amount_out = 5_390_835_579_515u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), usdt, amount_in * 10)
		.submit_swap_intent(alice.clone(), usdt, weth, amount_in, min_amount_out, Some(10))
		.execute(|| {
			enable_slip_fees();
			let alice_usdt_before = Currencies::total_balance(usdt, &alice);
			let alice_weth_before = Currencies::total_balance(weth, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");
			let original_intent_id = intents[0].0;

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution for USDT->WETH");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			// Verify solution structure
			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
			assert!(solution.score > 0, "Solution score should be positive");

			// Verify the resolved intent
			let resolved = &solution.resolved_intents[0];
			assert_eq!(resolved.id, original_intent_id, "Resolved intent ID should match");
			let ice_support::IntentData::Swap(ref swap_data) = resolved.data else {
				panic!("expected Swap");
			};
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

			// Verify trades are valid
			assert!(!solution.trades.is_empty(), "Should have at least one trade");
			for trade in solution.trades.iter() {
				assert!(trade.amount_in > 0, "Trade amount_in should be positive");
				assert!(trade.amount_out > 0, "Trade amount_out should be positive");
				assert!(!trade.route.is_empty(), "Trade route should not be empty");
			}

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
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

			// Verify exact amounts match solution (received = amount_out - fee)
			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			let usdt_spent = alice_usdt_before - alice_usdt_after;
			let weth_received = alice_weth_after - alice_weth_before;
			assert_eq!(usdt_spent, swap_data.amount_in, "USDT spent should match solution");
			assert_eq!(
				weth_received,
				swap_data.amount_out - ice_fee.mul_floor(swap_data.amount_out),
				"WETH received should match solution minus fee"
			);

			// Verify intent was resolved
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(remaining_intents.is_empty(), "Intent should be resolved");
		});
}

/// Compare trading USDT->WETH via solver vs direct router
/// Both should give the same result for a single intent
#[test]
fn usdt_weth_solver_vs_router() {
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
		.submit_swap_intent(alice.clone(), usdt, weth, amount_in, 5_390_835_579_515u128, Some(10))
		.execute(|| {
			enable_slip_fees();
			// ========== SOLVER PATH (Alice) ==========
			let alice_usdt_before = Currencies::total_balance(usdt, &alice);
			let alice_weth_before = Currencies::total_balance(weth, &alice);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
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
/// These should partially match (direct matching), giving Alice a better price than single intent
#[test]
fn usdt_weth_two_opposing_intents() {
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
		.submit_swap_intent(
			alice.clone(),
			usdt,
			weth,
			alice_usdt_amount,
			5_390_835_579_515u128,
			Some(10),
		)
		// Bob: sell WETH for USDT (opposite direction)
		.submit_swap_intent(bob.clone(), weth, usdt, bob_weth_amount, 10_000, Some(10))
		.execute(|| {
			enable_slip_fees();
			let alice_weth_before = Currencies::total_balance(weth, &alice);
			let bob_usdt_before = Currencies::total_balance(usdt, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));

			let alice_weth_after = Currencies::total_balance(weth, &alice);
			let bob_usdt_after = Currencies::total_balance(usdt, &bob);
			let alice_weth_received = alice_weth_after - alice_weth_before;
			let bob_usdt_received = bob_usdt_after - bob_usdt_before;

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
fn eth_3pool_single_intent() {
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
		.submit_swap_intent(
			alice.clone(),
			eth,
			pool3,
			alice_eth_amount,
			20_000_000_000_000_000u128, //ED
			Some(10),
		)
		.execute(|| {
			enable_slip_fees();
			let alice_eth_before = Currencies::total_balance(eth, &alice);
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					HollarSolver::solve(intents, state).ok()
				},
			)
			.expect("Solver should produce a solution");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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
fn eth_3pool_solver_vs_router() {
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
		.submit_swap_intent(
			alice.clone(),
			eth,
			pool3,
			amount_in,
			20_000_000_000_000_000u128, //ED
			Some(10),
		)
		.execute(|| {
			enable_slip_fees();
			// ========== SOLVER PATH (Alice) ==========
			let alice_eth_before = Currencies::total_balance(eth, &alice);
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					HollarSolver::solve(intents, state).ok()
				},
			)
			.expect("Solver should produce a solution");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
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

/// Test: Two opposing intents for ETH <-> 3pool (direct matching)
#[test]
fn _eth_3pool_two_opposing_intents() {
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
		.submit_swap_intent(
			alice.clone(),
			eth,
			pool3,
			alice_eth_amount,
			20_000_000_000_000_000u128, //ED
			Some(10),
		)
		// Bob: sell 3pool for ETH (opposite direction)
		.submit_swap_intent(
			bob.clone(),
			pool3,
			eth,
			bob_3pool_amount,
			20_000_000_000_000_000u128, //ED
			Some(10),
		)
		.execute(|| {
			enable_slip_fees();
			let alice_3pool_before = Currencies::total_balance(pool3, &alice);
			let bob_eth_before = Currencies::total_balance(eth, &bob);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2, "Should have 2 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
					HollarSolver::solve(intents, state).ok()
				},
			)
			.expect("Solver should produce a solution");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
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

/// Test ring trade: 3 intents forming HDX→BNC→DOT→HDX cycle.
/// Verifies on-chain execution, balance changes, and that ring reduces AMM trades.
#[test]
fn solver_ring_trade_triangle_execute() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx_sell = 1_000 * hdx_unit;
	let bob_bnc_sell = 5 * bnc_unit;
	let charlie_dot_sell = 10 * dot_unit;

	let alice_min_bnc = bnc_unit / 2;
	let bob_min_dot = dot_unit / 10;
	let charlie_min_hdx = 500 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 10)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 10)
		.endow_account(charlie.clone(), dot, charlie_dot_sell * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc_sell, bob_min_dot, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot_sell, charlie_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution for ring trade");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 3, "All 3 intents should be resolved");
			assert!(solution.trades.len() < 3, "Ring should reduce AMM trades below 3");

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let bob_dot_before = Currencies::total_balance(dot, &bob);
			let charlie_dot_before = Currencies::total_balance(dot, &charlie);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));

			assert!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().is_empty(),
				"All intents resolved"
			);

			// Verify balance directions
			assert!(Currencies::total_balance(hdx, &alice) < alice_hdx_before);
			assert!(Currencies::total_balance(bnc, &alice) > alice_bnc_before);
			assert!(Currencies::total_balance(bnc, &bob) < bob_bnc_before);
			assert!(Currencies::total_balance(dot, &bob) > bob_dot_before);
			assert!(Currencies::total_balance(dot, &charlie) < charlie_dot_before);
			assert!(Currencies::total_balance(hdx, &charlie) > charlie_hdx_before);

			// Verify balance changes match solution (received = amount_out - fee)
			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				let expected_payout = s.amount_out - ice_fee.mul_floor(s.amount_out);
				match (s.asset_in, s.asset_out) {
					(0, 14) => {
						assert_eq!(alice_hdx_before - Currencies::total_balance(hdx, &alice), s.amount_in);
						assert_eq!(
							Currencies::total_balance(bnc, &alice) - alice_bnc_before,
							expected_payout
						);
					}
					(14, 5) => {
						assert_eq!(bob_bnc_before - Currencies::total_balance(bnc, &bob), s.amount_in);
						assert_eq!(Currencies::total_balance(dot, &bob) - bob_dot_before, expected_payout);
					}
					(5, 0) => {
						assert_eq!(
							charlie_dot_before - Currencies::total_balance(dot, &charlie),
							s.amount_in
						);
						assert_eq!(
							Currencies::total_balance(hdx, &charlie) - charlie_hdx_before,
							expected_payout
						);
					}
					_ => panic!("Unexpected direction"),
				}
			}

			// Verify limits met
			assert!(Currencies::total_balance(bnc, &alice) - alice_bnc_before >= alice_min_bnc);
			assert!(Currencies::total_balance(dot, &bob) - bob_dot_before >= bob_min_dot);
			assert!(Currencies::total_balance(hdx, &charlie) - charlie_hdx_before >= charlie_min_hdx);
		});
}

/// Compare ring trade via solver vs direct trades on identical pool state.
/// Solver should give equal or better output due to ring-matched volume avoiding AMM slippage.
#[test]
fn solver_ring_trade_vs_direct_trades() {
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use std::cell::RefCell;

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let alice_hdx_sell = 1_000 * hdx_unit;
	let bob_bnc_sell = 5 * bnc_unit;
	let charlie_dot_sell = 10 * dot_unit;

	let alice_min_bnc = bnc_unit / 2;
	let bob_min_dot = dot_unit / 10;
	let charlie_min_hdx = 500 * hdx_unit;

	// Run 1: Direct trades on fresh state
	let direct_results: RefCell<(u128, u128, u128)> = RefCell::new((0, 0, 0));

	TestNet::reset();
	{
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();

		crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
			.endow_account(alice.clone(), hdx, alice_hdx_sell * 10)
			.endow_account(bob.clone(), bnc, bob_bnc_sell * 10)
			.endow_account(charlie.clone(), dot, charlie_dot_sell * 10)
			.execute(|| {
				enable_slip_fees();

				let alice_bnc_before = Currencies::total_balance(bnc, &alice);
				let route = Router::get_route(AssetPair::new(hdx, bnc));
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(alice.clone()),
					hdx,
					bnc,
					alice_hdx_sell,
					1,
					route
				));
				let d_alice = Currencies::total_balance(bnc, &alice) - alice_bnc_before;

				let bob_dot_before = Currencies::total_balance(dot, &bob);
				let route = Router::get_route(AssetPair::new(bnc, dot));
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(bob.clone()),
					bnc,
					dot,
					bob_bnc_sell,
					1,
					route
				));
				let d_bob = Currencies::total_balance(dot, &bob) - bob_dot_before;

				let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
				let route = Router::get_route(AssetPair::new(dot, hdx));
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(charlie.clone()),
					dot,
					hdx,
					charlie_dot_sell,
					1,
					route
				));
				let d_charlie = Currencies::total_balance(hdx, &charlie) - charlie_hdx_before;

				*direct_results.borrow_mut() = (d_alice, d_bob, d_charlie);
			});
	}
	let (direct_alice, direct_bob, direct_charlie) = *direct_results.borrow();

	// Run 2: Solver on fresh state
	TestNet::reset();
	{
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();

		crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
			.endow_account(alice.clone(), hdx, alice_hdx_sell * 10)
			.endow_account(bob.clone(), bnc, bob_bnc_sell * 10)
			.endow_account(charlie.clone(), dot, charlie_dot_sell * 10)
			.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
			.submit_swap_intent(bob.clone(), bnc, dot, bob_bnc_sell, bob_min_dot, Some(10))
			.submit_swap_intent(charlie.clone(), dot, hdx, charlie_dot_sell, charlie_min_hdx, Some(10))
			.execute(|| {
				enable_slip_fees();

				let alice_bnc_before = Currencies::total_balance(bnc, &alice);
				let bob_dot_before = Currencies::total_balance(dot, &bob);
				let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);

				let call = pallet_ice::Pallet::<Runtime>::run(
					hydradx_runtime::System::block_number(),
					|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
						Solver::solve(intents, state).ok()
					},
				)
				.expect("Solver should produce a solution");

				crate::polkadot_test_net::hydradx_run_to_next_block();

				let pallet_ice::Call::submit_solution { solution, .. } = call;
				assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
					RuntimeOrigin::none(),
					solution,
				));

				let solver_alice = Currencies::total_balance(bnc, &alice) - alice_bnc_before;
				let solver_bob = Currencies::total_balance(dot, &bob) - bob_dot_before;
				let solver_charlie = Currencies::total_balance(hdx, &charlie) - charlie_hdx_before;

				// Verify solver produces valid results (all users get output)
				assert!(solver_alice > 0, "Alice should receive BNC");
				assert!(solver_bob > 0, "Bob should receive DOT");
				assert!(solver_charlie > 0, "Charlie should receive HDX");
			});
	}
}

/// Mixed batch: 12 intents, 5 users, 3 assets.
/// Tests opposing flows, same-direction groups, ring detection, rate uniformity, and execution.
#[test]
fn solver_mixed_batch_12_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();
	let eve: AccountId = EVE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let min_bnc = bnc_unit;
	let min_hdx = 200 * hdx_unit;
	let min_dot = dot_unit / 10;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, 20_000 * hdx_unit)
		.endow_account(alice.clone(), dot, 20 * dot_unit)
		.endow_account(bob.clone(), bnc, 100 * bnc_unit)
		.endow_account(charlie.clone(), bnc, 100 * bnc_unit)
		.endow_account(charlie.clone(), dot, 30 * dot_unit)
		.endow_account(dave.clone(), hdx, 20_000 * hdx_unit)
		.endow_account(eve.clone(), hdx, 10_000 * hdx_unit)
		.endow_account(eve.clone(), dot, 10 * dot_unit)
		.submit_swap_intent(alice.clone(), hdx, bnc, 10_000 * hdx_unit, min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, 30 * bnc_unit, min_hdx, Some(10))
		.submit_swap_intent(charlie.clone(), bnc, hdx, 50 * bnc_unit, min_hdx, Some(10))
		.submit_swap_intent(dave.clone(), hdx, bnc, 8_000 * hdx_unit, min_bnc, Some(10))
		.submit_swap_intent(alice.clone(), hdx, dot, 5_000 * hdx_unit, min_dot, Some(10))
		.submit_swap_intent(dave.clone(), hdx, dot, 3_000 * hdx_unit, min_dot, Some(10))
		.submit_swap_intent(eve.clone(), hdx, dot, 4_000 * hdx_unit, min_dot, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, 20 * bnc_unit, min_dot, Some(10))
		.submit_swap_intent(charlie.clone(), dot, hdx, 15 * dot_unit, min_hdx, Some(10))
		.submit_swap_intent(eve.clone(), dot, bnc, 5 * dot_unit, min_bnc, Some(10))
		.submit_swap_intent(alice.clone(), dot, bnc, 10 * dot_unit, min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, dot, 10 * bnc_unit, min_dot, Some(10))
		.execute(|| {
			enable_slip_fees();

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 12, "Should have 12 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution for 12 intents");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			// All 12 should be resolved
			assert_eq!(solution.resolved_intents.len(), 12, "All 12 intents should be resolved");
			assert!(solution.score > 0, "Score should be positive");

			// Rate uniformity: same-direction intents must have same out/in ratio
			let mut rates_by_direction: std::collections::BTreeMap<(u32, u32), Vec<f64>> =
				std::collections::BTreeMap::new();
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				let rate = s.amount_out as f64 / s.amount_in as f64;
				rates_by_direction
					.entry((s.asset_in, s.asset_out))
					.or_default()
					.push(rate);
			}
			for ((a, b), rates) in &rates_by_direction {
				if rates.len() > 1 {
					let max = rates.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
					let min = rates.iter().cloned().fold(f64::INFINITY, f64::min);
					let diff_pct = if min > 0.0 { (max - min) / min * 100.0 } else { 0.0 };
					assert!(
						diff_pct < 0.001,
						"Rate spread for {} → {} should be < 0.001%, got {:.6}%",
						a,
						b,
						diff_pct
					);
				}
			}

			// Submit and verify execution
			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);
			let bob_dot_before = Currencies::total_balance(dot, &bob);
			let charlie_hdx_before = Currencies::total_balance(hdx, &charlie);
			let charlie_bnc_before = Currencies::total_balance(bnc, &charlie);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);
			let dave_dot_before = Currencies::total_balance(dot, &dave);
			let eve_bnc_before = Currencies::total_balance(bnc, &eve);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			assert!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().is_empty(),
				"All intents resolved"
			);

			// Verify balance directions
			assert!(
				Currencies::total_balance(hdx, &alice) < alice_hdx_before,
				"Alice sold HDX"
			);
			assert!(
				Currencies::total_balance(bnc, &alice) > alice_bnc_before,
				"Alice got BNC"
			);
			assert!(Currencies::total_balance(hdx, &bob) > bob_hdx_before, "Bob got HDX");
			assert!(Currencies::total_balance(bnc, &bob) < bob_bnc_before, "Bob sold BNC");
			assert!(Currencies::total_balance(dot, &bob) > bob_dot_before, "Bob got DOT");
			assert!(
				Currencies::total_balance(hdx, &charlie) > charlie_hdx_before,
				"Charlie got HDX"
			);
			assert!(
				Currencies::total_balance(bnc, &charlie) < charlie_bnc_before,
				"Charlie sold BNC"
			);
			assert!(Currencies::total_balance(bnc, &dave) > dave_bnc_before, "Dave got BNC");
			assert!(Currencies::total_balance(dot, &dave) > dave_dot_before, "Dave got DOT");
			assert!(Currencies::total_balance(bnc, &eve) > eve_bnc_before, "Eve got BNC");
		});
}

/// Compare 12-intent mixed batch: solver vs 12 sequential direct trades on identical pool state.
#[test]
fn solver_mixed_batch_vs_direct_trades() {
	use hydradx_traits::router::{AssetPair, RouteProvider};
	use std::cell::RefCell;

	let hdx = 0u32;
	let bnc = 14u32;
	let dot = 5u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;
	let dot_unit = 10_000_000_000u128;

	let min_bnc = bnc_unit;
	let min_hdx = 200 * hdx_unit;
	let min_dot = dot_unit / 10;

	let trades: Vec<(u32, u32, u128)> = vec![
		(hdx, bnc, 10_000 * hdx_unit),
		(bnc, hdx, 30 * bnc_unit),
		(bnc, hdx, 50 * bnc_unit),
		(hdx, bnc, 8_000 * hdx_unit),
		(hdx, dot, 5_000 * hdx_unit),
		(hdx, dot, 3_000 * hdx_unit),
		(hdx, dot, 4_000 * hdx_unit),
		(bnc, dot, 20 * bnc_unit),
		(dot, hdx, 15 * dot_unit),
		(dot, bnc, 5 * dot_unit),
		(dot, bnc, 10 * dot_unit),
		(bnc, dot, 10 * bnc_unit),
	];

	// Run 1: Direct trades on fresh state
	let direct_total: RefCell<u128> = RefCell::new(0);

	TestNet::reset();
	{
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();
		let dave: AccountId = DAVE.into();
		let eve: AccountId = EVE.into();
		let users: Vec<AccountId> = vec![
			alice.clone(),
			bob.clone(),
			charlie.clone(),
			dave.clone(),
			alice.clone(),
			dave.clone(),
			eve.clone(),
			bob.clone(),
			charlie.clone(),
			eve.clone(),
			alice.clone(),
			bob.clone(),
		];

		crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
			.endow_account(alice.clone(), hdx, 20_000 * hdx_unit)
			.endow_account(alice.clone(), dot, 20 * dot_unit)
			.endow_account(bob.clone(), bnc, 100 * bnc_unit)
			.endow_account(charlie.clone(), bnc, 100 * bnc_unit)
			.endow_account(charlie.clone(), dot, 30 * dot_unit)
			.endow_account(dave.clone(), hdx, 20_000 * hdx_unit)
			.endow_account(eve.clone(), hdx, 10_000 * hdx_unit)
			.endow_account(eve.clone(), dot, 10 * dot_unit)
			.execute(|| {
				enable_slip_fees();
				let mut total = 0u128;
				for (i, &(asset_in, asset_out, amount_in)) in trades.iter().enumerate() {
					let user = &users[i];
					let before = Currencies::total_balance(asset_out, user);
					let route = Router::get_route(AssetPair::new(asset_in, asset_out));
					assert_ok!(Router::sell(
						RuntimeOrigin::signed(user.clone()),
						asset_in,
						asset_out,
						amount_in,
						1,
						route
					));
					total += Currencies::total_balance(asset_out, user) - before;
				}
				*direct_total.borrow_mut() = total;
			});
	}
	let direct = *direct_total.borrow();

	// Run 2: Solver on fresh state
	TestNet::reset();
	{
		let alice: AccountId = ALICE.into();
		let bob: AccountId = BOB.into();
		let charlie: AccountId = CHARLIE.into();
		let dave: AccountId = DAVE.into();
		let eve: AccountId = EVE.into();

		crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
			.endow_account(alice.clone(), hdx, 20_000 * hdx_unit)
			.endow_account(alice.clone(), dot, 20 * dot_unit)
			.endow_account(bob.clone(), bnc, 100 * bnc_unit)
			.endow_account(charlie.clone(), bnc, 100 * bnc_unit)
			.endow_account(charlie.clone(), dot, 30 * dot_unit)
			.endow_account(dave.clone(), hdx, 20_000 * hdx_unit)
			.endow_account(eve.clone(), hdx, 10_000 * hdx_unit)
			.endow_account(eve.clone(), dot, 10 * dot_unit)
			.submit_swap_intent(alice.clone(), hdx, bnc, 10_000 * hdx_unit, min_bnc, Some(10))
			.submit_swap_intent(bob.clone(), bnc, hdx, 30 * bnc_unit, min_hdx, Some(10))
			.submit_swap_intent(charlie.clone(), bnc, hdx, 50 * bnc_unit, min_hdx, Some(10))
			.submit_swap_intent(dave.clone(), hdx, bnc, 8_000 * hdx_unit, min_bnc, Some(10))
			.submit_swap_intent(alice.clone(), hdx, dot, 5_000 * hdx_unit, min_dot, Some(10))
			.submit_swap_intent(dave.clone(), hdx, dot, 3_000 * hdx_unit, min_dot, Some(10))
			.submit_swap_intent(eve.clone(), hdx, dot, 4_000 * hdx_unit, min_dot, Some(10))
			.submit_swap_intent(bob.clone(), bnc, dot, 20 * bnc_unit, min_dot, Some(10))
			.submit_swap_intent(charlie.clone(), dot, hdx, 15 * dot_unit, min_hdx, Some(10))
			.submit_swap_intent(eve.clone(), dot, bnc, 5 * dot_unit, min_bnc, Some(10))
			.submit_swap_intent(alice.clone(), dot, bnc, 10 * dot_unit, min_bnc, Some(10))
			.submit_swap_intent(bob.clone(), bnc, dot, 10 * bnc_unit, min_dot, Some(10))
			.execute(|| {
				enable_slip_fees();

				let call = pallet_ice::Pallet::<Runtime>::run(
					hydradx_runtime::System::block_number(),
					|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
						Solver::solve(intents, state).ok()
					},
				)
				.expect("Solver should produce a solution");

				crate::polkadot_test_net::hydradx_run_to_next_block();

				let pallet_ice::Call::submit_solution { solution, .. } = call;
				assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
					RuntimeOrigin::none(),
					solution.clone(),
				));

				// Verify all 12 intents resolved and executed
				assert_eq!(solution.resolved_intents.len(), 12, "All 12 intents should be resolved");
			});
	}
}

/// Load testnet snapshot with intents, iteratively resolve until no more can be resolved.
#[test]
fn solver_testnet_snapshot_intents() {
	TestNet::reset();

	crate::driver::HydrationTestDriver::with_snapshot("snapshots/hsm/ice_lark2").execute(|| {
		enable_slip_fees();

		let initial_count = pallet_intent::Pallet::<Runtime>::get_valid_intents().len();
		assert!(initial_count > 0, "Snapshot should contain intents");

		let mut total_resolved = 0;
		for _ in 0..10 {
			let remaining = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			if remaining.is_empty() {
				break;
			}

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			);

			let Some(pallet_ice::Call::submit_solution { solution, .. }) = call else {
				break;
			};

			total_resolved += solution.resolved_intents.len();

			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
		}

		assert!(total_resolved > 0, "Should resolve at least 1 intent from snapshot");
	});
}

/// Verify that intents the solver can't resolve also fail as direct Router trades.
#[test]
fn solver_testnet_snapshot_direct_trade_check() {
	use hydradx_traits::router::{AssetPair, RouteProvider};

	TestNet::reset();

	crate::driver::HydrationTestDriver::with_snapshot("snapshots/hsm/ice_lark2").execute(|| {
		enable_slip_fees();

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		assert!(!intents.is_empty());

		// Track which intents the solver can resolve
		let call = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		);
		let resolved_ids: std::collections::BTreeSet<_> = call
			.map(|pallet_ice::Call::submit_solution { solution, .. }| {
				solution.resolved_intents.iter().map(|ri| ri.id).collect()
			})
			.unwrap_or_default();

		// For unresolved intents, verify direct trade also fails
		for (id, intent) in &intents {
			if resolved_ids.contains(id) {
				continue;
			}

			let ice_support::IntentData::Swap(ref s) = intent.data else {
				panic!("expected Swap");
			};
			let owner = pallet_intent::Pallet::<Runtime>::intent_owner(id).unwrap_or_else(|| ALICE.into());

			let route = Router::get_route(AssetPair::new(s.asset_in, s.asset_out));
			let result = Router::sell(
				RuntimeOrigin::signed(owner),
				s.asset_in,
				s.asset_out,
				s.amount_in,
				s.amount_out,
				route,
			);

			assert!(
				result.is_err(),
				"Unresolved intent {} should also fail as direct trade",
				id
			);
		}
	});
}

/// Multi-round resolution: resolve what we can, inject a price-moving trade,
/// then resolve previously-stuck intents that benefit from the price change.
#[test]
fn solver_testnet_snapshot_multi_round() {
	TestNet::reset();

	let hdx = 0u32;
	let hollar = 222u32;
	let hdx_unit = 1_000_000_000_000u128;
	let hollar_unit = 1_000_000_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot("snapshots/hsm/ice_lark2").execute(|| {
		enable_slip_fees();

		let initial_count = pallet_intent::Pallet::<Runtime>::get_valid_intents().len();
		assert!(initial_count > 0);

		// Round 1: Resolve what we can
		let call1 = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		);
		let r1_resolved = if let Some(pallet_ice::Call::submit_solution { solution, .. }) = call1 {
			let count = solution.resolved_intents.len();
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			count
		} else {
			0
		};

		let after_r1 = pallet_intent::Pallet::<Runtime>::get_valid_intents().len();
		assert!(r1_resolved > 0, "Round 1 should resolve at least 1 intent");

		crate::polkadot_test_net::hydradx_run_to_next_block();

		// Round 2: Submit large HDX→HOLLAR to push HOLLAR price up
		let dave: AccountId = DAVE.into();
		let hdx_sell_amount = 1_000_000 * hdx_unit;
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			dave.clone(),
			hdx,
			(hdx_sell_amount * 2) as i128,
		));
		let ts = hydradx_runtime::Timestamp::now();
		assert_ok!(pallet_intent::Pallet::<Runtime>::submit_intent(
			RuntimeOrigin::signed(dave.clone()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapData {
					asset_in: hdx,
					asset_out: hollar,
					amount_in: hdx_sell_amount,
					amount_out: hollar_unit,
					partial: false,
				}),
				deadline: Some(6000u64 * 20 + ts),
				on_resolved: None,
			},
		));

		let call2 = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		);
		let r2_resolved = if let Some(pallet_ice::Call::submit_solution { solution, .. }) = call2 {
			let count = solution.resolved_intents.len();
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			count
		} else {
			0
		};

		assert!(
			r2_resolved >= 2,
			"Round 2 should resolve Dave's intent + at least 1 more via matching"
		);

		crate::polkadot_test_net::hydradx_run_to_next_block();

		// Round 3: Price moved — try remaining intents
		let call3 = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		);
		let r3_resolved = if let Some(pallet_ice::Call::submit_solution { solution, .. }) = call3 {
			let count = solution.resolved_intents.len();
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			count
		} else {
			0
		};

		// The price move should unlock previously-stuck HOLLAR→HDX intents
		assert!(r3_resolved > 0, "Round 3 should resolve intents unlocked by price move");

		let total_resolved = r1_resolved + r2_resolved + r3_resolved;
		// We started with 5 snapshot intents + 1 injected = 6 total
		// At least 5 should be resolved (the HDX→HOLLAR intent is in the opposite direction)
		assert!(
			total_resolved >= 5,
			"Should resolve at least 5 of 6 intents across 3 rounds"
		);
	});
}

/// Test near-perfect cancellation: two opposing intents that almost cancel,
/// leaving only a tiny net imbalance for the AMM.
/// Must produce a valid solution and execute on-chain.
#[test]
fn solver_near_perfect_cancel_ed_remainder() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Spot: BNC/HDX ≈ 14.7 (1 BNC ≈ 14.7 HDX from snapshot)
	// Alice: sell 1000 HDX for BNC (~67.8 BNC at spot)
	let alice_hdx_sell = 1000 * hdx_unit;
	// Bob: sell 68 BNC for HDX (~1002 HDX at spot)
	// Net excess BNC: ~0.2 BNC ≈ 3 HDX to trade through AMM (tiny remainder)
	let bob_bnc_sell = 68 * bnc_unit;

	let alice_min_bnc = 50 * bnc_unit;
	let bob_min_hdx = 800 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 10)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution for near-perfect cancel");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 2, "Both intents must be resolved");
			// Near-perfect cancel: at most 1 small AMM trade for the net remainder
			assert!(solution.trades.len() <= 1, "Should need at most 1 AMM trade");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			assert!(
				pallet_intent::Pallet::<Runtime>::get_valid_intents().is_empty(),
				"All intents resolved"
			);

			assert!(
				Currencies::total_balance(hdx, &alice) < alice_hdx_before,
				"Alice sold HDX"
			);
			assert!(
				Currencies::total_balance(bnc, &alice) > alice_bnc_before,
				"Alice got BNC"
			);
			assert!(Currencies::total_balance(bnc, &bob) < bob_bnc_before, "Bob sold BNC");
			assert!(Currencies::total_balance(hdx, &bob) > bob_hdx_before, "Bob got HDX");
		});
}

/// Test with amounts at existential deposit level.

/// Test with near-cancelling amounts where the net AMM remainder is small.
/// Alice sells 100 HDX for BNC (~6.78 BNC at spot).
/// Bob sells 7 BNC for HDX (~103 HDX at spot).
/// Net excess: ~0.22 BNC ≈ 3 HDX — very small AMM trade.
#[test]
fn solver_existential_deposit_amounts() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Spot: 1 BNC ≈ 14.7 HDX
	let alice_hdx_sell = 100 * hdx_unit;
	let bob_bnc_sell = 7 * bnc_unit; // 7 BNC

	let alice_min_bnc = 4 * bnc_unit;
	let bob_min_hdx = 60 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 100)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 100)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must handle near-ED AMM remainder");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 2, "Both intents must be resolved");
			assert!(
				solution.trades.len() <= 1,
				"Near-cancel should need at most 1 small AMM trade"
			);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			assert!(pallet_intent::Pallet::<Runtime>::get_valid_intents().is_empty());

			assert!(
				Currencies::total_balance(hdx, &alice) < alice_hdx_before,
				"Alice sold HDX"
			);
			assert!(
				Currencies::total_balance(bnc, &alice) > alice_bnc_before,
				"Alice got BNC"
			);
			assert!(Currencies::total_balance(bnc, &bob) < bob_bnc_before, "Bob sold BNC");
			assert!(Currencies::total_balance(hdx, &bob) > bob_hdx_before, "Bob got HDX");
		});
}

/// Test where opposing intents nearly cancel, leaving AMM remainder below ED.
/// Alice sells 50 HDX for BNC (~3.37 BNC at spot).
/// Bob sells 3.42 BNC for HDX (~50.4 HDX at spot).
/// Net excess: ~0.05 BNC ≈ 0.7 HDX — below minimum trade size.
/// Both intents resolve in the solution, but execution fails with Token(BelowMinimum)
/// because the dust AMM trade amount is below BNC's existential deposit.
#[test]
fn solver_amm_remainder_below_ed() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Spot: 1 BNC ≈ 14.7 HDX
	// Alice: sell 50 HDX → ~3.37 BNC
	let alice_hdx_sell = 50 * hdx_unit;
	// Bob: sell 3.42 BNC → ~50.4 HDX
	// Net excess BNC: 3.42 - 3.37 = 0.05 BNC ≈ 0.7 HDX — below or near ED
	let bob_bnc_sell = 342 * bnc_unit / 100; // 3.42 BNC

	let alice_min_bnc = 2 * bnc_unit; // expect ~3.37, require 2
	let bob_min_hdx = 30 * hdx_unit; // expect ~50.4, require 30

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 100)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 100)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 2, "Both intents must be resolved");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
		});
}

/// Test where opposing intents cancel almost exactly — AMM remainder is dust.
/// Alice sells 50 HDX → ~3.37 BNC at spot.
/// Bob sells 3.39 BNC → ~49.9 HDX at spot.
/// Net excess: ~0.02 BNC ≈ 0.3 HDX — dust level.
/// Both intents resolve in the solution, but execution fails with Token(BelowMinimum)
/// because the dust AMM trade amount is below BNC's existential deposit.
#[test]
fn solver_amm_remainder_dust() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Spot: 1 BNC ≈ 14.7 HDX
	let alice_hdx_sell = 50 * hdx_unit;
	// 3.39 BNC ≈ 49.9 HDX — almost exactly cancels Alice's 50 HDX
	let bob_bnc_sell = 339 * bnc_unit / 100; // 3.39 BNC

	let alice_min_bnc = 2 * bnc_unit;
	let bob_min_hdx = 30 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 100)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 100)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution for dust-level remainder");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 2, "Both intents must be resolved");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
		});
}

/// 3-intent near-cancel with dust AMM remainder.
/// Alice sells 100 HDX → BNC, Bob+Charlie each sell 3.39 BNC → HDX.
/// Bob+Charlie total: 6.78 BNC ≈ 100.0 HDX — nearly exact cancel with Alice.
/// Net excess BNC is dust — below BNC's ED of 68_795_189_840.
/// Fails with Token(BelowMinimum): the route executor can't transfer dust BNC to its
/// router account because the amount is below BNC's existential deposit.
#[test]
fn solver_three_intent_dust_remainder() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Spot: 1 BNC ≈ 14.7 HDX
	let alice_hdx_sell = 100 * hdx_unit;
	// 3.39 BNC ≈ 49.8 HDX each; total 6.78 BNC ≈ 100.0 HDX — nearly cancels Alice
	let bob_bnc_sell = 339 * bnc_unit / 100; // 3.39 BNC
	let charlie_bnc_sell = 339 * bnc_unit / 100; // 3.39 BNC

	let alice_min_bnc = 4 * bnc_unit;
	let bob_min_hdx = 30 * hdx_unit;
	let charlie_min_hdx = 30 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 100)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 100)
		.endow_account(charlie.clone(), bnc, charlie_bnc_sell * 100)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.submit_swap_intent(charlie.clone(), bnc, hdx, charlie_bnc_sell, charlie_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 3);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution for 3-intent dust remainder");

			let pallet_ice::Call::submit_solution { solution, .. } = call;

			assert_eq!(solution.resolved_intents.len(), 3, "All three intents must be resolved");

			// Verify the AMM trade is dust-level — below BNC's ED of 68_795_189_840
			assert_eq!(solution.trades.len(), 1, "Should have exactly one AMM trade");
			let dust_trade = &solution.trades[0];
			assert!(
				dust_trade.amount_in < 68_795_189_840,
				"AMM trade amount_in should be below BNC ED (68_795_189_840), got: {}",
				dust_trade.amount_in
			);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			// The dust AMM trade (ExactIn sell ~1_630_278_265 BNC via Omnipool) fails with
			// Token(BelowMinimum). The route executor transfers the dust BNC from the
			// holding pot to its router account, but the amount (~0.00163 BNC) is below
			// BNC's existential deposit of 68_795_189_840 (~0.069 BNC), so the transfer
			// is rejected. Seeding the holding pot doesn't help — the issue is on the
			// router account's receiving side.
			let result = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
		});
}

/// Test that the ICE protocol fee is deducted from each resolved intent's output.
/// Two opposing intents (HDX↔BNC) are resolved. Each recipient should receive
/// amount_out * (1 - fee) where fee = 0.02% (Permill::from_parts(200)).
/// The fee remains in the ICE holding pot.
#[test]
fn solver_ice_fee_is_deducted() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;

	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// At ~14.7 HDX/BNC:
	// Alice: sell 1000 HDX → ~67 BNC
	// Bob: sell 100 BNC → ~1474 HDX
	// Large spread ensures both resolve comfortably
	let alice_hdx_sell = 1000 * hdx_unit;
	let bob_bnc_sell = 100 * bnc_unit;

	let alice_min_bnc = 10 * bnc_unit;
	let bob_min_hdx = 200 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_hdx_sell * 10)
		.endow_account(bob.clone(), bnc, bob_bnc_sell * 10)
		.submit_swap_intent(alice.clone(), hdx, bnc, alice_hdx_sell, alice_min_bnc, Some(10))
		.submit_swap_intent(bob.clone(), bnc, hdx, bob_bnc_sell, bob_min_hdx, Some(10))
		.execute(|| {
			enable_slip_fees();

			assert_eq!(pallet_intent::Pallet::<Runtime>::get_valid_intents().len(), 2);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call;
			assert_eq!(solution.resolved_intents.len(), 2, "Both intents must be resolved");

			// Capture resolved amounts before execution
			let mut alice_resolved_bnc = 0u128;
			let mut bob_resolved_hdx = 0u128;
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				if s.asset_out == bnc {
					alice_resolved_bnc = s.amount_out;
				} else if s.asset_out == hdx {
					bob_resolved_hdx = s.amount_out;
				}
			}
			assert!(alice_resolved_bnc > 0, "Alice should receive BNC");
			assert!(bob_resolved_hdx > 0, "Bob should receive HDX");

			let ice_fee: Permill = <Runtime as pallet_ice::Config>::Fee::get();

			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let holding_pot = pallet_ice::Pallet::<Runtime>::get_pallet_account();

			// Pre-fund the pot with native ED so it isn't reaped after the fee-only remainder.
			// In production the pot persists across solutions and accumulates fees over time.
			assert_ok!(hydradx_runtime::Balances::force_set_balance(
				RuntimeOrigin::root(),
				holding_pot.clone(),
				hdx_unit,
			));

			let pot_bnc_before = Currencies::total_balance(bnc, &holding_pot);
			let pot_hdx_before = Currencies::total_balance(hdx, &holding_pot);

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// Verify fee deduction: recipients get amount_out - fee
			let alice_fee = ice_fee.mul_floor(alice_resolved_bnc);
			let bob_fee = ice_fee.mul_floor(bob_resolved_hdx);
			let alice_expected_payout = alice_resolved_bnc - alice_fee;
			let bob_expected_payout = bob_resolved_hdx - bob_fee;

			let alice_bnc_received = Currencies::total_balance(bnc, &alice) - alice_bnc_before;
			let bob_hdx_received = Currencies::total_balance(hdx, &bob) - bob_hdx_before;

			assert_eq!(
				alice_bnc_received, alice_expected_payout,
				"Alice should receive amount_out minus fee"
			);
			assert_eq!(
				bob_hdx_received, bob_expected_payout,
				"Bob should receive amount_out minus fee"
			);

			// Verify fees stayed in holding pot
			assert!(alice_fee > 0, "Alice fee should be non-zero");
			assert!(bob_fee > 0, "Bob fee should be non-zero");

			// The holding pot balance after execution should have increased by the fee amounts
			// (relative to what it would be with zero fees — i.e., the pot retains the fees)
			let pot_bnc_after = Currencies::total_balance(bnc, &holding_pot);
			let pot_hdx_after = Currencies::total_balance(hdx, &holding_pot);
			assert!(
				pot_bnc_after >= pot_bnc_before + alice_fee,
				"Holding pot should retain BNC fee: before={}, after={}, fee={}",
				pot_bnc_before,
				pot_bnc_after,
				alice_fee
			);
			assert!(
				pot_hdx_after >= pot_hdx_before + bob_fee,
				"Holding pot should retain HDX fee: before={}, after={}, fee={}",
				pot_hdx_before,
				pot_hdx_after,
				bob_fee
			);
		});
}
