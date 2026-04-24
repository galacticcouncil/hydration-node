//! ICE Solver v2 — Per-Direction Clearing Prices with Partial Fills
//!
//! Extends v1 with variable fill amounts for partial intents.
//! Non-partial intents behave identically to v1 (binary include/exclude).
//!
//! Partial fill algorithm:
//! 1. Include all non-partial intents that pass clearing (same as v1)
//! 2. For each partial intent: binary search for maximum fill amount
//!    where `clearing_rate(total_volume) >= minimum_rate`
//! 3. ED guard: don't leave remaining < existential deposit
//!
//! Everything else (ring detection, AMM trades, unified rates, stabilization)
//! is identical to v1.

use crate::common;
use crate::common::flow_graph;
use crate::common::ring_detection;
use crate::common::FlowDirection;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::AMMInterface;
use hydradx_traits::router::Route;
use ice_support::{
	AssetId, Balance, Intent, IntentData, IntentId, PoolTrade, ResolvedIntent, ResolvedIntents, Solution,
	SolutionTrades, SwapData, SwapType, MAX_NUMBER_OF_RESOLVED_INTENTS,
};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
use sp_std::collections::btree_set::BTreeSet;
use sp_std::marker::PhantomData;
use sp_std::vec;
use sp_std::vec::Vec;

pub struct Solver<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

/// Unordered pair key.
type AssetPair = (AssetId, AssetId);

/// Intents grouped by direction: (forward A→B, backward B→A).
type DirectionGroups<T> = (Vec<T>, Vec<T>);

/// Per-direction clearing rates for an unordered pair (A, B).
#[derive(Debug, Clone)]
struct PairClearing {
	/// A→B direction: rate = n/d (B received per A sold)
	forward_n: U256,
	forward_d: U256,
	/// B→A direction: rate = n/d (A received per B sold)
	backward_n: U256,
	backward_d: U256,
}

/// A resolved intent with its fill amount (may be less than amount_in for partial intents).
#[derive(Debug, Clone)]
struct IntentFill<'a> {
	intent: &'a Intent,
	/// How much of amount_in to fill in this solution.
	fill_amount: Balance,
}

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

fn empty_solution() -> Solution {
	Solution {
		resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
		trades: SolutionTrades::truncate_from(Vec::new()),
		score: 0,
	}
}

fn unordered_pair(a: AssetId, b: AssetId) -> (AssetId, AssetId) {
	if a <= b {
		(a, b)
	} else {
		(b, a)
	}
}

/// Compute `amount_in * n / d` (integer floor), saturating to 0 on overflow or
/// division by zero. A zero denominator is always a bug — by construction every
/// clearing rate has a positive denominator — so we log at `warn!` when it
/// happens to aid diagnosis.
fn apply_rate(amount_in: Balance, n: U256, d: U256) -> Balance {
	if d.is_zero() {
		log::warn!(
			target: "solver::v2",
			"apply_rate called with zero denominator (amount_in={amount_in}, n={n}); returning 0",
		);
		return 0;
	}
	common::mul_div(U256::from(amount_in), n, d)
		.and_then(|v| v.try_into().ok())
		.unwrap_or(0)
}

/// Same tolerance as v1.
const AMM_SIMULATION_TOLERANCE_BPS: Balance = 1;

fn adjust_amm_output(simulated_out: Balance) -> Balance {
	simulated_out.saturating_sub(simulated_out * AMM_SIMULATION_TOLERANCE_BPS / 10_000)
}

/// Compute minimum rate for an intent: amount_out / amount_in.
/// Uses original (immutable) values, not remaining.
fn min_rate(swap: &SwapData) -> (U256, U256) {
	(U256::from(swap.amount_out), U256::from(swap.amount_in))
}

impl<A: AMMInterface> Solver<A> {
	fn select_best_route(
		routes: &[Route<AssetId>],
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Route<AssetId>, Balance, A::State)> {
		let best = routes
			.iter()
			.filter_map(
				|route| match A::sell(asset_in, asset_out, amount_in, route.clone(), state) {
					Ok((new_state, exec)) => Some((route.clone(), exec.amount_out, new_state)),
					Err(_) => None,
				},
			)
			.max_by_key(|(_, amount_out, _)| *amount_out);

		if let Some((ref route, amount_out, _)) = best {
			log::debug!(target: "solver::v2", "best route for {} -> {}: {} hops, amount_out: {}",
				asset_in, asset_out, route.as_slice().len(), amount_out);
		}
		best
	}

	/// Get the effective amount to use for an intent in flow calculations.
	/// For partial intents, this is the remaining (unfilled) amount.
	fn effective_amount(swap: &SwapData) -> Balance {
		swap.remaining()
	}

	/// Pre-compute spot prices for every asset appearing in the intent set,
	/// denominated in `A::price_denominator()`. Each asset is priced via the
	/// highest-rate available route to the denominator. Assets without a viable
	/// route are simply absent from the returned map; callers fall back to
	/// simulation or conservatively reject such intents.
	fn collect_spot_prices(intents: &[Intent], state: &A::State) -> BTreeMap<AssetId, Ratio> {
		let denominator = A::price_denominator();
		let mut spot_prices: BTreeMap<AssetId, Ratio> = BTreeMap::new();
		spot_prices.insert(denominator, Ratio::one());

		let assets = common::collect_unique_assets(intents);
		for asset in assets {
			if asset == denominator {
				continue;
			}
			let Ok(price_routes) = A::discover_routes(asset, denominator, state) else {
				continue;
			};
			for route in price_routes {
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
	/// Preference order:
	/// 1. Non-swap intents are dropped.
	/// 2. Intents with zero effective amount (fully filled partials) are dropped.
	/// 3. If route simulation at the effective amount meets the pro-rata minimum,
	///    the intent is kept — this is authoritative and avoids relying on spot.
	/// 4. If simulation fails but the intent is partial, keep it; joint fit will
	///    find a smaller viable fill.
	/// 5. Otherwise fall back to spot-price feasibility. An intent with an unknown
	///    spot price for either leg is rejected conservatively.
	fn is_satisfiable(intent: &Intent, spot_prices: &BTreeMap<AssetId, Ratio>, state: &A::State) -> bool {
		let IntentData::Swap(swap) = &intent.data else {
			return false;
		};
		let check_amount = Self::effective_amount(swap);
		if check_amount == 0 {
			log::debug!(target: "solver::v2", "intent {}: fully filled, skipping", intent.id);
			return false;
		}

		if let Ok(routes) = A::discover_routes(swap.asset_in, swap.asset_out, state) {
			if let Some((_, amount_out, _)) =
				Self::select_best_route(&routes, swap.asset_in, swap.asset_out, check_amount, state)
			{
				let pro_rata_min = apply_rate(check_amount, U256::from(swap.amount_out), U256::from(swap.amount_in));
				if amount_out >= pro_rata_min {
					return true;
				}
				log::debug!(target: "solver::v2", "intent {}: route output {} < pro_rata_min {} for {} -> {}",
					intent.id, amount_out, pro_rata_min, swap.asset_in, swap.asset_out);
			}
		}

		if swap.partial.is_partial() {
			// Partials can fit a smaller fill — joint fit will decide.
			return true;
		}

		let ok = common::is_satisfiable(intent, spot_prices);
		if !ok {
			log::debug!(target: "solver::v2", "intent {}: unsatisfiable at spot price", intent.id);
		}
		ok
	}

	/// Split satisfiable intents into partial and non-partial `IntentFill`s, each
	/// seeded with its effective (unfilled) amount. Intents with zero effective
	/// amount are dropped.
	fn initial_fill_plan<'a>(satisfiable: &[&'a Intent]) -> (Vec<IntentFill<'a>>, Vec<IntentFill<'a>>) {
		let mut non_partial_fills: Vec<IntentFill<'a>> = Vec::new();
		let mut partial_fills: Vec<IntentFill<'a>> = Vec::new();
		for &intent in satisfiable {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let fill_amount = Self::effective_amount(swap);
			if fill_amount == 0 {
				continue;
			}
			let fill = IntentFill { intent, fill_amount };
			if swap.partial.is_partial() {
				partial_fills.push(fill);
			} else {
				non_partial_fills.push(fill);
			}
		}
		(non_partial_fills, partial_fills)
	}

	/// Iteratively remove non-partial fills whose per-direction clearing output
	/// falls below the intent's absolute `amount_out` minimum. Converges quickly
	/// because each round can only drop intents; the clearing rate then improves
	/// (less volume through the AMM) for the survivors.
	fn stabilize_non_partials<'a>(
		non_partial_fills: &mut Vec<IntentFill<'a>>,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) {
		const MAX_ITERATIONS: u32 = 10;
		let mut pair_clearings: BTreeMap<AssetPair, PairClearing> = BTreeMap::new();

		for _iteration in 0..MAX_ITERATIONS {
			pair_clearings.clear();

			let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<&IntentFill<'_>>> = BTreeMap::new();
			for fill in non_partial_fills.iter() {
				let IntentData::Swap(swap) = &fill.intent.data else {
					continue;
				};
				let up = unordered_pair(swap.asset_in, swap.asset_out);
				let entry = pair_groups.entry(up).or_default();
				if swap.asset_in == up.0 {
					entry.0.push(fill);
				} else {
					entry.1.push(fill);
				}
			}

			for (&(asset_a, asset_b), (forward, backward)) in &pair_groups {
				if let Some(c) =
					Self::compute_pair_clearing_with_fills(asset_a, asset_b, forward, backward, spot_prices, state)
				{
					pair_clearings.insert((asset_a, asset_b), c);
				}
			}

			let before_count = non_partial_fills.len();
			non_partial_fills.retain(|fill| {
				let IntentData::Swap(swap) = &fill.intent.data else {
					return true;
				};
				let up = unordered_pair(swap.asset_in, swap.asset_out);
				let Some(clearing) = pair_clearings.get(&up) else {
					return true;
				};
				let (rate_n, rate_d) = if swap.asset_in == up.0 {
					(clearing.forward_n, clearing.forward_d)
				} else {
					(clearing.backward_n, clearing.backward_d)
				};
				let amount_out = apply_rate(fill.fill_amount, rate_n, rate_d);
				if amount_out < swap.amount_out {
					log::debug!(target: "solver::v2", "intent {}: filtered out — clearing output {} < min_out {}",
						fill.intent.id, amount_out, swap.amount_out);
					return false;
				}
				true
			});

			if non_partial_fills.len() == before_count {
				break;
			}
		}
	}

	pub fn solve(intents: Vec<Intent>, initial_state: A::State) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(empty_solution());
		}

		log::debug!(target: "solver::v2", "solve() called with {} intents", intents.len());

		// 1. Pre-compute spot prices once for every asset that appears in any intent.
		let spot_prices = Self::collect_spot_prices(&intents, &initial_state);

		// 2. Filter satisfiable intents. The simulation step is authoritative — if the
		// AMM can fulfil the (pro-rata) minimum at the intent's effective volume, keep
		// the intent. Only fall back to the spot-price check when simulation fails.
		let satisfiable_intents: Vec<&Intent> = intents
			.iter()
			.filter(|intent| Self::is_satisfiable(intent, &spot_prices, &initial_state))
			.collect();

		log::debug!(target: "solver::v2", "satisfiable: {}/{} intents", satisfiable_intents.len(), intents.len());

		if satisfiable_intents.is_empty() {
			return Ok(empty_solution());
		}

		if satisfiable_intents.len() == 1 {
			return Self::solve_single_intent(satisfiable_intents[0], &initial_state);
		}

		// 3. Build the initial fill plan and split by partial/non-partial.
		let (mut non_partial_fills, partial_fills) = Self::initial_fill_plan(&satisfiable_intents);

		// Phase A: iteratively drop non-partials that fail at the combined clearing rate.
		Self::stabilize_non_partials(&mut non_partial_fills, &spot_prices, &initial_state);

		// Phase B: joint per-pair partial-fill clearing. For each unordered pair (A, B),
		// binary-search a single scale factor `t ∈ [0,1]` applied uniformly to the
		// partials in both directions. The clearing rate at
		// (fixed_f + t·V_f_max, fixed_b + t·V_b_max) is monotonic in `t`, so we find the
		// largest `t` where both directions' clearing rates still satisfy the *tightest*
		// per-direction pro-rata minimum. Each partial then gets `remaining() · t`, which
		// restores same-direction-same-fill-fraction and removes the order dependence of
		// the previous per-partial sequential fit.
		let mut fills = non_partial_fills;
		Self::fit_partials_jointly(&mut fills, partial_fills, &spot_prices, &initial_state);

		if fills.is_empty() {
			log::debug!(target: "solver::v2", "all intents filtered out during iterative clearing");
			return Ok(empty_solution());
		}

		log::debug!(target: "solver::v2", "after iterative clearing: {} fills remaining", fills.len());

		// Cap to MAX_NUMBER_OF_RESOLVED_INTENTS. `ResolvedIntents::truncate_from` would
		// silently drop any overflow after score is computed, so we have to truncate up
		// front. Sort by estimated surplus descending first so the N best intents — not
		// just the first N by input order — survive the cap.
		if fills.len() > MAX_NUMBER_OF_RESOLVED_INTENTS as usize {
			log::debug!(target: "solver::v2", "capping fills from {} to {} (keeping highest surplus)",
				fills.len(), MAX_NUMBER_OF_RESOLVED_INTENTS);
			Self::sort_by_estimated_surplus(&mut fills, &spot_prices, &initial_state);
			fills.truncate(MAX_NUMBER_OF_RESOLVED_INTENTS as usize);
		}

		// Convert fills to included intents for the rest of the pipeline
		// (ring detection, AMM trades, unified rates, resolution)
		let mut included: Vec<&Intent> = fills.iter().map(|f| f.intent).collect();
		// Track fill amounts separately for resolution
		let fill_amounts: BTreeMap<IntentId, Balance> = fills.iter().map(|f| (f.intent.id, f.fill_amount)).collect();

		if included.len() == 1 {
			let intent = included[0];
			let fill = fill_amounts.get(&intent.id).copied().unwrap_or(0);
			return Self::solve_single_intent_with_fill(intent, fill, &initial_state);
		}

		// Stabilization loop: ring detection → AMM trades → unified rates → resolution.
		// Intents dropped during resolution trigger a retry with the reduced set.
		const MAX_STABILIZATION_ROUNDS: u32 = 5;

		for stabilization_round in 0..MAX_STABILIZATION_ROUNDS {
			log::debug!(target: "solver::v2", "stabilization round {}, {} included intents",
				stabilization_round, included.len());

			// Ring detection — cap each intent's volume at its solver-decided fill_amount
			// (falling back to `swap.remaining()` for anything that somehow isn't in
			// fill_amounts). Without this, ring detection could match more volume than
			// the user has reserved or the solver has allocated.
			let graph_entries: Vec<(&Intent, Balance)> = included
				.iter()
				.map(|intent| {
					let cap = match &intent.data {
						IntentData::Swap(swap) => fill_amounts
							.get(&intent.id)
							.copied()
							.unwrap_or_else(|| swap.remaining()),
						_ => 0,
					};
					(*intent, cap)
				})
				.collect();
			let mut graph = flow_graph::build_flow_graph(&graph_entries);
			let rings = ring_detection::detect_rings(&mut graph, &spot_prices);

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

			// AMM trades for net imbalances
			let mut state = initial_state.clone();
			let mut executed_trades: Vec<PoolTrade> = Vec::new();

			let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<(IntentId, &SwapData)>> = BTreeMap::new();
			for intent in &included {
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

			let mut directed_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();

			for (&(asset_a, asset_b), (forward, backward)) in &pair_groups {
				// Use fill_amounts for volume calculation instead of raw amount_in
				let total_a_sold: Balance = forward
					.iter()
					.map(|(id, swap)| {
						let base = fill_amounts.get(id).copied().unwrap_or(swap.remaining());
						base.saturating_sub(ring_fills.get(id).map(|(a, _)| *a).unwrap_or(0))
					})
					.sum();

				let total_b_sold: Balance = backward
					.iter()
					.map(|(id, swap)| {
						let base = fill_amounts.get(id).copied().unwrap_or(swap.remaining());
						base.saturating_sub(ring_fills.get(id).map(|(a, _)| *a).unwrap_or(0))
					})
					.sum();

				if total_a_sold == 0 && total_b_sold == 0 {
					continue;
				}

				let Some(pa) = spot_prices.get(&asset_a) else {
					continue;
				};
				let Some(pb) = spot_prices.get(&asset_b) else {
					continue;
				};

				let flow = common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb);

				match flow {
					FlowDirection::SingleForward { amount } => {
						if amount < A::existential_deposit(asset_a) {
							log::debug!(target: "solver::v2", "single forward {asset_a} -> {asset_b}: amount {amount} below ED");
						} else if let Some((route, amount_out, new_state)) =
							A::discover_routes(asset_a, asset_b, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(&routes, asset_a, asset_b, amount, &state))
						{
							let adjusted_out = adjust_amm_output(amount_out);
							directed_rates.insert((asset_a, asset_b), Ratio::new(adjusted_out, amount));
							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: amount,
								amount_out: adjusted_out,
								route,
							});
							state = new_state;
						}
					}
					FlowDirection::SingleBackward { amount } => {
						if amount < A::existential_deposit(asset_b) {
							log::debug!(target: "solver::v2", "single backward {asset_b} -> {asset_a}: amount {amount} below ED");
						} else if let Some((route, amount_out, new_state)) =
							A::discover_routes(asset_b, asset_a, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(&routes, asset_b, asset_a, amount, &state))
						{
							let adjusted_out = adjust_amm_output(amount_out);
							directed_rates.insert((asset_b, asset_a), Ratio::new(adjusted_out, amount));
							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: amount,
								amount_out: adjusted_out,
								route,
							});
							state = new_state;
						}
					}
					FlowDirection::ExcessForward {
						scarce_out,
						direct_match,
						net_sell,
					} => {
						if total_b_sold > 0 {
							directed_rates.insert((asset_b, asset_a), Ratio::new(scarce_out, total_b_sold));
						}
						if net_sell < A::existential_deposit(asset_a) {
							if total_a_sold > 0 {
								directed_rates.insert((asset_a, asset_b), Ratio::new(direct_match, total_a_sold));
							}
						} else {
							let best = A::discover_routes(asset_a, asset_b, &state).ok().and_then(|routes| {
								Self::select_best_route(&routes, asset_a, asset_b, net_sell, &state)
							});
							match best {
								Some((route, amount_out, new_state)) => {
									let adjusted_out = adjust_amm_output(amount_out);
									let total_out = direct_match.saturating_add(adjusted_out);
									if total_a_sold > 0 {
										directed_rates.insert((asset_a, asset_b), Ratio::new(total_out, total_a_sold));
									}
									executed_trades.push(PoolTrade {
										direction: SwapType::ExactIn,
										amount_in: net_sell,
										amount_out: adjusted_out,
										route,
									});
									state = new_state;
								}
								None => {
									let fallback = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);
									if total_a_sold > 0 {
										directed_rates.insert((asset_a, asset_b), Ratio::new(fallback, total_a_sold));
									}
								}
							}
						}
					}
					FlowDirection::ExcessBackward {
						scarce_out,
						direct_match,
						net_sell,
					} => {
						if total_a_sold > 0 {
							directed_rates.insert((asset_a, asset_b), Ratio::new(scarce_out, total_a_sold));
						}
						if net_sell < A::existential_deposit(asset_b) {
							if total_b_sold > 0 {
								directed_rates.insert((asset_b, asset_a), Ratio::new(direct_match, total_b_sold));
							}
						} else {
							let best = A::discover_routes(asset_b, asset_a, &state).ok().and_then(|routes| {
								Self::select_best_route(&routes, asset_b, asset_a, net_sell, &state)
							});
							match best {
								Some((route, amount_out, new_state)) => {
									let adjusted_out = adjust_amm_output(amount_out);
									let total_out = direct_match.saturating_add(adjusted_out);
									if total_b_sold > 0 {
										directed_rates.insert((asset_b, asset_a), Ratio::new(total_out, total_b_sold));
									}
									executed_trades.push(PoolTrade {
										direction: SwapType::ExactIn,
										amount_in: net_sell,
										amount_out: adjusted_out,
										route,
									});
									state = new_state;
								}
								None => {
									let fallback = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
									if total_b_sold > 0 {
										directed_rates.insert((asset_b, asset_a), Ratio::new(fallback, total_b_sold));
									}
								}
							}
						}
					}
					FlowDirection::PerfectCancel { a_as_b, b_as_a } => {
						if total_a_sold > 0 {
							directed_rates.insert((asset_a, asset_b), Ratio::new(a_as_b, total_a_sold));
						}
						if total_b_sold > 0 {
							directed_rates.insert((asset_b, asset_a), Ratio::new(b_as_a, total_b_sold));
						}
					}
				}
			}

			// Unified rates
			let mut unified_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
			{
				let mut accum: BTreeMap<AssetPair, DirAccum> = BTreeMap::new();

				for intent in &included {
					let IntentData::Swap(swap) = &intent.data else {
						continue;
					};
					let key = (swap.asset_in, swap.asset_out);
					let entry = accum.entry(key).or_default();
					let fill = fill_amounts.get(&intent.id).copied().unwrap_or(swap.remaining());
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

					let total_out = dir.ring_out.saturating_add(amm_out);
					if dir.total_in > 0 && total_out > 0 {
						unified_rates.insert(*key, Ratio::new(total_out, dir.total_in));
					}
				}
			}

			// Resolve intents using unified rate and fill amounts
			let mut canonical_prices: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
			let mut resolved_intents: Vec<ResolvedIntent> = Vec::new();
			let mut total_score: Balance = 0;

			for intent in &included {
				let IntentData::Swap(swap) = &intent.data else {
					continue;
				};
				let directed_key = (swap.asset_in, swap.asset_out);
				let fill = fill_amounts.get(&intent.id).copied().unwrap_or(swap.remaining());

				let total_out = if let Some(canonical) = canonical_prices.get(&directed_key) {
					apply_rate(fill, U256::from(canonical.n), U256::from(canonical.d))
				} else if let Some(rate) = unified_rates.get(&directed_key) {
					let amount_out = apply_rate(fill, U256::from(rate.n), U256::from(rate.d));
					if fill > 0 && amount_out > 0 {
						canonical_prices.insert(directed_key, Ratio::new(amount_out, fill));
					}
					amount_out
				} else {
					0
				};

				if fill == 0 || total_out == 0 {
					continue;
				}

				// Existential-deposit guard. A resolved intent whose `amount_in`
				// or `amount_out` is below its asset's ED is rejected on-chain
				// with `InvalidAmount` — so the solver must drop it here. The
				// stabilization loop will retry without this intent and the
				// clearing rate on this pair will improve for the survivors.
				let ed_in = A::existential_deposit(swap.asset_in);
				let ed_out = A::existential_deposit(swap.asset_out);
				if fill < ed_in || total_out < ed_out {
					log::debug!(
						target: "solver::v2",
						"intent {}: dropped — fill={} (ed_in={}) or total_out={} (ed_out={}) below ED",
						intent.id, fill, ed_in, total_out, ed_out,
					);
					continue;
				}

				// Pro-rata minimum for this fill amount
				let min_required = apply_rate(fill, U256::from(swap.amount_out), U256::from(swap.amount_in));

				if total_out < min_required {
					log::debug!(target: "solver::v2", "intent {}: skipped — output {} < pro_rata_min {} for fill {}",
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

			log::debug!(target: "solver::v2", "stabilization round {}: {} resolved, {} trades, score: {} (from {} included)",
				stabilization_round, resolved_intents.len(), executed_trades.len(), total_score, included.len());

			if resolved_intents.len() == included.len() {
				return Ok(Solution {
					resolved_intents: ResolvedIntents::truncate_from(resolved_intents),
					trades: SolutionTrades::truncate_from(executed_trades),
					score: total_score,
				});
			}

			// Shrink and retry
			let resolved_ids: BTreeSet<IntentId> = resolved_intents.iter().map(|r| r.id).collect();
			included.retain(|intent| resolved_ids.contains(&intent.id));

			log::debug!(target: "solver::v2", "stabilization round {}: dropped intents, {} remaining",
				stabilization_round, included.len());

			if included.is_empty() {
				return Ok(empty_solution());
			}
			if included.len() == 1 {
				let intent = included[0];
				let fill = fill_amounts.get(&intent.id).copied().unwrap_or(0);
				return Self::solve_single_intent_with_fill(intent, fill, &initial_state);
			}
		}

		log::warn!(target: "solver::v2", "stabilization did not converge after {MAX_STABILIZATION_ROUNDS} rounds");
		Ok(empty_solution())
	}

	/// Single intent path, supporting partial fills.
	fn solve_single_intent(intent: &Intent, initial_state: &A::State) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};
		let fill = Self::effective_amount(swap);
		Self::solve_single_intent_with_fill(intent, fill, initial_state)
	}

	/// Single intent with a specific fill amount.
	fn solve_single_intent_with_fill(
		intent: &Intent,
		fill: Balance,
		initial_state: &A::State,
	) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};

		if fill == 0 {
			return Ok(empty_solution());
		}

		log::debug!(target: "solver::v2", "solving single intent {}: {} -> {}, fill: {}, min_rate: {}/{}",
			intent.id, swap.asset_in, swap.asset_out, fill, swap.amount_out, swap.amount_in);

		let routes = A::discover_routes(swap.asset_in, swap.asset_out, initial_state)?;

		// For partial intents, try the fill amount first. If it doesn't meet minimum,
		// binary search for the maximum fill that does.
		let result = if swap.partial.is_partial() {
			Self::find_best_partial_fill(swap, fill, &routes, initial_state)
		} else {
			// Non-partial: try full amount or nothing
			let Some((route, amount_out, _)) =
				Self::select_best_route(&routes, swap.asset_in, swap.asset_out, fill, initial_state)
			else {
				return Ok(empty_solution());
			};
			if amount_out < swap.amount_out {
				return Ok(empty_solution());
			}
			Some((fill, amount_out, route))
		};

		let Some((actual_fill, amount_out, route)) = result else {
			return Ok(empty_solution());
		};

		let ed_out = A::existential_deposit(swap.asset_out);
		if amount_out < ed_out {
			return Ok(empty_solution());
		}

		let pro_rata_min = apply_rate(actual_fill, U256::from(swap.amount_out), U256::from(swap.amount_in));
		let surplus = amount_out.saturating_sub(pro_rata_min);

		let resolved = ResolvedIntent {
			id: intent.id,
			data: IntentData::Swap(SwapData {
				asset_in: swap.asset_in,
				asset_out: swap.asset_out,
				amount_in: actual_fill,
				amount_out,
				partial: swap.partial,
			}),
		};

		Ok(Solution {
			resolved_intents: ResolvedIntents::truncate_from(vec![resolved]),
			trades: SolutionTrades::truncate_from(vec![PoolTrade {
				direction: SwapType::ExactIn,
				amount_in: actual_fill,
				amount_out: adjust_amm_output(amount_out),
				route,
			}]),
			score: surplus,
		})
	}

	/// Binary search for the maximum partial fill amount where AMM output meets the minimum rate.
	/// Returns (fill_amount, amount_out, route) or None if no fill is possible.
	fn find_best_partial_fill(
		swap: &SwapData,
		max_fill: Balance,
		routes: &[Route<AssetId>],
		state: &A::State,
	) -> Option<(Balance, Balance, Route<AssetId>)> {
		let ed = A::existential_deposit(swap.asset_in);
		let ed_out = A::existential_deposit(swap.asset_out);
		let (min_n, min_d) = min_rate(swap);

		// First try the full remaining amount
		if let Some((route, amount_out, _)) =
			Self::select_best_route(routes, swap.asset_in, swap.asset_out, max_fill, state)
		{
			let pro_rata_min = apply_rate(max_fill, min_n, min_d);
			if amount_out >= pro_rata_min && amount_out >= ed_out {
				return Some((max_fill, amount_out, route));
			}
		}

		// Binary search: find max fill where output meets min rate
		let mut lo: Balance = ed; // minimum meaningful fill
		let mut hi: Balance = max_fill;
		let mut best: Option<(Balance, Balance, Route<AssetId>)> = None;

		const MAX_SEARCH_ITERATIONS: u32 = 20;
		for _ in 0..MAX_SEARCH_ITERATIONS {
			if lo > hi {
				break;
			}
			let mid = lo.saturating_add(hi) / 2;
			if mid < ed {
				break;
			}

			if let Some((route, amount_out, _)) =
				Self::select_best_route(routes, swap.asset_in, swap.asset_out, mid, state)
			{
				let pro_rata_min = apply_rate(mid, min_n, min_d);
				if amount_out >= pro_rata_min && amount_out >= ed_out {
					best = Some((mid, amount_out, route));
					lo = mid.saturating_add(1); // try larger
				} else {
					hi = mid.saturating_sub(1); // too much volume
				}
			} else {
				hi = mid.saturating_sub(1);
			}

			if hi.saturating_sub(lo) < ed {
				break; // convergence within ED precision
			}
		}

		// ED guard: ensure remaining amount is either zero or large enough to be
		// tradeable in the next round. "Tradeable" means both:
		//   (a) remaining >= ed (input asset ED) — won't be dust in the user's account
		//   (b) remaining can produce output >= ed_out — the next fill won't be blocked
		// If remaining fails either check, try filling everything instead.
		if let Some((fill, _, ref route)) = best {
			let remaining_after = max_fill.saturating_sub(fill);
			let remaining_untradeable = if remaining_after == 0 {
				false
			} else if remaining_after < ed {
				true
			} else {
				// Check if remaining can produce output above ed_out
				let remaining_out = apply_rate(remaining_after, min_n, min_d);
				remaining_out < ed_out
			};

			if remaining_untradeable {
				// Try filling everything
				let fill_all = max_fill;
				if let Some((_, all_out, _)) =
					Self::select_best_route(routes, swap.asset_in, swap.asset_out, fill_all, state)
				{
					let pro_rata_min = apply_rate(fill_all, min_n, min_d);
					if all_out >= pro_rata_min && all_out >= ed_out {
						return Some((fill_all, all_out, route.clone()));
					}
				}
				// Can't fill all — reduce to keep remaining >= ED
				let reduced = max_fill.saturating_sub(ed);
				if reduced >= ed {
					if let Some((route, out, _)) =
						Self::select_best_route(routes, swap.asset_in, swap.asset_out, reduced, state)
					{
						let pro_rata_min = apply_rate(reduced, min_n, min_d);
						if out >= pro_rata_min && out >= ed_out {
							return Some((reduced, out, route));
						}
					}
				}
			}
		}

		best
	}

	/// Compute clearing rates from summed per-direction volumes.
	fn compute_pair_clearing_from_totals(
		asset_a: AssetId,
		asset_b: AssetId,
		total_a_sold: Balance,
		total_b_sold: Balance,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) -> Option<PairClearing> {
		if total_a_sold == 0 && total_b_sold == 0 {
			return None;
		}

		let pa = spot_prices.get(&asset_a)?;
		let pb = spot_prices.get(&asset_b)?;

		let flow = common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb);

		match flow {
			FlowDirection::SingleForward { amount } => {
				let routes = A::discover_routes(asset_a, asset_b, state).ok()?;
				let (_, amount_out, _) = Self::select_best_route(&routes, asset_a, asset_b, amount, state)?;
				let adjusted_out = adjust_amm_output(amount_out);
				Some(PairClearing {
					forward_n: U256::from(adjusted_out),
					forward_d: U256::from(amount),
					backward_n: U256::zero(),
					backward_d: U256::one(),
				})
			}
			FlowDirection::SingleBackward { amount } => {
				let routes = A::discover_routes(asset_b, asset_a, state).ok()?;
				let (_, amount_out, _) = Self::select_best_route(&routes, asset_b, asset_a, amount, state)?;
				let adjusted_out = adjust_amm_output(amount_out);
				Some(PairClearing {
					forward_n: U256::zero(),
					forward_d: U256::one(),
					backward_n: U256::from(adjusted_out),
					backward_d: U256::from(amount),
				})
			}
			FlowDirection::ExcessForward {
				scarce_out,
				direct_match,
				net_sell,
			} => {
				let routes = A::discover_routes(asset_a, asset_b, state).ok()?;
				let (_, amount_out, _) = Self::select_best_route(&routes, asset_a, asset_b, net_sell, state)?;
				let adjusted_out = adjust_amm_output(amount_out);
				Some(PairClearing {
					forward_n: U256::from(direct_match.saturating_add(adjusted_out)),
					forward_d: U256::from(total_a_sold),
					backward_n: U256::from(scarce_out),
					backward_d: U256::from(total_b_sold),
				})
			}
			FlowDirection::ExcessBackward {
				scarce_out,
				direct_match,
				net_sell,
			} => {
				let routes = A::discover_routes(asset_b, asset_a, state).ok()?;
				let (_, amount_out, _) = Self::select_best_route(&routes, asset_b, asset_a, net_sell, state)?;
				let adjusted_out = adjust_amm_output(amount_out);
				Some(PairClearing {
					forward_n: U256::from(scarce_out),
					forward_d: U256::from(total_a_sold),
					backward_n: U256::from(direct_match.saturating_add(adjusted_out)),
					backward_d: U256::from(total_b_sold),
				})
			}
			FlowDirection::PerfectCancel { a_as_b, b_as_a } => Some(PairClearing {
				forward_n: U256::from(a_as_b),
				forward_d: U256::from(total_a_sold),
				backward_n: U256::from(b_as_a),
				backward_d: U256::from(total_b_sold),
			}),
		}
	}

	/// Compute clearing rates using fill amounts (not raw amount_in).
	fn compute_pair_clearing_with_fills(
		asset_a: AssetId,
		asset_b: AssetId,
		forward: &[&IntentFill],
		backward: &[&IntentFill],
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) -> Option<PairClearing> {
		if forward.is_empty() && backward.is_empty() {
			return None;
		}
		let total_a_sold: Balance = forward.iter().map(|f| f.fill_amount).sum();
		let total_b_sold: Balance = backward.iter().map(|f| f.fill_amount).sum();
		Self::compute_pair_clearing_from_totals(asset_a, asset_b, total_a_sold, total_b_sold, spot_prices, state)
	}

	/// Joint per-pair partial-fill fit.
	///
	/// For each pair (A,B) with partials in either direction, binary-search the
	/// largest `t ∈ [0, 1]` (represented as `u64 ∈ [0, GRANULARITY]`) such that
	/// the clearing rate at `(fixed_f + t·V_f_max, fixed_b + t·V_b_max)` still
	/// meets the tightest per-direction minimum rate. Then each partial receives
	/// `remaining() · t`, which preserves same-direction-same-fill-fraction and
	/// makes the fit independent of input order.
	fn fit_partials_jointly<'a>(
		fills: &mut Vec<IntentFill<'a>>,
		partial_fills: Vec<IntentFill<'a>>,
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) {
		// Group partials by unordered pair.
		let mut partials_by_pair: BTreeMap<AssetPair, Vec<IntentFill<'a>>> = BTreeMap::new();
		for pf in partial_fills {
			let IntentData::Swap(s) = &pf.intent.data else {
				continue;
			};
			let up = unordered_pair(s.asset_in, s.asset_out);
			partials_by_pair.entry(up).or_default().push(pf);
		}

		for (pair, pair_partials) in partials_by_pair {
			let (asset_a, asset_b) = pair;

			// Split partials by direction.
			let mut f_partials: Vec<IntentFill<'a>> = Vec::new();
			let mut b_partials: Vec<IntentFill<'a>> = Vec::new();
			for pf in pair_partials {
				let IntentData::Swap(s) = &pf.intent.data else {
					continue;
				};
				if s.asset_in == asset_a {
					f_partials.push(pf);
				} else {
					b_partials.push(pf);
				}
			}

			// Fixed (non-partial) volumes in each direction.
			let fixed_f: Balance = fills
				.iter()
				.filter_map(|f| match &f.intent.data {
					IntentData::Swap(s) if s.asset_in == asset_a && s.asset_out == asset_b => Some(f.fill_amount),
					_ => None,
				})
				.fold(0u128, |acc, v| acc.saturating_add(v));
			let fixed_b: Balance = fills
				.iter()
				.filter_map(|f| match &f.intent.data {
					IntentData::Swap(s) if s.asset_in == asset_b && s.asset_out == asset_a => Some(f.fill_amount),
					_ => None,
				})
				.fold(0u128, |acc, v| acc.saturating_add(v));

			const GRANULARITY: u64 = 1_000_000_000;
			const MAX_BINARY_SEARCH_ITER: u32 = 30;

			// Iterative drop loop: when the joint fit would set best_t=0, the
			// partial with the highest demanded rate in the blocking direction
			// is the cause. Drop it and retry; otherwise a single unreachable
			// intent poisons every other partial on the pair.
			let max_drop_rounds = f_partials.len() + b_partials.len() + 1;
			let mut best_t: u64 = 0;
			let mut v_f_max: Balance = 0;
			let mut v_b_max: Balance = 0;

			for _round in 0..max_drop_rounds {
				v_f_max = f_partials
					.iter()
					.map(|p| p.fill_amount)
					.fold(0u128, |acc, v| acc.saturating_add(v));
				v_b_max = b_partials
					.iter()
					.map(|p| p.fill_amount)
					.fold(0u128, |acc, v| acc.saturating_add(v));

				if v_f_max == 0 && v_b_max == 0 {
					break;
				}

				// Tightest per-direction minimum rate (n/d) — the highest
				// `amount_out/amount_in` demanded by any partial in that
				// direction. A single unreachable rate forces best_t = 0 below.
				let tight_f = Self::tightest_rate(&f_partials);
				let tight_b = Self::tightest_rate(&b_partials);

				// Binary search over `t`.
				let mut lo: u64 = 0;
				let mut hi: u64 = GRANULARITY;
				best_t = 0;

				for _ in 0..MAX_BINARY_SEARCH_ITER {
					if lo > hi {
						break;
					}
					let mid = lo.saturating_add(hi) / 2;
					let v_f = Self::scale_by_t(v_f_max, mid, GRANULARITY);
					let v_b = Self::scale_by_t(v_b_max, mid, GRANULARITY);

					let total_f = fixed_f.saturating_add(v_f);
					let total_b = fixed_b.saturating_add(v_b);

					let meets = if total_f == 0 && total_b == 0 {
						true
					} else if let Some(c) =
						Self::compute_pair_clearing_from_totals(asset_a, asset_b, total_f, total_b, spot_prices, state)
					{
						let f_ok = match tight_f {
							Some((tn, td)) => c.forward_n.saturating_mul(td) >= tn.saturating_mul(c.forward_d),
							None => true,
						};
						let b_ok = match tight_b {
							Some((tn, td)) => c.backward_n.saturating_mul(td) >= tn.saturating_mul(c.backward_d),
							None => true,
						};
						f_ok && b_ok
					} else {
						false
					};

					if meets {
						best_t = mid;
						lo = mid.saturating_add(1);
					} else {
						hi = mid.saturating_sub(1);
					}
				}

				if best_t > 0 {
					break;
				}

				// best_t == 0: identify which direction is blocking at minimum
				// volume (t=1 granularity unit) and drop that direction's
				// tightest-rate partial. Falling back to t=1 keeps the check
				// stable — at t=0 both directions look fine (no volume).
				let probe_v_f = Self::scale_by_t(v_f_max, 1, GRANULARITY).max(if v_f_max > 0 { 1 } else { 0 });
				let probe_v_b = Self::scale_by_t(v_b_max, 1, GRANULARITY).max(if v_b_max > 0 { 1 } else { 0 });
				let probe_total_f = fixed_f.saturating_add(probe_v_f);
				let probe_total_b = fixed_b.saturating_add(probe_v_b);

				let blocking: Option<bool> = match Self::compute_pair_clearing_from_totals(
					asset_a,
					asset_b,
					probe_total_f,
					probe_total_b,
					spot_prices,
					state,
				) {
					Some(c) => {
						let f_blocked = match tight_f {
							Some((tn, td)) => c.forward_n.saturating_mul(td) < tn.saturating_mul(c.forward_d),
							None => false,
						};
						let b_blocked = match tight_b {
							Some((tn, td)) => c.backward_n.saturating_mul(td) < tn.saturating_mul(c.backward_d),
							None => false,
						};
						// Prefer dropping from whichever direction is blocked.
						// If both blocked, drop forward first, then backward
						// on the next iteration.
						if f_blocked {
							Some(true)
						} else if b_blocked {
							Some(false)
						} else {
							None
						}
					}
					None => {
						// Clearing failed entirely — drop from whichever
						// direction still has partials.
						if !f_partials.is_empty() {
							Some(true)
						} else if !b_partials.is_empty() {
							Some(false)
						} else {
							None
						}
					}
				};

				let dropped = match blocking {
					Some(true) => Self::drop_tightest(&mut f_partials),
					Some(false) => Self::drop_tightest(&mut b_partials),
					None => None,
				};

				let Some(dropped_id) = dropped else {
					// No progress possible — bail.
					break;
				};
				log::debug!(
					target: "solver::v2",
					"fit_partials_jointly: pair ({asset_a}, {asset_b}) best_t=0; dropped tightest partial {dropped_id}, retrying",
				);
			}

			if best_t == 0 {
				continue;
			}

			// Pro-rate best_t-scaled volumes to each partial by its remaining().
			Self::distribute_fills(fills, &f_partials, v_f_max, best_t, GRANULARITY);
			Self::distribute_fills(fills, &b_partials, v_b_max, best_t, GRANULARITY);
		}
	}

	/// Remove and return the id of the partial with the highest
	/// `amount_out/amount_in` rate. Returns `None` if the slice is empty.
	fn drop_tightest<'a>(partials: &mut Vec<IntentFill<'a>>) -> Option<IntentId> {
		let mut best_idx: Option<usize> = None;
		let mut best_n: U256 = U256::zero();
		let mut best_d: U256 = U256::one();
		for (i, p) in partials.iter().enumerate() {
			let IntentData::Swap(s) = &p.intent.data else {
				continue;
			};
			let n = U256::from(s.amount_out);
			let d = U256::from(s.amount_in.max(1));
			// n/d > best_n/best_d  <=>  n*best_d > best_n*d
			if best_idx.is_none() || n.saturating_mul(best_d) > best_n.saturating_mul(d) {
				best_idx = Some(i);
				best_n = n;
				best_d = d;
			}
		}
		best_idx.map(|i| partials.remove(i).intent.id)
	}

	/// Largest `amount_out/amount_in` demanded by any intent in the list.
	/// Encoded as (n, d) with d ≥ 1.
	fn tightest_rate<'a>(partials: &[IntentFill<'a>]) -> Option<(U256, U256)> {
		let mut best: Option<(U256, U256)> = None;
		for p in partials {
			let IntentData::Swap(s) = &p.intent.data else {
				continue;
			};
			let n = U256::from(s.amount_out);
			let d = U256::from(s.amount_in.max(1));
			best = match best {
				None => Some((n, d)),
				Some((cn, cd)) => {
					// Compare n/d vs cn/cd: n*cd vs cn*d.
					if n.saturating_mul(cd) > cn.saturating_mul(d) {
						Some((n, d))
					} else {
						Some((cn, cd))
					}
				}
			};
		}
		best
	}

	/// `max_vol * t / granularity`, saturating on overflow.
	fn scale_by_t(max_vol: Balance, t: u64, granularity: u64) -> Balance {
		if max_vol == 0 || t == 0 {
			return 0;
		}
		let product = U256::from(max_vol).saturating_mul(U256::from(t));
		let scaled = product / U256::from(granularity);
		scaled.try_into().unwrap_or(Balance::MAX)
	}

	/// Sort `fills` by estimated surplus descending so that a later `truncate`
	/// keeps the highest-value intents. Surplus is estimated using the clearing
	/// rate computed from the current `fills` (all intents, both directions).
	/// Ties break by intent id for determinism.
	fn sort_by_estimated_surplus<'a>(
		fills: &mut [IntentFill<'a>],
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) {
		// Build per-pair clearings from current volumes.
		let mut pair_totals: BTreeMap<AssetPair, (Balance, Balance)> = BTreeMap::new();
		for f in fills.iter() {
			let IntentData::Swap(s) = &f.intent.data else {
				continue;
			};
			let up = unordered_pair(s.asset_in, s.asset_out);
			let entry = pair_totals.entry(up).or_default();
			if s.asset_in == up.0 {
				entry.0 = entry.0.saturating_add(f.fill_amount);
			} else {
				entry.1 = entry.1.saturating_add(f.fill_amount);
			}
		}
		let mut clearings: BTreeMap<AssetPair, PairClearing> = BTreeMap::new();
		for (&(a, b), &(ta, tb)) in &pair_totals {
			if let Some(c) = Self::compute_pair_clearing_from_totals(a, b, ta, tb, spot_prices, state) {
				clearings.insert((a, b), c);
			}
		}

		// Compute surplus estimate per fill.
		let surplus_of = |f: &IntentFill<'a>| -> Balance {
			let IntentData::Swap(s) = &f.intent.data else {
				return 0;
			};
			let up = unordered_pair(s.asset_in, s.asset_out);
			let Some(c) = clearings.get(&up) else {
				return 0;
			};
			let (rn, rd) = if s.asset_in == up.0 {
				(c.forward_n, c.forward_d)
			} else {
				(c.backward_n, c.backward_d)
			};
			let output = apply_rate(f.fill_amount, rn, rd);
			let pro_rata_min = apply_rate(f.fill_amount, U256::from(s.amount_out), U256::from(s.amount_in));
			output.saturating_sub(pro_rata_min)
		};

		fills.sort_by(|a, b| {
			let sa = surplus_of(a);
			let sb = surplus_of(b);
			// Descending by surplus, then by id for determinism on ties.
			sb.cmp(&sa).then(a.intent.id.cmp(&b.intent.id))
		});
	}

	/// Distribute a total fit-volume across a set of partials proportionally to their
	/// `remaining()` share, applying per-intent ED guards. Each produced `IntentFill` is
	/// pushed to `fills`.
	fn distribute_fills<'a>(
		fills: &mut Vec<IntentFill<'a>>,
		partials: &[IntentFill<'a>],
		v_max: Balance,
		best_t: u64,
		granularity: u64,
	) {
		if v_max == 0 || best_t == 0 || partials.is_empty() {
			return;
		}

		for p in partials {
			let IntentData::Swap(swap) = &p.intent.data else {
				continue;
			};
			let ed = A::existential_deposit(swap.asset_in);

			// share = remaining * best_t / granularity.
			let raw_share = Self::scale_by_t(p.fill_amount, best_t, granularity).min(p.fill_amount);
			if raw_share < ed {
				continue;
			}

			// ED guard on remaining-after. remaining_after is the unfilled residue of
			// the intent after this solution applies: swap.remaining() - raw_share.
			let remaining_after = swap.remaining().saturating_sub(raw_share);
			let share = if remaining_after > 0 && remaining_after < ed {
				// Either fill everything (if still feasible) or trim to leave ed behind.
				// We don't re-check feasibility of the full fill here — `best_t` was fitted
				// against the tightest rate, and increasing a single partial's share past
				// its pro-rata can push the clearing below tolerance. Safer to trim.
				let trimmed = swap.remaining().saturating_sub(ed);
				if trimmed >= ed {
					trimmed
				} else {
					continue;
				}
			} else {
				raw_share
			};

			fills.push(IntentFill {
				intent: p.intent,
				fill_amount: share,
			});
		}
	}
}
