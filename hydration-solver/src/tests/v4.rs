use crate::tests::load_amm_state;
use crate::types::*;
use crate::v4::SolverV4;
use std::time::Instant;

#[test]
fn solver_should_find_solution_for_one_small_amount_partial_intent() {
	let data = load_amm_state();
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 100_000_000_000_000,
		amount_out: 1_149_000_000_000,
		partial: true,
	}];
	let solution = SolverV4::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 100_000_000_000_000,
		amount_out: 1_149_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_one_large_amount_partial_intent() {
	let data = load_amm_state();
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
		partial: true,
	}];
	let solution = SolverV4::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}
#[test]
fn solver_should_find_solution_for_one_large_amount_full_intent() {
	let data = load_amm_state();
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
		partial: false,
	}];
	let solution = SolverV4::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_two_intents() {
	let data = load_amm_state();
	let intents = vec![
		Intent {
			intent_id: 0,
			asset_in: 0u32,
			asset_out: 27u32,
			amount_in: 1_000_000_000_000_000_000,
			amount_out: 1_149_000_000_000_000,
			partial: false,
		},
		Intent {
			intent_id: 1,
			asset_in: 20,
			asset_out: 8,
			amount_in: 165_453_758_222_187_283_838,
			amount_out: 2808781311006261193,
			partial: true,
		},
	];
	let start = Instant::now();
	let solution = SolverV4::solve(intents, data).unwrap();
	let duration = start.elapsed();
	println!("Time elapsed in solve() is: {:?}", duration);
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_two_partial_intents() {
	let data = load_amm_state();
	let intents = vec![
		Intent {
			intent_id: 0,
			asset_in: 12,
			asset_out: 14,
			amount_in: 9206039265427194,
			amount_out: 1,
			partial: true,
		},
		Intent {
			intent_id: 1,
			asset_in: 28,
			asset_out: 8,
			amount_in: 1076105965030805693,
			amount_out: 1,
			partial: true,
		},
	];
	let start = Instant::now();
	let solution = SolverV4::solve(intents, data).unwrap();
	let duration = start.elapsed();
	println!("Time elapsed in solve() is: {:?}", duration);
	let expected_solution = vec![
		ResolvedIntent {
			intent_id: 0,
			amount_in: 9206039265427194,
			amount_out: 1,
		},
		ResolvedIntent {
			intent_id: 1,
			amount_in: 1076105965030805693,
			amount_out: 1,
		},
	];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_exclude_an_intent_when_it_contains_non_existing_trade_assets() {
	let data = load_amm_state();
	let intents = vec![
		Intent {
			intent_id: 0,
			asset_in: 13,
			asset_out: 5,
			amount_in: 514888002332937478066650,
			amount_out: 664083505362373041510455118870258,
			partial: false,
		},
		Intent {
			intent_id: 1,
			asset_in: 20,
			asset_out: 14,
			amount_in: 165665617143487433531,
			amount_out: 12177733280754553178994,
			partial: true,
		},
		Intent {
			intent_id: 2,
			asset_in: 0,
			asset_out: 16,
			amount_in: 25528234672916292207,
			amount_out: 871403327041354,
			partial: false,
		},
		Intent {
			intent_id: 3,
			asset_in: 100,
			asset_out: 101,
			amount_in: 303603756622822659947591,
			amount_out: 20555903343957624238452664953,
			partial: false,
		},
	];
	let start = Instant::now();
	let solution = SolverV4::solve(intents, data).unwrap();
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
	assert_eq!(solution.resolved_intents, expected_solution);
}
