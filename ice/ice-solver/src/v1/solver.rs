//! ICE Solver — Per-Direction Clearing Prices with Direct Matching
//!
//! Algorithm:
//! 1. Get spot prices, filter satisfiable intents
//! 2. Single intent fast path → direct AMM trade
//! 3. Group intents by unordered pair, compute net flow
//! 4. Simulate selling net imbalance through AMM → per-direction clearing prices
//! 5. Iteratively filter intents unsatisfied at clearing price until stable
//! 6. Ring trade detection for cross-pair cycles
//! 7. Execute actual AMM trades for net imbalances
//! 8. Resolve intents: same direction = same rate, opposite directions may differ
//!
//! Per-direction clearing prices:
//! - All intents selling A→B get the same rate (B per A)
//! - All intents selling B→A get the same rate (A per B)
//! - These rates need NOT be inverses — the spread is surplus from direct matching
//! - Scarce side gets ~spot rate (no slippage), excess side bears AMM impact
//!
//! Rounding: for each direction, the first intent's amount_out establishes
//! a canonical Ratio; all other intents derive amounts from it, guaranteeing
//! `validate_price_consistency` tolerance ≤ 1.

use crate::common;
use crate::common::flow_graph;
use crate::common::ring_detection;
use crate::common::FlowDirection;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::AMMInterface;
use ice_support::{
	AssetId, Balance, Intent, IntentData, IntentId, PoolTrade, ResolvedIntent, ResolvedIntents, Solution,
	SolutionTrades, SwapData, SwapType,
};
use sp_core::U256;
use sp_std::collections::btree_map::BTreeMap;
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
/// Returns floor(amount_in * n / d), overflow-safe.
fn apply_rate(amount_in: Balance, n: U256, d: U256) -> Balance {
	common::mul_div(U256::from(amount_in), n, d)
		.and_then(|v| v.try_into().ok())
		.unwrap_or(0)
}

/// Tolerance for AMM simulation-vs-execution differences, in basis points.
///
/// The solver simulates AMM trades off-chain to compute expected outputs.
/// The on-chain execution may produce slightly different results due to
/// rounding differences between the simulator and the real AMM math
/// (e.g. slip fee calculations, intermediate precision).
///
/// This tolerance is applied to:
/// - `PoolTrade.amount_out` (used as `min_amount_out` by the on-chain router)
/// - `directed_rates` (clearing rates derived from AMM output)
///
/// Both must use the same adjusted value to ensure the pallet account
/// has enough tokens from AMM trades to pay all resolved intents.
///
/// 1 bps = 0.01%. Increase if simulation divergence grows (e.g. after AMM changes).
///
/// Note: for very small outputs (< 10,000), integer truncation means no deduction
/// is applied. This is acceptable since production token amounts are typically 10^12+.
const AMM_SIMULATION_TOLERANCE_BPS: Balance = 1;

/// Reduce simulated AMM output by [`AMM_SIMULATION_TOLERANCE_BPS`] to ensure
/// on-chain execution succeeds even if the real AMM produces slightly less.
fn adjust_amm_output(simulated_out: Balance) -> Balance {
	simulated_out.saturating_sub(simulated_out * AMM_SIMULATION_TOLERANCE_BPS / 10_000)
}

impl<A: AMMInterface> Solver<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(empty_solution());
		}

		log::trace!(target: "solver", "V3 solve() called with {} intents", intents.len());

		// 1. Get spot prices
		let denominator = A::price_denominator();
		let unique_assets = common::collect_unique_assets(&intents);
		let mut spot_prices: BTreeMap<AssetId, Ratio> = BTreeMap::new();

		for asset in unique_assets {
			if asset == denominator {
				spot_prices.insert(asset, Ratio::one());
			} else {
				match A::get_spot_price(asset, denominator, &initial_state) {
					Ok(price) => {
						spot_prices.insert(asset, price);
					}
					Err(_) => {
						log::trace!(target:"solver","Failed to get spot price for asset {}", asset);
						continue;
					}
				}
			}
		}
		if log::log_enabled!(log::Level::Trace) {
			log::trace!(target: "solver", "spot prices for {} assets: {:?}", spot_prices.len(),
				spot_prices.iter().map(|(a, r)| (*a, r.n as f64 / r.d as f64)).collect::<Vec<_>>());
		}

		// 2. Filter satisfiable intents
		let satisfiable_intents: Vec<&Intent> = intents
			.iter()
			.filter(|intent| common::is_satisfiable(intent, &spot_prices))
			.collect();

		log::trace!(target: "solver", "satisfiable: {}/{} intents", satisfiable_intents.len(), intents.len());

		if satisfiable_intents.is_empty() {
			return Ok(empty_solution());
		}

		if satisfiable_intents.len() == 1 {
			return Self::solve_single_intent(satisfiable_intents[0], &initial_state);
		}

		// 3. Iterative clearing price computation (simulation phase)
		let mut included: Vec<&Intent> = satisfiable_intents;
		let mut pair_clearings: BTreeMap<AssetPair, PairClearing> = BTreeMap::new();

		const MAX_ITERATIONS: u32 = 10;
		for _ in 0..MAX_ITERATIONS {
			pair_clearings.clear();

			let mut pair_groups: BTreeMap<AssetPair, DirectionGroups<&Intent>> = BTreeMap::new();
			for intent in &included {
				let IntentData::Swap(swap) = &intent.data else {
					continue;
				};
				let up = unordered_pair(swap.asset_in, swap.asset_out);
				let entry = pair_groups.entry(up).or_default();
				if swap.asset_in == up.0 {
					entry.0.push(intent);
				} else {
					entry.1.push(intent);
				}
			}

			for (&(asset_a, asset_b), (forward, backward)) in &pair_groups {
				if let Some(c) =
					Self::compute_pair_clearing(asset_a, asset_b, forward, backward, &spot_prices, &initial_state)
				{
					pair_clearings.insert((asset_a, asset_b), c);
				}
			}

			// Filter intents unsatisfied at their direction's clearing price
			let before_count = included.len();
			included.retain(|intent| {
				let IntentData::Swap(swap) = &intent.data else {
					return true;
				};
				let up = unordered_pair(swap.asset_in, swap.asset_out);
				let Some(clearing) = pair_clearings.get(&up) else {
					log::trace!(target: "solver", "intent {}: no clearing price for pair ({},{}), keeping", intent.id, up.0, up.1);
					return true;
				};

				let amount_out = if swap.asset_in == up.0 {
					apply_rate(swap.amount_in, clearing.forward_n, clearing.forward_d)
				} else {
					apply_rate(swap.amount_in, clearing.backward_n, clearing.backward_d)
				};
				if amount_out < swap.amount_out {
					log::trace!(target: "solver", "intent {}: filtered — clearing output {} < min_out {} for {} → {}",
						intent.id, amount_out, swap.amount_out, swap.asset_in, swap.asset_out);
				}
				amount_out >= swap.amount_out
			});

			if included.len() == before_count {
				break;
			}
		}
		if included.is_empty() {
			return Ok(empty_solution());
		}

		if included.len() == 1 {
			return Self::solve_single_intent(included[0], &initial_state);
		}

		// 4. Ring detection (accepts &[&Intent] — no clone needed)
		let mut graph = flow_graph::build_flow_graph(&included);

		let rings = ring_detection::detect_rings(&mut graph, &spot_prices);

		/// Ring fill accumulator: (total_amount_in, total_amount_out) matched via rings.
		type RingFill = (Balance, Balance);
		let mut ring_fills: BTreeMap<IntentId, RingFill> = BTreeMap::new();
		for ring in &rings {
			for (_pair, fills) in &ring.edges {
				for fill in fills {
					let entry = ring_fills.entry(fill.intent_id).or_default();
					entry.0 = entry.0.saturating_add(fill.amount_in);
					entry.1 = entry.1.saturating_add(fill.amount_out);
				}
			}
		}

		// 5. Execute actual AMM trades for net imbalances per pair
		let mut state = initial_state.clone();
		let mut executed_trades: Vec<PoolTrade> = Vec::new();

		// Group by unordered pair with remaining (non-ring) volumes
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

		// Per-direction canonical rates: (asset_in, asset_out) → Ratio
		// The canonical ratio is derived from the first intent's computed amount_out
		// to guarantee rounding consistency for validate_price_consistency.
		let mut directed_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();

		for (&(asset_a, asset_b), (forward, backward)) in &pair_groups {
			let total_a_sold: Balance = forward
				.iter()
				.map(|(id, swap)| {
					swap.amount_in
						.saturating_sub(ring_fills.get(id).map(|(a, _)| *a).unwrap_or(0))
				})
				.sum();

			let total_b_sold: Balance = backward
				.iter()
				.map(|(id, swap)| {
					swap.amount_in
						.saturating_sub(ring_fills.get(id).map(|(a, _)| *a).unwrap_or(0))
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
					if let Ok((new_state, exec)) = A::sell(asset_a, asset_b, amount, None, &state) {
						let adjusted_out = adjust_amm_output(exec.amount_out);
						directed_rates.insert((asset_a, asset_b), Ratio::new(adjusted_out, exec.amount_in));
						executed_trades.push(PoolTrade {
							direction: SwapType::ExactIn,
							amount_in: exec.amount_in,
							amount_out: adjusted_out,
							route: exec.route,
						});
						state = new_state;
					}
				}
				FlowDirection::SingleBackward { amount } => {
					if let Ok((new_state, exec)) = A::sell(asset_b, asset_a, amount, None, &state) {
						let adjusted_out = adjust_amm_output(exec.amount_out);
						directed_rates.insert((asset_b, asset_a), Ratio::new(adjusted_out, exec.amount_in));
						executed_trades.push(PoolTrade {
							direction: SwapType::ExactIn,
							amount_in: exec.amount_in,
							amount_out: adjusted_out,
							route: exec.route,
						});
						state = new_state;
					}
				}
				FlowDirection::ExcessForward {
					scarce_out,
					direct_match,
					net_sell,
				} => {
					// B→A (scarce): matched at spot rate
					if total_b_sold > 0 {
						directed_rates.insert((asset_b, asset_a), Ratio::new(scarce_out, total_b_sold));
					}
					// Sell net A through AMM
					match A::sell(asset_a, asset_b, net_sell, None, &state) {
						Ok((new_state, exec)) => {
							let adjusted_out = adjust_amm_output(exec.amount_out);
							let total_out = direct_match.saturating_add(adjusted_out);
							if total_a_sold > 0 {
								directed_rates.insert((asset_a, asset_b), Ratio::new(total_out, total_a_sold));
							}
							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: exec.amount_in,
								amount_out: adjusted_out,
								route: exec.route,
							});
							state = new_state;
						}
						Err(_) => {
							// Fallback: A→B at spot rate
							let fallback = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);
							if total_a_sold > 0 {
								directed_rates.insert((asset_a, asset_b), Ratio::new(fallback, total_a_sold));
							}
						}
					}
				}
				FlowDirection::ExcessBackward {
					scarce_out,
					direct_match,
					net_sell,
				} => {
					// A→B (scarce): matched at spot rate
					if total_a_sold > 0 {
						directed_rates.insert((asset_a, asset_b), Ratio::new(scarce_out, total_a_sold));
					}
					// Sell net B through AMM
					match A::sell(asset_b, asset_a, net_sell, None, &state) {
						Ok((new_state, exec)) => {
							let adjusted_out = adjust_amm_output(exec.amount_out);
							let total_out = direct_match.saturating_add(adjusted_out);
							if total_b_sold > 0 {
								directed_rates.insert((asset_b, asset_a), Ratio::new(total_out, total_b_sold));
							}
							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: exec.amount_in,
								amount_out: adjusted_out,
								route: exec.route,
							});
							state = new_state;
						}
						Err(_) => {
							// Fallback: B→A at spot rate
							let fallback = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
							if total_b_sold > 0 {
								directed_rates.insert((asset_b, asset_a), Ratio::new(fallback, total_b_sold));
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

		// 6. Compute unified rates per direction: blend ring fills + AMM fills.
		// For each directed pair: unified_rate = (ring_total_out + amm_portion_out) / total_in
		// This ensures all intents in the same direction get the same rate,
		// regardless of individual ring fill proportions.
		#[derive(Default)]
		struct DirAccum {
			total_in: Balance,
			ring_in: Balance,
			ring_out: Balance,
		}

		let mut unified_rates: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
		{
			let mut accum: BTreeMap<AssetPair, DirAccum> = BTreeMap::new();

			for intent in &included {
				let IntentData::Swap(swap) = &intent.data else {
					continue;
				};
				let key = (swap.asset_in, swap.asset_out);
				let entry = accum.entry(key).or_default();
				entry.total_in += swap.amount_in;
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

		// Resolve intents: derive canonical Ratio from first intent's amount_out
		// for rounding consistency, using the unified rate.
		// Note: the first intent encountered per direction establishes the canonical
		// Ratio. Iteration order of `included` affects rounding for subsequent intents.
		// This is by design — validate_price_consistency tolerates ±1 difference.
		let mut canonical_prices: BTreeMap<AssetPair, Ratio> = BTreeMap::new();
		let mut resolved_intents: Vec<ResolvedIntent> = Vec::new();
		let mut total_score: Balance = 0;

		for intent in &included {
			let IntentData::Swap(swap) = &intent.data else {
				continue;
			};
			let directed_key = (swap.asset_in, swap.asset_out);

			let total_in = swap.amount_in;

			let total_out = if let Some(canonical) = canonical_prices.get(&directed_key) {
				apply_rate(total_in, U256::from(canonical.n), U256::from(canonical.d))
			} else if let Some(rate) = unified_rates.get(&directed_key) {
				let amount_out = apply_rate(total_in, U256::from(rate.n), U256::from(rate.d));
				if total_in > 0 && amount_out > 0 {
					canonical_prices.insert(directed_key, Ratio::new(amount_out, total_in));
				}
				amount_out
			} else {
				0
			};

			if total_in == 0 || total_out == 0 {
				log::trace!(target: "solver", "intent {}: skipped in resolution — no rate for {} → {}",
					intent.id, swap.asset_in, swap.asset_out);
				continue;
			}

			let min_required = swap.amount_out;

			if total_out < min_required {
				log::trace!(target: "solver", "intent {}: skipped in resolution — output {} < min_out {} for {} → {}",
					intent.id, total_out, min_required, swap.asset_in, swap.asset_out);
				continue;
			}

			let surplus = total_out.saturating_sub(min_required);
			total_score = total_score.saturating_add(surplus);

			resolved_intents.push(ResolvedIntent {
				id: intent.id,
				data: IntentData::Swap(SwapData {
					asset_in: swap.asset_in,
					asset_out: swap.asset_out,
					amount_in: total_in,
					amount_out: total_out,
					partial: swap.partial,
				}),
			});
		}
		Ok(Solution {
			resolved_intents: ResolvedIntents::truncate_from(resolved_intents),
			trades: SolutionTrades::truncate_from(executed_trades),
			score: total_score,
		})
	}

	/// Single intent: direct AMM trade.
	///
	/// Note: the resolved intent gets the full `trade_execution.amount_out` (unadjusted),
	/// while the pool trade gets `adjust_amm_output(...)`. This is intentional — for a
	/// single intent, all AMM output goes directly to the user, so no tolerance buffer
	/// is needed for the intent itself. The pool trade's adjusted value is the on-chain
	/// `min_amount_out` safety net.
	fn solve_single_intent(intent: &Intent, initial_state: &A::State) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data else {
			return Ok(empty_solution());
		};

		match A::sell(swap.asset_in, swap.asset_out, swap.amount_in, None, initial_state) {
			Ok((_new_state, trade_execution)) => {
				if trade_execution.amount_out < swap.amount_out {
					return Ok(empty_solution());
				}

				let surplus = trade_execution.amount_out.saturating_sub(swap.amount_out);

				let resolved = ResolvedIntent {
					id: intent.id,
					data: IntentData::Swap(SwapData {
						asset_in: swap.asset_in,
						asset_out: swap.asset_out,
						amount_in: trade_execution.amount_in,
						amount_out: trade_execution.amount_out,
						partial: swap.partial,
					}),
				};

				Ok(Solution {
					resolved_intents: ResolvedIntents::truncate_from(vec![resolved]),
					trades: SolutionTrades::truncate_from(vec![PoolTrade {
						direction: SwapType::ExactIn,
						amount_in: trade_execution.amount_in,
						amount_out: adjust_amm_output(trade_execution.amount_out),
						route: trade_execution.route,
					}]),
					score: surplus,
				})
			}
			Err(_) => Ok(empty_solution()),
		}
	}

	/// Compute per-direction clearing prices for a pair.
	/// Used during iterative filtering (price discovery only, no state mutation).
	fn compute_pair_clearing(
		asset_a: AssetId,
		asset_b: AssetId,
		forward: &[&Intent],  // A→B sellers
		backward: &[&Intent], // B→A sellers
		spot_prices: &BTreeMap<AssetId, Ratio>,
		state: &A::State,
	) -> Option<PairClearing> {
		if forward.is_empty() && backward.is_empty() {
			return None;
		}

		let total_a_sold: Balance = forward
			.iter()
			.map(|i| {
				let IntentData::Swap(s) = &i.data else {
					return 0;
				};
				s.amount_in
			})
			.sum();

		let total_b_sold: Balance = backward
			.iter()
			.map(|i| {
				let IntentData::Swap(s) = &i.data else {
					return 0;
				};
				s.amount_in
			})
			.sum();

		let pa = spot_prices.get(&asset_a)?;
		let pb = spot_prices.get(&asset_b)?;

		let flow = common::analyze_pair_flow(total_a_sold, total_b_sold, pa, pb);

		match flow {
			FlowDirection::SingleForward { amount } => {
				let (_, exec) = A::sell(asset_a, asset_b, amount, None, state).ok()?;
				let adjusted_out = adjust_amm_output(exec.amount_out);
				Some(PairClearing {
					forward_n: U256::from(adjusted_out),
					forward_d: U256::from(exec.amount_in),
					backward_n: U256::zero(),
					backward_d: U256::one(),
				})
			}
			FlowDirection::SingleBackward { amount } => {
				let (_, exec) = A::sell(asset_b, asset_a, amount, None, state).ok()?;
				let adjusted_out = adjust_amm_output(exec.amount_out);
				Some(PairClearing {
					forward_n: U256::zero(),
					forward_d: U256::one(),
					backward_n: U256::from(adjusted_out),
					backward_d: U256::from(exec.amount_in),
				})
			}
			FlowDirection::ExcessForward {
				scarce_out,
				direct_match,
				net_sell,
			} => {
				let (_, exec) = A::sell(asset_a, asset_b, net_sell, None, state).ok()?;
				let adjusted_out = adjust_amm_output(exec.amount_out);
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
				let (_, exec) = A::sell(asset_b, asset_a, net_sell, None, state).ok()?;
				let adjusted_out = adjust_amm_output(exec.amount_out);
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

#[cfg(test)]
mod tests {
	use super::*;
	use hydra_dx_math::types::Ratio;
	use hydradx_traits::amm::{AMMInterface, TradeExecution};
	use hydradx_traits::router::{Route, Trade};
	use ice_support::IntentId;

	fn make_intent(
		id: IntentId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_out: Balance,
	) -> Intent {
		Intent {
			id,
			data: IntentData::Swap(SwapData {
				asset_in,
				asset_out,
				amount_in,
				amount_out: min_out,
				partial: false,
			}),
		}
	}

	fn make_partial(
		id: IntentId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_out: Balance,
	) -> Intent {
		Intent {
			id,
			data: IntentData::Swap(SwapData {
				asset_in,
				asset_out,
				amount_in,
				amount_out: min_out,
				partial: true,
			}),
		}
	}

	struct MockAMMOneToOne;

	impl AMMInterface for MockAMMOneToOne {
		type Error = ();
		type State = ();

		fn sell(
			asset_in: u32,
			asset_out: u32,
			amount_in: u128,
			_route: Option<Route<u32>>,
			_state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			Ok((
				(),
				TradeExecution {
					amount_in,
					amount_out: amount_in,
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn buy(
			asset_in: u32,
			asset_out: u32,
			amount_out: u128,
			_route: Option<Route<u32>>,
			_state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			Ok((
				(),
				TradeExecution {
					amount_in: amount_out,
					amount_out,
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn get_spot_price(_asset_in: u32, _asset_out: u32, _state: &Self::State) -> Result<Ratio, Self::Error> {
			Ok(Ratio::new(1, 1))
		}

		fn price_denominator() -> u32 {
			0
		}
	}

	/// Mock AMM with 2:1 price (asset 1 worth 2x asset 2) and 1% slippage.
	struct MockAMMWithSlippage;

	impl AMMInterface for MockAMMWithSlippage {
		type Error = ();
		type State = ();

		fn sell(
			asset_in: u32,
			asset_out: u32,
			amount_in: u128,
			_route: Option<Route<u32>>,
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
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn buy(
			asset_in: u32,
			asset_out: u32,
			amount_out: u128,
			_route: Option<Route<u32>>,
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
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn get_spot_price(asset_in: u32, _asset_out: u32, _state: &Self::State) -> Result<Ratio, Self::Error> {
			match asset_in {
				1 => Ok(Ratio::new(2, 1)),
				2 => Ok(Ratio::new(1, 1)),
				_ => Ok(Ratio::new(1, 1)),
			}
		}

		fn price_denominator() -> u32 {
			0
		}
	}

	#[test]
	fn test_solve_empty() {
		let result = Solver::<MockAMMOneToOne>::solve(vec![], ());
		assert!(result.is_ok());
		assert!(result.unwrap().resolved_intents.is_empty());
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
		// Perfect cancel at 1:1 — both sides get spot rate
		let intents = vec![make_intent(1, 1, 2, 100, 90), make_intent(2, 2, 1, 100, 90)];
		let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 2);
		assert_eq!(result.trades.len(), 0);

		let r1 = &result.resolved_intents[0];
		let r2 = &result.resolved_intents[1];
		assert_eq!(r1.data.amount_out(), 100);
		assert_eq!(r2.data.amount_out(), 100);
	}

	#[test]
	fn test_scarce_side_gets_spot() {
		// Asset 1 worth 2x asset 2. AMM has 1% slippage.
		// Alice: sell 100 of asset 1 → asset 2 (excess side)
		// Bob: sell 100 of asset 2 → asset 1 (scarce side — only 100 B vs 200 B-equivalent from Alice)
		//
		// At spot: Alice's 100 A = 200 B value. Bob's 100 B = 100 B value.
		// Excess A: net 50 A to sell through AMM (100 A - 50 A matched with Bob)
		// Bob (scarce): gets directly matched A = 50 A for his 100 B → rate = 0.5 A/B = spot rate
		// Alice (excess): gets 100 B (from Bob) + AMM output for 50 A
		//   AMM: sell 50 A → 50*2*0.99 = 99 B
		//   Alice total: 100 + 99 = 199 B for 100 A → rate = 1.99 B/A (vs spot 2.0)
		let intents = vec![
			make_intent(1, 1, 2, 100, 180), // Alice: sell A, want B
			make_intent(2, 2, 1, 100, 45),  // Bob: sell B, want A
		];
		let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 2);

		let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
		let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();

		// Bob (scarce) should get spot rate: 100 B → 50 A
		assert_eq!(bob.data.amount_out(), 50, "Bob should get spot rate (50 A for 100 B)");

		// Alice (excess) gets less than spot due to AMM slippage: 199 B instead of 200 B
		assert!(
			alice.data.amount_out() < 200,
			"Alice should get less than spot due to AMM slippage"
		);
		assert!(alice.data.amount_out() >= 195, "Alice should still get close to spot");

		// per-direction: rates are NOT inverses
		// Alice rate: B/A = alice.out / alice.in
		// Bob rate: A/B = bob.out / bob.in
		// If inverse: alice_rate * bob_rate = 1. With per-direction: < 1 (spread = surplus saved)
		let alice_rate_x1000 = alice.data.amount_out() * 1000 / alice.data.amount_in();
		let bob_rate_x1000 = bob.data.amount_out() * 1000 / bob.data.amount_in();
		// alice_rate ≈ 1.99, bob_rate ≈ 0.5, product ≈ 0.995 < 1.0
		let product_x1000000 = alice_rate_x1000 * bob_rate_x1000;
		assert!(
			product_x1000000 < 1_000_000,
			"per-direction: rates should NOT be exact inverses (product={}, expected < 1M)",
			product_x1000000
		);
	}

	#[test]
	fn test_same_direction_uniform_rate() {
		// 3 sellers in same direction should all get identical rate
		let intents = vec![
			make_intent(1, 1, 2, 100, 90),
			make_intent(2, 1, 2, 200, 180),
			make_intent(3, 1, 2, 50, 45),
		];
		let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 3);

		// All same-direction intents should get identical out/in ratio
		let rates: Vec<f64> = result
			.resolved_intents
			.iter()
			.map(|r| r.data.amount_out() as f64 / r.data.amount_in() as f64)
			.collect();

		for rate in &rates[1..] {
			let diff = (rate - rates[0]).abs() / rates[0];
			assert!(diff < 0.001, "Same-direction rates must be uniform, got diff {}", diff);
		}
	}

	#[test]
	fn test_iterative_filtering() {
		let intents = vec![
			make_intent(1, 1, 2, 100, 95),
			make_intent(2, 2, 1, 100, 95),
			make_intent(3, 1, 2, 100, 200), // impossible limit
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
		assert!(result.trades.len() >= 1);
		// Same rate for both
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
		// Alice sells 200 of asset 1 (excess), Bob sells 100 of asset 2 (scarce)
		// At spot: 200 A = 400 B value, 100 B = 100 B value
		// Net excess A: 200 - 50 = 150 A (50 A cancels with 100 B at spot)
		// Bob gets: 50 A at spot rate (no slippage)
		// Alice gets: 100 B + AMM(150 A → ~297 B) = ~397 B
		let intents = vec![make_partial(1, 1, 2, 200, 360), make_intent(2, 2, 1, 100, 45)];
		let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 2);

		let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
		let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();

		// Bob should get spot rate
		assert_eq!(bob.data.amount_out(), 50);

		// Alice should get less than 400 (spot) due to slippage on excess
		assert!(alice.data.amount_out() < 400);
		assert!(alice.data.amount_out() >= 390);
	}

	#[test]
	fn test_three_asset_ring() {
		// 3-asset cycle: 1→2, 2→3, 3→1, all at 1:1
		// Should be detected as a ring — no AMM trades needed
		let intents = vec![
			make_intent(1, 1, 2, 100, 90),
			make_intent(2, 2, 3, 100, 90),
			make_intent(3, 3, 1, 100, 90),
		];
		let result = Solver::<MockAMMOneToOne>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 3);
		// Ring fills all volume — no AMM trades needed
		assert_eq!(result.trades.len(), 0, "Ring trade should avoid AMM entirely");

		for ri in &result.resolved_intents {
			assert_eq!(ri.data.amount_in(), 100);
			assert_eq!(ri.data.amount_out(), 100);
		}
		assert_eq!(result.score, 30); // 3 * (100 - 90)
	}

	/// Mock AMM where sell of asset 1→2 fails for amounts > 50.
	struct MockAMMPartialFailure;

	impl AMMInterface for MockAMMPartialFailure {
		type Error = ();
		type State = ();

		fn sell(
			asset_in: u32,
			asset_out: u32,
			amount_in: u128,
			_route: Option<Route<u32>>,
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
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn buy(
			asset_in: u32,
			asset_out: u32,
			amount_out: u128,
			_route: Option<Route<u32>>,
			_state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			Ok((
				(),
				TradeExecution {
					amount_in: amount_out,
					amount_out,
					route: Route::try_from(vec![Trade {
						pool: hydradx_traits::router::PoolType::Omnipool,
						asset_in,
						asset_out,
					}])
					.unwrap(),
				},
			))
		}

		fn get_spot_price(_asset_in: u32, _asset_out: u32, _state: &Self::State) -> Result<Ratio, Self::Error> {
			Ok(Ratio::new(1, 1))
		}

		fn price_denominator() -> u32 {
			0
		}
	}

	#[test]
	fn test_amm_failure_fallback() {
		// AMM fails for sell(1→2) when amount > 50
		// Intent 1: sell 200 of 1→2 (excess A)
		// Intent 2: sell 50 of 2→1 (scarce B)
		// Net A = 200 - 50 = 150 > 50, so AMM fails
		// Both should resolve at spot rate via fallback
		let intents = vec![make_intent(1, 1, 2, 200, 180), make_intent(2, 2, 1, 50, 45)];
		let result = Solver::<MockAMMPartialFailure>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 2);
		// No AMM trades executed (the sell failed)
		assert_eq!(result.trades.len(), 0);

		let r1 = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
		let r2 = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();

		// Both get spot rate (1:1): 200 in → 200 out, 50 in → 50 out
		assert_eq!(r1.data.amount_out(), 200);
		assert_eq!(r2.data.amount_out(), 50);
	}

	#[test]
	fn test_excess_backward_scarce_gets_spot() {
		// Asset 1 worth 2x asset 2. AMM has 1% slippage.
		// Alice: sell 100 of asset 2 → asset 1 (excess side — 100 B worth 100 B, but 50 B from Bob worth 100 B)
		// Bob: sell 50 of asset 1 → asset 2 (scarce side — 50 A worth 100 B)
		//
		// ExcessBackward: B side has more value (100 B > 50 A equivalent of 100 B)
		// Bob (scarce A→B): gets spot rate
		// Alice (excess B→A): gets direct match + AMM for remainder
		let intents = vec![
			make_intent(1, 2, 1, 100, 45), // Alice: sell B, want A (excess)
			make_intent(2, 1, 2, 50, 90),  // Bob: sell A, want B (scarce)
		];
		let result = Solver::<MockAMMWithSlippage>::solve(intents, ()).unwrap();

		assert_eq!(result.resolved_intents.len(), 2);

		let alice = result.resolved_intents.iter().find(|r| r.id == 1).unwrap();
		let bob = result.resolved_intents.iter().find(|r| r.id == 2).unwrap();

		// Bob (scarce A→B) should get spot rate: 50 A → 100 B
		assert_eq!(bob.data.amount_out(), 100, "Bob should get spot rate (100 B for 50 A)");

		// Alice (excess B→A) gets less than spot due to AMM slippage on remainder
		assert!(alice.data.amount_out() > 0);
		assert!(alice.data.amount_out() >= 45, "Alice should meet her minimum");
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
		// Perfect cancel at 1:1
		assert_eq!(result.trades.len(), 0);
		for ri in &result.resolved_intents {
			assert_eq!(ri.data.amount_in(), 1_000_000 * unit);
			assert_eq!(ri.data.amount_out(), 1_000_000 * unit);
		}
	}
}
