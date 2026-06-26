//! ICE Solver v3 — Per-Pair Price Crossing with Price-Priority Fills
//!
//! Same inputs and outputs as v2; the clearing engine differs:
//!
//! 1. Routes are discovered once per directed pair and AMM quotes are memoized
//!    (v2 re-discovers and re-simulates inside every fitting probe).
//! 2. Each pair is cleared by a price crossing: intents are sorted by limit
//!    rate (price priority) and the tightest-limit intent is trimmed (partial)
//!    or dropped (non-partial) until the uniform per-direction rate clears
//!    every included intent. A tight-limit partial can therefore no longer
//!    throttle loose-limit fills the way v2's uniform `t`-scaling did.
//! 3. Limits decide only *inclusion and fill volume* — payouts always come
//!    from the uniform direction rate (matched-at-reference + AMM blend),
//!    never from an intent's own limit, so a zero-limit intent still receives
//!    the best rate the batch can produce.
//! 4. On stabilization failure the solver falls back to the best single-intent
//!    solution instead of returning an empty one.
//!
//! Ring detection, the matched-volume fee treatment, unified per-direction
//! rates and all on-chain validity rules (uniform price per directed pair,
//! pro-rata minimums, existential-deposit guards, intent/trade caps) are
//! preserved from v2.

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
	SolutionTrades, SwapData, SwapType, MAX_NUMBER_OF_RESOLVED_INTENTS,
};
use sp_core::U256;
use sp_std::cmp::Ordering;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::marker::PhantomData;
use sp_std::vec;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "solver::v3";

/// Protocol fee charged on matched (intent-to-intent) volume.
/// Same semantics as v2: the matched share of an output is paid out as
/// `gross × (1 − fee)`; AMM-routed volume is untouched.
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

/// Same simulation tolerance as v2 — AMM outputs are haircut by 1 bps so the
/// on-chain execution can never undershoot the solver's claim.
const AMM_SIMULATION_TOLERANCE_BPS: Balance = 1;

/// Bisection budget for fill searches. Enough for exact convergence on any
/// realistic fill range (2^64 ≫ practical balances) while staying bounded.
const MAX_SEARCH_ITERATIONS: u32 = 64;

/// Stabilization rounds for the trade/resolution loop.
const MAX_STABILIZATION_ROUNDS: u32 = 6;

fn empty_solution() -> Solution {
	Solution {
		resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
		trades: SolutionTrades::truncate_from(Vec::new()),
		score: 0,
	}
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

/// Compute `amount_in * n / d` (integer floor), saturating to 0 on overflow or
/// division by zero.
fn apply_rate(amount_in: Balance, n: U256, d: U256) -> Balance {
	if d.is_zero() {
		log::warn!(
			target: LOG_TARGET,
			"apply_rate called with zero denominator (amount_in={amount_in}, n={n}); returning 0",
		);
		return 0;
	}
	common::mul_div(U256::from(amount_in), n, d)
		.and_then(|v| v.try_into().ok())
		.unwrap_or(0)
}

/// `out / v ≥ limit_n / limit_d`, cross-multiplied in U256.
fn rate_meets_limit(out: Balance, v: Balance, limit_n: Balance, limit_d: Balance) -> bool {
	U256::from(out).saturating_mul(U256::from(limit_d.max(1))) >= U256::from(limit_n).saturating_mul(U256::from(v))
}

/// Route discovery + best-quote cache.
///
/// Routes are discovered once per directed pair (a discovery failure is cached
/// as an empty route set). Quotes are memoized per `(pair, amount)` and are
/// only valid against the *fitting* state they were computed for — the trade
/// building phase re-simulates against its own threaded state and must not use
/// `quote`.
struct QuoteCache<A: AMMInterface> {
	routes: BTreeMap<(AssetId, AssetId), Vec<Route<AssetId>>>,
	quotes: BTreeMap<(AssetId, AssetId, Balance), Option<(Balance, usize)>>,
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> QuoteCache<A> {
	fn new() -> Self {
		Self {
			routes: BTreeMap::new(),
			quotes: BTreeMap::new(),
			_phantom: PhantomData,
		}
	}

	fn ensure_routes(&mut self, asset_in: AssetId, asset_out: AssetId, state: &A::State) {
		self.routes
			.entry((asset_in, asset_out))
			.or_insert_with(|| A::discover_routes(asset_in, asset_out, state).unwrap_or_default());
	}

	fn routes(&mut self, asset_in: AssetId, asset_out: AssetId, state: &A::State) -> Vec<Route<AssetId>> {
		self.ensure_routes(asset_in, asset_out, state);
		self.routes.get(&(asset_in, asset_out)).cloned().unwrap_or_default()
	}

	/// Best sell quote (raw simulator output, no haircut) against the fitting state.
	fn quote(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Balance, Route<AssetId>)> {
		if amount_in == 0 {
			return None;
		}
		self.ensure_routes(asset_in, asset_out, state);
		let key = (asset_in, asset_out, amount_in);
		if let Some(cached) = self.quotes.get(&key) {
			let routes = self.routes.get(&(asset_in, asset_out))?;
			return match cached {
				Some((out, idx)) => routes.get(*idx).cloned().map(|r| (*out, r)),
				None => None,
			};
		}
		let routes = self.routes.get(&(asset_in, asset_out))?;
		let mut best: Option<(Balance, usize)> = None;
		for (i, route) in routes.iter().enumerate() {
			if let Ok((_, exec)) = A::sell(asset_in, asset_out, amount_in, route.clone(), state) {
				// `>=` mirrors v2's `max_by_key` (last maximum wins on ties).
				if best.map(|(out, _)| exec.amount_out >= out).unwrap_or(true) {
					best = Some((exec.amount_out, i));
				}
			}
		}
		let result = best.and_then(|(out, i)| {
			self.routes
				.get(&(asset_in, asset_out))?
				.get(i)
				.cloned()
				.map(|r| (out, r))
		});
		self.quotes.insert(key, best);
		result
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

pub struct Solver<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> Solver<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State, matched_fee: Permill) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(empty_solution());
		}

		log::debug!(target: LOG_TARGET, "solve() called with {} intents, matched_fee={:?}", intents.len(), matched_fee);

		let fee_ctx = FeeCtx::new(matched_fee);
		let mut cache = QuoteCache::<A>::new();

		let spot_prices = Self::collect_spot_prices(&intents, &initial_state, &mut cache);

		let candidates: Vec<&Intent> = intents
			.iter()
			.filter(|intent| Self::is_candidate(intent, &spot_prices, &initial_state, &mut cache))
			.collect();

		log::debug!(target: LOG_TARGET, "candidates: {}/{} intents", candidates.len(), intents.len());

		if candidates.is_empty() {
			return Ok(empty_solution());
		}
		if candidates.len() == 1 {
			return Self::solve_single_intent(candidates[0], &initial_state, &mut cache);
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
				limit_n: swap.amount_out,
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
				ed_a: A::existential_deposit(asset_a),
				ed_b: A::existential_deposit(asset_b),
				fee_ctx,
			};
			for (id, fill) in Self::cross_pair(&ctx, fwd, bwd, &initial_state, &mut cache) {
				fills.insert(id, fill);
			}
		}

		if fills.is_empty() {
			log::debug!(target: LOG_TARGET, "no intents survived pair crossing");
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
			let surpluses =
				Self::estimate_surpluses(&included, &fills, &spot_prices, &initial_state, &mut cache, fee_ctx);
			Self::sort_by_surplus_desc(&mut included, &surpluses);
			included.truncate(MAX_NUMBER_OF_RESOLVED_INTENTS as usize);
		}

		if included.len() == 1 {
			let intent = included[0];
			let fill = fills.get(&intent.id).copied().unwrap_or(0);
			return Self::solve_single_intent_with_fill(intent, fill, &initial_state, &mut cache);
		}

		// Stabilization rounds: rings → trades → unified rates → resolution.
		// Fills coming from the crossing are already near-feasible, so this
		// usually converges in round one; later pairs can still drift because
		// trades execute sequentially against a mutating state.
		for round in 0..MAX_STABILIZATION_ROUNDS {
			log::debug!(target: LOG_TARGET, "stabilization round {}, {} included intents", round, included.len());

			let (resolved_intents, executed_trades, total_score) =
				Self::run_round(&included, &fills, &spot_prices, &initial_state, &mut cache, fee_ctx);

			log::debug!(target: LOG_TARGET, "round {}: {} resolved, {} trades, score: {} (from {} included)",
				round, resolved_intents.len(), executed_trades.len(), total_score, included.len());

			if resolved_intents.len() == included.len() {
				return Ok(Solution {
					resolved_intents: ResolvedIntents::truncate_from(resolved_intents),
					trades: SolutionTrades::truncate_from(executed_trades),
					score: total_score,
				});
			}

			let resolved_ids: BTreeSet<IntentId> = resolved_intents.iter().map(|r| r.id).collect();
			included.retain(|intent| resolved_ids.contains(&intent.id));

			if included.is_empty() {
				break;
			}
			if included.len() == 1 {
				let intent = included[0];
				let fill = fills.get(&intent.id).copied().unwrap_or(0);
				return Self::solve_single_intent_with_fill(intent, fill, &initial_state, &mut cache);
			}
		}

		// Rounds exhausted — fall back to the best single-intent solution
		// instead of discarding everything.
		log::warn!(target: LOG_TARGET, "stabilization did not converge after {MAX_STABILIZATION_ROUNDS} rounds; trying single-intent fallback");
		let mut fallback: Vec<&Intent> = candidates.clone();
		let surpluses = Self::estimate_surpluses(&fallback, &fills, &spot_prices, &initial_state, &mut cache, fee_ctx);
		Self::sort_by_surplus_desc(&mut fallback, &surpluses);
		for intent in fallback {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill = fills.get(&intent.id).copied().unwrap_or_else(|| swap.remaining());
			let solution = Self::solve_single_intent_with_fill(intent, fill, &initial_state, &mut cache)?;
			if !solution.resolved_intents.is_empty() {
				return Ok(solution);
			}
		}
		Ok(empty_solution())
	}

	/// Pre-compute spot prices for every asset appearing in the intent set,
	/// denominated in `A::price_denominator()`. Same selection rule as v2:
	/// highest-rate route wins; assets without a viable route are absent.
	fn collect_spot_prices(
		intents: &[Intent],
		state: &A::State,
		cache: &mut QuoteCache<A>,
	) -> BTreeMap<AssetId, Ratio> {
		let denominator = A::price_denominator();
		let mut spot_prices: BTreeMap<AssetId, Ratio> = BTreeMap::new();
		spot_prices.insert(denominator, Ratio::one());

		for asset in common::collect_unique_assets(intents) {
			if asset == denominator {
				continue;
			}
			for route in cache.routes(asset, denominator, state) {
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
		cache: &mut QuoteCache<A>,
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
		if let Some((amount_out, _)) = cache.quote(swap.asset_in, swap.asset_out, remaining, state) {
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
	/// dropped. When reference prices are unknown, both directions are priced
	/// independently through the AMM (no matching).
	fn fit_outputs(
		ctx: &PairCtx,
		v_f: Balance,
		v_b: Balance,
		state: &A::State,
		cache: &mut QuoteCache<A>,
	) -> (Option<Balance>, Option<Balance>) {
		let quote_f = |cache: &mut QuoteCache<A>, amount: Balance| {
			cache
				.quote(ctx.asset_a, ctx.asset_b, amount, state)
				.map(|(out, _)| adjust_amm_output(out))
		};
		let quote_b = |cache: &mut QuoteCache<A>, amount: Balance| {
			cache
				.quote(ctx.asset_b, ctx.asset_a, amount, state)
				.map(|(out, _)| adjust_amm_output(out))
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

		let (Some(pa), Some(pb)) = (ctx.pa.as_ref(), ctx.pb.as_ref()) else {
			// No reference price — price both directions independently via AMM.
			return (quote_f(cache, v_f), quote_b(cache, v_b));
		};

		match common::analyze_pair_flow(v_f, v_b, pa, pb) {
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
	/// feasible fill (partials, once) or removed. Volumes only ratchet down,
	/// so the loop is bounded and rates monotonically improve for survivors.
	fn cross_pair<'a>(
		ctx: &PairCtx,
		mut fwd: Vec<Cand<'a>>,
		mut bwd: Vec<Cand<'a>>,
		state: &A::State,
		cache: &mut QuoteCache<A>,
	) -> Vec<(IntentId, Balance)> {
		fwd.retain(|c| c.fill >= ctx.ed_a.max(1));
		bwd.retain(|c| c.fill >= ctx.ed_b.max(1));
		Self::sort_by_limit_asc(&mut fwd);
		Self::sort_by_limit_asc(&mut bwd);

		let mut trimmed: BTreeSet<IntentId> = BTreeSet::new();
		let max_iters = 2 * (fwd.len() + bwd.len()) + 4;

		for _ in 0..max_iters {
			let v_f: Balance = fwd.iter().map(|c| c.fill).fold(0u128, |acc, v| acc.saturating_add(v));
			let v_b: Balance = bwd.iter().map(|c| c.fill).fold(0u128, |acc, v| acc.saturating_add(v));
			if v_f == 0 && v_b == 0 {
				break;
			}

			let (out_f, out_b) = Self::fit_outputs(ctx, v_f, v_b, state, cache);

			let f_blocked = v_f > 0 && !Self::dir_ok(out_f, v_f, fwd.last());
			let b_blocked = v_b > 0 && !Self::dir_ok(out_b, v_b, bwd.last());

			if !f_blocked && !b_blocked {
				break;
			}

			// Fix forward first (deterministic preference, mirrors v2).
			let (dir, is_fwd, v_dir, v_other) = if f_blocked {
				(&mut fwd, true, v_f, v_b)
			} else {
				(&mut bwd, false, v_b, v_f)
			};
			// dir is non-empty: its direction is blocked, which requires volume.
			let Some(tightest) = dir.last() else {
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

		// Existential-deposit guard on partial remainders: never leave an
		// unfillable dust remainder behind. Reducing a fill only improves the
		// clearing rate, so trimming here cannot invalidate the fit.
		for (cand, ed) in fwd
			.iter_mut()
			.map(|c| (c, ctx.ed_a))
			.chain(bwd.iter_mut().map(|c| (c, ctx.ed_b)))
		{
			if !cand.partial || cand.fill == 0 {
				continue;
			}
			let remaining_after = cand.remaining.saturating_sub(cand.fill);
			if remaining_after > 0 && remaining_after < ed {
				let reduced = cand.remaining.saturating_sub(ed);
				cand.fill = if reduced >= ed { reduced.min(cand.fill) } else { 0 };
			}
		}

		fwd.into_iter()
			.chain(bwd)
			.filter(|c| c.fill > 0)
			.map(|c| (c.intent.id, c.fill))
			.collect()
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
		cache: &mut QuoteCache<A>,
	) -> Option<Balance> {
		let ed_in = if is_fwd { ctx.ed_a } else { ctx.ed_b };
		let mut lo: Balance = ed_in.max(1);
		let mut hi: Balance = max_x;
		let mut best: Option<Balance> = None;

		for _ in 0..MAX_SEARCH_ITERATIONS {
			if lo > hi {
				break;
			}
			let mid = lo.saturating_add(hi) / 2;
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
		cache: &mut QuoteCache<A>,
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
				ed_a: A::existential_deposit(asset_a),
				ed_b: A::existential_deposit(asset_b),
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
		cache: &mut QuoteCache<A>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Route<AssetId>, Balance, A::State)> {
		cache
			.routes(asset_in, asset_out, state)
			.into_iter()
			.filter_map(
				|route| match A::sell(asset_in, asset_out, amount_in, route.clone(), state) {
					Ok((new_state, exec)) => Some((route, exec.amount_out, new_state)),
					Err(_) => None,
				},
			)
			.max_by_key(|(_, amount_out, _)| *amount_out)
	}

	/// One stabilization round: ring detection, sequential trade building,
	/// unified per-direction rates, and resolution. Returns the resolved
	/// intents (a subset of `included`), the trades and the score.
	fn run_round(
		included: &[&Intent],
		fills: &BTreeMap<IntentId, Balance>,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		initial_state: &A::State,
		cache: &mut QuoteCache<A>,
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

			let mut sell_via_amm =
				|sell_asset: AssetId, buy_asset: AssetId, amount: Balance, state: &mut A::State| -> Option<Balance> {
					let (route, amount_out, new_state) =
						Self::best_route_exec(cache, sell_asset, buy_asset, amount, state)?;
					let adjusted_out = adjust_amm_output(amount_out);
					executed_trades.push(PoolTrade {
						direction: SwapType::ExactIn,
						amount_in: amount,
						amount_out: adjusted_out,
						route,
					});
					*state = new_state;
					Some(adjusted_out)
				};

			let (pa, pb) = match (spot_prices.get(&asset_a), spot_prices.get(&asset_b)) {
				(Some(pa), Some(pb)) => (pa, pb),
				_ => {
					// No reference price — both directions execute independently
					// through the AMM, no direct matching.
					if total_a_sold >= A::existential_deposit(asset_a) {
						if let Some(out) = sell_via_amm(asset_a, asset_b, total_a_sold, &mut state) {
							directed_rates.insert((asset_a, asset_b), Ratio::new(out, total_a_sold));
						}
					}
					if total_b_sold >= A::existential_deposit(asset_b) {
						if let Some(out) = sell_via_amm(asset_b, asset_a, total_b_sold, &mut state) {
							directed_rates.insert((asset_b, asset_a), Ratio::new(out, total_b_sold));
						}
					}
					continue;
				}
			};

			match common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb) {
				FlowDirection::SingleForward { amount } => {
					if amount < A::existential_deposit(asset_a) {
						log::debug!(target: LOG_TARGET, "single forward {asset_a} -> {asset_b}: amount {amount} below ED");
					} else if let Some(out) = sell_via_amm(asset_a, asset_b, amount, &mut state) {
						directed_rates.insert((asset_a, asset_b), Ratio::new(out, amount));
					}
				}
				FlowDirection::SingleBackward { amount } => {
					if amount < A::existential_deposit(asset_b) {
						log::debug!(target: LOG_TARGET, "single backward {asset_b} -> {asset_a}: amount {amount} below ED");
					} else if let Some(out) = sell_via_amm(asset_b, asset_a, amount, &mut state) {
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
					if net_sell < A::existential_deposit(asset_a) {
						if total_a_sold > 0 {
							directed_rates.insert(
								(asset_a, asset_b),
								Ratio::new(fee_ctx.apply(direct_match), total_a_sold),
							);
						}
					} else if let Some(amm_out) = sell_via_amm(asset_a, asset_b, net_sell, &mut state) {
						// Matched portion carries the fee; AMM portion does not.
						let total_out = fee_ctx.apply(direct_match).saturating_add(amm_out);
						if total_a_sold > 0 {
							directed_rates.insert((asset_a, asset_b), Ratio::new(total_out, total_a_sold));
						}
					}
					// On AMM failure no forward rate is set — unlike v2 there is
					// no spot-valued fallback: it would promise output the
					// holding pot never receives. Affected intents resolve to 0
					// this round and the stabilization loop retries without them.
				}
				FlowDirection::ExcessBackward {
					scarce_out,
					direct_match,
					net_sell,
				} => {
					if total_a_sold > 0 {
						directed_rates.insert((asset_a, asset_b), Ratio::new(fee_ctx.apply(scarce_out), total_a_sold));
					}
					if net_sell < A::existential_deposit(asset_b) {
						if total_b_sold > 0 {
							directed_rates.insert(
								(asset_b, asset_a),
								Ratio::new(fee_ctx.apply(direct_match), total_b_sold),
							);
						}
					} else if let Some(amm_out) = sell_via_amm(asset_b, asset_a, net_sell, &mut state) {
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

		// Resolution: uniform price per directed pair. The canonical price is
		// anchored on the pair's *largest* fill and that intent is emitted
		// first, so the pallet's first-resolution anchor recomputes the
		// identical price and every smaller fill stays within the ±1
		// tolerance (deviation is bounded by fill_i / fill_anchor ≤ 1).
		// Anchoring on the largest fill also makes payouts independent of
		// intent input order and minimizes rounding loss.
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

				let ed_in = A::existential_deposit(swap.asset_in);
				let ed_out = A::existential_deposit(swap.asset_out);
				if fill < ed_in || total_out < ed_out {
					log::debug!(
						target: LOG_TARGET,
						"intent {}: dropped — fill={} (ed_in={}) or total_out={} (ed_out={}) below ED",
						intent.id, fill, ed_in, total_out, ed_out,
					);
					continue;
				}

				let min_required = apply_rate(fill, U256::from(swap.amount_out), U256::from(swap.amount_in));
				if total_out < min_required {
					log::debug!(target: LOG_TARGET, "intent {}: skipped — output {} < pro_rata_min {} for fill {}",
						intent.id, total_out, min_required, fill);
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

		(resolved_intents, executed_trades, total_score)
	}

	/// Single intent path, supporting partial fills.
	fn solve_single_intent(
		intent: &Intent,
		initial_state: &A::State,
		cache: &mut QuoteCache<A>,
	) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};
		Self::solve_single_intent_with_fill(intent, swap.remaining(), initial_state, cache)
	}

	/// Single intent with a specific fill amount.
	///
	/// Unlike v2, the payout is the *haircut* AMM output (the same amount the
	/// trade claims as its minimum) — paying the raw simulated output risks the
	/// holding pot coming up short when on-chain execution drifts below the
	/// simulation.
	fn solve_single_intent_with_fill(
		intent: &Intent,
		fill: Balance,
		initial_state: &A::State,
		cache: &mut QuoteCache<A>,
	) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};
		if fill == 0 {
			return Ok(empty_solution());
		}

		log::debug!(target: LOG_TARGET, "solving single intent {}: {} -> {}, fill: {}, min_rate: {}/{}",
			intent.id, swap.asset_in, swap.asset_out, fill, swap.amount_out, swap.amount_in);

		let min_n = U256::from(swap.amount_out);
		let min_d = U256::from(swap.amount_in);
		let ed_in = A::existential_deposit(swap.asset_in);
		let ed_out = A::existential_deposit(swap.asset_out);

		let try_fill = |cache: &mut QuoteCache<A>, amount: Balance| -> Option<(Balance, Balance, Route<AssetId>)> {
			let (raw_out, route) = cache.quote(swap.asset_in, swap.asset_out, amount, initial_state)?;
			let net_out = adjust_amm_output(raw_out);
			let pro_rata_min = apply_rate(amount, min_n, min_d);
			if net_out >= pro_rata_min && net_out >= ed_out {
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
					let mid = lo.saturating_add(hi) / 2;
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
		if actual_fill < ed_in || net_out < ed_out {
			return Ok(empty_solution());
		}

		let pro_rata_min = apply_rate(actual_fill, min_n, min_d);
		let surplus = net_out.saturating_sub(pro_rata_min);

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

		Ok(Solution {
			resolved_intents: ResolvedIntents::truncate_from(vec![resolved]),
			trades: SolutionTrades::truncate_from(vec![PoolTrade {
				direction: SwapType::ExactIn,
				amount_in: actual_fill,
				amount_out: net_out,
				route,
			}]),
			score: surplus,
		})
	}
}
