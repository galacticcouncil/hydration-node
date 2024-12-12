use crate::tests::{generate_random_intents, AssetId, DataProvider};
use crate::types::*;
use crate::v3::SolverV3;
use std::time::Instant;
use tracing::Instrument;

pub const ALICE: [u8; 32] = [4u8; 32];

#[test]
fn solver_should_find_solution_for_one_small_amount_partial_intent() {
	let data = DataProvider::assets(None);
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 100_000_000_000_000,
		amount_out: 1_149_000_000_000,
		partial: true,
	}];
	let solution = SolverV3::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 100_000_000_000_000,
		amount_out: 1_149_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_one_large_amount_partial_intent() {
	let data = DataProvider::assets(None);
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
		partial: true,
	}];
	let solution = SolverV3::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_one_large_amount_full_intent() {
	let data = DataProvider::assets(None);
	let intents = vec![Intent {
		intent_id: 0,
		asset_in: 0u32,
		asset_out: 27u32,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
		partial: false,
	}];
	let solution = SolverV3::solve(intents, data).unwrap();
	let expected_solution = vec![ResolvedIntent {
		intent_id: 0,
		amount_in: 1_000_000_000_000_000_000,
		amount_out: 1_149_000_000_000_000,
	}];
	assert_eq!(solution.resolved_intents, expected_solution);
}

#[test]
fn solver_should_find_solution_for_two_intents() {
	let data = DataProvider::assets(None);
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
			amount_in: 165453758222187283838,
			amount_out: 2808781311006261193,
			partial: true,
		},
	];
	let start = Instant::now();
	let solution = SolverV3::solve(intents, data).unwrap();
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

#[test]
fn solver_should_find_solution_for_two_partial_intents() {
	let data = DataProvider::assets(None);
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
	let solution = SolverV3::solve(intents, data).unwrap();
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

#[test]
fn solver_should_find_solution_for_four_intents() {
	let data = DataProvider::assets(None);
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
	let solution = SolverV3::solve(intents, data).unwrap();
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

#[test]
fn solver_should_find_solution_for_many_intents() {
	let data = DataProvider::assets(None);
	let intents = generate_random_intents(100, data.clone());
	println!("Generated intents {:?}", intents.len());
	let result = std::panic::catch_unwind(|| {
		let start = Instant::now();
		let solution = SolverV3::solve(intents.clone(), data).unwrap();
		let duration = start.elapsed();
		println!(
			"Time elapsed in solve() is: {:?} - resolved intents {:?}",
			duration,
			solution.resolved_intents.len()
		);
	});

	let filename = if result.is_err() {
		format!("testdata/failed_{}.json", chrono::Utc::now().timestamp())
	//write to file
	} else {
		format!("testdata/success_{}.json", chrono::Utc::now().timestamp())
	};
	let intents = intents
		.into_iter()
		.map(|intent| intent.into())
		.collect::<Vec<TestEntry>>();

	let serialized = serde_json::to_string(&intents).unwrap();
	std::fs::write(filename, serialized).unwrap();
	println!("Solver failed to find solution for many intents");
	//dbg!(&solution);
}

#[test]
fn test_scenario() {
	let data = DataProvider::assets(None);
	let testdata = std::fs::read_to_string("testdata/success_1732737492.json").unwrap();
	let intents: Vec<TestEntry> = serde_json::from_str(&testdata).unwrap();
	let intents: Vec<Intent> = intents
		.into_iter()
		.enumerate()
		.map(|(i, entry)| (i as u128, entry).into())
		.collect();
	//dbg!(&intents);
	dbg!(intents.len());
	let start = Instant::now();
	let solution = SolverV3::solve(intents.clone(), data).unwrap();
	let duration = start.elapsed();
	println!(
		"Time elapsed in solve() is: {:?} - resolved intents {:?}",
		duration,
		solution.resolved_intents.len()
	);
	//dbg!(solution);
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq, Eq)]
struct TestEntry {
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	partial: bool,
}

impl From<Intent> for TestEntry {
	fn from(value: Intent) -> Self {
		Self {
			asset_in: value.asset_in,
			asset_out: value.asset_out,
			amount_in: value.amount_in,
			amount_out: value.amount_out,
			partial: value.partial,
		}
	}
}

impl Into<Intent> for (u128, TestEntry) {
	fn into(self) -> Intent {
		Intent {
			intent_id: self.0,
			asset_in: self.1.asset_in,
			asset_out: self.1.asset_out,
			amount_in: self.1.amount_in,
			amount_out: self.1.amount_out,
			partial: self.1.partial,
		}
	}
}
