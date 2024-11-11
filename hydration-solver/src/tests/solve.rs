use crate::omni::OmniSolver;
use crate::tests::{generate_random_intents, AssetId, DataProvider};
use pallet_ice::traits::{OmnipoolInfo, Solver};
use pallet_ice::types::{Intent, ResolvedIntent, Swap, SwapType};

#[test]
fn solver_should_find_solution_for_one_intent() {
	let intents = vec![(
		0,
		Intent {
			who: 1,
			swap: Swap {
				asset_in: 0u32,
				asset_out: 27u32,
				amount_in: 100_000_000_000_000,
				amount_out: 1_149_000_000_000,
				swap_type: SwapType::ExactIn,
			},
			deadline: 0,
			partial: false,
			on_success: None,
			on_failure: None,
		},
	)];
	let (solution, _) = OmniSolver::<u64, AssetId, DataProvider>::solve(intents).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 98465458599392,
		amount_out: 1131368119307,
	}];
	assert_eq!(solution, expected_solution);
}

#[test]
fn solver_should_find_solution_with_twenty_intents() {
	let intents = generate_random_intents(10000, DataProvider::assets(None));
	let (solution, _) = OmniSolver::<u64, AssetId, DataProvider>::solve(intents).unwrap();
	dbg!(solution.len());
}
