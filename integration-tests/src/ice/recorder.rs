//! ICE solver fixture recorder.
//!
//! Active only under the `ice-record` feature flag. When on, `TestSimulator`
//! in `super::solver` is transparently wrapped so every AMM call the solver
//! makes is appended to a thread-local trace. Dumping tests then call
//! `dump_fixture` to freeze intents + solution + trace as a 3-line `.hex`
//! fixture consumable by `ice_solver::tests::regressions`.
//!
//! Without the feature, `RecordingAMM` doesn't exist and `clear`/`dump_fixture`
//! compile to no-ops — call sites in tests can stay permanent and untouched.
//!
//! `clear` and `dump_fixture` are `dead_code`-allowed: they exist as a dormant
//! hook. To capture a new regression fixture, temporarily add
//! `recorder::clear(); Solver::solve(...); recorder::dump_fixture("…hex", …)`
//! into an integration test that loads the chain state you want, then run
//! `cargo test -p runtime-integration-tests --features ice-record <test>`.

#![allow(dead_code)]

#[cfg(feature = "ice-record")]
mod active {
	use codec::Encode;
	use hydra_dx_math::types::Ratio;
	use hydradx_traits::amm::{AMMInterface, TradeExecution};
	use hydradx_traits::router::{PoolEdge, Route};
	use ice_solver::replay_format::{Response, Trace};
	use ice_support::{AssetId, Balance};
	use std::cell::RefCell;
	use std::marker::PhantomData;

	thread_local! {
		static TRACE: RefCell<Vec<Response>> = const { RefCell::new(Vec::new()) };
	}

	pub fn clear() {
		TRACE.with(|t| t.borrow_mut().clear());
	}

	fn take_responses() -> Vec<Response> {
		TRACE.with(|t| t.replace(Vec::new()))
	}

	fn record(r: Response) {
		TRACE.with(|t| t.borrow_mut().push(r));
	}

	pub struct RecordingAMM<A: AMMInterface>(PhantomData<A>);

	impl<A: AMMInterface> AMMInterface for RecordingAMM<A> {
		type Error = A::Error;
		type State = A::State;

		fn discover_routes(
			asset_in: AssetId,
			asset_out: AssetId,
			state: &Self::State,
		) -> Result<Vec<Route<AssetId>>, Self::Error> {
			let r = A::discover_routes(asset_in, asset_out, state);
			record(Response::DiscoverRoutes {
				asset_in,
				asset_out,
				result: match &r {
					Ok(v) => Ok(v.clone()),
					Err(_) => Err(()),
				},
			});
			r
		}

		fn sell(
			asset_in: AssetId,
			asset_out: AssetId,
			amount_in: Balance,
			route: Route<AssetId>,
			state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			let r = A::sell(asset_in, asset_out, amount_in, route, state);
			record(Response::Sell {
				asset_in,
				asset_out,
				amount_in,
				result: match &r {
					Ok((_, exec)) => Ok((exec.amount_out, exec.route.clone())),
					Err(_) => Err(()),
				},
			});
			r
		}

		fn buy(
			asset_in: AssetId,
			asset_out: AssetId,
			amount_out: Balance,
			route: Route<AssetId>,
			state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			let r = A::buy(asset_in, asset_out, amount_out, route, state);
			record(Response::Buy {
				asset_in,
				asset_out,
				amount_out,
				result: match &r {
					Ok((_, exec)) => Ok((exec.amount_in, exec.route.clone())),
					Err(_) => Err(()),
				},
			});
			r
		}

		fn get_spot_price(
			asset_in: AssetId,
			asset_out: AssetId,
			route: Route<AssetId>,
			state: &Self::State,
		) -> Result<Ratio, Self::Error> {
			let r = A::get_spot_price(asset_in, asset_out, route, state);
			record(Response::SpotPrice {
				asset_in,
				asset_out,
				result: match &r {
					Ok(p) => Ok(*p),
					Err(_) => Err(()),
				},
			});
			r
		}

		fn price_denominator() -> AssetId {
			A::price_denominator()
		}

		fn pool_edges(state: &Self::State) -> Vec<PoolEdge<AssetId>> {
			A::pool_edges(state)
		}

		fn existential_deposit(asset_id: AssetId) -> Balance {
			let ed = A::existential_deposit(asset_id);
			record(Response::ExistentialDeposit { asset_id, ed });
			ed
		}
	}

	pub fn dump_fixture<I: Encode, S: Encode>(path: &str, intents: &I, solution: &S, price_denominator: AssetId) {
		use std::io::Write;

		let trace = Trace {
			price_denominator,
			responses: take_responses(),
		};
		let fixture = Trace::encode_fixture(intents, solution, &trace);

		let mut f = std::fs::File::create(path).expect("create fixture file");
		f.write_all(fixture.as_bytes()).expect("write fixture file");

		println!(
			"DUMPED fixture to {} ({} responses, {} bytes)",
			path,
			trace.responses.len(),
			fixture.len(),
		);
	}
}

#[cfg(feature = "ice-record")]
#[allow(unused_imports)]
pub use active::{dump_fixture, RecordingAMM};

/// No-op without `ice-record`; clears the recorder's thread-local trace with it.
pub fn clear() {
	#[cfg(feature = "ice-record")]
	active::clear();
}

/// No-op without `ice-record`; writes a 3-line hex fixture to `path` with it.
#[cfg(not(feature = "ice-record"))]
pub fn dump_fixture<I, S>(_path: &str, _intents: &I, _solution: &S, _price_denominator: ice_support::AssetId) {}
