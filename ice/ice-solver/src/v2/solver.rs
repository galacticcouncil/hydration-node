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
	SolutionTrades, SwapData, SwapType,
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

/// Compute amount_out from a clearing rate, ensuring rounding consistency.
fn apply_rate(amount_in: Balance, n: U256, d: U256) -> Balance {
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
		routes: Vec<Route<AssetId>>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
	) -> Option<(Route<AssetId>, Balance, A::State)> {
		let best = routes
			.into_iter()
			.filter_map(
				|route| match A::sell(asset_in, asset_out, amount_in, route.clone(), state) {
					Ok((new_state, exec)) => Some((route, exec.amount_out, new_state)),
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

	pub fn solve(intents: Vec<Intent>, initial_state: A::State) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(empty_solution());
		}

		log::debug!(target: "solver::v2", "solve() called with {} intents", intents.len());

		// 1. Filter satisfiable intents
		let denominator = A::price_denominator();
		let mut spot_prices: BTreeMap<AssetId, Ratio> = BTreeMap::new();
		spot_prices.insert(denominator, Ratio::one());

		let satisfiable_intents: Vec<&Intent> = intents
			.iter()
			.filter(|intent| {
				let IntentData::Swap(swap) = &intent.data else {
					return false;
				};

				// Collect spot prices lazily
				for &asset in &[swap.asset_in, swap.asset_out] {
					let Ok(price_routes) = A::discover_routes(asset, denominator, &initial_state) else {
						continue;
					};
					for route in price_routes {
						if let Ok(price) = A::get_spot_price(asset, denominator, route, &initial_state) {
							let better = spot_prices.get(&asset).map_or(true, |existing| {
								U256::from(price.n) * U256::from(existing.d)
									> U256::from(existing.n) * U256::from(price.d)
							});
							if better {
								spot_prices.insert(asset, price);
							}
						}
					}
				}

				let check_amount = Self::effective_amount(swap);
				if check_amount == 0 {
					log::debug!(target:"solver::v2","intent {}: fully filled, skipping", intent.id);
					return false;
				}

				// Simulation check with effective (remaining) amount
				if let Ok(routes) = A::discover_routes(swap.asset_in, swap.asset_out, &initial_state) {
					if let Some((_, amount_out, _)) =
						Self::select_best_route(routes, swap.asset_in, swap.asset_out, check_amount, &initial_state)
					{
						// For partial: pro-rata minimum = check_amount * amount_out / amount_in
						let pro_rata_min =
							apply_rate(check_amount, U256::from(swap.amount_out), U256::from(swap.amount_in));
						if amount_out >= pro_rata_min {
							return true;
						}
						log::debug!(target:"solver::v2","intent {}: route output {} < pro_rata_min {} for {} -> {}",
							intent.id, amount_out, pro_rata_min, swap.asset_in, swap.asset_out);
					}
				}

				// Fallback: spot price check
				let ok = common::is_satisfiable(intent, &spot_prices);
				if !ok {
					log::debug!(target:"solver::v2","intent {}: unsatisfiable at spot price", intent.id);
				}
				ok
			})
			.collect();

		log::debug!(target: "solver::v2", "satisfiable: {}/{} intents", satisfiable_intents.len(), intents.len());

		if satisfiable_intents.is_empty() {
			return Ok(empty_solution());
		}

		if satisfiable_intents.len() == 1 {
			return Self::solve_single_intent(satisfiable_intents[0], &initial_state);
		}

		// 2. Build fill plan: for each intent, determine its fill amount.
		// Non-partial: full amount_in or excluded.
		// Partial: variable fill up to remaining().
		let mut fills: Vec<IntentFill> = satisfiable_intents
			.iter()
			.map(|&intent| {
				let IntentData::Swap(swap) = &intent.data else {
					return IntentFill { intent, fill_amount: 0 };
				};
				IntentFill {
					intent,
					fill_amount: Self::effective_amount(swap),
				}
			})
			.filter(|f| f.fill_amount > 0)
			.collect();

		// 3. Iterative clearing: first stabilize non-partial intents, then add partial fills.

		// Separate partial and non-partial fills
		let mut partial_fills: Vec<IntentFill> = Vec::new();
		let mut non_partial_fills: Vec<IntentFill> = Vec::new();
		for fill in fills {
			let IntentData::Swap(swap) = &fill.intent.data else {
				continue;
			};
			if swap.partial.is_partial() {
				partial_fills.push(fill);
			} else {
				non_partial_fills.push(fill);
			}
		}

		// Phase A: stabilize non-partial intents (same as v1)
		let mut pair_clearings: BTreeMap<AssetPair, PairClearing> = BTreeMap::new();
		const MAX_ITERATIONS: u32 = 10;
		for _iteration in 0..MAX_ITERATIONS {
			pair_clearings.clear();

			let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<&IntentFill>> = BTreeMap::new();
			for fill in &non_partial_fills {
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
				if let Some(c) = Self::compute_pair_clearing_with_fills(
					asset_a,
					asset_b,
					forward,
					backward,
					&spot_prices,
					&initial_state,
				) {
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

		// Phase B: add partial intents with binary-searched fill amounts.
		// For each partial intent, find the max fill amount where the clearing rate
		// (including non-partial volume + this fill) still meets the minimum rate.
		let mut fills = non_partial_fills;

		for pfill in &partial_fills {
			let IntentData::Swap(swap) = &pfill.intent.data else {
				continue;
			};
			let up = unordered_pair(swap.asset_in, swap.asset_out);
			let ed = A::existential_deposit(swap.asset_in);
			let (min_n, min_d) = min_rate(swap);

			// Binary search for max fill where clearing rate meets minimum
			let mut lo: Balance = ed;
			let mut hi: Balance = pfill.fill_amount; // remaining()
			let mut best_fill: Balance = 0;

			const MAX_SEARCH: u32 = 20;
			for _ in 0..MAX_SEARCH {
				if lo > hi {
					break;
				}
				let mid = lo.saturating_add(hi) / 2;
				if mid < ed {
					break;
				}

				// Temporarily add this partial fill and compute clearing
				let trial = IntentFill {
					intent: pfill.intent,
					fill_amount: mid,
				};
				let trial_ref = &trial;

				// Build pair groups including the trial fill
				let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<&IntentFill>> = BTreeMap::new();
				for fill in &fills {
					let IntentData::Swap(s) = &fill.intent.data else {
						continue;
					};
					let pair = unordered_pair(s.asset_in, s.asset_out);
					let entry = pair_groups.entry(pair).or_default();
					if s.asset_in == pair.0 {
						entry.0.push(fill);
					} else {
						entry.1.push(fill);
					}
				}
				// Add trial
				let pair = unordered_pair(swap.asset_in, swap.asset_out);
				let entry = pair_groups.entry(pair).or_default();
				if swap.asset_in == pair.0 {
					entry.0.push(trial_ref);
				} else {
					entry.1.push(trial_ref);
				}

				let clearing = Self::compute_pair_clearing_with_fills(
					up.0,
					up.1,
					&pair_groups.get(&up).map(|(f, _)| f.as_slice()).unwrap_or(&[]),
					&pair_groups.get(&up).map(|(_, b)| b.as_slice()).unwrap_or(&[]),
					&spot_prices,
					&initial_state,
				);

				let meets = if let Some(ref c) = clearing {
					let (rate_n, rate_d) = if swap.asset_in == up.0 {
						(c.forward_n, c.forward_d)
					} else {
						(c.backward_n, c.backward_d)
					};
					let amount_out = apply_rate(mid, rate_n, rate_d);
					let pro_rata_min = apply_rate(mid, min_n, min_d);
					amount_out >= pro_rata_min
				} else {
					false
				};

				if meets {
					best_fill = mid;
					lo = mid.saturating_add(1);
				} else {
					hi = mid.saturating_sub(1);
				}

				if hi.saturating_sub(lo) < ed {
					break;
				}
			}

			if best_fill >= ed {
				// ED guard on remaining
				let remaining_after = pfill.fill_amount.saturating_sub(best_fill);
				if remaining_after > 0 && remaining_after < ed {
					// Try filling everything
					let full = pfill.fill_amount;
					let trial = IntentFill {
						intent: pfill.intent,
						fill_amount: full,
					};
					let mut pg: BTreeMap<AssetPair, DirectionGroups<&IntentFill>> = BTreeMap::new();
					for fill in &fills {
						let IntentData::Swap(s) = &fill.intent.data else {
							continue;
						};
						let p = unordered_pair(s.asset_in, s.asset_out);
						let e = pg.entry(p).or_default();
						if s.asset_in == p.0 {
							e.0.push(fill);
						} else {
							e.1.push(fill);
						}
					}
					let p = unordered_pair(swap.asset_in, swap.asset_out);
					let e = pg.entry(p).or_default();
					if swap.asset_in == p.0 {
						e.0.push(&trial);
					} else {
						e.1.push(&trial);
					}
					let clearing = Self::compute_pair_clearing_with_fills(
						up.0,
						up.1,
						pg.get(&up).map(|(f, _)| f.as_slice()).unwrap_or(&[]),
						pg.get(&up).map(|(_, b)| b.as_slice()).unwrap_or(&[]),
						&spot_prices,
						&initial_state,
					);
					let full_ok = if let Some(ref c) = clearing {
						let (rn, rd) = if swap.asset_in == up.0 {
							(c.forward_n, c.forward_d)
						} else {
							(c.backward_n, c.backward_d)
						};
						apply_rate(full, rn, rd) >= apply_rate(full, min_n, min_d)
					} else {
						false
					};
					if full_ok {
						best_fill = full;
					} else {
						// Reduce to keep remaining >= ED
						best_fill = pfill.fill_amount.saturating_sub(ed);
					}
				}

				log::debug!(target: "solver::v2", "intent {} (partial): fill {} / {} ({:.1}%)",
					pfill.intent.id, best_fill, pfill.fill_amount,
					(best_fill as f64 / pfill.fill_amount as f64) * 100.0);

				fills.push(IntentFill {
					intent: pfill.intent,
					fill_amount: best_fill,
				});
			} else {
				log::debug!(target: "solver::v2", "intent {} (partial): no viable fill found", pfill.intent.id);
			}
		}

		if fills.is_empty() {
			log::debug!(target: "solver::v2", "all intents filtered out during iterative clearing");
			return Ok(empty_solution());
		}

		log::debug!(target: "solver::v2", "after iterative clearing: {} fills remaining", fills.len());

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

		// Stabilization loop (same structure as v1)
		const MAX_STABILIZATION_ROUNDS: u32 = 5;
		type RingFill = (Balance, Balance);

		#[derive(Default)]
		struct DirAccum {
			total_in: Balance,
			ring_in: Balance,
			ring_out: Balance,
		}

		for stabilization_round in 0..MAX_STABILIZATION_ROUNDS {
			log::debug!(target: "solver::v2", "stabilization round {}, {} included intents",
				stabilization_round, included.len());

			// Ring detection
			let mut graph = flow_graph::build_flow_graph(&included);
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
							log::debug!(target: "solver::v2", "single forward {} -> {}: amount {} below ED", asset_a, asset_b, amount);
						} else if let Some((route, amount_out, new_state)) =
							A::discover_routes(asset_a, asset_b, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(routes, asset_a, asset_b, amount, &state))
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
							log::debug!(target: "solver::v2", "single backward {} -> {}: amount {} below ED", asset_b, asset_a, amount);
						} else if let Some((route, amount_out, new_state)) =
							A::discover_routes(asset_b, asset_a, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(routes, asset_b, asset_a, amount, &state))
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
							let best = A::discover_routes(asset_a, asset_b, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(routes, asset_a, asset_b, net_sell, &state));
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
							let best = A::discover_routes(asset_b, asset_a, &state)
								.ok()
								.and_then(|routes| Self::select_best_route(routes, asset_b, asset_a, net_sell, &state));
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
					entry.total_in += fill;
					let (ri, ro) = ring_fills.get(&intent.id).copied().unwrap_or((0, 0));
					entry.ring_in += ri;
					entry.ring_out += ro;
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

		log::warn!(target: "solver::v2", "stabilization did not converge after {} rounds", MAX_STABILIZATION_ROUNDS);
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
				Self::select_best_route(routes, swap.asset_in, swap.asset_out, fill, initial_state)
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
		let (min_n, min_d) = min_rate(swap);

		// First try the full remaining amount
		if let Some((route, amount_out, _)) =
			Self::select_best_route(routes.to_vec(), swap.asset_in, swap.asset_out, max_fill, state)
		{
			let pro_rata_min = apply_rate(max_fill, min_n, min_d);
			if amount_out >= pro_rata_min {
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
				Self::select_best_route(routes.to_vec(), swap.asset_in, swap.asset_out, mid, state)
			{
				let pro_rata_min = apply_rate(mid, min_n, min_d);
				if amount_out >= pro_rata_min {
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

		// ED guard: if best fill leaves remaining < ED and > 0, adjust
		if let Some((fill, _, ref route)) = best {
			let remaining_after = max_fill.saturating_sub(fill);
			if remaining_after > 0 && remaining_after < ed {
				// Either fill the whole thing or reduce to keep remaining >= ED
				let fill_all = max_fill;
				if let Some((_, all_out, _)) =
					Self::select_best_route(routes.to_vec(), swap.asset_in, swap.asset_out, fill_all, state)
				{
					let pro_rata_min = apply_rate(fill_all, min_n, min_d);
					if all_out >= pro_rata_min {
						return Some((fill_all, all_out, route.clone()));
					}
				}
				// Can't fill all — reduce to keep remaining >= ED
				let reduced = max_fill.saturating_sub(ed);
				if reduced >= ed {
					if let Some((route, out, _)) =
						Self::select_best_route(routes.to_vec(), swap.asset_in, swap.asset_out, reduced, state)
					{
						let pro_rata_min = apply_rate(reduced, min_n, min_d);
						if out >= pro_rata_min {
							return Some((reduced, out, route));
						}
					}
				}
			}
		}

		best
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

		let pa = spot_prices.get(&asset_a)?;
		let pb = spot_prices.get(&asset_b)?;

		let flow = common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb);

		match flow {
			FlowDirection::SingleForward { amount } => {
				let routes = A::discover_routes(asset_a, asset_b, state).ok()?;
				let (_, amount_out, _) = Self::select_best_route(routes, asset_a, asset_b, amount, state)?;
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
				let (_, amount_out, _) = Self::select_best_route(routes, asset_b, asset_a, amount, state)?;
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
				let (_, amount_out, _) = Self::select_best_route(routes, asset_a, asset_b, net_sell, state)?;
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
				let (_, amount_out, _) = Self::select_best_route(routes, asset_b, asset_a, net_sell, state)?;
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
}
