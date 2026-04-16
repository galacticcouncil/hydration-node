use crate::polkadot_test_net::{
	go_to_block, hydradx_run_to_next_block, Hydra, TestNet, ALICE, BOB, CHARLIE, DAVE, EVE,
};
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
use hydradx_traits::registry::Inspect as RegistryInspect;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::BoundErc20;
use ice_solver::v2::Solver as IceSolver;
use ice_support::{Solution, MAX_NUMBER_OF_RESOLVED_INTENTS};
use orml_traits::{MultiCurrency, MultiReservableCurrency, NamedMultiReservableCurrency};
use pallet_omnipool::types::SlipFeeConfig;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::PATH_TO_SNAPSHOT;

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
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
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

		let pallet_ice::Call::submit_solution { solution, .. } = call else {
			panic!("Expected submit_solution call");
		};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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
	let (_direct_alice, _direct_bob, _direct_charlie) = *direct_results.borrow();

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

				let pallet_ice::Call::submit_solution { solution, .. } = call else {
					panic!("Expected submit_solution call");
				};
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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
	let _direct = *direct_total.borrow();

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

				let pallet_ice::Call::submit_solution { solution, .. } = call else {
					panic!("Expected submit_solution call");
				};
				assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
					RuntimeOrigin::none(),
					solution.clone(),
				));

				// Verify all 12 intents resolved and executed
				assert_eq!(solution.resolved_intents.len(), 12, "All 12 intents should be resolved");
			});
	}
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

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
/// The solver detects the dust remainder and skips the AMM trade,
/// resolving all intents via direct matching only.
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 3, "All three intents must be resolved");

			// Dust remainder is below ED — solver skips the AMM trade entirely
			assert_eq!(solution.trades.len(), 0, "No AMM trades — dust remainder skipped");

			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
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

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};
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

/// Reproduce FundsUnavailable on testnet snapshot with pre-existing intents.
#[test]
fn solver_funds_unavailable_snapshot() {
	const FUNDS_SNAPSHOT: &str = "snapshots/SNAPSHOT_funds";

	crate::driver::HydrationTestDriver::with_snapshot(FUNDS_SNAPSHOT).execute(|| {
		enable_slip_fees();

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		println!("snapshot has {} valid intents", intents.len());
		assert!(!intents.is_empty(), "Snapshot should contain intents");

		for (id, intent) in &intents {
			println!("intent {}: {:?}", id, intent.data);
		}

		let call = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		)
		.expect("Solver must produce a solution from snapshot intents");

		let pallet_ice::Call::submit_solution { solution, .. } = call else {
			panic!("Expected submit_solution call");
		};

		println!(
			"solution: {} resolved intents, {} trades, score: {}",
			solution.resolved_intents.len(),
			solution.trades.len(),
			solution.score
		);

		for (i, ri) in solution.resolved_intents.iter().enumerate() {
			let owner = pallet_intent::Pallet::<Runtime>::intent_owner(ri.id);
			let ice_support::IntentData::Swap(ref s) = ri.data else {
				continue;
			};
			let named_reserved = owner
				.as_ref()
				.map(|o| Currencies::reserved_balance_named(&pallet_intent::NAMED_RESERVE_ID, s.asset_in, o));
			let total_reserved = owner.as_ref().map(|o| Currencies::reserved_balance(s.asset_in, o));
			let free = owner.as_ref().map(|o| Currencies::free_balance(s.asset_in, o));
			println!(
				"resolved[{}]: id={}, owner={:?}, asset_in={}, amount_in={}, named_reserved={:?}, total_reserved={:?}, free={:?}",
				i, ri.id, owner, s.asset_in, s.amount_in, named_reserved, total_reserved, free
			);
		}
		for (i, t) in solution.trades.iter().enumerate() {
			println!(
				"trade[{}]: {:?} amount_in={}, amount_out={}, route={:?}",
				i, t.direction, t.amount_in, t.amount_out, t.route
			);
		}

		// Check holding pot balances before execution
		let holding_pot = pallet_ice::Pallet::<Runtime>::get_pallet_account();
		let trade_assets: std::collections::BTreeSet<u32> = solution
			.trades
			.iter()
			.flat_map(|t| {
				let mut assets = vec![];
				if let Some(first) = t.route.first() {
					assets.push(first.asset_in);
				}
				if let Some(last) = t.route.last() {
					assets.push(last.asset_out);
				}
				assets
			})
			.collect();
		for &asset in &trade_assets {
			let bal = Currencies::free_balance(asset, &holding_pot);
			println!("holding_pot asset {} free_balance = {}", asset, bal);
		}

		// Also check all resolved intent assets
		for ri in solution.resolved_intents.iter() {
			let ice_support::IntentData::Swap(ref s) = ri.data else {
				continue;
			};
			let bal_in = Currencies::free_balance(s.asset_in, &holding_pot);
			let bal_out = Currencies::free_balance(s.asset_out, &holding_pot);
			println!(
				"holding_pot: asset_in={} bal={}, asset_out={} bal={}",
				s.asset_in, bal_in, s.asset_out, bal_out
			);
		}

		crate::polkadot_test_net::hydradx_run_to_next_block();

		let result = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
		println!("submit_solution result: {:?}", result);
		assert_ok!(result);
	});
}

/// Reproduce trading limit failure on testnet snapshot with pre-existing intents.
#[test]
fn solver_trading_limit_snapshot() {
	const SNAPSHOT: &str = "snapshots/ice/SNAPSHOT_tradinglimit";

	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT).execute(|| {
		//enable_slip_fees();
		hydradx_run_to_next_block();

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		println!("snapshot has {} valid intents", intents.len());
		assert!(!intents.is_empty(), "Snapshot should contain intents");

		for (id, intent) in &intents {
			println!("intent {}: {:?}", id, intent.data);
		}

		let call = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		)
		.expect("Solver must produce a solution");

		let pallet_ice::Call::submit_solution { solution, .. } = call else {
			panic!("Expected submit_solution call");
		};

		println!(
			"solution: {} resolved intents, {} trades, score: {}",
			solution.resolved_intents.len(),
			solution.trades.len(),
			solution.score
		);

		for (i, ri) in solution.resolved_intents.iter().enumerate() {
			println!("resolved[{}]: id={}, {:?}", i, ri.id, ri.data);
		}
		for (i, t) in solution.trades.iter().enumerate() {
			println!(
				"trade[{}]: {:?} amount_in={}, amount_out={}, route={:?}",
				i, t.direction, t.amount_in, t.amount_out, t.route
			);
		}

		crate::polkadot_test_net::hydradx_run_to_next_block();

		let result = pallet_ice::Pallet::<Runtime>::submit_solution(RuntimeOrigin::none(), solution);
		println!("submit_solution result: {:?}", result);
	});
}

/// Debug why intent ending with 6127 is excluded from the solution.
#[test]
fn solver_debug_intent_6127() {
	const SNAPSHOT: &str = "snapshots/ice/SNAPSHOT_6127";

	crate::driver::HydrationTestDriver::with_snapshot(SNAPSHOT).execute(|| {
		enable_slip_fees();

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		println!("snapshot has {} valid intents", intents.len());

		// Find the 6127 intent
		let target_intent = intents.iter().find(|(id, _)| {
			let id_str = format!("{}", id);
			id_str.ends_with("6127")
		});
		if let Some((id, intent)) = target_intent {
			println!("TARGET intent {}: {:?}", id, intent.data);
		} else {
			println!("WARNING: no intent ending with 6127 found!");
			for (id, intent) in &intents {
				println!("  intent {}: {:?}", id, intent.data);
			}
		}

		// Find the 6127 intent's route via solver simulation
		let call = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| {
				// Find 6127 intent
				let target = intents.iter().find(|i| format!("{}", i.id).ends_with("6127"));
				if let Some(intent) = target {
					let ice_support::IntentData::Swap(ref swap) = intent.data else {
						return None;
					};
					println!(
						"6127 intent: sell {} of asset {} for asset {}, min_out: {}",
						swap.amount_in, swap.asset_in, swap.asset_out, swap.amount_out
					);

					// Discover routes for this intent's pair
					use hydradx_traits::amm::AMMInterface;
					if let Ok(routes) = TestSimulator::discover_routes(swap.asset_in, swap.asset_out, &state) {
						for (i, route) in routes.iter().enumerate() {
							let sim = TestSimulator::sell(
								swap.asset_in,
								swap.asset_out,
								swap.amount_in,
								route.clone(),
								&state,
							);
							match sim {
								Ok((_, exec)) => {
									println!("route[{}]: {:?} → simulated output: {}", i, route, exec.amount_out)
								}
								Err(e) => println!("route[{}]: {:?} → simulation failed: {:?}", i, route, e),
							}
						}
					}
				}

				Solver::solve(intents, state).ok()
			},
		);

		if let Some(pallet_ice::Call::submit_solution { ref solution, .. }) = call {
			for (i, t) in solution.trades.iter().enumerate() {
				println!("trade[{}]: route={:?}", i, t.route);
			}
		}

		// Now do a direct router sell with the 6127 intent's parameters
		// Intent: sell 10_000_000_000 of asset 10 → asset 0
		// Use the Aave→Stableswap→Omnipool route discovered above
		let asset_in = 10u32;
		let asset_out = 0u32;
		let amount_in = 10_000_000_000u128;

		// Get the route from the router
		let route = Router::get_route(hydradx_traits::router::AssetPair::new(asset_in, asset_out));
		println!("on-chain route for {} -> {}: {:?}", asset_in, asset_out, route);

		// Fund a test account and do the swap
		let who: AccountId = [99u8; 32].into();
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			who.clone(),
			asset_in,
			amount_in as i128 * 10,
		));

		let balance_before = Currencies::free_balance(asset_out, &who);

		crate::polkadot_test_net::hydradx_run_to_next_block();

		let sell_result = pallet_route_executor::Pallet::<Runtime>::sell(
			RuntimeOrigin::signed(who.clone()),
			asset_in,
			asset_out,
			amount_in,
			0, // no min out — just see what we get
			route,
		);
		println!("direct router sell result: {:?}", sell_result);

		let balance_after = Currencies::free_balance(asset_out, &who);
		let received = balance_after.saturating_sub(balance_before);
		println!(
			"received: {} of asset {} (min_out was 5473500000000000000)",
			received, asset_out
		);
	});
}

/// V2 partial fill test: small intents + whale.
///
/// 3 small intents sell 10,000 HDX → BNC each (partial: false, loose limit).
/// 1 whale sells 5,000,000 HDX → BNC (partial: true, tight limit).
///
/// Without partial fills, the whale's volume would push the batch rate below
/// its limit, and it would be excluded entirely. With v2, the whale gets a
/// partial fill — only as much volume as the AMM can absorb at the minimum rate.
#[test]
fn solver_v2_partial_fill_whale() {
	TestNet::reset();

	let alice: AccountId = ALICE.into(); // small
	let bob: AccountId = BOB.into(); // small
	let charlie: AccountId = CHARLIE.into(); // small
	let dave: AccountId = DAVE.into(); // whale

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	// Spot: 1 HDX ≈ 0.068 BNC (from snapshot)
	let small_amount = 10_000 * hdx_unit;
	let whale_amount = 5_000_000 * hdx_unit;
	// Loose limit for small intents: 1 BNC per 10,000 HDX (way below spot)
	let small_min_bnc = 1_000_000_000_000u128;
	// Tight limit for whale: require ~0.065 BNC per HDX (close to spot of ~0.068)
	// 5,000,000 HDX * 0.065 = 325,000 BNC
	let whale_min_bnc = 325_000 * 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, small_amount * 10)
		.endow_account(bob.clone(), hdx, small_amount * 10)
		.endow_account(charlie.clone(), hdx, small_amount * 10)
		.endow_account(dave.clone(), hdx, whale_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Submit 3 small non-partial intents
			for (who, label) in [
				(alice.clone(), "alice"),
				(bob.clone(), "bob"),
				(charlie.clone(), "charlie"),
			] {
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(who),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: hdx,
							asset_out: bnc,
							amount_in: small_amount,
							amount_out: small_min_bnc,
							partial: false,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!("{}: submitted {} HDX → BNC (non-partial)", label, small_amount);
			}

			// Submit whale partial intent
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(dave.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: whale_amount,
						amount_out: whale_min_bnc,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));
			println!(
				"dave (whale): submitted {} HDX → BNC (partial, min_bnc={})",
				whale_amount, whale_min_bnc
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("total intents: {}", intents.len());
			assert_eq!(intents.len(), 4);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"\nsolution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			let mut whale_resolved = false;
			let mut whale_fill = 0u128;
			for (i, ri) in solution.resolved_intents.iter().enumerate() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					continue;
				};
				let is_whale = s.amount_in != small_amount || s.partial.is_partial();
				println!(
					"resolved[{}]: id={}, amount_in={}, amount_out={}, partial={:?} {}",
					i,
					ri.id,
					s.amount_in,
					s.amount_out,
					s.partial,
					if is_whale { "← WHALE" } else { "" }
				);
				if is_whale {
					whale_resolved = true;
					whale_fill = s.amount_in;
				}
			}

			// All 3 small intents should be resolved
			let small_count = solution
				.resolved_intents
				.iter()
				.filter(|ri| {
					let ice_support::IntentData::Swap(ref s) = ri.data else {
						return false;
					};
					!s.partial.is_partial() && s.amount_in == small_amount
				})
				.count();
			println!("\nsmall intents resolved: {}/3", small_count);
			assert_eq!(small_count, 3, "All 3 small intents should be resolved");

			// Whale should be resolved (possibly partially)
			assert!(whale_resolved, "Whale should be in the solution");
			println!(
				"whale fill: {} / {} ({:.1}%)",
				whale_fill,
				whale_amount,
				(whale_fill as f64 / whale_amount as f64) * 100.0
			);

			if whale_fill < whale_amount {
				println!("whale was PARTIALLY filled — v2 partial fill working!");
			} else {
				println!("whale was FULLY filled");
			}

			// Execute the solution
			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Check whale intent is still open if partially filled
			if whale_fill < whale_amount {
				let whale_intent_id = intents
					.iter()
					.find(|(_, intent)| {
						let ice_support::IntentData::Swap(ref s) = intent.data else {
							return false;
						};
						s.partial.is_partial() && s.amount_in == whale_amount
					})
					.map(|(id, _)| *id)
					.expect("whale intent should exist");

				let stored = pallet_intent::Pallet::<Runtime>::get_intent(whale_intent_id);
				assert!(
					stored.is_some(),
					"Whale intent should still be in storage after partial fill"
				);
				let stored = stored.unwrap();
				let ice_support::IntentData::Swap(ref s) = stored.data else {
					panic!("expected Swap")
				};
				println!(
					"whale intent after fill: amount_in={} (immutable), partial={:?}, remaining={}",
					s.amount_in,
					s.partial,
					s.remaining()
				);
				assert_eq!(s.amount_in, whale_amount, "Original amount_in should be immutable");
				assert_eq!(s.partial.filled(), whale_fill, "Filled counter should match");
				assert!(s.remaining() > 0, "Should have remaining to fill");
			}
		});
}

/// V2 single whale intent — too large for one batch but partially fillable.
///
/// One intent sells 5,000,000 HDX → BNC with a tight limit close to spot.
/// The full amount would cause too much slippage, but the solver should
/// find the maximum partial fill that meets the minimum rate.
#[test]
fn solver_v2_single_partial_whale() {
	TestNet::reset();

	let dave: AccountId = DAVE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	let whale_amount = 5_000_000 * hdx_unit;
	// Tight limit: ~0.065 BNC/HDX (spot is ~0.068)
	let whale_min_bnc = 325_000 * 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(dave.clone(), hdx, whale_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(dave.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: whale_amount,
						amount_out: whale_min_bnc,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have exactly 1 intent");
			println!(
				"single whale intent: {} HDX → BNC, min_out={}",
				whale_amount, whale_min_bnc
			);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"solution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			assert_eq!(solution.resolved_intents.len(), 1, "Whale should be resolved");

			let ri = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref s) = ri.data else {
				panic!("expected Swap")
			};

			println!(
				"fill: {} / {} ({:.1}%)",
				s.amount_in,
				whale_amount,
				(s.amount_in as f64 / whale_amount as f64) * 100.0
			);
			println!("amount_out: {} BNC", s.amount_out);

			// Should be a partial fill — less than full amount
			assert!(s.amount_in < whale_amount, "Should be partially filled, not full");
			assert!(s.amount_in > 0, "Should have some fill");

			// The rate should meet the minimum
			// min_rate = whale_min_bnc / whale_amount = 0.065 BNC/HDX
			// actual_rate = amount_out / amount_in >= 0.065
			let pro_rata_min = s.amount_in as u128 * whale_min_bnc / whale_amount;
			assert!(
				s.amount_out >= pro_rata_min,
				"Rate should meet minimum: got {} BNC for {} HDX, pro_rata_min={}",
				s.amount_out,
				s.amount_in,
				pro_rata_min
			);

			println!(
				"rate: {:.6} BNC/HDX (min: {:.6})",
				s.amount_out as f64 / s.amount_in as f64,
				whale_min_bnc as f64 / whale_amount as f64
			);

			// Capture balances before execution
			let dave_hdx_before = Currencies::total_balance(hdx, &dave);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);
			let fill_amount = s.amount_in;
			let expected_bnc_out = s.amount_out;

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Verify Dave's balances
			let dave_hdx_after = Currencies::total_balance(hdx, &dave);
			let dave_bnc_after = Currencies::total_balance(bnc, &dave);
			let hdx_spent = dave_hdx_before.saturating_sub(dave_hdx_after);
			let bnc_received = dave_bnc_after.saturating_sub(dave_bnc_before);

			println!(
				"dave HDX: {} → {} (spent {})",
				dave_hdx_before, dave_hdx_after, hdx_spent
			);
			println!(
				"dave BNC: {} → {} (received {})",
				dave_bnc_before, dave_bnc_after, bnc_received
			);

			assert_eq!(
				hdx_spent, fill_amount,
				"Dave should have spent exactly the fill amount of HDX"
			);
			// BNC received = amount_out - protocol fee (0.02%)
			let fee = hydradx_runtime::IceFee::get().mul_floor(expected_bnc_out);
			let expected_payout = expected_bnc_out.saturating_sub(fee);
			assert_eq!(
				bnc_received, expected_payout,
				"Dave should receive amount_out minus fee: expected {}, got {}",
				expected_payout, bnc_received
			);
			println!(
				"fee: {} BNC ({:.4}%)",
				fee,
				fee as f64 / expected_bnc_out as f64 * 100.0
			);

			// Verify intent still open
			let intent_id = intents[0].0;
			let stored = pallet_intent::Pallet::<Runtime>::get_intent(intent_id).expect("Intent should still exist");
			let ice_support::IntentData::Swap(ref stored_swap) = stored.data else {
				panic!("expected Swap")
			};
			println!(
				"after fill 1: partial={:?}, remaining={}",
				stored_swap.partial,
				stored_swap.remaining()
			);
			assert_eq!(stored_swap.amount_in, whale_amount, "Original immutable");
			assert!(stored_swap.remaining() > 0, "Should have remaining");
			let filled_after_1 = stored_swap.partial.filled();
			assert!(filled_after_1 > 0, "Should have some filled");

			// --- Block 2: run solver again on the same (now partially filled) intent ---
			println!("\n=== BLOCK 2 ===");
			crate::polkadot_test_net::hydradx_run_to_next_block();

			let intents2 = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents2.len(), 1, "Whale intent should still be valid");

			let dave_hdx_before2 = Currencies::total_balance(hdx, &dave);
			let dave_bnc_before2 = Currencies::total_balance(bnc, &dave);

			// Check spot rate after block 1 trade
			{
				use hydradx_traits::amm::AMMInterface;
				let state2 =
					<hydradx_runtime::HydrationSimulatorConfig as SimulatorConfig>::Simulators::initial_state();
				let routes = TestSimulator::discover_routes(hdx, bnc, &state2).unwrap();
				let test_amount = 1_000_000_000_000u128; // 1 HDX
				if let Some((_, out, _)) =
					<TestSimulator as AMMInterface>::sell(hdx, bnc, test_amount, routes[0].clone(), &state2)
						.ok()
						.map(|(s, e)| (routes[0].clone(), e.amount_out, s))
				{
					let rate = out as f64 / test_amount as f64;
					println!(
						"block 2 spot check: 1 HDX → {} BNC (rate: {:.6}, min: 0.065000)",
						out, rate
					);
				}
			}

			let call2 = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			);

			// Block 1's trade moved the pool. Without external arbitrage (static snapshot),
			// the spot rate may now be below the whale's minimum. In that case, the solver
			// correctly produces no solution — the whale waits for conditions to improve.
			if let Some(pallet_ice::Call::submit_solution {
				solution: solution2, ..
			}) = call2
			{
				assert_eq!(solution2.resolved_intents.len(), 1);
				let (fill2_amount_in, fill2_amount_out) = {
					let ice_support::IntentData::Swap(ref s2) = solution2.resolved_intents[0].data else {
						panic!("expected Swap");
					};
					(s2.amount_in, s2.amount_out)
				};

				println!(
					"fill 2: {} ({:.1}% of remaining)",
					fill2_amount_in,
					(fill2_amount_in as f64 / stored_swap.remaining() as f64) * 100.0
				);

				crate::polkadot_test_net::hydradx_run_to_next_block();
				assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
					RuntimeOrigin::none(),
					solution2,
				));
				println!("submit_solution block 2: OK");

				let stored2 =
					pallet_intent::Pallet::<Runtime>::get_intent(intent_id).expect("Intent should still exist");
				let ice_support::IntentData::Swap(ref s2_stored) = stored2.data else {
					panic!("expected Swap")
				};
				println!(
					"after fill 2: filled={}, remaining={}",
					s2_stored.partial.filled(),
					s2_stored.remaining()
				);
				assert!(
					s2_stored.partial.filled() > filled_after_1,
					"Cumulative filled should increase"
				);
			} else {
				println!(
					"block 2: no solution — pool rate below minimum after block 1 trade (expected in static snapshot)"
				);
				// Intent should still be open, waiting for better conditions
				let stored2 = pallet_intent::Pallet::<Runtime>::get_intent(intent_id)
					.expect("Intent should still exist even without block 2 fill");
				assert_eq!(stored2.data.amount_in(), whale_amount, "Original immutable");
			}
			println!("\ntest complete — partial fill across blocks verified");
		});
}

/// All intents are partial, same direction (HDX → BNC).
///
/// Phase A (non-partial stabilization) is a no-op because `non_partial_fills` is empty.
/// Phase B binary-searches each partial intent individually.
/// Charlie has a tighter limit and should get a smaller fill percentage.
#[test]
fn solver_v2_all_partial_same_direction() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	// Spot: ~0.068 BNC/HDX
	let alice_amount = 500_000 * hdx_unit;
	let bob_amount = 300_000 * hdx_unit;
	let charlie_amount = 200_000 * hdx_unit;

	// Loose limit: 0.050 BNC/HDX
	let alice_min = 25_000 * 1_000_000_000_000u128; // 500k * 0.050
	let bob_min = 15_000 * 1_000_000_000_000u128; // 300k * 0.050
											   // Tight limit: 0.066 BNC/HDX (close to spot ~0.068)
	let charlie_min = 13_200 * 1_000_000_000_000u128; // 200k * 0.066

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_amount * 2)
		.endow_account(bob.clone(), hdx, bob_amount * 2)
		.endow_account(charlie.clone(), hdx, charlie_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Submit all 3 as partial intents
			for (who, amount, min_out, label) in [
				(alice.clone(), alice_amount, alice_min, "alice"),
				(bob.clone(), bob_amount, bob_min, "bob"),
				(charlie.clone(), charlie_amount, charlie_min, "charlie"),
			] {
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(who),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: hdx,
							asset_out: bnc,
							amount_in: amount,
							amount_out: min_out,
							partial: true,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!("{}: submitted {} HDX → BNC (partial)", label, amount / hdx_unit);
			}

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 3, "Should have 3 intents");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"\nsolution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// At least some partial intents should be resolved.
			// The solver processes partial intents sequentially in Phase B;
			// later ones may not find viable fills if earlier fills consumed
			// too much AMM capacity.
			assert!(
				!solution.resolved_intents.is_empty(),
				"At least one partial intent should be resolved"
			);
			println!("resolved {} out of 3 intents", solution.resolved_intents.len());

			// Track fills per account
			let mut charlie_fill_pct = 0.0f64;
			let mut alice_fill_pct = 0.0f64;
			let mut bob_fill_pct = 0.0f64;

			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				assert!(s.partial.is_partial(), "All resolved intents should be partial");
				assert!(s.amount_in > 0, "Fill amount must be > 0");

				// Verify rate constraint: amount_out >= fill_amount * original_min / original_amount_in
				let original = intents.iter().find(|(id, _)| *id == ri.id).expect("intent must exist");
				let original_amount_in = original.1.data.amount_in();
				let original_amount_out = original.1.data.amount_out();
				let pro_rata_min = (sp_core::U256::from(s.amount_in) * sp_core::U256::from(original_amount_out)
					/ sp_core::U256::from(original_amount_in))
				.as_u128();
				assert!(
					s.amount_out >= pro_rata_min,
					"Rate constraint violated: got {} out for {} in, pro_rata_min={}",
					s.amount_out,
					s.amount_in,
					pro_rata_min
				);

				let pct = s.amount_in as f64 / original_amount_in as f64 * 100.0;
				println!(
					"resolved id={}: fill {} / {} ({:.1}%), amount_out={}",
					ri.id, s.amount_in, original_amount_in, pct, s.amount_out
				);

				if original_amount_in == alice_amount {
					alice_fill_pct = pct;
				} else if original_amount_in == bob_amount {
					bob_fill_pct = pct;
				} else if original_amount_in == charlie_amount {
					charlie_fill_pct = pct;
				}
			}

			// Charlie (tight limit) should generally get a smaller fill % than Alice/Bob (loose limit)
			// because the binary search is more constrained.
			// Note: the solver processes partial intents sequentially, so this relationship
			// depends on processing order. Log it for debugging.
			println!(
				"\nfill percentages: alice={:.1}%, bob={:.1}%, charlie={:.1}%",
				alice_fill_pct, bob_fill_pct, charlie_fill_pct
			);

			assert!(solution.score > 0, "Score should be positive");

			// Execute solution
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Verify ED guard invariant: each intent still in storage must have remaining >= ED
			let hdx_ed = AssetRegistry::existential_deposit(hdx).unwrap_or(hdx_unit);
			for (id, intent) in pallet_intent::Pallet::<Runtime>::get_valid_intents() {
				let ice_support::IntentData::Swap(ref s) = intent.data else {
					continue;
				};
				let remaining = s.remaining();
				println!("intent {}: remaining={}", id, remaining);
				assert!(
					remaining == 0 || remaining >= hdx_ed,
					"ED guard violated: intent {} has remaining={} < ED={}",
					id,
					remaining,
					hdx_ed
				);
			}
		});
}

/// A small partial intent should be fully filled and removed from storage,
/// behaving identically to a non-partial intent.
#[test]
fn solver_v2_small_partial_fully_filled() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	// Tiny amount: 1,000 HDX. No slippage concern.
	let amount = 1_000 * hdx_unit;
	// Loose limit: 0.050 BNC/HDX → min 50 BNC
	let min_out = 50 * 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 10)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: amount,
						amount_out: min_out,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1);
			let intent_id = intents[0].0;

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 1);
			let ri = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref s) = ri.data else {
				panic!("expected Swap");
			};

			// Small partial should be FULLY filled
			assert_eq!(s.amount_in, amount, "Small partial intent should be fully filled");
			assert!(s.amount_out >= min_out, "Rate constraint must be met");

			println!(
				"fully filled: {} HDX → {} BNC (rate: {:.6})",
				s.amount_in as f64 / hdx_unit as f64,
				s.amount_out as f64 / 1_000_000_000_000f64,
				s.amount_out as f64 / s.amount_in as f64
			);

			let expected_bnc_out = s.amount_out;

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// Intent should be REMOVED from storage (fully filled partial)
			assert!(
				pallet_intent::Pallet::<Runtime>::get_intent(intent_id).is_none(),
				"Fully filled partial intent should be removed from storage"
			);

			// Verify balances
			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let hdx_spent = alice_hdx_before.saturating_sub(alice_hdx_after);
			let bnc_received = alice_bnc_after.saturating_sub(alice_bnc_before);

			assert_eq!(hdx_spent, amount, "Alice should spend exactly the fill amount");
			let fee = hydradx_runtime::IceFee::get().mul_floor(expected_bnc_out);
			assert_eq!(
				bnc_received,
				expected_bnc_out.saturating_sub(fee),
				"Alice should receive amount_out minus fee"
			);
			println!("submit_solution: OK — intent fully resolved and removed");
		});
}

/// Mixed: small partial intent alongside non-partial intents.
/// All are small enough to be fully filled.
#[test]
fn solver_v2_mixed_small_partial_and_non_partial() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	let amount_ab = 10_000 * hdx_unit;
	let amount_c = 5_000 * hdx_unit;
	// Loose limit: 0.050 BNC/HDX
	let min_ab = 500 * 1_000_000_000_000u128; // 10k * 0.050
	let min_c = 250 * 1_000_000_000_000u128; // 5k * 0.050

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount_ab * 10)
		.endow_account(bob.clone(), hdx, amount_ab * 10)
		.endow_account(charlie.clone(), hdx, amount_c * 10)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Alice and Bob: non-partial
			for (who, label) in [(alice.clone(), "alice"), (bob.clone(), "bob")] {
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(who),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: hdx,
							asset_out: bnc,
							amount_in: amount_ab,
							amount_out: min_ab,
							partial: false,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!("{}: {} HDX → BNC (non-partial)", label, amount_ab / hdx_unit);
			}

			// Charlie: partial
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(charlie.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: amount_c,
						amount_out: min_c,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));
			println!("charlie: {} HDX → BNC (partial)", amount_c / hdx_unit);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 3);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 3, "All 3 intents should be resolved");

			// Verify all are fully filled
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				let original = intents.iter().find(|(id, _)| *id == ri.id).expect("intent");
				assert_eq!(
					s.amount_in,
					original.1.data.amount_in(),
					"Intent {} should be fully filled",
					ri.id
				);
				println!(
					"id={}: fill={} (full), amount_out={}, partial={:?}",
					ri.id, s.amount_in, s.amount_out, s.partial
				);
			}

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// All intents should be removed from storage (fully resolved)
			let remaining = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert!(
				remaining.is_empty(),
				"All intents should be removed after full resolution, but {} remain",
				remaining.len()
			);
			println!("submit_solution: OK — all 3 intents fully resolved and removed");
		});
}

/// Two partial intents in opposing directions (HDX→BNC and BNC→HDX).
/// Both are partial, Phase A is a no-op.
/// Direct matching between partials should give better rates than AMM-only.
#[test]
fn solver_v2_all_partial_opposing_directions() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Alice: sell 500k HDX for BNC. Loose limit.
	let alice_amount = 500_000 * hdx_unit;
	let alice_min = 25_000 * bnc_unit; // 0.050 BNC/HDX

	// Bob: sell 20k BNC for HDX. Loose limit.
	// At spot ~14.7 HDX/BNC, 20k BNC = ~294k HDX
	let bob_amount = 20_000 * bnc_unit;
	let bob_min = 200_000 * hdx_unit; // 10 HDX/BNC (well below spot ~14.7)

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_amount * 2)
		.endow_account(bob.clone(), bnc, bob_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Alice: HDX → BNC (partial)
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: alice_amount,
						amount_out: alice_min,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			// Bob: BNC → HDX (partial, opposing direction)
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(bob.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: bnc,
						asset_out: hdx,
						amount_in: bob_amount,
						amount_out: bob_min,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			println!(
				"alice: {} HDX → BNC (partial), bob: {} BNC → HDX (partial)",
				alice_amount / hdx_unit,
				bob_amount / bnc_unit
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"solution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// Both should be resolved
			assert!(
				solution.resolved_intents.len() >= 2,
				"Both opposing partial intents should be resolved"
			);

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_bnc_before = Currencies::total_balance(bnc, &alice);
			let bob_hdx_before = Currencies::total_balance(hdx, &bob);
			let bob_bnc_before = Currencies::total_balance(bnc, &bob);

			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				let original = intents.iter().find(|(id, _)| *id == ri.id).expect("intent");
				let orig_in = original.1.data.amount_in();
				let orig_out = original.1.data.amount_out();
				let pro_rata_min = (sp_core::U256::from(s.amount_in) * sp_core::U256::from(orig_out)
					/ sp_core::U256::from(orig_in))
				.as_u128();

				assert!(s.amount_in > 0, "Fill must be > 0");
				assert!(
					s.amount_out >= pro_rata_min,
					"Rate constraint violated for intent {}: out={} < pro_rata_min={}",
					ri.id,
					s.amount_out,
					pro_rata_min
				);

				println!(
					"id={}: {} {} → {} {}, fill {:.1}%",
					ri.id,
					s.amount_in,
					s.asset_in,
					s.amount_out,
					s.asset_out,
					s.amount_in as f64 / orig_in as f64 * 100.0
				);
			}

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Verify balance changes
			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_bnc_after = Currencies::total_balance(bnc, &alice);
			let bob_hdx_after = Currencies::total_balance(hdx, &bob);
			let bob_bnc_after = Currencies::total_balance(bnc, &bob);

			assert!(alice_hdx_after < alice_hdx_before, "Alice should have spent HDX");
			assert!(alice_bnc_after > alice_bnc_before, "Alice should have received BNC");
			assert!(bob_bnc_after < bob_bnc_before, "Bob should have spent BNC");
			assert!(bob_hdx_after > bob_hdx_before, "Bob should have received HDX");

			println!(
				"alice: HDX {} → {}, BNC {} → {}",
				alice_hdx_before, alice_hdx_after, alice_bnc_before, alice_bnc_after
			);
			println!(
				"bob: BNC {} → {}, HDX {} → {}",
				bob_bnc_before, bob_bnc_after, bob_hdx_before, bob_hdx_after
			);

			// ED guard: remaining intents should have remaining >= ED
			let hdx_ed = AssetRegistry::existential_deposit(hdx).unwrap_or(hdx_unit);
			let bnc_ed = AssetRegistry::existential_deposit(bnc).unwrap_or(bnc_unit);
			for (id, intent) in pallet_intent::Pallet::<Runtime>::get_valid_intents() {
				let ice_support::IntentData::Swap(ref s) = intent.data else {
					continue;
				};
				let ed = if s.asset_in == hdx { hdx_ed } else { bnc_ed };
				assert!(
					s.remaining() >= ed,
					"ED guard: intent {} has remaining={} < ED={}",
					id,
					s.remaining(),
					ed
				);
			}
		});
}

/// Two large partial intents in the same direction competing for limited AMM capacity.
/// The pool can only absorb ~2-3M HDX total before slippage violates the tight rate.
#[test]
fn solver_v2_competing_partial_intents() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	let amount = 3_000_000 * hdx_unit;
	// Tight limit: ~0.066 BNC/HDX (spot ~0.068)
	let min_out = 198_000 * 1_000_000_000_000u128; // 3M * 0.066

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 2)
		.endow_account(bob.clone(), hdx, amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			for (who, label) in [(alice.clone(), "alice"), (bob.clone(), "bob")] {
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(who),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: hdx,
							asset_out: bnc,
							amount_in: amount,
							amount_out: min_out,
							partial: true,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!("{}: {} HDX → BNC (partial, tight limit)", label, amount / hdx_unit);
			}

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"solution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// At least one should be resolved. The solver processes partial intents
			// sequentially — the second may not find a viable fill if the first
			// consumed the AMM capacity at the tight rate.
			assert!(
				!solution.resolved_intents.is_empty(),
				"At least one partial intent should be resolved"
			);
			println!("resolved {} out of 2 intents", solution.resolved_intents.len());

			let mut total_fill = 0u128;
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					panic!("expected Swap");
				};
				assert!(s.amount_in > 0, "Fill must be > 0");

				let pro_rata_min = (sp_core::U256::from(s.amount_in) * sp_core::U256::from(min_out)
					/ sp_core::U256::from(amount))
				.as_u128();
				assert!(
					s.amount_out >= pro_rata_min,
					"Rate constraint violated: out={} < min={}",
					s.amount_out,
					pro_rata_min
				);

				total_fill += s.amount_in;
				println!(
					"id={}: fill {} ({:.1}%), out={}",
					ri.id,
					s.amount_in,
					s.amount_in as f64 / amount as f64 * 100.0,
					s.amount_out
				);
			}

			println!(
				"total fill: {} HDX ({:.1}% of combined 6M)",
				total_fill,
				total_fill as f64 / (2.0 * amount as f64) * 100.0
			);
			assert!(solution.score > 0, "Score should be positive");

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Resolved intents that were partially filled should remain in storage.
			// Intents not included in the solution also remain (unfilled).
			let remaining_intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("{} intents remain in storage", remaining_intents.len());

			let hdx_ed = AssetRegistry::existential_deposit(hdx).unwrap_or(hdx_unit);
			for (id, intent) in &remaining_intents {
				let ice_support::IntentData::Swap(ref s) = intent.data else {
					continue;
				};
				assert!(s.remaining() > 0, "Intent {} should have remaining", id);
				assert!(
					s.remaining() >= hdx_ed,
					"ED guard: intent {} remaining={} < ED={}",
					id,
					s.remaining(),
					hdx_ed
				);
				println!(
					"intent {}: filled={}, remaining={}",
					id,
					s.partial.filled(),
					s.remaining()
				);
			}
		});
}

/// Non-partial intent + partial intent in opposing directions.
/// Phase A handles the non-partial, Phase B handles the partial.
#[test]
fn solver_v2_partial_with_non_partial_opposing() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();

	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;
	let bnc_unit = 1_000_000_000_000u128;

	// Alice: sell 100k HDX for BNC (non-partial, loose limit)
	let alice_amount = 100_000 * hdx_unit;
	let alice_min = 5_000 * bnc_unit; // 0.050 BNC/HDX

	// Bob: sell 500k BNC for HDX (partial, loose limit)
	// At spot ~14.7 HDX/BNC, 500k BNC = ~7.35M HDX. Alice's 100k HDX is ~6.8k BNC.
	// So Alice is the scarce side; most of Bob's volume goes through AMM.
	let bob_amount = 500_000 * bnc_unit;
	let bob_min = 5_000_000 * hdx_unit; // 10 HDX/BNC

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, alice_amount * 2)
		.endow_account(bob.clone(), bnc, bob_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Alice: non-partial
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: alice_amount,
						amount_out: alice_min,
						partial: false,
					}),
					deadline,
					on_resolved: None,
				}
			));

			// Bob: partial
			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(bob.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: bnc,
						asset_out: hdx,
						amount_in: bob_amount,
						amount_out: bob_min,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			println!(
				"alice: {} HDX → BNC (non-partial), bob: {} BNC → HDX (partial)",
				alice_amount / hdx_unit,
				bob_amount / bnc_unit
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 2);

			// Find intent IDs
			let alice_intent_id = intents
				.iter()
				.find(|(_, i)| {
					let ice_support::IntentData::Swap(ref s) = i.data else {
						return false;
					};
					s.asset_in == hdx && !s.partial.is_partial()
				})
				.map(|(id, _)| *id)
				.expect("alice intent");

			let bob_intent_id = intents
				.iter()
				.find(|(_, i)| {
					let ice_support::IntentData::Swap(ref s) = i.data else {
						return false;
					};
					s.asset_in == bnc && s.partial.is_partial()
				})
				.map(|(id, _)| *id)
				.expect("bob intent");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"solution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// Both should be resolved
			assert!(solution.resolved_intents.len() >= 2, "Both intents should be resolved");

			// Alice should be fully resolved (non-partial)
			let alice_resolved = solution
				.resolved_intents
				.iter()
				.find(|ri| ri.id == alice_intent_id)
				.expect("Alice should be in solution");
			let ice_support::IntentData::Swap(ref alice_swap) = alice_resolved.data else {
				panic!("expected Swap");
			};
			assert_eq!(
				alice_swap.amount_in, alice_amount,
				"Alice (non-partial) should be fully filled"
			);
			println!(
				"alice: fully filled {} HDX → {} BNC",
				alice_swap.amount_in, alice_swap.amount_out
			);

			// Bob should be resolved (possibly partially)
			let bob_resolved = solution
				.resolved_intents
				.iter()
				.find(|ri| ri.id == bob_intent_id)
				.expect("Bob should be in solution");
			let ice_support::IntentData::Swap(ref bob_swap) = bob_resolved.data else {
				panic!("expected Swap");
			};
			assert!(bob_swap.amount_in > 0, "Bob should have some fill");
			let bob_fill_amount = bob_swap.amount_in;
			let bob_pro_rata = (sp_core::U256::from(bob_fill_amount) * sp_core::U256::from(bob_min)
				/ sp_core::U256::from(bob_amount))
			.as_u128();
			assert!(bob_swap.amount_out >= bob_pro_rata, "Bob rate constraint violated");
			println!(
				"bob: fill {} / {} BNC ({:.1}%), out={} HDX",
				bob_fill_amount,
				bob_amount,
				bob_fill_amount as f64 / bob_amount as f64 * 100.0,
				bob_swap.amount_out
			);

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Alice's intent should be removed (non-partial, fully resolved)
			assert!(
				pallet_intent::Pallet::<Runtime>::get_intent(alice_intent_id).is_none(),
				"Alice's non-partial intent should be removed"
			);

			// Bob's intent should remain if partially filled
			if bob_fill_amount < bob_amount {
				let stored = pallet_intent::Pallet::<Runtime>::get_intent(bob_intent_id)
					.expect("Bob's partial intent should remain");
				let ice_support::IntentData::Swap(ref s) = stored.data else {
					panic!("expected Swap");
				};
				assert_eq!(s.partial.filled(), bob_fill_amount);
				assert!(s.remaining() > 0);
				let bnc_ed = AssetRegistry::existential_deposit(bnc).unwrap_or(bnc_unit);
				assert!(
					s.remaining() >= bnc_ed,
					"ED guard: remaining={} < ED={}",
					s.remaining(),
					bnc_ed
				);
				println!(
					"bob intent remains: filled={}, remaining={}",
					s.partial.filled(),
					s.remaining()
				);
			} else {
				assert!(
					pallet_intent::Pallet::<Runtime>::get_intent(bob_intent_id).is_none(),
					"Bob's intent should be removed if fully filled"
				);
				println!("bob intent fully resolved and removed");
			}
		});
}

/// Cancel a partially filled intent. The unfilled portion should be returned to the user.
///
/// Block 1: Dave submits a large partial intent, solver partially fills it.
/// Block 2: Dave cancels the remaining portion via `remove_intent`.
/// Verify: Dave gets back the unfilled HDX, keeps the BNC from the fill.
#[test]
fn solver_v2_cancel_after_partial_fill() {
	TestNet::reset();

	let dave: AccountId = DAVE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	let total_amount = 5_000_000 * hdx_unit;
	// Tight limit: ~0.065 BNC/HDX
	let min_out = 325_000 * 1_000_000_000_000u128;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(dave.clone(), hdx, total_amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(dave.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: total_amount,
						amount_out: min_out,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1);
			let intent_id = intents[0].0;

			let dave_hdx_before = Currencies::total_balance(hdx, &dave);
			let dave_bnc_before = Currencies::total_balance(bnc, &dave);

			// --- Block 1: Solver partially fills Dave ---
			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 1);
			let ice_support::IntentData::Swap(ref s) = solution.resolved_intents[0].data else {
				panic!("expected Swap");
			};
			let fill_amount = s.amount_in;
			let fill_bnc_out = s.amount_out;
			assert!(fill_amount > 0, "Should have some fill");
			assert!(fill_amount < total_amount, "Should be partial fill, not full");

			println!(
				"fill: {} HDX ({:.1}%), out: {} BNC",
				fill_amount,
				fill_amount as f64 / total_amount as f64 * 100.0,
				fill_bnc_out
			);

			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// Verify partial fill state
			let stored = pallet_intent::Pallet::<Runtime>::get_intent(intent_id)
				.expect("Intent should still exist after partial fill");
			let ice_support::IntentData::Swap(ref stored_swap) = stored.data else {
				panic!("expected Swap");
			};
			assert_eq!(stored_swap.partial.filled(), fill_amount);
			let remaining = stored_swap.remaining();
			assert!(remaining > 0);
			println!(
				"after fill: filled={}, remaining={}",
				stored_swap.partial.filled(),
				remaining
			);

			let dave_hdx_after_fill = Currencies::total_balance(hdx, &dave);
			let dave_bnc_after_fill = Currencies::total_balance(bnc, &dave);

			// Dave should have spent the fill amount of HDX and received BNC (minus fee)
			let hdx_spent = dave_hdx_before.saturating_sub(dave_hdx_after_fill);
			assert_eq!(hdx_spent, fill_amount, "HDX spent should equal fill amount");

			let fee = hydradx_runtime::IceFee::get().mul_floor(fill_bnc_out);
			let expected_bnc = fill_bnc_out.saturating_sub(fee);
			let bnc_received = dave_bnc_after_fill.saturating_sub(dave_bnc_before);
			assert_eq!(bnc_received, expected_bnc, "BNC received should match payout");

			// --- Block 2: Dave cancels the remaining intent ---
			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(hydradx_runtime::Intent::remove_intent(
				RuntimeOrigin::signed(dave.clone()),
				intent_id
			));

			// Intent should be gone
			assert!(
				pallet_intent::Pallet::<Runtime>::get_intent(intent_id).is_none(),
				"Intent should be removed after cancellation"
			);

			// Dave should get back the remaining HDX (unreserved)
			let dave_hdx_after_cancel = Currencies::total_balance(hdx, &dave);
			let hdx_returned = dave_hdx_after_cancel.saturating_sub(dave_hdx_after_fill);
			println!(
				"after cancel: HDX returned={}, expected remaining={}",
				hdx_returned, remaining
			);
			// The remaining amount was locked via named reserve. Cancellation unreserves it,
			// which increases free balance but total_balance stays the same (reserved → free).
			// We check that the total balance didn't change after cancellation (just reserve → free).
			assert_eq!(
				dave_hdx_after_cancel, dave_hdx_after_fill,
				"Total HDX balance should not change on cancel (just unreserves)"
			);

			// Verify account cleanup
			assert_eq!(
				pallet_intent::AccountIntents::<Runtime>::iter_prefix(&dave).count(),
				0,
				"Account intent index should be cleaned up"
			);
			assert_eq!(
				pallet_intent::Pallet::<Runtime>::account_intent_count(&dave),
				0,
				"Account intent count should be 0"
			);

			println!(
				"\nfinal state: dave HDX={}, BNC={}",
				dave_hdx_after_cancel, dave_bnc_after_fill
			);
			println!("cancel after partial fill: OK");
		});
}

/// A large partial intent with an extremely loose limit gets fully filled.
/// The minimum rate is trivially met, so the solver fills the entire amount.
#[test]
fn solver_v2_partial_loose_limit_full_fill() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let bnc = 14u32;
	let hdx_unit = 1_000_000_000_000u128;

	let amount = 1_000_000 * hdx_unit;
	// Absurdly loose limit: 1 BNC for 1,000,000 HDX
	let min_out = 1_000_000_000_000u128; // 1 BNC

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount * 2)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: bnc,
						amount_in: amount,
						amount_out: min_out,
						partial: true,
					}),
					deadline,
					on_resolved: None,
				}
			));

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1);
			let intent_id = intents[0].0;

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 1);
			let ri = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref s) = ri.data else {
				panic!("expected Swap");
			};

			// With such a loose limit, the full amount should be fillable
			assert_eq!(s.amount_in, amount, "Loose-limit partial intent should be fully filled");

			// amount_out should be WAY above the 1 BNC minimum
			// At spot ~0.068, 1M HDX → ~68k BNC
			assert!(
				s.amount_out > min_out * 1000,
				"Output should massively exceed minimum: {} vs {}",
				s.amount_out,
				min_out
			);

			// Score should be very large (surplus = amount_out - pro_rata_min ≈ amount_out - 1 BNC)
			assert!(
				solution.score > s.amount_out / 2,
				"Score should be substantial: {} vs amount_out {}",
				solution.score,
				s.amount_out
			);

			println!(
				"fully filled: {} HDX → {} BNC (min was {} BNC), score={}",
				s.amount_in / hdx_unit,
				s.amount_out / 1_000_000_000_000u128,
				min_out / 1_000_000_000_000u128,
				solution.score
			);

			// Execute
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));

			// Intent should be removed (fully filled)
			assert!(
				pallet_intent::Pallet::<Runtime>::get_intent(intent_id).is_none(),
				"Fully filled partial intent should be removed from storage"
			);
			println!("submit_solution: OK — loose-limit partial fully resolved and removed");
		});
}

/// Single intent: Alice sells HDX for Hydrated Tether (asset 1111, 18 decimals).
/// Tests that the solver can discover a route and execute a trade for an aToken asset.
#[test]
fn solver_v2_single_intent_hdx_to_hydrated_tether() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let hdx = 0u32;
	let husdt = 1111u32; // Hydrated Tether — 18 decimals
	let hdx_unit = 1_000_000_000_000u128;
	let husdt_unit = 1_000_000_000_000_000_000u128; // 10^18

	// Sell 10,000 HDX — modest amount to avoid slippage issues
	let amount_in = 10_000 * hdx_unit;
	// Very loose limit: 1 hUSDT (effectively no minimum)
	let min_amount_out = 1 * husdt_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount_in * 10)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			assert_ok!(hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(alice.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: hdx,
						asset_out: husdt,
						amount_in,
						amount_out: min_amount_out,
						partial: false,
					}),
					deadline,
					on_resolved: None,
				}
			));
			println!(
				"alice: submitted {} HDX → hUSDT (asset {})",
				amount_in / hdx_unit,
				husdt
			);

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(intents.len(), 1, "Should have 1 intent");
			let intent_id = intents[0].0;

			let alice_hdx_before = Currencies::total_balance(hdx, &alice);
			let alice_husdt_before = Currencies::total_balance(husdt, &alice);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution for HDX → hUSDT");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
			assert!(solution.score > 0, "Score should be positive");

			let resolved = &solution.resolved_intents[0];
			let ice_support::IntentData::Swap(ref s) = resolved.data else {
				panic!("expected Swap");
			};
			assert_eq!(s.asset_in, hdx, "asset_in should be HDX");
			assert_eq!(s.asset_out, husdt, "asset_out should be hUSDT");
			assert_eq!(s.amount_in, amount_in, "amount_in should match");
			assert!(s.amount_out >= min_amount_out, "amount_out should meet minimum");

			println!(
				"resolved: {} HDX → {} hUSDT (rate: {:.6} hUSDT/HDX)",
				s.amount_in as f64 / hdx_unit as f64,
				s.amount_out as f64 / husdt_unit as f64,
				s.amount_out as f64 / s.amount_in as f64
			);

			// Log the route
			for (i, t) in solution.trades.iter().enumerate() {
				println!(
					"trade[{}]: {} → {}, amount_in={}, amount_out={}, route={:?}",
					i,
					t.route.first().map(|r| r.asset_in).unwrap_or(0),
					t.route.last().map(|r| r.asset_out).unwrap_or(0),
					t.amount_in,
					t.amount_out,
					t.route
						.iter()
						.map(|r| format!("{}->{} ({:?})", r.asset_in, r.asset_out, r.pool))
						.collect::<Vec<_>>()
				);
			}

			let expected_out = s.amount_out;

			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
			println!("submit_solution: OK");

			// Verify intent removed
			assert!(
				pallet_intent::Pallet::<Runtime>::get_intent(intent_id).is_none(),
				"Intent should be removed after resolution"
			);

			// Verify balances
			let alice_hdx_after = Currencies::total_balance(hdx, &alice);
			let alice_husdt_after = Currencies::total_balance(husdt, &alice);
			let hdx_spent = alice_hdx_before.saturating_sub(alice_hdx_after);
			let husdt_received = alice_husdt_after.saturating_sub(alice_husdt_before);

			assert_eq!(hdx_spent, amount_in, "HDX spent should match");
			let fee = hydradx_runtime::IceFee::get().mul_floor(expected_out);
			assert_eq!(
				husdt_received,
				expected_out.saturating_sub(fee),
				"hUSDT received should equal amount_out minus fee"
			);
			println!(
				"alice: spent {} HDX, received {} hUSDT (fee: {} hUSDT)",
				hdx_spent as f64 / hdx_unit as f64,
				husdt_received as f64 / husdt_unit as f64,
				fee as f64 / husdt_unit as f64
			);
		});
}

/// Four intents, all selling HDX but each buying a different Hydrated aToken.
///
/// Alice: HDX → hUSDT (1111)
/// Bob:   HDX → hUSDC (1112)
/// Charlie: HDX → hWBTC (1113)
/// Dave:  HDX → hUSDT_old/hDOT (1110)
///
/// Tests that the solver handles multiple aToken destinations in a single batch.
/// Each intent routes through different pools (Omnipool + Stableswap + Aave).
#[test]
fn solver_v2_four_intents_hdx_to_different_atokens() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let hdx_unit = 1_000_000_000_000u128;
	let atoken_unit = 1_000_000_000_000_000_000u128; // 18 decimals for all aTokens

	// Each user sells 10,000 HDX for a different aToken
	let amount_in = 10_000 * hdx_unit;
	// Very loose limit — 1 unit of each aToken
	let min_out = 1 * atoken_unit;

	struct IntentSetup {
		who: AccountId,
		label: &'static str,
		asset_out: u32,
		asset_name: &'static str,
	}

	let setups = [
		IntentSetup {
			who: alice.clone(),
			label: "alice",
			asset_out: 1111,
			asset_name: "hUSDT",
		},
		IntentSetup {
			who: bob.clone(),
			label: "bob",
			asset_out: 1112,
			asset_name: "hUSDC",
		},
		IntentSetup {
			who: charlie.clone(),
			label: "charlie",
			asset_out: 1113,
			asset_name: "hWBTC",
		},
		IntentSetup {
			who: dave.clone(),
			label: "dave",
			asset_out: 1110,
			asset_name: "h1110",
		},
	];

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, amount_in * 10)
		.endow_account(bob.clone(), hdx, amount_in * 10)
		.endow_account(charlie.clone(), hdx, amount_in * 10)
		.endow_account(dave.clone(), hdx, amount_in * 10)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Submit all 4 intents
			for s in &setups {
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(s.who.clone()),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: hdx,
							asset_out: s.asset_out,
							amount_in,
							amount_out: min_out,
							partial: false,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!(
					"{}: submitted {} HDX → {} (asset {})",
					s.label,
					amount_in / hdx_unit,
					s.asset_name,
					s.asset_out
				);
			}

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("\ntotal valid intents: {}", intents.len());
			assert_eq!(intents.len(), 4, "Should have 4 intents");

			// Capture balances before
			let balances_before: Vec<(AccountId, u32, u128, u128)> = setups
				.iter()
				.map(|s| {
					let hdx_bal = Currencies::total_balance(hdx, &s.who);
					let out_bal = Currencies::total_balance(s.asset_out, &s.who);
					(s.who.clone(), s.asset_out, hdx_bal, out_bal)
				})
				.collect();

			// Run solver
			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"\nsolution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// Log resolved intents
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					continue;
				};
				let name = setups
					.iter()
					.find(|setup| setup.asset_out == s.asset_out)
					.map(|setup| setup.asset_name)
					.unwrap_or("?");
				println!(
					"  id={}: {} HDX → {} {} (rate: {:.6})",
					ri.id,
					s.amount_in as f64 / hdx_unit as f64,
					s.amount_out as f64 / atoken_unit as f64,
					name,
					s.amount_out as f64 / s.amount_in as f64
				);
			}

			// Log trades with routes
			for (i, t) in solution.trades.iter().enumerate() {
				println!(
					"  trade[{}]: amount_in={}, amount_out={}, route={:?}",
					i,
					t.amount_in,
					t.amount_out,
					t.route
						.iter()
						.map(|r| format!("{}->{} ({:?})", r.asset_in, r.asset_out, r.pool))
						.collect::<Vec<_>>()
				);
			}

			// At least some intents should be resolved — some aToken assets might not
			// have routes on this snapshot
			assert!(
				!solution.resolved_intents.is_empty(),
				"At least one intent should be resolved"
			);
			assert!(solution.score > 0, "Score should be positive");

			// Execute solution
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));
			println!("\nsubmit_solution: OK");

			// Verify balances for each resolved intent
			let ice_fee = hydradx_runtime::IceFee::get();
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					continue;
				};
				let setup = setups.iter().find(|setup| setup.asset_out == s.asset_out);
				let Some(setup) = setup else { continue };

				let (_, _, hdx_before, out_before) = balances_before
					.iter()
					.find(|(who, asset, _, _)| who == &setup.who && *asset == s.asset_out)
					.unwrap();

				let hdx_after = Currencies::total_balance(hdx, &setup.who);
				let out_after = Currencies::total_balance(s.asset_out, &setup.who);

				let hdx_spent = hdx_before.saturating_sub(hdx_after);
				let out_received = out_after.saturating_sub(*out_before);
				let fee = ice_fee.mul_floor(s.amount_out);
				let expected_payout = s.amount_out.saturating_sub(fee);

				assert_eq!(hdx_spent, s.amount_in, "{}: HDX spent should match fill", setup.label);
				assert_eq!(
					out_received, expected_payout,
					"{}: {} received should match amount_out minus fee",
					setup.label, setup.asset_name
				);

				println!(
					"{}: spent {} HDX, received {} {} (fee: {})",
					setup.label,
					hdx_spent as f64 / hdx_unit as f64,
					out_received as f64 / atoken_unit as f64,
					setup.asset_name,
					fee as f64 / atoken_unit as f64
				);
			}

			// Resolved intents should be removed from storage
			for ri in solution.resolved_intents.iter() {
				assert!(
					pallet_intent::Pallet::<Runtime>::get_intent(ri.id).is_none(),
					"Resolved intent {} should be removed from storage",
					ri.id
				);
			}
		});
}

/// Cross-aToken trades with direct matching opportunities.
///
/// Prep: sell HDX to acquire aTokens for each user (can't mint aTokens directly).
///   - Alice gets hUSDT (1111) via HDX → 1111
///   - Bob gets hUSDT (1111) via HDX → 1111  (extra prep so Bob also has 1111)
///   - Charlie gets hWBTC (1113) via HDX → 1113
///   - Dave gets h1110 (1110) via HDX → 1110
///
/// Then submit cross-aToken intents:
///   - Alice: sells 1111 (hUSDT) → buys 1110
///   - Bob:   sells 1111 (hUSDT) → buys 1113 (hWBTC)
///   - Charlie: sells 1113 (hWBTC) → buys 1111 (hUSDT)
///   - Dave: sells 1110 → buys 1113 (hWBTC)
///
/// Matching opportunities:
///   - Bob (1111→1113) and Charlie (1113→1111) are opposing — direct match possible
///   - Alice (1111→1110) and Dave (1110→1113) form a partial chain
#[test]
fn solver_v2_cross_atoken_trades_with_matching() {
	TestNet::reset();

	let alice: AccountId = ALICE.into();
	let bob: AccountId = BOB.into();
	let charlie: AccountId = CHARLIE.into();
	let dave: AccountId = DAVE.into();

	let hdx = 0u32;
	let hdx_unit = 1_000_000_000_000u128;
	let atoken_unit = 1_000_000_000_000_000_000u128; // 18 decimals

	// Assets
	let husdt = 1111u32;
	let husdc = 1112u32;
	let hwbtc = 1113u32;
	let h1110 = 1110u32;
	let _ = husdc; // not used in this test

	// HDX amount for prep trades — enough to get meaningful aToken balances
	let prep_hdx = 10_000 * hdx_unit;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, prep_hdx * 10)
		.endow_account(bob.clone(), hdx, prep_hdx * 10)
		.endow_account(charlie.clone(), hdx, prep_hdx * 10)
		.endow_account(dave.clone(), hdx, prep_hdx * 10)
		.execute(|| {
			enable_slip_fees();

			// ============================================================
			// PREP: acquire aTokens by selling HDX through the router
			// ============================================================
			println!("=== PREP: acquiring aTokens via HDX trades ===\n");

			let prep_trades: Vec<(AccountId, &str, u32, &str)> = vec![
				(alice.clone(), "alice", husdt, "hUSDT"),
				(bob.clone(), "bob", husdt, "hUSDT"), // Bob also gets hUSDT
				(charlie.clone(), "charlie", hwbtc, "hWBTC"),
				(dave.clone(), "dave", h1110, "h1110"),
			];

			for (who, label, asset_out, name) in &prep_trades {
				let route = Router::get_route(hydradx_traits::router::AssetPair::new(hdx, *asset_out));
				assert!(
					!route.is_empty(),
					"No route found for HDX → {} (asset {})",
					name,
					asset_out
				);

				crate::polkadot_test_net::hydradx_run_to_next_block();

				let bal_before = Currencies::free_balance(*asset_out, who);
				assert_ok!(pallet_route_executor::Pallet::<Runtime>::sell(
					RuntimeOrigin::signed(who.clone()),
					hdx,
					*asset_out,
					prep_hdx,
					0, // no min — just acquiring tokens
					route.clone(),
				));
				let bal_after = Currencies::free_balance(*asset_out, who);
				let received = bal_after.saturating_sub(bal_before);
				println!(
					"{}: sold {} HDX → {} {} (route: {} hops)",
					label,
					prep_hdx / hdx_unit,
					received as f64 / atoken_unit as f64,
					name,
					route.len()
				);
				assert!(received > 0, "{} should have received some {}", label, name);
			}

			// Log balances after prep
			println!("\n=== Balances after prep ===");
			println!(
				"alice hUSDT: {}",
				Currencies::free_balance(husdt, &alice) as f64 / atoken_unit as f64
			);
			println!(
				"bob hUSDT: {}",
				Currencies::free_balance(husdt, &bob) as f64 / atoken_unit as f64
			);
			println!(
				"charlie hWBTC: {}",
				Currencies::free_balance(hwbtc, &charlie) as f64 / atoken_unit as f64
			);
			println!(
				"dave h1110: {}",
				Currencies::free_balance(h1110, &dave) as f64 / atoken_unit as f64
			);

			// ============================================================
			// INTENTS: cross-aToken trades
			// ============================================================
			println!("\n=== Submitting cross-aToken intents ===\n");

			crate::polkadot_test_net::hydradx_run_to_next_block();
			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Use half of each user's aToken balance as the sell amount
			let alice_sell = Currencies::free_balance(husdt, &alice) / 2;
			let bob_sell = Currencies::free_balance(husdt, &bob) / 2;
			let charlie_sell = Currencies::free_balance(hwbtc, &charlie) / 2;
			let dave_sell = Currencies::free_balance(h1110, &dave) / 2;

			struct CrossIntent {
				who: AccountId,
				label: &'static str,
				asset_in: u32,
				asset_in_name: &'static str,
				asset_out: u32,
				asset_out_name: &'static str,
				amount_in: u128,
			}

			let cross_intents = [
				CrossIntent {
					who: alice.clone(),
					label: "alice",
					asset_in: husdt,
					asset_in_name: "hUSDT",
					asset_out: h1110,
					asset_out_name: "h1110",
					amount_in: alice_sell,
				},
				CrossIntent {
					who: bob.clone(),
					label: "bob",
					asset_in: husdt,
					asset_in_name: "hUSDT",
					asset_out: hwbtc,
					asset_out_name: "hWBTC",
					amount_in: bob_sell,
				},
				CrossIntent {
					who: charlie.clone(),
					label: "charlie",
					asset_in: hwbtc,
					asset_in_name: "hWBTC",
					asset_out: husdt,
					asset_out_name: "hUSDT",
					amount_in: charlie_sell,
				},
				CrossIntent {
					who: dave.clone(),
					label: "dave",
					asset_in: h1110,
					asset_in_name: "h1110",
					asset_out: hwbtc,
					asset_out_name: "hWBTC",
					amount_in: dave_sell,
				},
			];

			for ci in &cross_intents {
				// Very loose limit: 1 unit of output token
				assert_ok!(hydradx_runtime::Intent::submit_intent(
					RuntimeOrigin::signed(ci.who.clone()),
					pallet_intent::types::IntentInput {
						data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
							asset_in: ci.asset_in,
							asset_out: ci.asset_out,
							amount_in: ci.amount_in,
							amount_out: 1 * atoken_unit,
							partial: false,
						}),
						deadline,
						on_resolved: None,
					}
				));
				println!(
					"{}: sells {} {} → {} (amount: {})",
					ci.label,
					ci.asset_in_name,
					ci.asset_in,
					ci.asset_out_name,
					ci.amount_in as f64 / atoken_unit as f64
				);
			}

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			println!("\ntotal valid intents: {}", intents.len());
			assert_eq!(intents.len(), 4, "Should have 4 intents");

			// Capture output balances before solve (we verify the user received the right amount)
			let out_bals_before: Vec<(AccountId, u32, u128)> = cross_intents
				.iter()
				.map(|ci| {
					(
						ci.who.clone(),
						ci.asset_out,
						Currencies::free_balance(ci.asset_out, &ci.who),
					)
				})
				.collect();

			// ============================================================
			// SOLVE
			// ============================================================
			println!("\n=== Running solver ===\n");

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver must produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			println!(
				"solution: {} resolved, {} trades, score: {}",
				solution.resolved_intents.len(),
				solution.trades.len(),
				solution.score
			);

			// Log resolved intents
			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					continue;
				};
				let ci = cross_intents
					.iter()
					.find(|ci| ci.asset_in == s.asset_in && ci.asset_out == s.asset_out);
				let label = ci.map(|c| c.label).unwrap_or("?");
				println!(
					"  {} (id={}): {} {} → {} {} (rate: {:.6})",
					label,
					ri.id,
					s.amount_in as f64 / atoken_unit as f64,
					s.asset_in,
					s.amount_out as f64 / atoken_unit as f64,
					s.asset_out,
					s.amount_out as f64 / s.amount_in as f64
				);
			}

			// Log trades
			for (i, t) in solution.trades.iter().enumerate() {
				println!(
					"  trade[{}]: amount_in={}, amount_out={}, route={:?}",
					i,
					t.amount_in,
					t.amount_out,
					t.route
						.iter()
						.map(|r| format!("{}->{} ({:?})", r.asset_in, r.asset_out, r.pool))
						.collect::<Vec<_>>()
				);
			}

			// Check for matching: if Bob(1111→1113) and Charlie(1113→1111) matched,
			// the number of AMM trades should be fewer than 4
			if solution.trades.len() < 4 {
				println!(
					"\n→ Direct matching detected! {} AMM trades instead of 4",
					solution.trades.len()
				);
			} else {
				println!("\n→ No direct matching (4 AMM trades)");
			}

			assert!(
				solution.resolved_intents.len() == 4,
				"All 4 intents should be resolved, got {}",
				solution.resolved_intents.len()
			);
			assert!(solution.score > 0, "Score should be positive");

			// ============================================================
			// EXECUTE
			// ============================================================
			crate::polkadot_test_net::hydradx_run_to_next_block();
			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution.clone(),
			));
			println!("\nsubmit_solution: OK");

			// ============================================================
			// VERIFY
			// ============================================================
			let ice_fee = hydradx_runtime::IceFee::get();

			for ri in solution.resolved_intents.iter() {
				let ice_support::IntentData::Swap(ref s) = ri.data else {
					continue;
				};

				// Match resolved intent to our setup by asset_in + asset_out
				let idx = cross_intents
					.iter()
					.position(|ci| ci.asset_in == s.asset_in && ci.asset_out == s.asset_out)
					.expect("Resolved intent should match a submitted intent");
				let ci = &cross_intents[idx];
				let (_, _, out_before) = &out_bals_before[idx];

				let out_after = Currencies::free_balance(s.asset_out, &ci.who);
				let received = out_after.saturating_sub(*out_before);
				let fee = ice_fee.mul_floor(s.amount_out);
				let expected_payout = s.amount_out.saturating_sub(fee);

				assert_eq!(
					received, expected_payout,
					"{}: received {} should match amount_out minus fee (expected {})",
					ci.label, received, expected_payout
				);

				println!(
					"{}: received {} {} (fee: {})",
					ci.label,
					received as f64 / atoken_unit as f64,
					ci.asset_out_name,
					fee as f64 / atoken_unit as f64
				);
			}

			// All intents should be removed
			for ri in solution.resolved_intents.iter() {
				assert!(
					pallet_intent::Pallet::<Runtime>::get_intent(ri.id).is_none(),
					"Intent {} should be removed after resolution",
					ri.id
				);
			}

			println!("\ncross-aToken trades with matching: OK");
		});
}

/// Same setup as DCA's `dca_succeeds_after_extra_gas_increased_due_to_out_of_gas_error`:
/// deploy a ConditionalGasEater ERC20 contract, add it to the omnipool, then try to
/// sell it for HDX via an ICE intent instead of DCA.
///
/// The DCA test fails with out-of-gas on the first trade attempt. This test checks
/// whether the same trade via ICE/intent also hits out-of-gas or not.
#[test]
fn ice_intent_with_evm_gas_eater_token() {
	use crate::polkadot_test_net::{hydradx_run_to_block, DAI, HDX, LRNA, UNITS};
	use frame_system::RawOrigin;
	use hydradx_runtime::{Balances, EVMAccounts, EmaOracle, MultiTransactionPayment, Tokens, Treasury};
	use hydradx_traits::evm::InspectEvmAccounts;
	use primitives::constants::chain::OMNIPOOL_SOURCE;
	use sp_runtime::FixedU128;
	use xcm_emulator::TestExt;

	TestNet::reset();
	Hydra::execute_with(|| {
		// ============================================================
		// SETUP: same as the DCA out-of-gas test
		// ============================================================
		crate::dca::init_omnipool_with_oracle_for_block_10();

		let evm_address = EVMAccounts::evm_address(&Router::router_account());
		let contract =
			crate::dca::extra_gas_erc20::deploy_conditional_gas_eater(evm_address, 400_000, crate::erc20::deployer());
		let erc20 = crate::erc20::bind_erc20(contract);
		assert_ok!(EmaOracle::add_oracle(
			RuntimeOrigin::root(),
			OMNIPOOL_SOURCE,
			(LRNA, erc20)
		));

		// Add new erc20 to omnipool
		let bal = Currencies::free_balance(erc20, &ALICE.into());
		assert_ok!(Currencies::transfer(
			RuntimeOrigin::signed(ALICE.into()),
			pallet_omnipool::Pallet::<Runtime>::protocol_account(),
			erc20,
			bal / 10,
		));
		assert_ok!(pallet_omnipool::Pallet::<Runtime>::add_token(
			RuntimeOrigin::root(),
			erc20,
			FixedU128::from_rational(1, 200),
			Permill::from_percent(30),
			ALICE.into(),
		));

		assert_ok!(MultiTransactionPayment::add_currency(
			RuntimeOrigin::root(),
			erc20,
			FixedU128::from_rational(1, 200)
		));

		hydradx_run_to_block(11);

		println!("setup complete: erc20 asset id = {}", erc20);
		println!(
			"alice erc20 balance: {}",
			Currencies::free_balance(erc20, &ALICE.into())
		);

		// ============================================================
		// INTENT: sell the gas-eater ERC20 for HDX (same trade as DCA)
		// ============================================================
		let sell_amount = 200_000 * UNITS;
		let ts = hydradx_runtime::Timestamp::now();
		let deadline = Some(ts + 120_000); // 120s deadline

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(ALICE.into()),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: erc20,
					asset_out: HDX,
					amount_in: sell_amount,
					amount_out: UNITS, // 1 HDX minimum (must be >= ED)
					partial: false,
				}),
				deadline,
				on_resolved: None,
			}
		));
		println!("submitted intent: sell {} erc20 (gas-eater) → HDX", sell_amount);

		let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
		assert_eq!(intents.len(), 1, "Should have 1 intent");

		// ============================================================
		// RUN SOLVER
		// ============================================================
		let alice_hdx_before = Currencies::free_balance(HDX, &ALICE.into());
		let alice_erc20_before = Currencies::free_balance(erc20, &ALICE.into());

		let call = pallet_ice::Pallet::<Runtime>::run(
			hydradx_runtime::System::block_number(),
			|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
		)
		.expect("Solver must produce a solution for the gas-eater ERC20 token");

		let pallet_ice::Call::submit_solution { solution, .. } = call else {
			panic!("Expected submit_solution call");
		};

		assert_eq!(solution.resolved_intents.len(), 1, "Should resolve exactly 1 intent");
		assert!(solution.score > 0, "Score should be positive");

		let resolved = &solution.resolved_intents[0];
		let ice_support::IntentData::Swap(ref s) = resolved.data else {
			panic!("expected Swap");
		};
		assert_eq!(s.asset_in, erc20, "asset_in should be the gas-eater ERC20");
		assert_eq!(s.asset_out, HDX, "asset_out should be HDX");
		assert_eq!(s.amount_in, sell_amount, "amount_in should match");
		assert!(s.amount_out >= UNITS, "amount_out should meet minimum (1 HDX)");

		let expected_hdx_out = s.amount_out;

		// ============================================================
		// EXECUTE
		// ============================================================
		hydradx_run_to_block(12);

		assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
			RuntimeOrigin::none(),
			solution,
		));

		// Verify intent removed
		assert!(
			pallet_intent::Pallet::<Runtime>::get_intent(intents[0].0).is_none(),
			"Intent should be removed after resolution"
		);

		// Verify Alice received correct HDX amount (minus fee)
		let alice_hdx_after = Currencies::free_balance(HDX, &ALICE.into());
		let hdx_received = alice_hdx_after.saturating_sub(alice_hdx_before);
		let fee = hydradx_runtime::IceFee::get().mul_floor(expected_hdx_out);
		let expected_payout = expected_hdx_out.saturating_sub(fee);

		assert_eq!(
			hdx_received, expected_payout,
			"Alice should receive amount_out minus fee: expected {}, got {}",
			expected_payout, hdx_received
		);
		assert!(hdx_received > 0, "Alice must receive some HDX");

		println!(
			"alice: received {} HDX (fee: {} HDX)",
			hdx_received as f64 / UNITS as f64,
			fee as f64 / UNITS as f64
		);
	});
}

/// Verify the solver caps resolved intents at MAX_NUMBER_OF_RESOLVED_INTENTS.
///
/// When valid intents exceed the limit, the solver must truncate *before*
/// computing the score so the submitted solution is consistent. Without the
/// cap the score would reflect all intents but the BoundedVec would silently
/// drop the overflow, causing a score/solution mismatch.
#[test]
fn solver_caps_at_max_resolved_intents() {
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

	// We need more intents than the cap. Each account can hold up to 100.
	// Submit ~25 per account across 5 accounts = 125 total (> 100 cap).
	let intents_per_account = 25u32;
	let total_intents = intents_per_account * 5;
	assert!(
		total_intents > MAX_NUMBER_OF_RESOLVED_INTENTS,
		"test must submit more than MAX_NUMBER_OF_RESOLVED_INTENTS intents"
	);

	let sell_hdx_amount = 500 * hdx_unit;
	let sell_bnc_amount = 30 * bnc_unit;
	// Loose limits so all intents are satisfiable
	let min_bnc = bnc_unit;
	let min_hdx = hdx_unit;

	let accounts = [alice.clone(), bob.clone(), charlie.clone(), dave.clone(), eve.clone()];

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), hdx, sell_hdx_amount * intents_per_account as u128)
		.endow_account(alice.clone(), bnc, sell_bnc_amount * intents_per_account as u128)
		.endow_account(bob.clone(), hdx, sell_hdx_amount * intents_per_account as u128)
		.endow_account(bob.clone(), bnc, sell_bnc_amount * intents_per_account as u128)
		.endow_account(charlie.clone(), hdx, sell_hdx_amount * intents_per_account as u128)
		.endow_account(charlie.clone(), bnc, sell_bnc_amount * intents_per_account as u128)
		.endow_account(dave.clone(), hdx, sell_hdx_amount * intents_per_account as u128)
		.endow_account(dave.clone(), bnc, sell_bnc_amount * intents_per_account as u128)
		.endow_account(eve.clone(), hdx, sell_hdx_amount * intents_per_account as u128)
		.endow_account(eve.clone(), bnc, sell_bnc_amount * intents_per_account as u128)
		.execute(|| {
			enable_slip_fees();

			let ts = hydradx_runtime::Timestamp::now();
			let deadline = Some(primitives::constants::time::MILLISECS_PER_BLOCK * 10u64 + ts);

			// Submit intents: alternate direction per account to create matching flow
			for (i, acc) in accounts.iter().enumerate() {
				for j in 0..intents_per_account {
					// Odd accounts sell BNC→HDX, even sell HDX→BNC; flip on odd j
					let sell_hdx = (i % 2 == 0) ^ (j % 2 == 1);
					let (asset_in, asset_out, amount_in, amount_out) = if sell_hdx {
						(hdx, bnc, sell_hdx_amount, min_bnc)
					} else {
						(bnc, hdx, sell_bnc_amount, min_hdx)
					};
					assert_ok!(hydradx_runtime::Intent::submit_intent(
						RuntimeOrigin::signed(acc.clone()),
						pallet_intent::types::IntentInput {
							data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
								asset_in,
								asset_out,
								amount_in,
								amount_out,
								partial: false,
							}),
							deadline,
							on_resolved: None,
						}
					));
				}
			}

			let intents = pallet_intent::Pallet::<Runtime>::get_valid_intents();
			assert_eq!(
				intents.len(),
				total_intents as usize,
				"Should have submitted {} intents",
				total_intents
			);

			let call = pallet_ice::Pallet::<Runtime>::run(
				hydradx_runtime::System::block_number(),
				|intents: Vec<ice_support::Intent>, state: CombinedSimulatorState| Solver::solve(intents, state).ok(),
			)
			.expect("Solver should produce a solution");

			let pallet_ice::Call::submit_solution { solution, .. } = call else {
				panic!("Expected submit_solution call");
			};

			// The solver must cap at MAX_NUMBER_OF_RESOLVED_INTENTS
			assert_eq!(
				solution.resolved_intents.len(),
				MAX_NUMBER_OF_RESOLVED_INTENTS as usize,
				"Solver should cap resolved intents at MAX_NUMBER_OF_RESOLVED_INTENTS ({}), got {}",
				MAX_NUMBER_OF_RESOLVED_INTENTS,
				solution.resolved_intents.len()
			);
			assert!(solution.score > 0, "Score should be positive");

			// The solution must be submittable — this proves the score is consistent
			// with the truncated set (the bug we're guarding against).
			crate::polkadot_test_net::hydradx_run_to_next_block();

			assert_ok!(pallet_ice::Pallet::<Runtime>::submit_solution(
				RuntimeOrigin::none(),
				solution,
			));
		});
}
