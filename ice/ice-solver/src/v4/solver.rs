//! ICE Solver v4 — global netting (per-asset imbalance clearing).
//!
//! The production ICE solver. Inputs are the batch of valid intents, a
//! simulator snapshot and the matched-volume fee; the output is a `Solution`
//! the pallet can execute verbatim.
//!
//! Pipeline:
//!
//! 1. **Spot prices** for every intent asset, denominated in
//!    [`AMMInterface::price_denominator`], are collected once.
//! 2. **Candidate filter** — intents that cannot plausibly clear are dropped.
//! 3. **Per-pair crossing** — each unordered pair is cleared at a uniform
//!    per-direction rate. Intents are sorted by limit rate (price priority) and
//!    the tightest-limit intent is trimmed (partial) or dropped (non-partial)
//!    until the rate clears every included intent. Limits decide only inclusion
//!    and fill volume: payouts always come from the uniform direction rate, so a
//!    zero-limit intent still receives the best rate the batch can produce.
//! 4. **Global netting** — flows are netted at the *asset* level across the whole
//!    batch, so chains and cycles of any length internalize and only each asset's
//!    true residual imbalance is routed through the AMM. Each output asset's
//!    distributable pot is then split pro-rata at a uniform per-directed-pair
//!    rate. When any intent asset lacks a spot price the batch falls back to
//!    [`Solver::pairwise_round`], which clears pair by pair.
//! 5. **Stabilization** — trades execute sequentially against a mutating state,
//!    so resolution is retried without the intents that failed to clear. If the
//!    rounds are exhausted the solver falls back to the best single-intent
//!    solution rather than returning nothing.
//!
//! Every solution respects the on-chain validity rules the pallet re-checks:
//! uniform price per directed pair, pro-rata minimums, existential-deposit
//! guards on both resolved intents *and* emitted trades, and the
//! intent/trade caps.

use crate::common;
use crate::common::flow_graph;
use crate::common::ring_detection;
use crate::common::FlowDirection;
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::AMMInterface;
use hydradx_traits::router::Route;
use ice_support::{
	AssetId, Balance, Intent, IntentData, IntentId, PoolTrade, ResolvedIntent, ResolvedIntents, Solution,
	SolutionTrades, SwapData, SwapType, MAX_NUMBER_OF_RESOLVED_INTENTS, MAX_NUMBER_OF_SOLUTION_TRADES,
};
use sp_core::U256;
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::marker::PhantomData;
use sp_std::vec;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "solver::v4";

/// Protocol fee charged on matched (intent-to-intent) volume: the matched share
/// of an output is paid out as `gross × (1 − fee)`; AMM-routed volume is
/// untouched.
#[derive(Clone, Copy, Default, Debug)]
struct FeeCtx {
	matched: Permill,
}

impl FeeCtx {
	fn new(matched: Permill) -> Self {
		Self { matched }
	}

	fn apply(self, gross: Balance) -> Balance {
		gross.saturating_sub(self.matched.mul_floor(gross))
	}

	fn rate(self) -> Permill {
		self.matched
	}
}

/// Unordered pair key.
type AssetPair = (AssetId, AssetId);

/// Minimum output the chain enforces at resolution, for the intents where it is
/// stricter than their own `amount_out`.
///
/// Admission only. `amount_out` remains the sole basis for `surplus`, because the
/// chain re-derives the score from storage and any divergence is a `ScoreMismatch`.
/// These floors are recomputed from an oracle and must never reach the score.
type MinOuts = BTreeMap<IntentId, Balance>;

/// Numerator of the rate an intent must clear to be admitted: its floor when one
/// is supplied, otherwise its own `amount_out`.
fn admission_n(id: IntentId, swap: &SwapData, min_outs: &MinOuts) -> Balance {
	min_outs.get(&id).copied().unwrap_or(swap.amount_out)
}

/// Intents grouped by direction: (forward A→B, backward B→A).
type DirectionGroups<T> = (Vec<T>, Vec<T>);

/// `(amount_in, amount_out)` accumulated from ring matches for a single intent.
type RingFill = (Balance, Balance);

/// Per-direction accumulator used to blend ring fills with AMM output when
/// computing unified rates.
#[derive(Default)]
struct DirAccum {
	total_in: Balance,
	ring_in: Balance,
	ring_out: Balance,
}

/// AMM outputs are haircut by 1 bps so the on-chain execution can never
/// undershoot the solver's claim.
const AMM_SIMULATION_TOLERANCE_BPS: Balance = 1;

/// Bisection budget for fill searches. A `Balance` search interval halves every
/// step, so 128 steps make every search exact over the full `u128` range; the
/// loop exits as soon as the interval is empty, which for realistic balances is
/// far sooner.
const MAX_SEARCH_ITERATIONS: u32 = 128;

/// Stabilization rounds for the trade/resolution loop.
const MAX_STABILIZATION_ROUNDS: u32 = 6;

fn empty_solution() -> Solution {
	Solution::new(
		ResolvedIntents::truncate_from(Vec::new()),
		SolutionTrades::truncate_from(Vec::new()),
		0,
	)
}

fn unordered_pair(a: AssetId, b: AssetId) -> AssetPair {
	if a <= b {
		(a, b)
	} else {
		(b, a)
	}
}

fn adjust_amm_output(simulated_out: Balance) -> Balance {
	simulated_out.saturating_sub(simulated_out * AMM_SIMULATION_TOLERANCE_BPS / 10_000)
}

/// `amount_in * n / d` (integer floor), exact in `U512`.
///
/// A zero denominator or a quotient that does not fit 128 bits yields 0, which
/// every caller reads as "this intent or pair cannot be settled" — the failure
/// is loud in the log and inert in the solution, never a silently wrong payout.
fn apply_rate(amount_in: Balance, n: U256, d: U256) -> Balance {
	if d.is_zero() {
		log::warn!(target: LOG_TARGET, "zero-denominator rate applied to amount {amount_in}; treating as unpayable");
		return 0;
	}
	match common::mul_div(U256::from(amount_in), n, d).and_then(|v| Balance::try_from(v).ok()) {
		Some(v) => v,
		None => {
			log::warn!(
				target: LOG_TARGET,
				"rate {n}/{d} on amount {amount_in} does not fit 128 bits; treating as unpayable",
			);
			0
		}
	}
}

/// Overflow-safe midpoint of an inclusive `[lo, hi]` search interval.
///
/// `(lo + hi) / 2` overflows for balances above `u128::MAX / 2` and a
/// saturating add silently collapses the interval to a fixed point, so the
/// bisection would stop converging exactly where the amounts are largest.
fn midpoint(lo: Balance, hi: Balance) -> Balance {
	lo.saturating_add((hi.saturating_sub(lo)) / 2)
}

/// `out / v ≥ limit_n / limit_d`, cross-multiplied in U256.
fn rate_meets_limit(out: Balance, v: Balance, limit_n: Balance, limit_d: Balance) -> bool {
	U256::from(out).saturating_mul(U256::from(limit_d.max(1))) >= U256::from(limit_n).saturating_mul(U256::from(v))
}

/// Per-solve memo: route discovery, AMM quotes and existential deposits.
///
/// Routes are discovered once per directed pair (a discovery failure is cached
/// as an empty route set). Quotes are memoized per `(pair, amount)` and are
/// only valid against the *fitting* state they were computed for — the trade
/// building phase re-simulates against its own threaded state and must not use
/// `quote_out`/`quote`. Existential deposits are memoized because on-chain they
/// are a registry read and the resolution stage asks for the same handful of
/// assets over and over.
struct SolveCache<A: AMMInterface> {
	routes: BTreeMap<(AssetId, AssetId), Vec<Route<AssetId>>>,
	/// Best `(amount_out, route index)` for a `(pair, amount_in)` probe.
	quotes: BTreeMap<(AssetId, AssetId, Balance), Option<(Balance, usize)>>,
	existential_deposits: BTreeMap<AssetId, Balance>,
	/// Directed pairs with no route at all — reported in the solve summary.
	unroutable: BTreeSet<(AssetId, AssetId)>,
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> SolveCache<A> {
	fn new() -> Self {
		Self {
			routes: BTreeMap::new(),
			quotes: BTreeMap::new(),
			existential_deposits: BTreeMap::new(),
			unroutable: BTreeSet::new(),
			_phantom: PhantomData,
		}
	}

	fn ed(&mut self, asset: AssetId) -> Balance {
		*self
			.existential_deposits
			.entry(asset)
			.or_insert_with(|| A::existential_deposit(asset))
	}

	/// Cached route set for a directed pair; empty when discovery failed.
	fn routes(&mut self, asset_in: AssetId, asset_out: AssetId, state: &A::State) -> &[Route<AssetId>] {
		let key = (asset_in, asset_out);
		if !self.routes.contains_key(&key) {
			let discovered = A::discover_routes(asset_in, asset_out, state).unwrap_or_default();
			if discovered.is_empty() {
				log::debug!(target: LOG_TARGET, "no route for {asset_in} -> {asset_out}");
				self.unroutable.insert(key);
			}
			self.routes.insert(key, discovered);
		}
		self.routes.get(&key).map(|v| v.as_slice()).unwrap_or_default()
	}

	/// Clone of the `i`-th already-discovered route for a pair. Callers that
	/// iterate route sets need an owned `Route` because the AMM interface takes
	/// it by value, but they must not re-run discovery per index.
	fn route_at(&self, asset_in: AssetId, asset_out: AssetId, i: usize) -> Option<Route<AssetId>> {
		self.routes.get(&(asset_in, asset_out))?.get(i).cloned()
	}

	/// Best sell quote (raw simulator output, no haircut) against the fitting
	/// state, together with the route that produced it.
	fn quote(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Balance, Route<AssetId>)> {
		let (out, idx) = self.best_quote(asset_in, asset_out, amount_in, state)?;
		let route = self.route_at(asset_in, asset_out, idx)?;
		Some((out, route))
	}

	/// As [`Self::quote`] but without cloning the winning route — the fitting
	/// phase only ever needs the amount.
	fn quote_out(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<Balance> {
		self.best_quote(asset_in, asset_out, amount_in, state)
			.map(|(out, _)| out)
	}

	fn best_quote(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Balance, usize)> {
		if amount_in == 0 {
			return None;
		}
		let key = (asset_in, asset_out, amount_in);
		if let Some(cached) = self.quotes.get(&key) {
			return *cached;
		}
		let mut best: Option<(Balance, usize)> = None;
		for i in 0..self.routes(asset_in, asset_out, state).len() {
			let Some(route) = self.route_at(asset_in, asset_out, i) else {
				break;
			};
			if let Ok((_, exec)) = A::sell(asset_in, asset_out, amount_in, route, state) {
				// `>=` keeps the last maximum on ties, matching `max_by_key`; the
				// route list is deterministic, so the choice is stable across
				// collators.
				if best.map(|(out, _)| exec.amount_out >= out).unwrap_or(true) {
					best = Some((exec.amount_out, i));
				}
			}
		}
		self.quotes.insert(key, best);
		best
	}

	/// One structured line per solve. An empty solution is otherwise
	/// indistinguishable from "nothing was submitted"; `outcome` names the
	/// stage that emptied the batch and the counters say how much routing and
	/// quoting the batch actually got.
	fn report(&self, outcome: Outcome, intents: usize, candidates: usize, solution: &Solution) {
		let resolved = solution.resolved_intents.len();
		let trades = solution.trades.len();
		let score = solution.score;
		let pairs = self.routes.len();
		let unroutable = self.unroutable.len();
		let quotes = self.quotes.len();
		if resolved == 0 {
			log::info!(
				target: LOG_TARGET,
				"solve produced no solution: {outcome:?} (intents={intents}, candidates={candidates}, \
				 pairs={pairs}, unroutable_pairs={unroutable}, quotes={quotes})",
			);
		} else {
			log::info!(
				target: LOG_TARGET,
				"solve {outcome:?}: resolved={resolved}/{intents} trades={trades} score={score} \
				 (candidates={candidates}, pairs={pairs}, unroutable_pairs={unroutable}, quotes={quotes})",
			);
		}
	}
}

/// A candidate fill for one intent during pair crossing.
struct Cand<'a> {
	intent: &'a Intent,
	/// `swap.remaining()` at solve time — upper bound for `fill`.
	remaining: Balance,
	/// Current working fill amount.
	fill: Balance,
	/// Limit rate numerator (`amount_out` of the original intent).
	limit_n: Balance,
	/// Limit rate denominator (`amount_in` of the original intent).
	limit_d: Balance,
	partial: bool,
}

/// Per-pair context shared by the fitting helpers.
struct PairCtx {
	asset_a: AssetId,
	asset_b: AssetId,
	pa: Option<Ratio>,
	pb: Option<Ratio>,
	ed_a: Balance,
	ed_b: Balance,
	fee_ctx: FeeCtx,
}

/// What the solve actually did, logged once per call. Empty solutions are the
/// hard case to debug in production: this says *which* stage emptied the batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
	/// No intents were supplied.
	NoIntents,
	/// Every intent was filtered out before crossing.
	NoCandidates,
	/// Crossing left no intent with a viable fill.
	NoFillsAfterCrossing,
	/// The single-intent path produced the solution.
	SingleIntent,
	/// A stabilization round resolved every included intent.
	Stabilized { round: u32 },
	/// Stabilization never converged; a single-intent fallback was used.
	SingleIntentFallback,
	/// Stabilization never converged and no fallback intent could be resolved.
	Exhausted,
}

pub struct Solver<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> Solver<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State, matched_fee: Permill) -> Result<Solution, A::Error> {
		Self::solve_with_limits(intents, MinOuts::new(), initial_state, matched_fee)
	}

	/// As `solve`, with per-intent admission floors (see `MinOuts`). An intent
	/// that cannot be paid its floor at the batch's uniform rate is excluded,
	/// exactly as one whose own `amount_out` cannot be met.
	pub fn solve_with_limits(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
	) -> Result<Solution, A::Error> {
		let mut cache = SolveCache::<A>::new();
		Self::run_solve(&intents, min_outs, initial_state, matched_fee, &mut cache)
	}

	fn run_solve(
		intents: &[Intent],
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
		cache: &mut SolveCache<A>,
	) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			cache.report(Outcome::NoIntents, 0, 0, &empty_solution());
			return Ok(empty_solution());
		}

		log::debug!(target: LOG_TARGET, "solve() called with {} intents, matched_fee={:?}", intents.len(), matched_fee);

		let fee_ctx = FeeCtx::new(matched_fee);

		let spot_prices = Self::collect_spot_prices(intents, &initial_state, cache);

		let candidates: Vec<&Intent> = intents
			.iter()
			.filter(|intent| Self::is_candidate(intent, &spot_prices, &initial_state, cache))
			.collect();

		log::debug!(target: LOG_TARGET, "candidates: {}/{} intents", candidates.len(), intents.len());

		if candidates.is_empty() {
			cache.report(Outcome::NoCandidates, intents.len(), 0, &empty_solution());
			return Ok(empty_solution());
		}
		if candidates.len() == 1 {
			let solution = Self::solve_single_intent(candidates[0], &min_outs, &initial_state, cache)?;
			cache.report(Outcome::SingleIntent, intents.len(), 1, &solution);
			return Ok(solution);
		}

		// Group candidates per unordered pair, split by direction.
		let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<Cand>> = BTreeMap::new();
		for &intent in &candidates {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let remaining = swap.remaining();
			let cand = Cand {
				intent,
				remaining,
				fill: remaining,
				// The crossing engine must sort and trim on the limit the chain
				// enforces, not the one stored on the intent.
				limit_n: admission_n(intent.id, swap, &min_outs),
				limit_d: swap.amount_in,
				partial: swap.partial.is_partial(),
			};
			let up = unordered_pair(swap.asset_in, swap.asset_out);
			let entry = pair_groups.entry(up).or_default();
			if swap.asset_in == up.0 {
				entry.0.push(cand);
			} else {
				entry.1.push(cand);
			}
		}

		// Per-pair price crossing.
		let mut fills: BTreeMap<IntentId, Balance> = BTreeMap::new();
		for ((asset_a, asset_b), (fwd, bwd)) in pair_groups {
			let ctx = PairCtx {
				asset_a,
				asset_b,
				pa: spot_prices.get(&asset_a).cloned(),
				pb: spot_prices.get(&asset_b).cloned(),
				ed_a: cache.ed(asset_a),
				ed_b: cache.ed(asset_b),
				fee_ctx,
			};
			for (id, fill) in Self::cross_pair(&ctx, fwd, bwd, &initial_state, cache) {
				fills.insert(id, fill);
			}
		}

		if fills.is_empty() {
			log::debug!(target: LOG_TARGET, "no intents survived pair crossing");
			cache.report(
				Outcome::NoFillsAfterCrossing,
				intents.len(),
				candidates.len(),
				&empty_solution(),
			);
			return Ok(empty_solution());
		}

		let mut included: Vec<&Intent> = candidates
			.iter()
			.copied()
			.filter(|intent| fills.contains_key(&intent.id))
			.collect();

		// Cap to MAX_NUMBER_OF_RESOLVED_INTENTS, keeping the highest estimated surplus.
		if included.len() > MAX_NUMBER_OF_RESOLVED_INTENTS as usize {
			log::debug!(target: LOG_TARGET, "capping included from {} to {} (keeping highest surplus)",
				included.len(), MAX_NUMBER_OF_RESOLVED_INTENTS);
			let surpluses = Self::estimate_surpluses(&included, &fills, &spot_prices, &initial_state, cache, fee_ctx);
			Self::sort_by_surplus_desc(&mut included, &surpluses);
			included.truncate(MAX_NUMBER_OF_RESOLVED_INTENTS as usize);
		}

		if included.len() == 1 {
			let intent = included[0];
			let fill = fills.get(&intent.id).copied().unwrap_or(0);
			let solution = Self::solve_single_intent_with_fill(intent, fill, &min_outs, &initial_state, cache)?;
			cache.report(Outcome::SingleIntent, intents.len(), candidates.len(), &solution);
			return Ok(solution);
		}

		// Stabilization rounds: netting → trades → unified rates → resolution.
		// Fills coming from the crossing are already near-feasible, so this
		// usually converges in round one; later pairs can still drift because
		// trades execute sequentially against a mutating state.
		for round in 0..MAX_STABILIZATION_ROUNDS {
			log::debug!(target: LOG_TARGET, "stabilization round {}, {} included intents", round, included.len());

			let (resolved_intents, executed_trades, total_score) = Self::netting_round(
				&included,
				&fills,
				&min_outs,
				&spot_prices,
				&initial_state,
				cache,
				fee_ctx,
			);

			log::debug!(target: LOG_TARGET, "round {}: {} resolved, {} trades, score: {} (from {} included)",
				round, resolved_intents.len(), executed_trades.len(), total_score, included.len());

			if resolved_intents.len() == included.len() {
				let solution = Solution::new(
					ResolvedIntents::truncate_from(resolved_intents),
					SolutionTrades::truncate_from(executed_trades),
					total_score,
				);
				cache.report(
					Outcome::Stabilized { round },
					intents.len(),
					candidates.len(),
					&solution,
				);
				return Ok(solution);
			}

			let resolved_ids: BTreeSet<IntentId> = resolved_intents.iter().map(|r| r.id).collect();
			included.retain(|intent| resolved_ids.contains(&intent.id));

			if included.is_empty() {
				break;
			}
			if included.len() == 1 {
				let intent = included[0];
				let fill = fills.get(&intent.id).copied().unwrap_or(0);
				let solution = Self::solve_single_intent_with_fill(intent, fill, &min_outs, &initial_state, cache)?;
				cache.report(Outcome::SingleIntent, intents.len(), candidates.len(), &solution);
				return Ok(solution);
			}
		}

		// Rounds exhausted — fall back to the best single-intent solution
		// instead of discarding everything.
		log::warn!(target: LOG_TARGET, "stabilization did not converge after {MAX_STABILIZATION_ROUNDS} rounds; trying single-intent fallback");
		let mut fallback: Vec<&Intent> = candidates.clone();
		let surpluses = Self::estimate_surpluses(&fallback, &fills, &spot_prices, &initial_state, cache, fee_ctx);
		Self::sort_by_surplus_desc(&mut fallback, &surpluses);
		for intent in fallback {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
			let solution = Self::solve_single_intent_with_fill(intent, fill, &min_outs, &initial_state, cache)?;
			if !solution.resolved_intents.is_empty() {
				cache.report(
					Outcome::SingleIntentFallback,
					intents.len(),
					candidates.len(),
					&solution,
				);
				return Ok(solution);
			}
		}
		cache.report(Outcome::Exhausted, intents.len(), candidates.len(), &empty_solution());
		Ok(empty_solution())
	}

	/// Pre-compute spot prices for every asset appearing in the intent set,
	/// denominated in `A::price_denominator()`: highest-rate route wins; assets
	/// without a viable route are absent.
	fn collect_spot_prices(
		intents: &[Intent],
		state: &A::State,
		cache: &mut SolveCache<A>,
	) -> BTreeMap<AssetId, Ratio> {
		let denominator = A::price_denominator();
		let mut spot_prices: BTreeMap<AssetId, Ratio> = BTreeMap::new();
		spot_prices.insert(denominator, Ratio::one());

		for asset in common::collect_unique_assets(intents) {
			if asset == denominator {
				continue;
			}
			for i in 0..cache.routes(asset, denominator, state).len() {
				let Some(route) = cache.route_at(asset, denominator, i) else {
					break;
				};
				let Ok(price) = A::get_spot_price(asset, denominator, route, state) else {
					continue;
				};
				let better = spot_prices.get(&asset).is_none_or(|existing| {
					U256::from(price.n).saturating_mul(U256::from(existing.d))
						> U256::from(existing.n).saturating_mul(U256::from(price.d))
				});
				if better {
					spot_prices.insert(asset, price);
				}
			}
			if !spot_prices.contains_key(&asset) {
				log::debug!(target: LOG_TARGET, "no spot price for asset {asset}; pairs touching it cannot be netted");
			}
		}
		spot_prices
	}

	/// Decide whether an intent can plausibly be resolved in this round.
	///
	/// Partials are always kept — the crossing will trim them to a viable fill
	/// or drop them. Non-partials are kept when either the spot-price check or
	/// a direct route quote at full volume meets the (pro-rata) minimum.
	fn is_candidate(
		intent: &Intent,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
		cache: &mut SolveCache<A>,
	) -> bool {
		let IntentData::Swap(swap) = &intent.data else {
			return false;
		};
		let remaining = swap.remaining();
		if remaining == 0 {
			log::debug!(target: LOG_TARGET, "intent {}: fully filled, skipping", intent.id);
			return false;
		}
		if swap.partial.is_partial() {
			return true;
		}
		if common::is_satisfiable(intent, spot_prices) {
			return true;
		}
		// Spot check failed or prices unknown — a direct route quote is authoritative.
		if let Some(amount_out) = cache.quote_out(swap.asset_in, swap.asset_out, remaining, state) {
			let pro_rata_min = apply_rate(remaining, U256::from(swap.amount_out), U256::from(swap.amount_in));
			if amount_out >= pro_rata_min {
				return true;
			}
		}
		log::debug!(target: LOG_TARGET, "intent {}: unsatisfiable at spot and via direct quote", intent.id);
		false
	}

	/// Compute the total post-fee, post-haircut outputs per direction for the
	/// given volumes against the fitting state.
	///
	/// Matched volume is valued at the reference (spot) price with the matched
	/// fee applied; the net residual is quoted through the AMM with the 1 bps
	/// haircut. `None` for a direction with volume means the direction cannot
	/// be priced (no route / quote failure) — its intents must be trimmed or
	/// dropped. When reference prices are unknown or the pair cannot be valued
	/// at them, both directions are priced independently through the AMM (no
	/// matching).
	fn fit_outputs(
		ctx: &PairCtx,
		v_f: Balance,
		v_b: Balance,
		state: &A::State,
		cache: &mut SolveCache<A>,
	) -> (Option<Balance>, Option<Balance>) {
		let quote_f = |cache: &mut SolveCache<A>, amount: Balance| {
			cache
				.quote_out(ctx.asset_a, ctx.asset_b, amount, state)
				.map(adjust_amm_output)
		};
		let quote_b = |cache: &mut SolveCache<A>, amount: Balance| {
			cache
				.quote_out(ctx.asset_b, ctx.asset_a, amount, state)
				.map(adjust_amm_output)
		};

		if v_f == 0 && v_b == 0 {
			return (None, None);
		}
		if v_b == 0 {
			return (quote_f(cache, v_f), None);
		}
		if v_f == 0 {
			return (None, quote_b(cache, v_b));
		}

		let flow = match (ctx.pa.as_ref(), ctx.pb.as_ref()) {
			(Some(pa), Some(pb)) => common::analyze_pair_flow(v_f, v_b, pa, pb),
			_ => None,
		};
		let Some(flow) = flow else {
			// No usable reference price — price both directions independently.
			return (quote_f(cache, v_f), quote_b(cache, v_b));
		};

		match flow {
			FlowDirection::SingleForward { amount } => (quote_f(cache, amount), None),
			FlowDirection::SingleBackward { amount } => (None, quote_b(cache, amount)),
			FlowDirection::PerfectCancel { a_as_b, b_as_a } => {
				(Some(ctx.fee_ctx.apply(a_as_b)), Some(ctx.fee_ctx.apply(b_as_a)))
			}
			FlowDirection::ExcessForward {
				scarce_out,
				direct_match,
				net_sell,
			} => {
				let out_b = Some(ctx.fee_ctx.apply(scarce_out));
				let out_f = if net_sell < ctx.ed_a {
					Some(ctx.fee_ctx.apply(direct_match))
				} else {
					quote_f(cache, net_sell).map(|amm_out| ctx.fee_ctx.apply(direct_match).saturating_add(amm_out))
				};
				(out_f, out_b)
			}
			FlowDirection::ExcessBackward {
				scarce_out,
				direct_match,
				net_sell,
			} => {
				let out_f = Some(ctx.fee_ctx.apply(scarce_out));
				let out_b = if net_sell < ctx.ed_b {
					Some(ctx.fee_ctx.apply(direct_match))
				} else {
					quote_b(cache, net_sell).map(|amm_out| ctx.fee_ctx.apply(direct_match).saturating_add(amm_out))
				};
				(out_f, out_b)
			}
		}
	}

	/// Uniform-price crossing for one unordered pair.
	///
	/// Both direction groups are sorted by limit rate ascending (price
	/// priority, loosest first). While any direction's uniform rate fails its
	/// tightest included limit, the tightest intent is trimmed to the largest
	/// feasible fill (partials, once) or removed. Once both directions clear,
	/// the existential-deposit remainder rule is applied; because that lowers
	/// fills — and a lower forward volume can starve the backward direction of
	/// matched volume — the fit is re-checked afterwards instead of assumed.
	/// Volumes only ever ratchet down, so the loop is bounded.
	fn cross_pair<'a>(
		ctx: &PairCtx,
		mut fwd: Vec<Cand<'a>>,
		mut bwd: Vec<Cand<'a>>,
		state: &A::State,
		cache: &mut SolveCache<A>,
	) -> Vec<(IntentId, Balance)> {
		fwd.retain(|c| c.fill >= ctx.ed_a.max(1));
		bwd.retain(|c| c.fill >= ctx.ed_b.max(1));
		Self::sort_by_limit_asc(&mut fwd);
		Self::sort_by_limit_asc(&mut bwd);

		let mut trimmed: BTreeSet<IntentId> = BTreeSet::new();
		let mut ed_adjusted: BTreeSet<IntentId> = BTreeSet::new();
		// Per candidate the loop can spend at most two ED adjustments, the
		// re-trim each of them re-enables, and one final drop.
		let max_iters = 6 * (fwd.len() + bwd.len()) + 8;
		let mut converged = false;

		for _ in 0..max_iters {
			let v_f: Balance = fwd.iter().map(|c| c.fill).fold(0u128, |acc, v| acc.saturating_add(v));
			let v_b: Balance = bwd.iter().map(|c| c.fill).fold(0u128, |acc, v| acc.saturating_add(v));
			if v_f == 0 && v_b == 0 {
				converged = true;
				break;
			}

			let (out_f, out_b) = Self::fit_outputs(ctx, v_f, v_b, state, cache);

			let f_blocked = v_f > 0 && !Self::dir_ok(out_f, v_f, fwd.last());
			let b_blocked = v_b > 0 && !Self::dir_ok(out_b, v_b, bwd.last());

			if !f_blocked && !b_blocked {
				// Feasible at these volumes. Enforcing the ED remainder rule can
				// lower a fill, which invalidates the fit that was just proven —
				// so loop once more instead of returning.
				if Self::enforce_ed_remainder(ctx, &mut fwd, &mut bwd, &mut ed_adjusted, &mut trimmed) {
					continue;
				}
				converged = true;
				break;
			}

			// Fix forward first (deterministic preference).
			let (dir, is_fwd, v_dir, v_other) = if f_blocked {
				(&mut fwd, true, v_f, v_b)
			} else {
				(&mut bwd, false, v_b, v_f)
			};
			// dir is non-empty: its direction is blocked, which requires volume.
			let Some(tightest) = dir.last() else {
				converged = true;
				break;
			};
			let id = tightest.intent.id;

			if tightest.partial && !trimmed.contains(&id) {
				let base = v_dir.saturating_sub(tightest.fill);
				let limit = (tightest.limit_n, tightest.limit_d);
				let max_x = tightest.fill;
				match Self::trim_search(ctx, is_fwd, base, max_x, v_other, limit, state, cache) {
					Some(x) => {
						log::debug!(target: LOG_TARGET, "pair ({}, {}): trimmed partial {} to fill {}",
							ctx.asset_a, ctx.asset_b, id, x);
						if let Some(t) = dir.last_mut() {
							t.fill = x;
						}
						trimmed.insert(id);
					}
					None => {
						log::debug!(target: LOG_TARGET, "pair ({}, {}): dropped partial {} (no feasible fill)",
							ctx.asset_a, ctx.asset_b, id);
						dir.pop();
					}
				}
			} else {
				log::debug!(target: LOG_TARGET, "pair ({}, {}): dropped intent {} (limit above clearing rate)",
					ctx.asset_a, ctx.asset_b, id);
				dir.pop();
			}
		}

		if !converged {
			log::warn!(target: LOG_TARGET, "pair ({}, {}): crossing iteration budget ({max_iters}) exhausted without converging",
				ctx.asset_a, ctx.asset_b);
		}

		fwd.into_iter()
			.chain(bwd)
			.filter(|c| c.fill > 0)
			.map(|c| (c.intent.id, c.fill))
			.collect()
	}

	/// Existential-deposit guard on partial remainders: never leave an
	/// unfillable dust remainder behind. Returns `true` when a fill changed, in
	/// which case the caller must re-check the fit.
	///
	/// An adjusted candidate is un-marked as trimmed so the crossing may search
	/// a new feasible fill for it: a lower fill can starve the *other* direction
	/// of matched volume, and dropping the partial outright would lose an intent
	/// a smaller fill could still have served.
	///
	/// A candidate that still leaves dust after a second adjustment is dropped
	/// outright — a zero fill leaves the intent's whole (≥ ED) remaining amount
	/// for a later block, so dropping is always remainder-safe and terminates
	/// the adjustment.
	fn enforce_ed_remainder<'a>(
		ctx: &PairCtx,
		fwd: &mut [Cand<'a>],
		bwd: &mut [Cand<'a>],
		ed_adjusted: &mut BTreeSet<IntentId>,
		trimmed: &mut BTreeSet<IntentId>,
	) -> bool {
		let mut changed = false;
		for (cand, ed) in fwd
			.iter_mut()
			.map(|c| (c, ctx.ed_a))
			.chain(bwd.iter_mut().map(|c| (c, ctx.ed_b)))
		{
			if !cand.partial || cand.fill == 0 {
				continue;
			}
			let remaining_after = cand.remaining.saturating_sub(cand.fill);
			if remaining_after == 0 || remaining_after >= ed {
				continue;
			}
			let new_fill = if ed_adjusted.contains(&cand.intent.id) {
				0
			} else {
				let reduced = cand.remaining.saturating_sub(ed);
				if reduced >= ed.max(1) {
					reduced.min(cand.fill)
				} else {
					0
				}
			};
			ed_adjusted.insert(cand.intent.id);
			if new_fill != cand.fill {
				log::debug!(target: LOG_TARGET, "pair ({}, {}): intent {} fill {} -> {} (remainder {} below ed {})",
					ctx.asset_a, ctx.asset_b, cand.intent.id, cand.fill, new_fill, remaining_after, ed);
				cand.fill = new_fill;
				trimmed.remove(&cand.intent.id);
				changed = true;
			}
		}
		changed
	}

	/// The direction clears iff its uniform rate meets the tightest included limit.
	fn dir_ok(out: Option<Balance>, v: Balance, tightest: Option<&Cand>) -> bool {
		let Some(t) = tightest else {
			return true;
		};
		match out {
			Some(out) => rate_meets_limit(out, v, t.limit_n, t.limit_d),
			None => false,
		}
	}

	/// Bisect the largest fill `x` for the blocked direction's tightest intent
	/// such that the direction's uniform rate still meets its limit. Returns
	/// `None` when no fill ≥ max(ED, 1) is feasible.
	#[allow(clippy::too_many_arguments)]
	fn trim_search(
		ctx: &PairCtx,
		is_fwd: bool,
		base: Balance,
		max_x: Balance,
		v_other: Balance,
		limit: (Balance, Balance),
		state: &A::State,
		cache: &mut SolveCache<A>,
	) -> Option<Balance> {
		let ed_in = if is_fwd { ctx.ed_a } else { ctx.ed_b };
		let mut lo: Balance = ed_in.max(1);
		let mut hi: Balance = max_x;
		let mut best: Option<Balance> = None;

		for _ in 0..MAX_SEARCH_ITERATIONS {
			if lo > hi {
				break;
			}
			let mid = midpoint(lo, hi);
			let v_dir = base.saturating_add(mid);
			let (v_f, v_b) = if is_fwd { (v_dir, v_other) } else { (v_other, v_dir) };
			let (out_f, out_b) = Self::fit_outputs(ctx, v_f, v_b, state, cache);
			let out = if is_fwd { out_f } else { out_b };
			let ok = match out {
				Some(out) => rate_meets_limit(out, v_dir, limit.0, limit.1),
				None => false,
			};
			if ok {
				best = Some(mid);
				lo = mid.saturating_add(1);
			} else {
				hi = mid.saturating_sub(1);
			}
		}
		best
	}

	fn sort_by_limit_asc(cands: &mut [Cand]) {
		cands.sort_by(|a, b| {
			let lhs = U256::from(a.limit_n).saturating_mul(U256::from(b.limit_d.max(1)));
			let rhs = U256::from(b.limit_n).saturating_mul(U256::from(a.limit_d.max(1)));
			lhs.cmp(&rhs).then(a.intent.id.cmp(&b.intent.id))
		});
	}

	/// Estimate per-intent surplus at the current fills using the fitting-state
	/// pair outputs. Used for the resolved-intents cap and the fallback order.
	fn estimate_surpluses(
		included: &[&Intent],
		fills: &BTreeMap<IntentId, Balance>,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
		cache: &mut SolveCache<A>,
		fee_ctx: FeeCtx,
	) -> BTreeMap<IntentId, Balance> {
		let mut pair_totals: BTreeMap<AssetPair, (Balance, Balance)> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
			let up = unordered_pair(swap.asset_in, swap.asset_out);
			let entry = pair_totals.entry(up).or_default();
			if swap.asset_in == up.0 {
				entry.0 = entry.0.saturating_add(fill);
			} else {
				entry.1 = entry.1.saturating_add(fill);
			}
		}

		let mut pair_outputs: BTreeMap<AssetPair, (Option<Balance>, Option<Balance>)> = BTreeMap::new();
		for (&(asset_a, asset_b), &(v_f, v_b)) in &pair_totals {
			let ctx = PairCtx {
				asset_a,
				asset_b,
				pa: spot_prices.get(&asset_a).cloned(),
				pb: spot_prices.get(&asset_b).cloned(),
				ed_a: cache.ed(asset_a),
				ed_b: cache.ed(asset_b),
				fee_ctx,
			};
			pair_outputs.insert((asset_a, asset_b), Self::fit_outputs(&ctx, v_f, v_b, state, cache));
		}

		let mut surpluses: BTreeMap<IntentId, Balance> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
			let up = unordered_pair(swap.asset_in, swap.asset_out);
			let (Some(&(v_f, v_b)), Some(&(out_f, out_b))) = (pair_totals.get(&up), pair_outputs.get(&up)) else {
				surpluses.insert(intent.id, 0);
				continue;
			};
			let (out, v) = if swap.asset_in == up.0 {
				(out_f, v_f)
			} else {
				(out_b, v_b)
			};
			let share = match out {
				Some(out) if v > 0 => apply_rate(fill, U256::from(out), U256::from(v)),
				_ => 0,
			};
			let pro_rata_min = apply_rate(fill, U256::from(swap.amount_out), U256::from(swap.amount_in));
			surpluses.insert(intent.id, share.saturating_sub(pro_rata_min));
		}
		surpluses
	}

	fn sort_by_surplus_desc(included: &mut [&Intent], surpluses: &BTreeMap<IntentId, Balance>) {
		included.sort_by(|a, b| {
			let sa = surpluses.get(&a.id).copied().unwrap_or(0);
			let sb = surpluses.get(&b.id).copied().unwrap_or(0);
			match sb.cmp(&sa) {
				Ordering::Equal => a.id.cmp(&b.id),
				other => other,
			}
		});
	}

	/// Pick the best route by simulating every cached route against `state`.
	/// Used by the trade-building phase where the state is threaded between
	/// trades and memoized quotes would be stale.
	fn best_route_exec(
		cache: &mut SolveCache<A>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Route<AssetId>, Balance, A::State)> {
		let mut best: Option<(Route<AssetId>, Balance, A::State)> = None;
		for i in 0..cache.routes(asset_in, asset_out, state).len() {
			let Some(route) = cache.route_at(asset_in, asset_out, i) else {
				break;
			};
			let Ok((new_state, exec)) = A::sell(asset_in, asset_out, amount_in, route.clone(), state) else {
				continue;
			};
			if best.as_ref().map(|(_, out, _)| exec.amount_out >= *out).unwrap_or(true) {
				best = Some((route, exec.amount_out, new_state));
			}
		}
		best
	}

	/// Emit an AMM trade only if the pallet would actually execute it.
	///
	/// `submit_solution` *skips* any trade whose `amount_in` is below the ED of
	/// its first asset or whose `amount_out` is below the ED of its last asset.
	/// A skipped trade never pays into the holding pot, so a solution that
	/// counted its output would promise users more than the pot receives and
	/// abort on the conservation check.
	fn trade_is_executable(
		cache: &mut SolveCache<A>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		amount_out: Balance,
	) -> bool {
		if amount_in < cache.ed(asset_in).max(1) {
			log::debug!(target: LOG_TARGET, "trade {asset_in} -> {asset_out}: input {amount_in} below ED; not emitting");
			return false;
		}
		if amount_out < cache.ed(asset_out).max(1) {
			log::debug!(target: LOG_TARGET, "trade {asset_in} -> {asset_out}: output {amount_out} below ED; not emitting");
			return false;
		}
		true
	}

	/// Global-netting round. Nets every asset's flow across the whole batch
	/// (chains and cycles of any length internalize), routes only each asset's
	/// residual imbalance through the AMM, then distributes what the pot
	/// actually holds at a uniform per-directed-pair rate. Conservation-safe by
	/// construction: for every asset the payout is
	/// `sold + pool_out − pool_in − matched·fee`, so the pallet's
	/// `residual ≥ matched·fee` invariant holds.
	///
	/// Falls back to [`Self::pairwise_round`] when any intent asset lacks a spot
	/// price (the batch can't be valued globally).
	fn netting_round(
		included: &[&Intent],
		fills: &BTreeMap<IntentId, Balance>,
		min_outs: &MinOuts,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		initial_state: &A::State,
		cache: &mut SolveCache<A>,
		fee_ctx: FeeCtx,
	) -> (Vec<ResolvedIntent>, Vec<PoolTrade>, Balance) {
		let fill_of = |intent: &Intent| -> Balance {
			match &intent.data {
				IntentData::Swap(s) => fills.get(&intent.id).copied().unwrap_or_else(|| s.remaining()),
				_ => 0,
			}
		};
		// HDX-numeraire value of `amount` units of `asset` (None if unpriced or
		// not representable).
		let to_hdx = |amount: Balance, asset: AssetId| -> Option<U256> {
			let p = spot_prices.get(&asset)?;
			common::mul_div(U256::from(amount), U256::from(p.n), U256::from(p.d))
		};
		// Native `asset` amount worth `v_hdx` of HDX value.
		let from_hdx = |v_hdx: U256, asset: AssetId| -> Option<Balance> {
			let p = spot_prices.get(&asset)?;
			common::mul_div(v_hdx, U256::from(p.d), U256::from(p.n)).and_then(|v| v.try_into().ok())
		};

		// Per-asset sold (native) and the HDX-valued sold/demand across the batch.
		let mut sold_native: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut sold_hdx: BTreeMap<AssetId, U256> = BTreeMap::new();
		let mut demand_hdx: BTreeMap<AssetId, U256> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fill_of(intent);
			if fill == 0 {
				continue;
			}
			let (Some(v), true) = (to_hdx(fill, swap.asset_in), spot_prices.contains_key(&swap.asset_out)) else {
				// Unpriced asset — fall back to the per-pair engine for the batch.
				log::debug!(target: LOG_TARGET, "intent {}: {} -> {} cannot be valued globally; using the pairwise round",
					intent.id, swap.asset_in, swap.asset_out);
				return Self::pairwise_round(included, fills, min_outs, spot_prices, initial_state, cache, fee_ctx);
			};
			let si = sold_native.entry(swap.asset_in).or_insert(0);
			*si = si.saturating_add(fill);
			let sh = sold_hdx.entry(swap.asset_in).or_insert_with(U256::zero);
			*sh = sh.saturating_add(v);
			let dh = demand_hdx.entry(swap.asset_out).or_insert_with(U256::zero);
			*dh = dh.saturating_add(v);
		}

		let mut assets: BTreeSet<AssetId> = BTreeSet::new();
		assets.extend(sold_hdx.keys().copied());
		assets.extend(demand_hdx.keys().copied());

		let mut state = initial_state.clone();
		let mut executed_trades: Vec<PoolTrade> = Vec::new();
		let mut pool_in: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let mut pool_out: BTreeMap<AssetId, Balance> = BTreeMap::new();
		let add = |map: &mut BTreeMap<AssetId, Balance>, k: AssetId, v: Balance| {
			let e = map.entry(k).or_insert(0);
			*e = e.saturating_add(v);
		};

		// Residual routing: move each asset's surplus DIRECTLY to the assets in
		// deficit (no forced hub hop). This keeps matched volume off the AMM — only
		// the true cross-asset imbalance is routed — and settles a non-HDX
		// coincidence in a single direct trade. Deterministic: surpluses and
		// deficits are both walked in ascending asset-id order.
		let mut surplus: Vec<(AssetId, U256)> = Vec::new();
		let mut deficit: Vec<(AssetId, U256)> = Vec::new();
		for &asset in &assets {
			let s = sold_hdx.get(&asset).copied().unwrap_or_else(U256::zero);
			let d = demand_hdx.get(&asset).copied().unwrap_or_else(U256::zero);
			if s > d {
				surplus.push((asset, s - d));
			} else if d > s {
				deficit.push((asset, d - s));
			}
		}
		'surplus: for (sx, mut s_rem) in surplus {
			for d in deficit.iter_mut() {
				if s_rem.is_zero() {
					break;
				}
				if d.1.is_zero() {
					continue;
				}
				if executed_trades.len() >= MAX_NUMBER_OF_SOLUTION_TRADES as usize {
					// A single warning, then a clean exit: every remaining
					// surplus/deficit pair would hit the same cap, so looping on
					// would only repeat this log without routing anything.
					log::warn!(target: LOG_TARGET, "solution trade cap reached; remaining batch imbalance stays unrouted");
					break 'surplus;
				}
				let move_hdx = s_rem.min(d.1);
				let Some(amount) = from_hdx(move_hdx, sx) else {
					log::warn!(target: LOG_TARGET, "cannot convert {move_hdx} of reference value back into asset {sx}");
					continue;
				};
				let Some((route, out, ns)) = Self::best_route_exec(cache, sx, d.0, amount, &state) else {
					continue;
				};
				let adj = adjust_amm_output(out);
				if !Self::trade_is_executable(cache, sx, d.0, amount, adj) {
					continue;
				}
				executed_trades.push(PoolTrade {
					direction: SwapType::ExactIn,
					amount_in: amount,
					amount_out: adj,
					route,
				});
				state = ns;
				add(&mut pool_in, sx, amount);
				add(&mut pool_out, d.0, adj);
				// Only a trade that was actually emitted consumes the imbalance;
				// otherwise the surplus stays available for the next deficit asset.
				s_rem = s_rem.saturating_sub(move_hdx);
				d.1 = d.1.saturating_sub(move_hdx);
			}
		}

		// Per directed pair: total input and the pair's HDX-valued claim on its
		// output asset (used to split that asset's distributable pot pro-rata).
		let mut pair_in: BTreeMap<AssetPair, Balance> = BTreeMap::new();
		let mut pair_claim: BTreeMap<AssetPair, U256> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fill_of(intent);
			if fill == 0 {
				continue;
			}
			let key = (swap.asset_in, swap.asset_out);
			let pi = pair_in.entry(key).or_insert(0);
			*pi = pi.saturating_add(fill);
			if let Some(v) = to_hdx(fill, swap.asset_in) {
				let pc = pair_claim.entry(key).or_insert_with(U256::zero);
				*pc = pc.saturating_add(v);
			}
		}

		// Distributable per output asset B: what the pot can pay B-buyers without
		// breaking conservation = sold[B] + pool_out[B] − pool_in[B] − matched·fee,
		// where matched[B] = sold[B] − pool_in[B] is the internally-matched volume.
		let mut unified_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
		for (&(a, b), &total_in) in &pair_in {
			if total_in == 0 {
				continue;
			}
			let sold_b = sold_native.get(&b).copied().unwrap_or(0);
			let pin_b = pool_in.get(&b).copied().unwrap_or(0);
			let pout_b = pool_out.get(&b).copied().unwrap_or(0);
			let available = sold_b.saturating_add(pout_b).saturating_sub(pin_b);
			let matched = sold_b.saturating_sub(pin_b);
			let distributable = available.saturating_sub(fee_ctx.rate().mul_floor(matched));
			if distributable == 0 {
				continue;
			}
			let claim = pair_claim.get(&(a, b)).copied().unwrap_or_else(U256::zero);
			let total_claim = demand_hdx.get(&b).copied().unwrap_or_else(U256::zero);
			if total_claim.is_zero() {
				continue;
			}
			let Some(share) =
				common::mul_div(U256::from(distributable), claim, total_claim).and_then(|v| Balance::try_from(v).ok())
			else {
				log::warn!(target: LOG_TARGET, "pair ({a}, {b}): distributable share not representable; pair unpaid this round");
				continue;
			};
			if share == 0 {
				continue;
			}
			unified_rates.insert((a, b), Ratio::new(share, total_in));
		}

		let (resolved_intents, total_score) = Self::resolve_at_rates(included, fills, min_outs, &unified_rates, cache);
		(resolved_intents, executed_trades, total_score)
	}

	/// One pairwise round: ring detection, sequential trade building, unified
	/// per-direction rates, and resolution. Used when the batch cannot be valued
	/// globally (an intent asset has no spot price). Returns the resolved
	/// intents (a subset of `included`), the trades and the score.
	fn pairwise_round(
		included: &[&Intent],
		fills: &BTreeMap<IntentId, Balance>,
		min_outs: &MinOuts,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		initial_state: &A::State,
		cache: &mut SolveCache<A>,
		fee_ctx: FeeCtx,
	) -> (Vec<ResolvedIntent>, Vec<PoolTrade>, Balance) {
		// Ring detection, capped at the solver-decided fills.
		let graph_entries: Vec<(&Intent, Balance)> = included
			.iter()
			.map(|intent| {
				let cap = match &intent.data {
					IntentData::Swap(swap) => fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining()),
					_ => 0,
				};
				(*intent, cap)
			})
			.collect();
		let mut graph = flow_graph::build_flow_graph(&graph_entries);
		let rings = ring_detection::detect_rings(&mut graph, spot_prices, fee_ctx.rate());

		let mut ring_fills: BTreeMap<IntentId, RingFill> = BTreeMap::new();
		for ring in &rings {
			for (_pair, ring_fill_list) in &ring.edges {
				for fill in ring_fill_list {
					let entry = ring_fills.entry(fill.intent_id).or_default();
					entry.0 = entry.0.saturating_add(fill.amount_in);
					entry.1 = entry.1.saturating_add(fill.amount_out);
				}
			}
		}

		// Sequential trade building for the net imbalances. The state is
		// threaded between pairs in deterministic (asset-id) order, exactly
		// matching the order the pallet will execute the trades in.
		let mut state = initial_state.clone();
		let mut executed_trades: Vec<PoolTrade> = Vec::new();
		// Warned once: once the cap is hit every remaining pair in this round
		// would fail `sell_via_amm` the same way, so logging per pair would
		// just repeat the same message.
		let mut trade_cap_reported = false;
		let mut directed_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();

		let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<(IntentId, &SwapData)>> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let up = unordered_pair(swap.asset_in, swap.asset_out);
			let entry = pair_groups.entry(up).or_default();
			if swap.asset_in == up.0 {
				entry.0.push((intent.id, swap));
			} else {
				entry.1.push((intent.id, swap));
			}
		}

		for (&(asset_a, asset_b), (forward, backward)) in &pair_groups {
			let net_volume = |entries: &[(IntentId, &SwapData)]| -> Balance {
				entries
					.iter()
					.map(|(id, swap)| {
						let base = fills.get(id).copied().unwrap_or_else(|| swap.remaining());
						base.saturating_sub(ring_fills.get(id).map(|(a, _)| *a).unwrap_or(0))
					})
					.fold(0u128, |acc, v| acc.saturating_add(v))
			};
			let total_a_sold = net_volume(forward);
			let total_b_sold = net_volume(backward);

			if total_a_sold == 0 && total_b_sold == 0 {
				continue;
			}

			let mut sell_via_amm = |sell_asset: AssetId,
			                        buy_asset: AssetId,
			                        amount: Balance,
			                        state: &mut A::State,
			                        cache: &mut SolveCache<A>|
			 -> Option<Balance> {
				if executed_trades.len() >= MAX_NUMBER_OF_SOLUTION_TRADES as usize {
					if !trade_cap_reported {
						log::warn!(target: LOG_TARGET, "solution trade cap reached; remaining pairs stay unrouted this round");
						trade_cap_reported = true;
					}
					return None;
				}
				let (route, amount_out, new_state) =
					Self::best_route_exec(cache, sell_asset, buy_asset, amount, state)?;
				let adjusted_out = adjust_amm_output(amount_out);
				if !Self::trade_is_executable(cache, sell_asset, buy_asset, amount, adjusted_out) {
					return None;
				}
				executed_trades.push(PoolTrade {
					direction: SwapType::ExactIn,
					amount_in: amount,
					amount_out: adjusted_out,
					route,
				});
				*state = new_state;
				Some(adjusted_out)
			};

			let flow = match (spot_prices.get(&asset_a), spot_prices.get(&asset_b)) {
				(Some(pa), Some(pb)) => common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb),
				_ => None,
			};

			let Some(flow) = flow else {
				// No usable reference price — both directions execute
				// independently through the AMM, no direct matching.
				if total_a_sold >= cache.ed(asset_a) {
					if let Some(out) = sell_via_amm(asset_a, asset_b, total_a_sold, &mut state, cache) {
						directed_rates.insert((asset_a, asset_b), Ratio::new(out, total_a_sold));
					}
				}
				if total_b_sold >= cache.ed(asset_b) {
					if let Some(out) = sell_via_amm(asset_b, asset_a, total_b_sold, &mut state, cache) {
						directed_rates.insert((asset_b, asset_a), Ratio::new(out, total_b_sold));
					}
				}
				continue;
			};

			match flow {
				FlowDirection::SingleForward { amount } => {
					if amount < cache.ed(asset_a) {
						log::debug!(target: LOG_TARGET, "single forward {asset_a} -> {asset_b}: amount {amount} below ED");
					} else if let Some(out) = sell_via_amm(asset_a, asset_b, amount, &mut state, cache) {
						directed_rates.insert((asset_a, asset_b), Ratio::new(out, amount));
					}
				}
				FlowDirection::SingleBackward { amount } => {
					if amount < cache.ed(asset_b) {
						log::debug!(target: LOG_TARGET, "single backward {asset_b} -> {asset_a}: amount {amount} below ED");
					} else if let Some(out) = sell_via_amm(asset_b, asset_a, amount, &mut state, cache) {
						directed_rates.insert((asset_b, asset_a), Ratio::new(out, amount));
					}
				}
				FlowDirection::ExcessForward {
					scarce_out,
					direct_match,
					net_sell,
				} => {
					// Backward direction is fully matched (scarce side); fee applies.
					if total_b_sold > 0 {
						directed_rates.insert((asset_b, asset_a), Ratio::new(fee_ctx.apply(scarce_out), total_b_sold));
					}
					if net_sell < cache.ed(asset_a) {
						if total_a_sold > 0 {
							directed_rates.insert(
								(asset_a, asset_b),
								Ratio::new(fee_ctx.apply(direct_match), total_a_sold),
							);
						}
					} else if let Some(amm_out) = sell_via_amm(asset_a, asset_b, net_sell, &mut state, cache) {
						// Matched portion carries the fee; AMM portion does not.
						let total_out = fee_ctx.apply(direct_match).saturating_add(amm_out);
						if total_a_sold > 0 {
							directed_rates.insert((asset_a, asset_b), Ratio::new(total_out, total_a_sold));
						}
					}
					// On AMM failure no forward rate is set — there is no
					// spot-valued fallback: it would promise output the holding
					// pot never receives. Affected intents resolve to 0 this
					// round and the stabilization loop retries without them.
				}
				FlowDirection::ExcessBackward {
					scarce_out,
					direct_match,
					net_sell,
				} => {
					if total_a_sold > 0 {
						directed_rates.insert((asset_a, asset_b), Ratio::new(fee_ctx.apply(scarce_out), total_a_sold));
					}
					if net_sell < cache.ed(asset_b) {
						if total_b_sold > 0 {
							directed_rates.insert(
								(asset_b, asset_a),
								Ratio::new(fee_ctx.apply(direct_match), total_b_sold),
							);
						}
					} else if let Some(amm_out) = sell_via_amm(asset_b, asset_a, net_sell, &mut state, cache) {
						let total_out = fee_ctx.apply(direct_match).saturating_add(amm_out);
						if total_b_sold > 0 {
							directed_rates.insert((asset_b, asset_a), Ratio::new(total_out, total_b_sold));
						}
					}
				}
				FlowDirection::PerfectCancel { a_as_b, b_as_a } => {
					if total_a_sold > 0 {
						directed_rates.insert((asset_a, asset_b), Ratio::new(fee_ctx.apply(a_as_b), total_a_sold));
					}
					if total_b_sold > 0 {
						directed_rates.insert((asset_b, asset_a), Ratio::new(fee_ctx.apply(b_as_a), total_b_sold));
					}
				}
			}
		}

		// Unified rates: blend ring fills (matched — fee applies) with the
		// directed rates (already net of fee on their matched share).
		let mut unified_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
		{
			let mut accum: BTreeMap<AssetPair, DirAccum> = BTreeMap::new();

			for intent in included {
				let IntentData::Swap(swap) = &intent.data else {
					continue;
				};
				let key = (swap.asset_in, swap.asset_out);
				let entry = accum.entry(key).or_default();
				let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
				entry.total_in = entry.total_in.saturating_add(fill);
				let (ri, ro) = ring_fills.get(&intent.id).copied().unwrap_or((0, 0));
				entry.ring_in = entry.ring_in.saturating_add(ri);
				entry.ring_out = entry.ring_out.saturating_add(ro);
			}

			for (key, dir) in &accum {
				let remaining_in = dir.total_in.saturating_sub(dir.ring_in);
				let amm_out = if remaining_in > 0 {
					if let Some(rate) = directed_rates.get(key) {
						apply_rate(remaining_in, U256::from(rate.n), U256::from(rate.d))
					} else {
						0
					}
				} else {
					0
				};
				let ring_out_net = fee_ctx.apply(dir.ring_out);
				let total_out = ring_out_net.saturating_add(amm_out);
				if dir.total_in > 0 && total_out > 0 {
					unified_rates.insert(*key, Ratio::new(total_out, dir.total_in));
				}
			}
		}

		let (resolved_intents, total_score) = Self::resolve_at_rates(included, fills, min_outs, &unified_rates, cache);
		(resolved_intents, executed_trades, total_score)
	}

	/// Resolution: uniform price per directed pair. The canonical price is
	/// anchored on the pair's *largest* fill and that intent is emitted first,
	/// so the pallet's first-resolution anchor recomputes the identical price
	/// and every smaller fill stays within the ±1 tolerance (deviation is
	/// bounded by `fill_i / fill_anchor ≤ 1`). Anchoring on the largest fill
	/// also makes payouts independent of intent input order and minimizes
	/// rounding loss.
	fn resolve_at_rates(
		included: &[&Intent],
		fills: &BTreeMap<IntentId, Balance>,
		min_outs: &MinOuts,
		unified_rates: &BTreeMap<AssetPair, Ratio>,
		cache: &mut SolveCache<A>,
	) -> (Vec<ResolvedIntent>, Balance) {
		let mut by_direction: BTreeMap<AssetPair, Vec<(&Intent, &SwapData, Balance)>> = BTreeMap::new();
		for intent in included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
			if fill == 0 {
				continue;
			}
			by_direction
				.entry((swap.asset_in, swap.asset_out))
				.or_default()
				.push((intent, swap, fill));
		}

		let mut resolved_intents: Vec<ResolvedIntent> = Vec::new();
		let mut total_score: Balance = 0;

		for (directed_key, mut members) in by_direction {
			members.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.id.cmp(&b.0.id)));

			let Some(rate) = unified_rates.get(&directed_key) else {
				continue;
			};
			// members is non-empty by construction (only non-zero fills are pushed).
			let Some(&(_, _, anchor_fill)) = members.first() else {
				continue;
			};
			let anchor_out = apply_rate(anchor_fill, U256::from(rate.n), U256::from(rate.d));
			if anchor_out == 0 {
				continue;
			}
			let canonical = Ratio::new(anchor_out, anchor_fill);

			for (intent, swap, fill) in members {
				let total_out = apply_rate(fill, U256::from(canonical.n), U256::from(canonical.d));
				if total_out == 0 {
					continue;
				}

				let ed_in = cache.ed(swap.asset_in);
				let ed_out = cache.ed(swap.asset_out);
				if fill < ed_in || total_out < ed_out {
					log::debug!(
						target: LOG_TARGET,
						"intent {}: dropped — fill={} (ed_in={}) or total_out={} (ed_out={}) below ED",
						intent.id, fill, ed_in, total_out, ed_out,
					);
					continue;
				}

				// Admission on the enforced floor, score on the stored `amount_out`
				// — the chain re-derives the score from storage.
				let admission = apply_rate(
					fill,
					U256::from(admission_n(intent.id, swap, min_outs)),
					U256::from(swap.amount_in),
				);
				let min_required = apply_rate(fill, U256::from(swap.amount_out), U256::from(swap.amount_in));
				if total_out < admission || total_out < min_required {
					log::debug!(target: LOG_TARGET, "intent {}: skipped — output {} < pro_rata_min {} (admission {}) for fill {}",
						intent.id, total_out, min_required, admission, fill);
					continue;
				}

				let surplus = total_out.saturating_sub(min_required);
				total_score = total_score.saturating_add(surplus);

				resolved_intents.push(ResolvedIntent {
					id: intent.id,
					data: IntentData::Swap(SwapData {
						asset_in: swap.asset_in,
						asset_out: swap.asset_out,
						amount_in: fill,
						amount_out: total_out,
						partial: swap.partial,
					}),
				});
			}
		}

		(resolved_intents, total_score)
	}

	/// Single intent path, supporting partial fills.
	fn solve_single_intent(
		intent: &Intent,
		min_outs: &MinOuts,
		initial_state: &A::State,
		cache: &mut SolveCache<A>,
	) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};
		Self::solve_single_intent_with_fill(intent, swap.remaining(), min_outs, initial_state, cache)
	}

	/// Single intent with a specific fill amount.
	///
	/// The payout is the *haircut* AMM output (the same amount the trade claims
	/// as its minimum) — paying the raw simulated output risks the holding pot
	/// coming up short when on-chain execution drifts below the simulation.
	fn solve_single_intent_with_fill(
		intent: &Intent,
		fill: Balance,
		min_outs: &MinOuts,
		initial_state: &A::State,
		cache: &mut SolveCache<A>,
	) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};
		if fill == 0 {
			return Ok(empty_solution());
		}

		log::debug!(target: LOG_TARGET, "solving single intent {}: {} -> {}, fill: {}, min_rate: {}/{}",
			intent.id, swap.asset_in, swap.asset_out, fill, swap.amount_out, swap.amount_in);

		// Admission clears the enforced floor; the score below stays on `amount_out`.
		let min_n = U256::from(admission_n(intent.id, swap, min_outs));
		let score_n = U256::from(swap.amount_out);
		let min_d = U256::from(swap.amount_in);
		let ed_in = cache.ed(swap.asset_in);
		let ed_out = cache.ed(swap.asset_out);

		let try_fill = |cache: &mut SolveCache<A>, amount: Balance| -> Option<(Balance, Balance, Route<AssetId>)> {
			if amount < ed_in.max(1) {
				return None;
			}
			let (raw_out, route) = cache.quote(swap.asset_in, swap.asset_out, amount, initial_state)?;
			let net_out = adjust_amm_output(raw_out);
			let pro_rata_min = apply_rate(amount, min_n, min_d);
			// `net_out >= ed_out` is both the resolved-intent guard and the trade
			// guard: the solution's single trade is exactly (amount, net_out).
			if net_out >= pro_rata_min && net_out >= ed_out.max(1) {
				Some((amount, net_out, route))
			} else {
				None
			}
		};

		let result = if swap.partial.is_partial() {
			// Full fill first, then bisect for the largest feasible fill.
			let mut best = try_fill(cache, fill);
			if best.is_none() {
				let mut lo: Balance = ed_in.max(1);
				let mut hi: Balance = fill;
				for _ in 0..MAX_SEARCH_ITERATIONS {
					if lo > hi {
						break;
					}
					let mid = midpoint(lo, hi);
					match try_fill(cache, mid) {
						Some(found) => {
							best = Some(found);
							lo = mid.saturating_add(1);
						}
						None => {
							hi = mid.saturating_sub(1);
						}
					}
				}
			}
			// ED guard on the remainder: never leave dust behind.
			if let Some(found_fill) = best.as_ref().map(|(f, _, _)| *f) {
				let remaining_after = swap.remaining().saturating_sub(found_fill);
				if remaining_after > 0 && remaining_after < ed_in {
					let reduced = swap.remaining().saturating_sub(ed_in).min(fill);
					best = if reduced >= ed_in.max(1) {
						try_fill(cache, reduced)
					} else {
						None
					};
				}
			}
			best
		} else {
			try_fill(cache, fill)
		};

		let Some((actual_fill, net_out, route)) = result else {
			return Ok(empty_solution());
		};

		let surplus = net_out.saturating_sub(apply_rate(actual_fill, score_n, min_d));

		let resolved = ResolvedIntent {
			id: intent.id,
			data: IntentData::Swap(SwapData {
				asset_in: swap.asset_in,
				asset_out: swap.asset_out,
				amount_in: actual_fill,
				amount_out: net_out,
				partial: swap.partial,
			}),
		};

		Ok(Solution::new(
			ResolvedIntents::truncate_from(vec![resolved]),
			SolutionTrades::truncate_from(vec![PoolTrade {
				direction: SwapType::ExactIn,
				amount_in: actual_fill,
				amount_out: net_out,
				route,
			}]),
			surplus,
		))
	}
}
