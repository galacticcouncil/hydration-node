//! Pass-through builder behaviour, and the properties that separate it from v4.
//!
//! Both generations are driven through the shared [`IceSolver`] interface, so
//! the conformance the design asks for is compiler-checked here rather than
//! conventional.

use crate::{passthrough, v4, IceSolver, MinOuts};
use codec::Encode;
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, PoolType, Route, Trade};
use ice_support::{
	AssetId, Balance, Intent, IntentData, IntentId, Partial, ResolvedIntent, Solution, SwapData, SwapType,
};
use std::collections::BTreeMap;

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

fn dummy_route(asset_in: AssetId, asset_out: AssetId) -> Route<AssetId> {
	Route::try_from(vec![Trade {
		pool: PoolType::Omnipool,
		asset_in,
		asset_out,
	}])
	.expect("single-hop route fits the bound")
}

// ---------- mock ----------

/// Reserve set of the mock market. Cloned and mutated per simulated sell, so a
/// solver that threads the state sees its own trades in the next quote.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Reserves(BTreeMap<AssetId, Balance>);

impl Reserves {
	fn get(&self, asset: AssetId) -> Option<Balance> {
		self.0.get(&asset).copied()
	}
}

fn reserves(entries: &[(AssetId, Balance)]) -> Reserves {
	Reserves(entries.iter().copied().collect())
}

/// Every asset at 1_000_000, so every spot price is 1:1 and asset 0 (the price
/// denominator) is quotable.
fn balanced() -> Reserves {
	reserves(&[(0, 1_000_000), (1, 1_000_000), (2, 1_000_000), (3, 1_000_000)])
}

/// Constant-product market over one shared reserve set: selling `x` of `a` for
/// `b` pays `reserve_b * x / (reserve_a + x)` and moves both reserves. `ED` is
/// the existential deposit of every asset.
struct CpAmm<const ED: u128>;

impl<const ED: u128> AMMInterface for CpAmm<ED> {
	type Error = ();
	type State = Reserves;

	fn discover_routes(
		asset_in: AssetId,
		asset_out: AssetId,
		state: &Self::State,
	) -> Result<Vec<Route<AssetId>>, Self::Error> {
		if asset_in != asset_out && state.get(asset_in).is_some() && state.get(asset_out).is_some() {
			Ok(vec![dummy_route(asset_in, asset_out)])
		} else {
			Ok(Vec::new())
		}
	}

	fn sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_route: Route<AssetId>,
		state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		let reserve_in = state.get(asset_in).ok_or(())?;
		let reserve_out = state.get(asset_out).ok_or(())?;
		let amount_out = reserve_out
			.checked_mul(amount_in)
			.ok_or(())?
			.checked_div(reserve_in.checked_add(amount_in).ok_or(())?)
			.ok_or(())?;
		if amount_out >= reserve_out {
			return Err(());
		}
		let mut next = state.clone();
		next.0.insert(asset_in, reserve_in + amount_in);
		next.0.insert(asset_out, reserve_out - amount_out);
		Ok((
			next,
			TradeExecution {
				amount_in,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		_asset_in: AssetId,
		_asset_out: AssetId,
		_amount_out: Balance,
		_route: Route<AssetId>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Err(())
	}

	fn get_spot_price(
		asset_in: AssetId,
		asset_out: AssetId,
		_route: Route<AssetId>,
		state: &Self::State,
	) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(
			state.get(asset_out).ok_or(())?,
			state.get(asset_in).ok_or(())?,
		))
	}

	fn price_denominator() -> AssetId {
		0
	}

	fn pool_edges(_state: &Self::State) -> Vec<PoolEdge<AssetId>> {
		Vec::new()
	}

	fn existential_deposit(_asset_id: AssetId) -> Balance {
		ED
	}
}

type Amm = CpAmm<0>;
type AmmWithEd = CpAmm<1_000>;

// ---------- harness ----------

/// Every solve in this file goes through the shared interface, so the two
/// generations stay swappable by type parameter.
fn solve<A: AMMInterface, S: IceSolver<A>>(intents: Vec<Intent>, state: A::State, fee: Permill) -> Solution {
	S::solve(intents, state, fee).unwrap_or_else(|_| panic!("solve must not fail"))
}

fn solve_with_limits<A: AMMInterface, S: IceSolver<A>>(
	intents: Vec<Intent>,
	min_outs: MinOuts,
	state: A::State,
	fee: Permill,
) -> Solution {
	S::solve_with_limits(intents, min_outs, state, fee).unwrap_or_else(|_| panic!("solve must not fail"))
}

fn passthrough<A: AMMInterface>(intents: Vec<Intent>, state: A::State) -> Solution {
	solve::<A, passthrough::Solver<A>>(intents, state, Permill::zero())
}

fn v4<A: AMMInterface>(intents: Vec<Intent>, state: A::State) -> Solution {
	solve::<A, v4::Solver<A>>(intents, state, Permill::zero())
}

fn resolved_ids(solution: &Solution) -> Vec<IntentId> {
	solution.resolved_intents.iter().map(|r| r.id).collect()
}

fn find_resolved(solution: &Solution, id: IntentId) -> &ResolvedIntent {
	solution
		.resolved_intents
		.iter()
		.find(|r| r.id == id)
		.expect("intent should be resolved")
}

fn amounts(solution: &Solution) -> Vec<(IntentId, Balance, Balance)> {
	solution
		.resolved_intents
		.iter()
		.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
		.collect()
}

/// The three-intent batch used by the shape/determinism tests.
fn mixed_batch() -> Vec<Intent> {
	vec![
		make_intent(1, 1, 2, 100_000, 1),
		make_intent(2, 2, 1, 50_000, 1),
		make_intent(3, 1, 3, 20_000, 1),
	]
}

// ---------- tests ----------

#[test]
fn passthrough_should_produce_an_identical_solution_when_solved_twice() {
	let first = passthrough::<Amm>(mixed_batch(), balanced());
	let second = passthrough::<Amm>(mixed_batch(), balanced());

	assert_eq!(first.encode(), second.encode());
	// 1 -> 2 at 1_000_000/1_100_000, then 2 -> 1 and 1 -> 3 on the moved reserves.
	assert_eq!(
		amounts(&first),
		vec![(1, 100_000, 90_909), (2, 50_000, 57_345), (3, 20_000, 18_820)]
	);
	assert_eq!(first.score, 167_071);
	assert_eq!(first.built_at, 0);
}

#[test]
fn passthrough_should_emit_one_exact_in_trade_per_resolved_intent_when_batch_settles() {
	let solution = passthrough::<Amm>(mixed_batch(), balanced());

	assert_eq!(solution.resolved_intents.len(), 3);
	assert_eq!(solution.trades.len(), 3);
	for (resolved, trade) in solution.resolved_intents.iter().zip(solution.trades.iter()) {
		assert_eq!(trade.direction, SwapType::ExactIn);
		assert_eq!(trade.amount_in, resolved.data.amount_in());
		assert_eq!(trade.amount_out, resolved.data.amount_out());
		assert_eq!(
			trade.route.first().expect("route is never empty").asset_in,
			resolved.data.asset_in()
		);
		assert_eq!(
			trade.route.last().expect("route is never empty").asset_out,
			resolved.data.asset_out()
		);
	}
}

#[test]
fn second_intent_should_be_quoted_worse_when_it_shares_the_pair_with_an_accepted_one() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 1, 2, 100_000, 1)];
	let solution = passthrough::<Amm>(intents, balanced());

	// 1_000_000 * 100_000 / 1_100_000, then 909_091 * 100_000 / 1_200_000.
	assert_eq!(find_resolved(&solution, 1).data.amount_out(), 90_909);
	assert_eq!(find_resolved(&solution, 2).data.amount_out(), 75_757);
	assert_eq!(solution.score, 166_664);
}

#[test]
fn same_pair_intents_should_settle_at_different_prices_when_solved_by_passthrough() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 1, 2, 100_000, 1)];

	let pass = passthrough::<Amm>(intents.clone(), balanced());
	let matched = v4::<Amm>(intents, balanced());

	// Pass-through has no common clearing price to protect, so the two same-pair
	// intents keep their own execution prices — a shape v4 may never emit.
	assert_eq!(amounts(&pass), vec![(1, 100_000, 90_909), (2, 100_000, 75_757)]);
	assert_eq!(amounts(&matched), vec![(1, 100_000, 83_325), (2, 100_000, 83_325)]);
}

#[test]
fn intent_should_be_excluded_when_its_own_limit_cannot_be_cleared() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 99_000),
		make_intent(2, 1, 2, 100_000, 1),
		make_intent(3, 1, 2, 100_000, 1),
	];
	let solution = passthrough::<Amm>(intents, balanced());

	// The rejected intent leaves the running state untouched, so intent 2 is
	// quoted exactly as if it had been first.
	assert_eq!(resolved_ids(&solution), vec![2, 3]);
	assert_eq!(find_resolved(&solution, 2).data.amount_out(), 90_909);
	assert_eq!(find_resolved(&solution, 3).data.amount_out(), 75_757);
	assert_eq!(solution.trades.len(), 2);
}

#[test]
fn intent_should_be_excluded_when_min_outs_floor_exceeds_its_quote() {
	// Intent 1's own limit (80_000) clears at the 90_909 quote; only the
	// admission floor keeps it out.
	let intents = vec![make_intent(1, 1, 2, 100_000, 80_000), make_intent(2, 1, 2, 100_000, 1)];
	let min_outs: MinOuts = [(1u128, 95_000u128)].into_iter().collect();
	let solution = solve_with_limits::<Amm, passthrough::Solver<Amm>>(intents, min_outs, balanced(), Permill::zero());

	assert_eq!(resolved_ids(&solution), vec![2]);
	assert_eq!(find_resolved(&solution, 2).data.amount_out(), 90_909);
	assert_eq!(solution.score, 90_908);
}

#[test]
fn partial_intent_should_be_quoted_for_its_full_remaining_when_admitted() {
	// 100_000 of 200_000 already filled; the pro-rata limit on the remainder is
	// 100_000 * 150_000 / 200_000 = 75_000.
	let intents = vec![make_partial_filled(1, 1, 2, 200_000, 150_000, 100_000)];
	let solution = passthrough::<Amm>(intents, balanced());

	let resolved = find_resolved(&solution, 1);
	assert_eq!(resolved.data.amount_in(), 100_000);
	assert_eq!(resolved.data.amount_out(), 90_909);
	let IntentData::Swap(swap) = &resolved.data else {
		panic!("expected swap data");
	};
	assert_eq!(swap.partial, Partial::Yes(100_000));
	assert_eq!(solution.trades[0].amount_in, 100_000);
	assert_eq!(solution.score, 15_909);
}

#[test]
fn partial_intent_should_be_excluded_when_its_full_remaining_misses_the_limit() {
	// 200_000 at a 0.95 limit: the full remaining only quotes 166_666, and
	// pass-through never proposes the smaller fill that would clear it.
	let intents = vec![make_partial_filled(1, 1, 2, 200_000, 190_000, 0)];

	let pass = passthrough::<Amm>(intents.clone(), balanced());
	let matched = v4::<Amm>(intents, balanced());

	assert_eq!(resolved_ids(&pass), Vec::<IntentId>::new());
	assert_eq!(pass.trades.len(), 0);
	// v4 bisects to a smaller feasible fill (49_935 is exactly its pro-rata
	// minimum) — the policy difference, not an accident of the fixture.
	assert_eq!(amounts(&matched), vec![(1, 52_564, 49_935)]);
}

#[test]
fn intent_should_be_excluded_when_remaining_is_below_the_existential_deposit() {
	let intents = vec![
		make_partial_filled(1, 1, 2, 100_000, 1, 99_500),
		make_intent(2, 1, 2, 100_000, 1),
	];
	let solution = passthrough::<AmmWithEd>(intents, balanced());

	assert_eq!(resolved_ids(&solution), vec![2]);
	assert_eq!(find_resolved(&solution, 2).data.amount_out(), 90_909);
}

#[test]
fn intent_should_be_excluded_when_its_quote_is_below_the_existential_deposit() {
	// Asset 2 is nearly drained: selling 1_000 of asset 1 quotes a single unit.
	let shallow = reserves(&[(0, 1_000_000), (1, 1_000_000), (2, 2_000)]);
	let intents = vec![make_intent(1, 1, 2, 1_000, 1)];

	let gated = passthrough::<AmmWithEd>(intents.clone(), shallow.clone());
	let ungated = passthrough::<Amm>(intents, shallow);

	assert_eq!(resolved_ids(&gated), Vec::<IntentId>::new());
	assert_eq!(gated.trades.len(), 0);
	assert_eq!(amounts(&ungated), vec![(1, 1_000, 1)]);
}

#[test]
fn resolved_intents_should_be_capped_at_the_maximum_when_batch_is_larger() {
	let intents: Vec<Intent> = (1..=105u128).map(|id| make_intent(id, 1, 2, 1_000, 1)).collect();
	let solution = passthrough::<Amm>(intents, balanced());

	assert_eq!(
		solution.resolved_intents.len(),
		ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS as usize
	);
	assert_eq!(solution.trades.len(), 100);
	// Ascending id order, so the leftovers are the newest ids.
	assert_eq!(resolved_ids(&solution), (1..=100u128).collect::<Vec<_>>());
}

#[test]
fn matched_fee_should_not_change_the_solution_when_solver_is_passthrough() {
	let free = solve::<Amm, passthrough::Solver<Amm>>(mixed_batch(), balanced(), Permill::zero());
	let charged = solve::<Amm, passthrough::Solver<Amm>>(mixed_batch(), balanced(), Permill::from_percent(10));

	assert_eq!(free, charged);
	assert_eq!(charged.score, 167_071);
}

#[test]
fn passthrough_should_resolve_a_subset_of_v4_when_an_intent_only_clears_by_matching() {
	// Intent 1's limit (0.99) is unreachable at the AMM but clears exactly when
	// netted against the opposing intent 2.
	let intents = vec![make_intent(1, 1, 2, 100_000, 99_000), make_intent(2, 2, 1, 100_000, 1)];

	let pass = passthrough::<Amm>(intents.clone(), balanced());
	let matched = v4::<Amm>(intents, balanced());

	assert_eq!(amounts(&pass), vec![(2, 100_000, 90_909)]);
	assert_eq!(pass.trades.len(), pass.resolved_intents.len());
	assert_eq!(amounts(&matched), vec![(1, 100_000, 100_000), (2, 100_000, 100_000)]);
	assert_eq!(matched.trades.len(), 0);
	assert_eq!(matched.score, 100_999);
	// v4 resolves a superset: matching pays intent 1, pass-through skips it.
	for id in resolved_ids(&pass) {
		assert!(resolved_ids(&matched).contains(&id), "intent {id} lost by v4");
	}
}
