//! Wire format shared between the ICE solver fixture recorder (in
//! `runtime-integration-tests`) and the regression-test replay harness
//! (in `src/tests/regressions.rs`).
//!
//! Defining it in one place prevents silent drift: if a variant or field
//! is added here, both writer and reader pick it up and fail to compile
//! until they're updated.

use codec::{Decode, Encode};
use core::fmt::Write;
use hydra_dx_math::types::Ratio;
use hydradx_traits::router::Route;
use ice_support::{AssetId, Balance};

/// One recorded AMM method call: inputs the solver passed in, plus the
/// observable result (errors collapse to `()` — only success shape is replayed).
#[derive(Debug, Clone, Encode, Decode)]
pub enum Response {
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

/// A full trace: the AMM's price denominator (static across the run) plus
/// the ordered list of calls the solver made.
#[derive(Debug, Clone, Encode, Decode)]
pub struct Trace {
	pub price_denominator: AssetId,
	pub responses: Vec<Response>,
}

impl Trace {
	/// Produce the 3-line hex fixture: SCALE(intents), SCALE(solution), SCALE(trace).
	pub fn encode_fixture<I: Encode, S: Encode>(intents: &I, solution: &S, trace: &Self) -> String {
		let mut out = String::new();
		out.push_str(&encode_hex(&intents.encode()));
		out.push('\n');
		out.push_str(&encode_hex(&solution.encode()));
		out.push('\n');
		out.push_str(&encode_hex(&trace.encode()));
		out.push('\n');
		out
	}

	/// Inverse of `encode_fixture`. Returns raw bytes for intents/solution so
	/// callers can decode them with their own concrete types, plus the
	/// already-decoded `Trace`.
	pub fn decode_fixture(raw: &str) -> (Vec<u8>, Vec<u8>, Self) {
		let mut lines = raw.lines().filter(|l| !l.is_empty());
		let intents_hex = lines.next().expect("intents line");
		let solution_hex = lines.next().expect("solution line");
		let trace_hex = lines.next().expect("trace line");
		let intents = decode_hex(intents_hex);
		let solution = decode_hex(solution_hex);
		let trace = Trace::decode(&mut &decode_hex(trace_hex)[..]).expect("decode trace");
		(intents, solution, trace)
	}
}

fn encode_hex(bytes: &[u8]) -> String {
	let mut s = String::with_capacity(bytes.len() * 2);
	for b in bytes {
		write!(s, "{b:02x}").unwrap();
	}
	s
}

fn decode_hex(s: &str) -> Vec<u8> {
	assert!(s.len() % 2 == 0, "hex length must be even");
	(0..s.len())
		.step_by(2)
		.map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("valid hex"))
		.collect()
}
