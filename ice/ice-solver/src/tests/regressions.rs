//! Regression tests for the v2 solver.
//!
//! Each test pins the exact solution the solver produced against a real
//! testnet snapshot at the time a bug was discovered. The snapshot is *not*
//! re-loaded; instead every AMM call the solver made during the original
//! integration-test run was recorded and is replayed here in order via a
//! `ReplayAMM` mock.
//!
//! Each `*.hex` fixture has three lines: SCALE-encoded `Vec<Intent>`,
//! `Solution`, and `Trace { price_denominator, responses }`.

use crate::v2::Solver;
use codec::{Decode, Encode};
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, Route};
use ice_support::{AssetId, Balance, Intent, Solution};
use std::cell::RefCell;
use std::collections::VecDeque;

// ---------- replay trace format ----------

#[derive(Debug, Clone, Encode, Decode)]
enum Response {
	DiscoverRoutes {
		asset_in: AssetId,
		asset_out: AssetId,
		result: Result<Vec<Route<AssetId>>, ()>,
	},
	Sell {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		result: Result<(Balance, Route<AssetId>), ()>,
	},
	Buy {
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		result: Result<(Balance, Route<AssetId>), ()>,
	},
	SpotPrice {
		asset_in: AssetId,
		asset_out: AssetId,
		result: Result<Ratio, ()>,
	},
	ExistentialDeposit {
		asset_id: AssetId,
		ed: Balance,
	},
}

#[derive(Debug, Clone, Encode, Decode)]
struct Trace {
	price_denominator: AssetId,
	responses: Vec<Response>,
}

// ---------- replay AMM ----------

thread_local! {
	static RESPONSES: RefCell<VecDeque<Response>> = const { RefCell::new(VecDeque::new()) };
	static PRICE_DENOM: RefCell<AssetId> = const { RefCell::new(0) };
}

struct ReplayAMM;

impl ReplayAMM {
	fn install(trace: Trace) {
		RESPONSES.with(|q| *q.borrow_mut() = trace.responses.into_iter().collect());
		PRICE_DENOM.with(|d| *d.borrow_mut() = trace.price_denominator);
	}

	fn next() -> Response {
		RESPONSES.with(|q| {
			q.borrow_mut()
				.pop_front()
				.expect("replay trace exhausted — solver made more calls than were recorded")
		})
	}
}

impl AMMInterface for ReplayAMM {
	type Error = ();
	type State = ();

	fn discover_routes(
		asset_in: AssetId,
		asset_out: AssetId,
		_state: &Self::State,
	) -> Result<Vec<Route<AssetId>>, Self::Error> {
		match Self::next() {
			Response::DiscoverRoutes {
				asset_in: a,
				asset_out: b,
				result,
			} if a == asset_in && b == asset_out => result,
			other => panic!("replay mismatch: expected discover_routes({asset_in}, {asset_out}), got {other:?}"),
		}
	}

	fn sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		match Self::next() {
			Response::Sell {
				asset_in: a,
				asset_out: b,
				amount_in: v,
				result,
			} if a == asset_in && b == asset_out && v == amount_in => result.map(|(amount_out, route)| {
				(
					(),
					TradeExecution {
						amount_in,
						amount_out,
						route,
					},
				)
			}),
			other => panic!("replay mismatch: expected sell({asset_in}, {asset_out}, {amount_in}), got {other:?}"),
		}
	}

	fn buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		match Self::next() {
			Response::Buy {
				asset_in: a,
				asset_out: b,
				amount_out: v,
				result,
			} if a == asset_in && b == asset_out && v == amount_out => result.map(|(amount_in, route)| {
				(
					(),
					TradeExecution {
						amount_in,
						amount_out,
						route,
					},
				)
			}),
			other => panic!("replay mismatch: expected buy({asset_in}, {asset_out}, {amount_out}), got {other:?}"),
		}
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<Ratio, Self::Error> {
		match Self::next() {
			Response::SpotPrice {
				asset_in: a,
				asset_out: b,
				result,
			} if a == asset_in && b == asset_out => result,
			other => panic!("replay mismatch: expected get_spot_price({asset_in}, {asset_out}), got {other:?}"),
		}
	}

	fn price_denominator() -> AssetId {
		PRICE_DENOM.with(|d| *d.borrow())
	}

	fn pool_edges(_state: &Self::State) -> Vec<PoolEdge<AssetId>> {
		Vec::new()
	}

	fn existential_deposit(asset_id: AssetId) -> Balance {
		match Self::next() {
			Response::ExistentialDeposit { asset_id: a, ed } if a == asset_id => ed,
			other => panic!("replay mismatch: expected existential_deposit({asset_id}), got {other:?}"),
		}
	}
}

// ---------- fixtures ----------

fn decode_hex(s: &str) -> Vec<u8> {
	assert!(s.len() % 2 == 0, "hex length must be even");
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
		.collect()
}

fn load_fixture(raw: &str) -> (Vec<Intent>, Solution, Trace) {
	let mut lines = raw.lines().filter(|l| !l.is_empty());
	let intents_hex = lines.next().expect("intents line");
	let solution_hex = lines.next().expect("solution line");
	let trace_hex = lines.next().expect("trace line");
	let intents = Vec::<Intent>::decode(&mut &decode_hex(intents_hex)[..]).expect("decode intents");
	let solution = Solution::decode(&mut &decode_hex(solution_hex)[..]).expect("decode solution");
	let trace = Trace::decode(&mut &decode_hex(trace_hex)[..]).expect("decode trace");
	(intents, solution, trace)
}

fn run_fixture(raw: &str) -> (Solution, Solution) {
	let (intents, expected, trace) = load_fixture(raw);
	ReplayAMM::install(trace);
	let actual = Solver::<ReplayAMM>::solve(intents, ()).expect("solver should succeed");
	// trace should be fully consumed
	let remaining = RESPONSES.with(|q| q.borrow().len());
	assert_eq!(
		remaining, 0,
		"solver consumed fewer AMM calls than were recorded — {remaining} leftover",
	);
	(actual, expected)
}

// ---------- tests ----------

/// Regression: snapshot where one partial intent had an unreachable min rate,
/// which was poisoning the entire pair and dropping all other partial fills
/// on it to zero. After the fix, Alice's two loose-limit 10k-HOLLAR→HDX
/// partial intents resolve and the rest of the pair's intents are dropped
/// individually as the unreachable-rate intent had specified.
///
/// Snapshot: `SNAPSHOT_notworking` (chain at testnet block referenced in the
/// ICE partial-fill bug report).
#[test]
fn unreachable_rate_poisons_pair() {
	let raw = include_str!("fixtures/unreachable_rate.hex");
	let (actual, expected) = run_fixture(raw);
	assert_eq!(actual, expected, "solver produced different solution than expected");
}

/// Regression: snapshot where the solver produced a resolved intent with
/// amount below the asset's existential deposit, which caused
/// `submit_solution` to fail with `InvalidAmount`. After the fix, the solver
/// enforces ED on every resolved intent.
///
/// Snapshot: `SNAPSHOT_invalidagain`.
#[test]
fn resolved_respects_existential_deposit() {
	let raw = include_str!("fixtures/existential_deposit.hex");
	let (actual, expected) = run_fixture(raw);
	assert_eq!(actual, expected, "solver produced different solution than expected");
}
