use crate::tests::{generate_random_intents, AssetId, DataProvider};
use crate::v3::SolverV3;
use pallet_ice::traits::{OmnipoolInfo, Solver};
use pallet_ice::types::{Intent, ResolvedIntent, Swap, SwapType};
use primitives::AccountId;
use sp_core::crypto::AccountId32;

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
							   partial: false,
							   on_success: None,
							   on_failure: None,
						   },
					   )

	];
	let (solution, _) = SolverV3::<DataProvider>::solve(intents).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution, expected_solution);
}