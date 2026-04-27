use crate::v2::Solver;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, Route, Trade};
use ice_support::{
	AssetId, Balance, Intent, IntentData, IntentId, Partial, ResolvedIntent, SwapData, MAX_NUMBER_OF_RESOLVED_INTENTS,
};
use sp_core::U256;

// ---------- fixtures ----------

fn make_intent(id: IntentId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial: Partial::No,
		}),
	}
}

fn make_partial(id: IntentId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> Intent {
	make_partial_filled(id, asset_in, asset_out, amount_in, min_out, 0)
}

fn make_partial_filled(
	id: IntentId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
	already_filled: Balance,
) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial: Partial::Yes(already_filled),
		}),
	}
}

fn dummy_route(asset_in: u32, asset_out: u32) -> Route<u32> {
	Route::try_from(vec![Trade {
		pool: hydradx_traits::router::PoolType::Omnipool,
		asset_in,
		asset_out,
	}])
	.unwrap()
}

/// Mirrors the on-chain `validate_price_consistency` predicate:
/// two resolved intents in the same direction are rate-compatible iff
/// `|a.out * b.in - b.out * a.in| <= max(a.in, b.in)` — conservative bound
/// that tolerates 1-sat rounding in either expected-out calculation.
fn same_rate_within(a: &ResolvedIntent, b: &ResolvedIntent, tol: u128) -> bool {
	let a_in = a.data.amount_in();
	let a_out = a.data.amount_out();
	let b_in = b.data.amount_in();
	let b_out = b.data.amount_out();
	let lhs = U256::from(a_out) * U256::from(b_in);
	let rhs = U256::from(b_out) * U256::from(a_in);
	let diff = if lhs >= rhs { lhs - rhs } else { rhs - lhs };
	// Normalise tolerance against the larger side: 1 sat of rounding on each side
	// maps to at most max(a.in, b.in) in the cross-product.
	let tol_scaled = U256::from(a_in.max(b_in)) * U256::from(tol);
	diff <= tol_scaled
}

/// Sum of `IntentData::surplus` — the formula the pallet uses to recompute score.
fn pallet_score(originals: &[Intent], resolved: &[ResolvedIntent]) -> Balance {
	let mut total: Balance = 0;
	for r in resolved {
		let original = originals.iter().find(|i| i.id == r.id).unwrap();
		let surplus = original.data.surplus(&r.data).expect("surplus should be computable");
		total = total.saturating_add(surplus);
	}
	total
}

// ---------- mocks ----------

/// 1:1 price, no slippage, zero existential deposit.
struct MockAMMOneToOne;

impl AMMInterface for MockAMMOneToOne {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out: amount_in,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in: amount_out,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(1, 1))
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
}

/// Asset 1 is worth 2× asset 2; 1% slippage on every sell.
struct MockAMMWithSlippage;

impl AMMInterface for MockAMMWithSlippage {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let base_out = if asset_in == 1 && asset_out == 2 {
			amount_in * 2
		} else if asset_in == 2 && asset_out == 1 {
			amount_in / 2
		} else {
			amount_in
		};
		let amount_out = base_out * 99 / 100;
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let amount_in = if asset_in == 1 && asset_out == 2 {
			amount_out / 2 + 1
		} else if asset_in == 2 && asset_out == 1 {
			amount_out * 2 + 1
		} else {
			amount_out + 1
		};
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(asset_in: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		match asset_in {
			1 => Ok(Ratio::new(2, 1)),
			2 => Ok(Ratio::new(1, 1)),
			_ => Ok(Ratio::new(1, 1)),
		}
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
}

/// Sell(1→2) fails for amount_in > 50. Other trades behave as 1:1.
struct MockAMMPartialFailure;

impl AMMInterface for MockAMMPartialFailure {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		if asset_in == 1 && asset_out == 2 && amount_in > 50 {
			return Err(());
		}
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out: amount_in,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in: amount_out,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(1, 1))
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
}

/// 1:1 price, zero slippage, existential deposit of 10 for every asset.
struct MockAMMWithED;

impl AMMInterface for MockAMMWithED {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out: amount_in,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in: amount_out,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(1, 1))
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
	fn existential_deposit(_asset_id: AssetId) -> Balance {
		10
	}
}

// ---------- v1 parity tests ----------

#[test]
fn test_solve_empty() {
	let result = Solver::<MockAMMOneToOne>::solve(vec![], ()).unwrap();
	assert!(result.resolved_intents.is_empty());
}

#[test]
fn test_solve_single_intent() {
	let intents = vec![make_intent(1, 1, 2, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 1);
	assert_eq!(result.trades.len(), 1);
	assert_eq!(result.resolved_intents[0].data.amount_in(), 100);
	assert_eq!(result.resolved_intents[0].data.amount_out(), 100);
	assert_eq!(result.score, 10);
}

#[test]
fn test_uniform_price_two_opposing() {
	let intents = vec![make_intent(1, 1, 2, 100, 90), make_intent(2, 2, 1, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	assert_eq!(result.trades.len(), 0);
	assert_eq!(result.resolved_intents[0].data.amount_out(), 100);
	assert_eq!(result.resolved_intents[1].data.amount_out(), 100);
}

#[test]
fn test_scarce_side_gets_spot() {
	let intents = vec![make_intent(1, 1, 2, 100, 180), make_intent(2, 2, 1, 100, 45)];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert_eq!(bob.data.amount_out(), 50, "scarce side should get spot rate");
	assert!(alice.data.amount_out() < 200);
	assert!(alice.data.amount_out() >= 195);
}

#[test]
fn test_same_direction_uniform_rate() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 1, 2, 200, 180),
		make_intent(3, 1, 2, 50, 45),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 3);
	let rates: Vec<f64> = result
		.resolved_intents
		.iter()
		.map(|r| r.data.amount_out() as f64 / r.data.amount_in() as f64)
		.collect();
	for rate in &rates[1..] {
		let diff = (rate - rates[0]).abs() / rates[0];
		assert!(diff < 0.001, "Same-direction rates must be uniform, got diff {diff}");
	}
}

#[test]
fn test_iterative_filtering() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 95),
		make_intent(2, 2, 1, 100, 95),
		make_intent(3, 1, 2, 100, 200),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	let ids: Vec<_> = result.resolved_intents.iter().map(|r| r.id).collect();
	assert!(ids.contains(&1));
	assert!(ids.contains(&2));
	assert!(!ids.contains(&3));
}

#[test]
fn test_no_opposing_flow() {
	let intents = vec![make_intent(1, 1, 2, 100, 90), make_intent(2, 1, 2, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	assert!(!result.trades.is_empty());
	assert_eq!(result.resolved_intents[0].data.amount_out(), 100);
	assert_eq!(result.resolved_intents[1].data.amount_out(), 100);
}

#[test]
fn test_perfect_match_cancel() {
	let intents = vec![make_intent(1, 1, 2, 100, 90), make_intent(2, 2, 1, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	assert_eq!(result.trades.len(), 0);
}

#[test]
fn test_nonpartial_full_fill() {
	let intents = vec![make_intent(1, 1, 2, 100, 90), make_intent(2, 2, 1, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	for ri in &result.resolved_intents {
		assert_eq!(ri.data.amount_in(), 100);
	}
}

#[test]
fn test_partial_intent_at_clearing() {
	let intents = vec![make_partial(1, 1, 2, 200, 180), make_intent(2, 2, 1, 100, 90)];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	let r1 = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	assert_eq!(r1.data.amount_in(), 200);
	assert!(r1.data.amount_out() >= 180);
}

#[test]
fn test_asymmetric_volumes_with_slippage() {
	let intents = vec![make_partial(1, 1, 2, 200, 360), make_intent(2, 2, 1, 100, 45)];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert_eq!(bob.data.amount_out(), 50);
	assert!(alice.data.amount_out() < 400);
	assert!(alice.data.amount_out() >= 390);
}

#[test]
fn test_three_asset_ring() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 2, 3, 100, 90),
		make_intent(3, 3, 1, 100, 90),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 3);
	assert_eq!(result.trades.len(), 0, "Ring trade should avoid AMM entirely");
	for ri in &result.resolved_intents {
		assert_eq!(ri.data.amount_in(), 100);
		assert_eq!(ri.data.amount_out(), 100);
	}
	assert_eq!(result.score, 30);
}

#[test]
fn test_amm_failure_fallback() {
	let intents = vec![make_intent(1, 1, 2, 200, 180), make_intent(2, 2, 1, 50, 45)];
	let result = Solver::<MockAMMPartialFailure>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	assert_eq!(result.trades.len(), 0);
	let r1 = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let r2 = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert_eq!(r1.data.amount_out(), 200);
	assert_eq!(r2.data.amount_out(), 50);
}

#[test]
fn test_excess_backward_scarce_gets_spot() {
	let intents = vec![make_intent(1, 2, 1, 100, 45), make_intent(2, 1, 2, 50, 90)];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert_eq!(bob.data.amount_out(), 100, "scarce A→B should get spot rate");
	assert!(alice.data.amount_out() > 0);
	assert!(alice.data.amount_out() >= 45);
}

#[test]
fn test_large_amounts_overflow_safe() {
	let unit: Balance = 1_000_000_000_000;
	let intents = vec![
		make_intent(1, 1, 2, 1_000_000 * unit, 900_000 * unit),
		make_intent(2, 2, 1, 1_000_000 * unit, 900_000 * unit),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert_eq!(result.resolved_intents.len(), 2);
	assert_eq!(result.trades.len(), 0);
	for ri in &result.resolved_intents {
		assert_eq!(ri.data.amount_in(), 1_000_000 * unit);
		assert_eq!(ri.data.amount_out(), 1_000_000 * unit);
	}
}

// ---------- new v2-specific correctness tests ----------

/// Two partials in the same direction must resolve at the same rate within 1 sat
/// (the tolerance enforced by the pallet's `validate_price_consistency`).
#[test]
fn test_two_partials_same_direction_get_same_rate() {
	let intents = vec![
		make_partial(1, 1, 2, 100, 80),
		make_partial(2, 1, 2, 200, 150),
		make_intent(3, 2, 1, 100, 90),
	];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	let p1 = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let p2 = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert!(
		same_rate_within(p1, p2, 1),
		"partials {} and {} must share rate within 1 sat: p1 {}→{}, p2 {}→{}",
		p1.id,
		p2.id,
		p1.data.amount_in(),
		p1.data.amount_out(),
		p2.data.amount_in(),
		p2.data.amount_out(),
	);
}

/// The same input in a different order must produce the same fills.
/// Parameters are tight on purpose: tolerance 1.95 against 2.0 spot with 1% slippage
/// forces the binary search to bite, so order-of-fitting matters.
#[test]
fn test_partial_fill_order_independence() {
	let forward = vec![
		make_partial(1, 1, 2, 100, 195),
		make_partial(2, 1, 2, 100, 195),
		make_intent(3, 2, 1, 20, 9),
	];
	let reversed = vec![
		make_intent(3, 2, 1, 20, 9),
		make_partial(2, 1, 2, 100, 195),
		make_partial(1, 1, 2, 100, 195),
	];
	let r1 = Solver::<MockAMMWithSlippage>::solve(forward, ()).unwrap();
	let r2 = Solver::<MockAMMWithSlippage>::solve(reversed, ()).unwrap();
	for id in [1u128, 2, 3] {
		let a = r1.resolved_intents.iter().find(|r| r.id == id);
		let b = r2.resolved_intents.iter().find(|r| r.id == id);
		match (a, b) {
			(Some(a), Some(b)) => {
				assert_eq!(a.data.amount_in(), b.data.amount_in(), "intent {id} amount_in differs");
				assert_eq!(
					a.data.amount_out(),
					b.data.amount_out(),
					"intent {id} amount_out differs"
				);
			}
			(None, None) => {}
			_ => panic!("intent {id} presence differs between orderings"),
		}
	}
}

/// Two identical partials in the same direction must receive identical fills
/// (not just identical rates). The Phase B sequential fit can produce
/// different fills for otherwise-identical partials when the clearing rate
/// degrades as more partials are added.
#[test]
fn test_identical_partials_get_identical_fills() {
	// P1 and P2 are identical: 100→195 at 2:1 spot with 1% slippage (~1.98 realised).
	// The 1.95 limit is above the combined-volume clearing, forcing binary-search to
	// shrink one (or both) of them. Whichever is fitted second should be treated
	// the same as the one fitted first.
	let intents = vec![
		make_partial(1, 1, 2, 100, 195),
		make_partial(2, 1, 2, 100, 195),
		make_intent(3, 2, 1, 20, 9),
	];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	let p1 = result.resolved_intents.iter().find(|r| r.id == 1);
	let p2 = result.resolved_intents.iter().find(|r| r.id == 2);
	match (p1, p2) {
		(Some(a), Some(b)) => {
			assert_eq!(
				a.data.amount_in(),
				b.data.amount_in(),
				"identical partials got different fills: {} vs {}",
				a.data.amount_in(),
				b.data.amount_in(),
			);
		}
		(None, None) => {} // both excluded — still symmetric
		_ => panic!("identical partials had asymmetric inclusion: p1={p1:?}, p2={p2:?}"),
	}
}

/// Partial and non-partial in the same direction must share a rate within 1 sat.
#[test]
fn test_partial_plus_non_partial_same_direction_uniform() {
	let intents = vec![
		make_partial(1, 1, 2, 200, 180),
		make_intent(2, 1, 2, 100, 90),
		make_intent(3, 2, 1, 100, 90),
	];
	let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();
	let r1 = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
	let r2 = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();
	assert!(
		same_rate_within(r1, r2, 1),
		"partial {} and non-partial {} must share rate within 1 sat: r1 {}→{}, r2 {}→{}",
		r1.id,
		r2.id,
		r1.data.amount_in(),
		r1.data.amount_out(),
		r2.data.amount_in(),
		r2.data.amount_out(),
	);
}

/// Ring detection must not over-consume a partial's `remaining()`. With the bug,
/// ring treats the partial as having its full `amount_in` available, inflating the
/// user's output rate past what the AMM (1:1) can actually deliver. The cleanest
/// observable failure: the resolved `amount_out` must be at most `fill * spot_rate`.
#[test]
fn test_ring_respects_partial_remaining() {
	// A→B partial: amount_in=100, already filled 60, so remaining=40.
	// B→C and C→A are full 100 each. Without the fix, ring consumes 100 of A→B.
	let intents = vec![
		make_partial_filled(1, 1, 2, 100, 90, 60),
		make_intent(2, 2, 3, 100, 90),
		make_intent(3, 3, 1, 100, 90),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();

	let p = result.resolved_intents.iter().find(|r| r.id == 1);
	if let Some(p) = p {
		assert!(
			p.data.amount_in() <= 40,
			"ring must not fill more than remaining: got amount_in={}, remaining=40",
			p.data.amount_in(),
		);
		// At 1:1 spot, amount_out is capped by amount_in.
		assert!(
			p.data.amount_out() <= p.data.amount_in(),
			"amount_out={} exceeds amount_in={} at 1:1 spot; ring over-consumed",
			p.data.amount_out(),
			p.data.amount_in(),
		);
	}
}

/// A partial whose `remaining()` is below the asset's ED must be filtered before Phase B.
#[test]
fn test_partial_below_ed_rejected() {
	// ED = 10 (MockAMMWithED). remaining = amount_in - already_filled = 100 - 95 = 5 < 10.
	let intents = vec![make_partial_filled(1, 1, 2, 100, 90, 95), make_intent(2, 2, 1, 100, 90)];
	let result = Solver::<MockAMMWithED>::solve(intents, ()).unwrap();
	assert!(
		result.resolved_intents.iter().all(|r| r.id != 1),
		"partial below ED must be filtered; got {:?}",
		result.resolved_intents.iter().map(|r| r.id).collect::<Vec<_>>(),
	);
}

/// A partial must never be filled in a way that leaves 0 < remaining < ed.
#[test]
fn test_partial_leaves_no_untradeable_dust() {
	let intents = vec![
		// original amount 100, ED 10: a fill of 95 would leave remaining=5 which < ED.
		// Solver must either fill all 100 or cap at ≤90.
		make_partial(1, 1, 2, 100, 90),
	];
	let result = Solver::<MockAMMWithED>::solve(intents, ()).unwrap();
	if let Some(r) = result.resolved_intents.iter().find(|r| r.id == 1) {
		let original = 100u128;
		let filled = r.data.amount_in();
		let remaining_after = original - filled;
		let ed = 10u128;
		assert!(
			remaining_after == 0 || remaining_after >= ed,
			"partial fill left untradeable dust: fill={filled}, remaining_after={remaining_after}, ed={ed}",
		);
	}
}

/// After the `remaining_untradeable` retry, the chosen fill must still satisfy ed_out.
#[test]
fn test_partial_retry_honors_ed_out() {
	// All resolved outputs must be ≥ ed_out = 10.
	let intents = vec![
		make_partial(1, 1, 2, 100, 90),
		make_partial(2, 1, 2, 50, 45),
		make_intent(3, 2, 1, 100, 90),
	];
	let result = Solver::<MockAMMWithED>::solve(intents, ()).unwrap();
	for r in &result.resolved_intents {
		assert!(
			r.data.amount_out() >= 10,
			"resolved intent {} has amount_out={} below ed_out=10",
			r.id,
			r.data.amount_out(),
		);
	}
}

/// When more than MAX_NUMBER_OF_RESOLVED_INTENTS fills are viable, the cap should
/// keep the highest-surplus intents, not the first N by input order.
#[test]
fn test_cap_by_surplus_not_input_order() {
	let mut intents: Vec<Intent> = (0..MAX_NUMBER_OF_RESOLVED_INTENTS as u128)
		.map(|id| make_intent(id + 1, 1, 2, 100, 99))
		.collect();
	// A high-surplus opposite-direction intent at the end — should survive any cap.
	intents.push(make_intent(u128::MAX, 2, 1, 100, 10));
	let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
	assert!(
		result.resolved_intents.iter().any(|r| r.id == u128::MAX),
		"high-surplus intent was dropped by first-N cap",
	);
}

/// The solver's `solution.score` must equal the pallet's recompute over all
/// resolved intents, exactly.
#[test]
fn test_score_matches_pallet_recompute() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 2, 1, 100, 90),
		make_partial(3, 1, 2, 200, 180),
		make_intent(4, 1, 2, 50, 40),
	];
	let result = Solver::<MockAMMOneToOne>::solve(intents.clone(), ()).unwrap();
	let pallet_recomputed = pallet_score(&intents, result.resolved_intents.as_slice());
	assert_eq!(
		result.score, pallet_recomputed,
		"solver score {} diverges from pallet recompute {}",
		result.score, pallet_recomputed,
	);
}

/// Every resolved intent's amount_in and amount_out must be ≥ ED for their assets.
#[test]
fn test_all_resolved_amounts_above_ed() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 2, 1, 100, 90),
		make_partial(3, 1, 2, 200, 180),
	];
	let result = Solver::<MockAMMWithED>::solve(intents, ()).unwrap();
	for r in &result.resolved_intents {
		assert!(
			r.data.amount_in() >= 10,
			"intent {} amount_in {} < ed",
			r.id,
			r.data.amount_in()
		);
		assert!(
			r.data.amount_out() >= 10,
			"intent {} amount_out {} < ed",
			r.id,
			r.data.amount_out()
		);
	}
}

/// Accumulating many large intents must not panic from unchecked overflow.
/// Individual intents are below `u128::MAX / 100` so the pair-per-direction
/// totals can't exceed u128, but summing across many directions in the same
/// call touches the DirAccum path.
#[test]
fn test_saturating_accumulation() {
	let per_intent: Balance = u128::MAX / 1_000; // safely representable per-intent
	let mut intents = Vec::new();
	for i in 0..50u128 {
		intents.push(make_intent(i * 2 + 1, 1, 2, per_intent, per_intent / 2));
		intents.push(make_intent(i * 2 + 2, 2, 1, per_intent, per_intent / 2));
	}
	// Must not panic.
	let _ = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();
}

/// Simulate two solver calls on the same partial intent, emulating two
/// on-chain rounds. The second call's resolved amount_in must not exceed
/// the remaining after the first fill.
#[test]
fn test_cumulative_partial_fill_across_calls() {
	let original_amount_in: Balance = 200;
	let intent1 = make_partial(1, 1, 2, original_amount_in, 150);
	let opposite = make_intent(2, 2, 1, 100, 90);

	let r1 = Solver::<MockAMMWithSlippage>::solve(vec![intent1, opposite.clone()], ()).unwrap();
	let first_fill = r1
		.resolved_intents
		.iter()
		.find(|r| r.id == 1)
		.map(|r| r.data.amount_in())
		.unwrap_or(0);
	assert!(first_fill > 0, "first call should resolve at least some of the partial");
	assert!(first_fill <= original_amount_in);

	// Pallet would advance `filled` by first_fill. Second call sees remaining = original - first_fill.
	if first_fill < original_amount_in {
		let intent2 = make_partial_filled(1, 1, 2, original_amount_in, 150, first_fill);
		let r2 = Solver::<MockAMMWithSlippage>::solve(vec![intent2, opposite], ()).unwrap();
		let second_fill = r2
			.resolved_intents
			.iter()
			.find(|r| r.id == 1)
			.map(|r| r.data.amount_in())
			.unwrap_or(0);
		let remaining_after_first = original_amount_in - first_fill;
		assert!(
			second_fill <= remaining_after_first,
			"second-call fill {second_fill} exceeds remaining {remaining_after_first}",
		);
		assert!(
			first_fill + second_fill <= original_amount_in,
			"cumulative fill {first_fill} + {second_fill} > original {original_amount_in}",
		);
	}
}
