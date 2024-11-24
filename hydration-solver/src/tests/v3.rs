use crate::data::AssetData;
use crate::omni::OmniSolver;
use crate::problem::FloatType;
use crate::tests::{generate_random_intents, AssetId, DataProvider};
use crate::v3::SolverV3;
use pallet_ice::traits::{OmnipoolInfo, Solver};
use pallet_ice::types::{Intent, ResolvedIntent, Swap, SwapType};
use primitives::AccountId;
use sp_core::crypto::AccountId32;
use std::time::Instant;

pub const ALICE: [u8; 32] = [4u8; 32];

#[test]
fn solver_should_find_solution_for_one_small_amount_partial_intent() {
	let intents = vec![(
		0,
		Intent {
			who: ALICE.into(),
			swap: Swap {
				asset_in: 0u32,
				asset_out: 27u32,
				amount_in: 100_000_000_000_000,
				amount_out: 1_149_000_000_000,
				swap_type: SwapType::ExactIn,
			},
			deadline: 0,
			partial: true,
			on_success: None,
			on_failure: None,
		},
	)];
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 100_000_000_000_000,
		amount_out: 1_149_000_000_000,
	}];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_one_large_amount_partial_intent() {
	let intents = vec![(
		0,
		Intent {
			who: ALICE.into(),
			swap: Swap {
				asset_in: 0u32,
				asset_out: 27u32,
				amount_in: 1_000_000_000_000_000_000,
				amount_out: 1_149_000_000_000_000,
				swap_type: SwapType::ExactIn,
			},
			deadline: 0,
			partial: true,
			on_success: None,
			on_failure: None,
		},
	)];
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_one_large_amount_full_intent() {
	let intents = vec![(
		0,
		Intent {
			who: ALICE.into(),
			swap: Swap {
				asset_in: 0u32,
				asset_out: 27u32,
				amount_in: 1_000_000_000_000_000_000,
				amount_out: 1_149_000_000_000_000,
				swap_type: SwapType::ExactIn,
			},
			deadline: 0,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	)];
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_two_intents() {
	let intents = vec![
		(
			0,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 0u32,
					asset_out: 27u32,
					amount_in: 1_000_000_000_000_000_000,
					amount_out: 1_149_000_000_000_000,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		),
		(
			1,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 20,
					asset_out: 8,
					amount_in: 165453758222187283838,
					amount_out: 2808781311006261193,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		),
	];
	let start = Instant::now();
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let duration = start.elapsed();
	println!("Time elapsed in solve() is: {:?}", duration);
	let expected_solution = vec![
		ResolvedIntent {
			intent_id: 0,
			amount_in: 1_000_000_000_000_000_000,
			amount_out: 1_149_000_000_000_000,
		},
		ResolvedIntent {
			intent_id: 1,
			amount_in: 36895351807444140032,
			amount_out: 626344035537618048,
		},
	];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_two_partial_intents() {
	let intents = vec![
		(
			0,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 12,
					asset_out: 14,
					amount_in: 9206039265427194,
					amount_out: 1,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		),
		(
			1,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 28,
					asset_out: 8,
					amount_in: 1076105965030805693,
					amount_out: 1,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		),
	];
	let start = Instant::now();
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let duration = start.elapsed();
	println!("Time elapsed in solve() is: {:?}", duration);
	let expected_solution = vec![
		ResolvedIntent {
			intent_id: 0,
			amount_in: 1_000_000_000_000_000_000,
			amount_out: 1_149_000_000_000_000,
		},
		ResolvedIntent {
			intent_id: 1,
			amount_in: 36895351807444140032,
			amount_out: 626344035537618048,
		},
	];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_four_intents() {
	let intents = vec![
		(
			0,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 13,
					asset_out: 5,
					amount_in: 514888002332937478066650,
					amount_out: 664083505362373041510455118870258,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		),
		(
			1,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 20,
					asset_out: 14,
					amount_in: 165665617143487433531,
					amount_out: 12177733280754553178994,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: true,
				on_success: None,
				on_failure: None,
			},
		),
		(
			2,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 0,
					asset_out: 16,
					amount_in: 25528234672916292207,
					amount_out: 871403327041354,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		),
		(
			3,
			Intent {
				who: ALICE.into(),
				swap: Swap {
					asset_in: 100,
					asset_out: 101,
					amount_in: 303603756622822659947591,
					amount_out: 20555903343957624238452664953,
					swap_type: SwapType::ExactIn,
				},
				deadline: 0,
				partial: false,
				on_success: None,
				on_failure: None,
			},
		),
	];
	let start = Instant::now();
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let duration = start.elapsed();
	println!("Time elapsed in solve() is: {:?}", duration);
	let expected_solution = vec![
		ResolvedIntent {
			intent_id: 0,
			amount_in: 1_000_000_000_000_000_000,
			amount_out: 1_149_000_000_000_000,
		},
		ResolvedIntent {
			intent_id: 1,
			amount_in: 36895351807444140032,
			amount_out: 626344035537618048,
		},
	];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_for_many_intents() {
	let intents = generate_random_intents(4, DataProvider::assets(None));
	dbg!(&intents);
	let start = Instant::now();
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let duration = start.elapsed();
	println!(
		"Time elapsed in solve() is: {:?} - resolved intents {:?}",
		duration,
		solution.len()
	);

	dbg!(&solution);
}
