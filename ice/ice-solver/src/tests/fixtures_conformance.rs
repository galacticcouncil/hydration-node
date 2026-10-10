//! Conformance tests over recorded market fixtures.
//!
//! Each `*.hex` fixture was captured from a real chain snapshot at the time an
//! ICE bug was reported: SCALE-encoded `Vec<Intent>`, the solution produced at
//! the time, and a `Trace` of every AMM call the solver made (routes, spot
//! prices, sell quotes, existential deposits).
//!
//! The trace is replayed *by key*, not by call order: the solver under test
//! makes its own decisions and probes its own amounts, so a strict
//! call-for-call replay would only ever confirm that the recorded solver still
//! behaves like the recorded solver. Sell quotes at amounts that were never
//! recorded are interpolated from the neighbouring recorded samples for that
//! pair, which keeps the reconstructed market monotone and deterministic.
//!
//! What is asserted is therefore what the *chain* asserts — existential
//! deposits, pro-rata limits, one price per directed pair, per-asset
//! conservation, the score recompute, the solution bounds — plus a pin on the
//! headline shape of the solve so a behaviour change shows up in review.

use crate::replay_format::{Response, Trace};
use crate::v4::Solver;
use codec::{Decode, Encode};
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, Route};
use ice_support::{
	AssetId, Balance, Intent, IntentId, Solution, MAX_NUMBER_OF_RESOLVED_INTENTS, MAX_NUMBER_OF_SOLUTION_TRADES,
};
use sp_core::U256;
use std::cell::RefCell;
use std::collections::BTreeMap;

// ---------- keyed market replay ----------

type Pair = (AssetId, AssetId);

#[derive(Default)]
struct Market {
	price_denominator: AssetId,
	routes: BTreeMap<Pair, Result<Vec<Route<AssetId>>, ()>>,
	spot: BTreeMap<Pair, Result<Ratio, ()>>,
	eds: BTreeMap<AssetId, Balance>,
	/// Recorded sell samples per pair: `amount_in -> amount_out` (failures are
	/// recorded as an absent output so they can bound an interpolation).
	sells: BTreeMap<Pair, BTreeMap<Balance, Option<Balance>>>,
}

impl Market {
	fn from_trace(trace: Trace) -> Self {
		let mut m = Market {
			price_denominator: trace.price_denominator,
			..Market::default()
		};
		for response in trace.responses {
			match response {
				Response::DiscoverRoutes {
					asset_in,
					asset_out,
					result,
				} => {
					m.routes.insert((asset_in, asset_out), result);
				}
				Response::SpotPrice {
					asset_in,
					asset_out,
					result,
				} => {
					m.spot.insert((asset_in, asset_out), result);
				}
				Response::ExistentialDeposit { asset_id, ed } => {
					m.eds.insert(asset_id, ed);
				}
				Response::Sell {
					asset_in,
					asset_out,
					amount_in,
					result,
				} => {
					m.sells
						.entry((asset_in, asset_out))
						.or_default()
						.insert(amount_in, result.ok().map(|(out, _)| out));
				}
				Response::Buy { .. } => {}
			}
		}
		m
	}

	fn route_for(&self, pair: Pair) -> Option<Route<AssetId>> {
		self.routes.get(&pair)?.as_ref().ok()?.first().cloned()
	}

	/// Sell output for an arbitrary amount, reconstructed from the recorded
	/// samples: exact hit, linear interpolation between the two neighbours, or
	/// linear scaling from the single nearest sample.
	fn sell_out(&self, pair: Pair, amount_in: Balance) -> Option<Balance> {
		let samples = self.sells.get(&pair)?;
		if let Some(exact) = samples.get(&amount_in) {
			return *exact;
		}
		let below = samples.range(..amount_in).next_back().map(|(k, v)| (*k, *v));
		let above = samples.range(amount_in..).next().map(|(k, v)| (*k, *v));
		match (below, above) {
			(Some((lo_in, Some(lo_out))), Some((hi_in, Some(hi_out)))) => {
				let span = hi_in.checked_sub(lo_in).filter(|s| *s > 0)?;
				let step = U256::from(hi_out.saturating_sub(lo_out)) * U256::from(amount_in - lo_in) / U256::from(span);
				Balance::try_from(U256::from(lo_out) + step).ok()
			}
			(Some((s_in, Some(s_out))), _) | (_, Some((s_in, Some(s_out)))) => {
				let scaled = U256::from(s_out) * U256::from(amount_in) / U256::from(s_in.max(1));
				Balance::try_from(scaled).ok()
			}
			_ => None,
		}
	}
}

thread_local! {
	static MARKET: RefCell<Market> = RefCell::new(Market::default());
}

struct MarketAmm;

impl AMMInterface for MarketAmm {
	type Error = ();
	type State = ();

	fn discover_routes(
		asset_in: AssetId,
		asset_out: AssetId,
		_state: &Self::State,
	) -> Result<Vec<Route<AssetId>>, Self::Error> {
		MARKET.with(|m| {
			m.borrow()
				.routes
				.get(&(asset_in, asset_out))
				.cloned()
				.unwrap_or(Err(()))
		})
	}

	fn sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		MARKET.with(|m| {
			let m = m.borrow();
			let amount_out = m.sell_out((asset_in, asset_out), amount_in).ok_or(())?;
			let route = m.route_for((asset_in, asset_out)).ok_or(())?;
			Ok((
				(),
				TradeExecution {
					amount_in,
					amount_out,
					route,
				},
			))
		})
	}

	fn buy(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		MARKET.with(|m| {
			let route = m.borrow().route_for((asset_in, asset_out)).ok_or(())?;
			Ok((
				(),
				TradeExecution {
					amount_in: amount_out,
					amount_out,
					route,
				},
			))
		})
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<Ratio, Self::Error> {
		MARKET.with(|m| m.borrow().spot.get(&(asset_in, asset_out)).copied().unwrap_or(Err(())))
	}

	fn price_denominator() -> AssetId {
		MARKET.with(|m| m.borrow().price_denominator)
	}

	fn pool_edges(_state: &Self::State) -> Vec<PoolEdge<AssetId>> {
		Vec::new()
	}

	fn existential_deposit(asset_id: AssetId) -> Balance {
		MARKET.with(|m| m.borrow().eds.get(&asset_id).copied().unwrap_or(0))
	}
}

// ---------- chain-side invariants ----------

fn ed(asset: AssetId) -> Balance {
	MarketAmm::existential_deposit(asset)
}

/// Everything `submit_solution` and `validate_unsigned_solution` re-check.
fn assert_chain_invariants(intents: &[Intent], solution: &Solution) {
	assert!(
		solution.resolved_intents.len() <= MAX_NUMBER_OF_RESOLVED_INTENTS as usize,
		"resolved-intent bound exceeded",
	);
	assert!(
		solution.trades.len() <= MAX_NUMBER_OF_SOLUTION_TRADES as usize,
		"solution-trade bound exceeded",
	);

	let by_id: BTreeMap<IntentId, &Intent> = intents.iter().map(|i| (i.id, i)).collect();
	let mut seen: BTreeMap<IntentId, ()> = BTreeMap::new();
	let mut prices: BTreeMap<Pair, Ratio> = BTreeMap::new();
	let mut credit: BTreeMap<AssetId, i128> = BTreeMap::new();
	let mut score: Balance = 0;

	for r in solution.resolved_intents.iter() {
		assert!(seen.insert(r.id, ()).is_none(), "intent {} resolved twice", r.id);
		let original = by_id
			.get(&r.id)
			.unwrap_or_else(|| panic!("intent {} is not in the batch", r.id));

		assert!(
			r.data.amount_in() >= ed(r.data.asset_in()),
			"intent {} input {} is below the ED of asset {}",
			r.id,
			r.data.amount_in(),
			r.data.asset_in(),
		);
		assert!(
			r.data.amount_out() >= ed(r.data.asset_out()),
			"intent {} output {} is below the ED of asset {}",
			r.id,
			r.data.amount_out(),
			r.data.asset_out(),
		);
		let ice_support::IntentData::Swap(original_swap) = &original.data else {
			panic!("intent {} is not a swap", r.id);
		};
		assert!(
			r.data.amount_in() <= original_swap.remaining(),
			"intent {} fills {} of {} remaining",
			r.id,
			r.data.amount_in(),
			original_swap.remaining(),
		);

		// Pro-rata limit: amount_out ≥ amount_in · limit_out / limit_in.
		let min_out = U256::from(r.data.amount_in()) * U256::from(original.data.amount_out())
			/ U256::from(original.data.amount_in().max(1));
		assert!(
			U256::from(r.data.amount_out()) >= min_out,
			"intent {} is paid {} below its pro-rata minimum {}",
			r.id,
			r.data.amount_out(),
			min_out,
		);

		// One execution price per directed pair, ±1 unit of rounding.
		let key = (r.data.asset_in(), r.data.asset_out());
		let price = prices.entry(key).or_insert(Ratio {
			n: r.data.amount_out(),
			d: r.data.amount_in(),
		});
		let expected = U256::from(r.data.amount_in()) * U256::from(price.n) / U256::from(price.d.max(1));
		let expected = Balance::try_from(expected).expect("expected output fits a balance");
		assert!(
			expected.abs_diff(r.data.amount_out()) <= 1,
			"intent {} breaks the uniform price for {key:?}: {} vs {expected}",
			r.id,
			r.data.amount_out(),
		);

		score = score.saturating_add(original.data.surplus(&r.data).expect("surplus is computable"));

		*credit.entry(r.data.asset_in()).or_default() += r.data.amount_in() as i128;
		*credit.entry(r.data.asset_out()).or_default() -= r.data.amount_out() as i128;
	}

	assert_eq!(solution.score, score, "score diverges from the pallet recompute");

	for t in solution.trades.iter() {
		let asset_in = t.route.first().expect("trade route is never empty").asset_in;
		let asset_out = t.route.last().expect("trade route is never empty").asset_out;
		assert!(
			t.amount_in >= ed(asset_in).max(1),
			"trade {asset_in} -> {asset_out} input {} is below ED {}",
			t.amount_in,
			ed(asset_in),
		);
		assert!(
			t.amount_out >= ed(asset_out).max(1),
			"trade {asset_in} -> {asset_out} output {} is below ED {}",
			t.amount_out,
			ed(asset_out),
		);
		*credit.entry(asset_in).or_default() -= t.amount_in as i128;
		*credit.entry(asset_out).or_default() += t.amount_out as i128;
	}

	for (asset, balance) in credit {
		assert!(balance >= 0, "asset {asset} is over-paid by {} units", -balance);
	}
}

/// Solve a fixture twice against the replayed market and return the solution.
fn solve_fixture(raw: &str) -> (Vec<Intent>, Solution) {
	let (intents_bytes, _recorded_solution, trace) = Trace::decode_fixture(raw);
	let intents = Vec::<Intent>::decode(&mut &intents_bytes[..]).expect("decode intents");
	MARKET.with(|m| *m.borrow_mut() = Market::from_trace(trace));

	let solution = Solver::<MarketAmm>::solve(intents.clone(), (), Permill::zero()).expect("solver should succeed");
	let again = Solver::<MarketAmm>::solve(intents.clone(), (), Permill::zero()).expect("solver should succeed");
	assert_eq!(
		solution.encode(),
		again.encode(),
		"solver is not deterministic on this fixture",
	);

	assert_chain_invariants(&intents, &solution);
	(intents, solution)
}

/// Pins the resolved intents' exact `(id, amount_in, amount_out)` triples, in
/// solver output order — a behaviour change to *which* intents are filled, or
/// by how much, must show up here rather than only in the aggregate score.
fn assert_resolved(solution: &Solution, expected: &[(IntentId, Balance, Balance)]) {
	let actual: Vec<(IntentId, Balance, Balance)> = solution
		.resolved_intents
		.iter()
		.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
		.collect();
	assert_eq!(
		actual.as_slice(),
		expected,
		"resolved intents diverged from the pinned fixture shape"
	);
}

/// Pins the executed trades' exact `(amount_in, amount_out)` pairs, in solver
/// output order.
fn assert_trades(solution: &Solution, expected: &[(Balance, Balance)]) {
	let actual: Vec<(Balance, Balance)> = solution.trades.iter().map(|t| (t.amount_in, t.amount_out)).collect();
	assert_eq!(
		actual.as_slice(),
		expected,
		"trades diverged from the pinned fixture shape"
	);
}

// ---------- fixtures ----------
//
// Each fixture pins the shape of the solve (intents in, intents resolved,
// trades emitted, score) on top of the chain invariants `solve_fixture`
// already checks for every one of them.

/// Snapshot where one partial intent had an unreachable min rate and was
/// poisoning its whole pair, dropping every other partial fill on it to zero.
/// The pair must still clear for the intents whose limits it can meet.
#[test]
fn solve_should_clear_the_pair_when_one_intent_has_an_unreachable_rate() {
	let (intents, solution) = solve_fixture(include_str!("fixtures/unreachable_rate.hex"));
	assert_eq!(intents.len(), 11);
	assert_resolved(
		&solution,
		&[
			(
				32777128788196670979641966592018,
				10_000_000_000_000_000_000_000,
				3_052_735_438_444_530_704,
			),
			(
				32777135429024537515080548352020,
				10_000_000_000_000_000_000_000,
				3_052_735_438_444_530_704,
			),
		],
	);
	assert_trades(
		&solution,
		&[(19_999_999_999_999_999_994_503, 6_105_470_876_889_061_408)],
	);
	assert_eq!(solution.score, 55_470_876_889_061_408);
}

/// Snapshot where the solver produced a resolved amount below the asset's
/// existential deposit and `submit_solution` failed with `InvalidAmount`.
#[test]
fn solve_should_respect_existential_deposits_on_the_recorded_dust_batch() {
	let (intents, solution) = solve_fixture(include_str!("fixtures/existential_deposit.hex"));
	assert_eq!(intents.len(), 47);
	assert_resolved(
		&solution,
		&[
			(32777287393302216734366760960077, 680_270_995_611_151_924, 2_182_318_743),
			(
				32777309529395105185828700160107,
				100_000_000_000_000,
				321_745_489_053_649_374,
			),
			(
				32777310636199749608401797120108,
				100_000_000_000_000,
				321_745_489_053_649_374,
			),
			(
				32777311743004394030974894080109,
				100_000_000_000_000,
				321_745_489_053_649_374,
			),
			(
				32777312849809038453547991040111,
				100_000_000_000_000,
				321_745_489_053_649_374,
			),
			(
				32777313071169967338062610432112,
				100_000_000_000_000,
				321_745_489_053_649_374,
			),
			(
				32777317277027616143840378880121,
				93_248_996_000_000,
				300_024_438_217_817_942,
			),
			(32777323917855482679278960640123, 12_193_700, 12_218_638_127_483_426_528),
			(
				32777327238269415946998251520124,
				10_000_000_000_000_000_000_000,
				3_029_993_676_200_775_556,
			),
			(
				32777274111646483663489597440030,
				1_851_851_851_851_851_851,
				561_109_940_037_180,
			),
			(
				32777274111646483663489597440034,
				1_800_000_000_000_000_000,
				545_398_861_716_139,
			),
		],
	);
	assert_trades(
		&solution,
		&[
			(7_813_001_858_249_136_131_457, 2_350_235_940_395_376_953),
			(2_176_522_603_591_746_229_047, 2_170_125_043),
		],
	);
	assert_eq!(solution.score, 1_350_607_772_644_467_006);
}

/// Snapshot where owners of several same-direction intents had their sell asset
/// locked in named reserves from earlier rounds.
#[test]
fn solve_should_produce_a_conserving_plan_on_the_locked_funds_batch() {
	let (intents, solution) = solve_fixture(include_str!("fixtures/funds_unavailable.hex"));
	assert_eq!(intents.len(), 37);
	assert_resolved(
		&solution,
		&[
			(
				32754977200043197601679409152061,
				10_000_000_000,
				5_180_895_782_507_360_335,
			),
			(32754976757321339832650170368060, 1_000_000_000, 518_089_578_250_736_033),
			(32754981959303168618743726080076, 1_000_000_000, 518_089_578_250_736_033),
			(32754982180664097503258345472078, 1_000_000_000, 518_089_578_250_736_033),
			(32754982402025026387772964864079, 1_000_000_000, 518_089_578_250_736_033),
		],
	);
	assert_trades(&solution, &[(13_999_999_999, 7_253_254_095_510_304_470)]);
	assert_eq!(solution.score, 675_464_095_510_304_467);
}

/// Snapshot where a single large partial hit the pool's per-block trading limit
/// and the solver had to cap its fill.
#[test]
fn solve_should_cap_the_fill_when_the_pool_trading_limit_is_hit() {
	let (intents, solution) = solve_fixture(include_str!("fixtures/trading_limit.hex"));
	assert_eq!(intents.len(), 38);
	assert_resolved(
		&solution,
		&[
			(
				32754979635013415331340222464071,
				22_222_000_000,
				10_566_720_622_398_913_362,
			),
			(
				32754977200043197601679409152061,
				10_000_000_000,
				4_755_071_830_797_818_991,
			),
			(32754981959303168618743726080076, 1_000_000_000, 475_507_183_079_781_899),
			(32754982180664097503258345472078, 1_000_000_000, 475_507_183_079_781_899),
			(32754982402025026387772964864079, 1_000_000_000, 475_507_183_079_781_899),
		],
	);
	assert_trades(&solution, &[(35_221_999_999, 16_748_314_002_436_078_051)]);
	assert_eq!(solution.score, 589_044_002_436_078_050);
}

/// Snapshot where the intent whose id ends in `6127` was being excluded from
/// every solution.
#[test]
fn solve_should_resolve_the_recorded_batch_that_previously_excluded_intent_6127() {
	let (intents, solution) = solve_fixture(include_str!("fixtures/intent_6127.hex"));
	assert_eq!(intents.len(), 46);
	assert_resolved(
		&solution,
		&[
			(
				32755004427437450396977594368117,
				322_143_967_535_788_898,
				569_853_362_969_577_662_029,
			),
			(
				32755009518738814740813840384136,
				5_000_000_000_000_000_000_000,
				2_708_531_845_549_579_079,
			),
		],
	);
	assert_trades(&solution, &[(4_430_146_637_030_422_337_971, 2_386_387_878_013_790_181)]);
	assert_eq!(solution.score, 19_078_094_815_127_241_108);
}
