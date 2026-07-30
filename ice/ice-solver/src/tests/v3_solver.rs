use crate::v3::Solver;
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, Route, Trade};
use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Partial, ResolvedIntent, SwapData};
use sp_core::U256;
use sp_std::collections::btree_set::BTreeSet;

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

/// Mirrors the on-chain `validate_price_consistency` predicate (±1 rounding).
fn same_rate_within(a: &ResolvedIntent, b: &ResolvedIntent, tol: u128) -> bool {
	let a_in = a.data.amount_in();
	let a_out = a.data.amount_out();
	let b_in = b.data.amount_in();
	let b_out = b.data.amount_out();
	let lhs = U256::from(a_out) * U256::from(b_in);
	let rhs = U256::from(b_out) * U256::from(a_in);
	let diff = if lhs >= rhs { lhs - rhs } else { rhs - lhs };
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

fn find_resolved(resolved: &[ResolvedIntent], id: IntentId) -> &ResolvedIntent {
	resolved.iter().find(|r| r.id == id).expect("intent should be resolved")
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

/// Constant-product style pool with volume-dependent slippage:
/// `out = in * depth / (depth + in)`, spot 1:1.
fn cp_out(amount_in: u128, depth: u128) -> u128 {
	amount_in * depth / (depth + amount_in)
}

macro_rules! depth_mock {
	($name:ident, $depth:expr, $ed:expr) => {
		struct $name;

		impl AMMInterface for $name {
			type Error = ();
			type State = ();

			fn discover_routes(
				asset_in: u32,
				asset_out: u32,
				_s: &Self::State,
			) -> Result<Vec<Route<u32>>, Self::Error> {
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
						amount_out: cp_out(amount_in, $depth),
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
						amount_in: amount_out + 1,
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
				$ed
			}
		}
	};
}

depth_mock!(MockAMMDepth, 10_000u128, 0u128);
depth_mock!(MockAMMDepthWithED, 10_000u128, 300u128);
depth_mock!(MockAMMDepth12, 12_000u128, 0u128);

/// 1:1, no slippage, existential deposit of 1000 on every asset.
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
		1_000
	}
}

// ---------- tests ----------

#[test]
fn solve_should_return_empty_solution_when_no_intents() {
	let solution = Solver::<MockAMMOneToOne>::solve(vec![], (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 0);
}

#[test]
fn solve_should_resolve_single_intent_when_route_meets_limit() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 90_000)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 100_000);
	// AMM output 100_000 minus the 1 bps haircut (10).
	assert_eq!(r.data.amount_out(), 99_990);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 100_000);
	assert_eq!(solution.trades[0].amount_out, 99_990);
	assert_eq!(solution.score, 9_990);
}

#[test]
fn solve_should_return_empty_solution_when_limit_unreachable() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 200_000)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
}

#[test]
fn opposing_intents_should_settle_without_amm_trade_when_volumes_cancel() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// Direct matching at the 1:1 reference price, no fee — full output, no trades.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 100_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 20_000);
}

#[test]
fn matched_volume_should_pay_fee_when_intents_cancel() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::from_percent(1)).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// 100% matched volume — both sides pay the 1% matched fee.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 99_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 99_000);
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 18_000);
}

#[test]
fn scarce_side_should_get_spot_rate_when_opposing_flow_is_excess() {
	// Asset 1 is worth 2× asset 2. id 1 sells 100 of asset 1 (200 in asset-2 value),
	// id 2 sells 100 of asset 2 — excess on the forward side, net 50 of asset 1
	// goes through the AMM at 1% slippage.
	let intents = vec![make_intent(1, 1, 2, 100, 150), make_intent(2, 2, 1, 100, 40)];
	let solution = Solver::<MockAMMWithSlippage>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// Forward: 100 matched at spot (100 of asset 2 from the scarce side... valued
	// as direct_match=100) + AMM output 99 for the net 50 → 199 total.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 199);
	// Scarce side gets the spot rate: 100 of asset 2 → 50 of asset 1.
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 50);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 50);
	assert_eq!(solution.trades[0].amount_out, 99);
	assert_eq!(solution.score, 59);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn tight_partial_should_not_throttle_loose_partial_when_sharing_direction() {
	// Depth-10_000 pool. Combined volume 2_000 clears at ~0.83 — below the tight
	// partial's 0.9 limit. Price priority: the loose partial fills fully, the
	// tight partial is trimmed to a fill that keeps the uniform rate at its
	// limit (the bisection lands on 105 — integer floor jitter makes the exact
	// feasibility boundary non-monotone, so the result is conservative).
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 900)];
	let solution = Solver::<MockAMMDepth>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	let loose = find_resolved(&solution.resolved_intents, 1);
	let tight = find_resolved(&solution.resolved_intents, 2);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 900);
	assert_eq!(tight.data.amount_in(), 105);
	assert_eq!(tight.data.amount_out(), 94);
	assert!(same_rate_within(loose, tight, 1));
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 1_105);
	assert_eq!(solution.trades[0].amount_out, 995);
	assert_eq!(solution.score, 400);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn partial_fills_should_be_input_order_independent_when_solved() {
	let a = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 900)];
	let b = vec![make_partial(2, 1, 2, 1_000, 900), make_partial(1, 1, 2, 1_000, 500)];

	let sol_a = Solver::<MockAMMDepth>::solve(a, (), Permill::zero()).unwrap();
	let sol_b = Solver::<MockAMMDepth>::solve(b, (), Permill::zero()).unwrap();

	let amounts = |s: &ice_support::Solution| -> Vec<(IntentId, Balance, Balance)> {
		let mut v: Vec<(IntentId, Balance, Balance)> = s
			.resolved_intents
			.iter()
			.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
			.collect();
		v.sort();
		v
	};
	assert_eq!(amounts(&sol_a), amounts(&sol_b));
	assert_eq!(sol_a.score, sol_b.score);
}

#[test]
fn infeasible_partial_should_be_dropped_when_no_fill_meets_limit() {
	// The tight partial demands 1.5× — unreachable at any volume. It must be
	// dropped without affecting the loose intent.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 1_500)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let loose = find_resolved(&solution.resolved_intents, 1);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 909);
	assert_eq!(solution.score, 409);
}

#[test]
fn nonpartial_should_be_dropped_when_clearing_rate_below_limit() {
	// Non-partial demanding 0.95 at a combined volume that clears at ~0.83:
	// all-or-nothing, so it is excluded entirely; the loose intent still fills.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_intent(2, 1, 2, 1_000, 950)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let loose = find_resolved(&solution.resolved_intents, 1);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 909);
	assert_eq!(solution.score, 409);
}

#[test]
fn partial_should_leave_no_dust_remainder_when_trim_lands_below_ed() {
	// ED 300. The tight partial's best feasible fill is 762, which would leave
	// an untradeable remainder of 238 — the fill is reduced to 700 so the
	// remainder (300) stays at the ED.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 850)];
	let solution = Solver::<MockAMMDepthWithED>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	let loose = find_resolved(&solution.resolved_intents, 1);
	let tight = find_resolved(&solution.resolved_intents, 2);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 854);
	assert_eq!(tight.data.amount_in(), 700);
	assert_eq!(tight.data.amount_out(), 597);
	assert!(same_rate_within(loose, tight, 1));
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 1_700);
	assert_eq!(solution.trades[0].amount_out, 1_452);
	assert_eq!(solution.score, 356);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn intent_should_be_excluded_when_amount_below_existential_deposit() {
	let intents = vec![make_intent(1, 1, 2, 500, 400)];
	let solution = Solver::<MockAMMWithED>::solve(intents, (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
}

#[test]
fn resolved_intents_should_be_capped_by_surplus_when_exceeding_max() {
	// 105 same-direction intents; min_out = 1_000 − (id − 1), so higher ids have
	// looser limits and more surplus. id 1 (limit 1.0) is dropped by the
	// crossing (the 1 bps haircut makes 1.0 unreachable); ids 2..=5 are the
	// lowest-surplus survivors and get cut by the cap.
	let intents: Vec<Intent> = (1..=105u128)
		.map(|id| make_intent(id, 1, 2, 1_000, 1_000 - (id - 1)))
		.collect();
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 100);
	let resolved_ids: BTreeSet<IntentId> = solution.resolved_intents.iter().map(|r| r.id).collect();
	let expected_ids: BTreeSet<IntentId> = (6..=105u128).collect();
	assert_eq!(resolved_ids, expected_ids);
	for r in solution.resolved_intents.iter() {
		assert_eq!(r.data.amount_in(), 1_000);
		assert_eq!(r.data.amount_out(), 999);
	}
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 100_000);
	assert_eq!(solution.trades[0].amount_out, 99_990);
	// Σ_{id=6}^{105} (999 − (1_000 − (id−1))) = Σ_{k=4}^{103} k = 5_350.
	assert_eq!(solution.score, 5_350);
}

#[test]
fn cumulative_partial_should_resolve_remaining_when_partially_filled() {
	// 400 of 1_000 already filled — only the remaining 600 may be spent.
	let intents = vec![make_partial_filled(1, 1, 2, 1_000, 500, 400)];
	let solution = Solver::<MockAMMDepth>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 600);
	assert_eq!(r.data.amount_out(), 566);
	let IntentData::Swap(swap) = &r.data else {
		panic!("expected swap data");
	};
	assert_eq!(swap.partial, Partial::Yes(400));
	// Pro-rata minimum for 600/1_000 of the original 500 limit is 300.
	assert_eq!(solution.score, 266);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn partial_should_fill_maximum_when_full_amount_infeasible() {
	// Depth-12_000 pool, limit 0.75. The full 10_000 only yields 5_454 (rate
	// 0.55) — the bisection finds the largest feasible fill instead.
	let intents = vec![make_partial(1, 1, 2, 10_000, 7_500)];
	let solution = Solver::<MockAMMDepth12>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 4_002);
	assert_eq!(r.data.amount_out(), 3_001);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 4_002);
	assert_eq!(solution.trades[0].amount_out, 3_001);
	assert_eq!(solution.score, 0);
}

#[test]
fn zero_limit_intent_should_receive_market_rate_when_resolved() {
	// A "don't care" minimum of 1 must not be paid out as the limit — the user
	// receives the full market-rate output.
	let intents = vec![make_intent(1, 1, 2, 1_000, 1)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_out(), 909);
	assert_eq!(solution.score, 908);
}

#[test]
fn zero_limit_intent_should_share_uniform_rate_when_matched_against_opposing_flow() {
	// Two zero-ish-limit intents matched directly: both must settle at the
	// reference (spot) rate, not at each other's limit.
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 2, 1, 100_000, 1)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 100_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert!(solution.trades.is_empty());
}
