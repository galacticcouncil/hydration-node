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

/// Directed clearing rate: (numerator, denominator) for amount_out per amount_in.

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
		log::trace!(target: "solver", "spot prices for {} assets: {:?}", spot_prices.len(),
			spot_prices.iter().map(|(a, r)| (*a, r.n as f64 / r.d as f64)).collect::<Vec<_>>());

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
		let mut pair_clearings: BTreeMap<(AssetId, AssetId), PairClearing> = BTreeMap::new();

		const MAX_ITERATIONS: u32 = 10;
		for _ in 0..MAX_ITERATIONS {
			pair_clearings.clear();

			let mut pair_groups: BTreeMap<(AssetId, AssetId), (Vec<&Intent>, Vec<&Intent>)> = BTreeMap::new();
			for intent in &included {
				let IntentData::Swap(swap) = &intent.data;
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
				let IntentData::Swap(swap) = &intent.data;
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

		// 4. Ring detection
		let included_owned: Vec<Intent> = included.iter().map(|i| (*i).clone()).collect();
		let mut graph = flow_graph::build_flow_graph(&included_owned);

		let rings = ring_detection::detect_rings(&mut graph, &spot_prices);

		let mut ring_fills: BTreeMap<IntentId, (Balance, Balance)> = BTreeMap::new();
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
		let mut pair_groups: BTreeMap<(AssetId, AssetId), (Vec<(IntentId, &SwapData)>, Vec<(IntentId, &SwapData)>)> =
			BTreeMap::new();
		for intent in &included {
			let IntentData::Swap(swap) = &intent.data;
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
		let mut directed_rates: BTreeMap<(AssetId, AssetId), Ratio> = BTreeMap::new();

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

			let price_a = spot_prices.get(&asset_a);
			let price_b = spot_prices.get(&asset_b);

			// Single direction: pure AMM trade
			if total_a_sold == 0 || total_b_sold == 0 {
				let (sell_asset, buy_asset, sell_amount) = if total_a_sold > 0 {
					(asset_a, asset_b, total_a_sold)
				} else {
					(asset_b, asset_a, total_b_sold)
				};

				match A::sell(sell_asset, buy_asset, sell_amount, None, &state) {
					Ok((new_state, exec)) => {
						// Single direction: rate from AMM execution
						let adjusted_out = adjust_amm_output(exec.amount_out);
						directed_rates.insert((sell_asset, buy_asset), Ratio::new(adjusted_out, exec.amount_in));

						executed_trades.push(PoolTrade {
							direction: SwapType::ExactIn,
							amount_in: exec.amount_in,
							amount_out: adjust_amm_output(exec.amount_out),
							route: exec.route,
						});
						state = new_state;
					}
					Err(_) => continue,
				}
				continue;
			}

			// Both directions have flow — compute net imbalance and per-direction rates
			let (Some(pa), Some(pb)) = (price_a, price_b) else {
				continue;
			};

			// Compare values using overflow-safe calc_amount_out
			let a_as_b = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);

			if a_as_b > total_b_sold {
				// Excess A: more A value than B value
				let matched_a_for_b = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
				let net_a = total_a_sold.saturating_sub(matched_a_for_b);

				// B→A (scarce side): gets directly matched A at spot rate
				if total_b_sold > 0 {
					directed_rates.insert((asset_b, asset_a), Ratio::new(matched_a_for_b, total_b_sold));
				}

				if net_a == 0 {
					// Perfect cancel — A→B gets spot rate
					if total_a_sold > 0 {
						directed_rates.insert((asset_a, asset_b), Ratio::new(a_as_b, total_a_sold));
					}
				} else {
					// Sell net A through AMM
					match A::sell(asset_a, asset_b, net_a, None, &state) {
						Ok((new_state, exec)) => {
							// A→B sellers get: total_b_sold (from direct match) + amm_b_out
							let total_b_for_a_sellers = total_b_sold.saturating_add(adjust_amm_output(exec.amount_out));
							if total_a_sold > 0 {
								directed_rates
									.insert((asset_a, asset_b), Ratio::new(total_b_for_a_sellers, total_a_sold));
							}

							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: exec.amount_in,
								amount_out: adjust_amm_output(exec.amount_out),
								route: exec.route,
							});
							state = new_state;
						}
						Err(_) => {
							if total_a_sold > 0 {
								directed_rates.insert((asset_a, asset_b), Ratio::new(a_as_b, total_a_sold));
							}
						}
					}
				}
			} else if total_b_sold > a_as_b || a_as_b == 0 {
				// Excess B (or can't compute)
				let b_as_a = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
				let matched_b_for_a = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);
				let net_b = total_b_sold.saturating_sub(matched_b_for_a);

				// A→B (scarce side): gets directly matched B at spot rate
				if total_a_sold > 0 {
					directed_rates.insert((asset_a, asset_b), Ratio::new(matched_b_for_a, total_a_sold));
				}

				if net_b == 0 {
					if total_b_sold > 0 {
						directed_rates.insert((asset_b, asset_a), Ratio::new(b_as_a, total_b_sold));
					}
				} else {
					match A::sell(asset_b, asset_a, net_b, None, &state) {
						Ok((new_state, exec)) => {
							let total_a_for_b_sellers = total_a_sold.saturating_add(adjust_amm_output(exec.amount_out));
							if total_b_sold > 0 {
								directed_rates
									.insert((asset_b, asset_a), Ratio::new(total_a_for_b_sellers, total_b_sold));
							}

							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: exec.amount_in,
								amount_out: adjust_amm_output(exec.amount_out),
								route: exec.route,
							});
							state = new_state;
						}
						Err(_) => {
							if total_b_sold > 0 {
								directed_rates.insert((asset_b, asset_a), Ratio::new(b_as_a, total_b_sold));
							}
						}
					}
				}
			} else {
				// Perfect cancel — both sides get spot rate
				if total_a_sold > 0 {
					directed_rates.insert((asset_a, asset_b), Ratio::new(a_as_b, total_a_sold));
				}
				if let Some(b_as_a) = common::calc_amount_out(total_b_sold, pb, pa) {
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
		let mut unified_rates: BTreeMap<(AssetId, AssetId), Ratio> = BTreeMap::new();
		{
			// Accumulate per-direction totals
			let mut dir_total_in: BTreeMap<(AssetId, AssetId), Balance> = BTreeMap::new();
			let mut dir_ring_in: BTreeMap<(AssetId, AssetId), Balance> = BTreeMap::new();
			let mut dir_ring_out: BTreeMap<(AssetId, AssetId), Balance> = BTreeMap::new();

			for intent in &included {
				let IntentData::Swap(swap) = &intent.data;
				let key = (swap.asset_in, swap.asset_out);
				*dir_total_in.entry(key).or_default() += swap.amount_in;
				let (ri, ro) = ring_fills.get(&intent.id).copied().unwrap_or((0, 0));
				*dir_ring_in.entry(key).or_default() += ri;
				*dir_ring_out.entry(key).or_default() += ro;
			}

			for (key, total_in) in &dir_total_in {
				let ring_in = dir_ring_in.get(key).copied().unwrap_or(0);
				let ring_out = dir_ring_out.get(key).copied().unwrap_or(0);
				let remaining_in = total_in.saturating_sub(ring_in);

				// AMM portion: use directed_rate for the remaining volume
				let amm_out = if remaining_in > 0 {
					if let Some(rate) = directed_rates.get(key) {
						apply_rate(remaining_in, U256::from(rate.n), U256::from(rate.d))
					} else {
						0
					}
				} else {
					0
				};

				let total_out = ring_out.saturating_add(amm_out);
				if *total_in > 0 && total_out > 0 {
					unified_rates.insert(*key, Ratio::new(total_out, *total_in));
				}
			}
		}

		// Resolve intents: derive canonical Ratio from first intent's amount_out
		// for rounding consistency, using the unified rate.
		let mut canonical_prices: BTreeMap<(AssetId, AssetId), Ratio> = BTreeMap::new();
		let mut resolved_intents: Vec<ResolvedIntent> = Vec::new();
		let mut total_score: Balance = 0;

		for intent in &included {
			let IntentData::Swap(swap) = &intent.data;
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
	fn solve_single_intent(intent: &Intent, initial_state: &A::State) -> Result<Solution, A::Error> {
		let IntentData::Swap(swap) = &intent.data;

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
	/// Used during iterative filtering (price discovery only).
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
				let IntentData::Swap(s) = &i.data;
				s.amount_in
			})
			.sum();

		let total_b_sold: Balance = backward
			.iter()
			.map(|i| {
				let IntentData::Swap(s) = &i.data;
				s.amount_in
			})
			.sum();

		let pa = spot_prices.get(&asset_a)?;
		let pb = spot_prices.get(&asset_b)?;

		// Single direction: AMM rate for that direction, no opposing rate needed
		if total_a_sold == 0 || total_b_sold == 0 {
			let (sell_asset, _buy_asset, sell_amount) = if total_a_sold > 0 {
				(asset_a, asset_b, total_a_sold)
			} else {
				(asset_b, asset_a, total_b_sold)
			};

			match A::sell(sell_asset, _buy_asset, sell_amount, None, state) {
				Ok((_new_state, exec)) => {
					let (fwd_n, fwd_d, bwd_n, bwd_d) = if sell_asset == asset_a {
						(
							U256::from(exec.amount_out),
							U256::from(exec.amount_in),
							U256::zero(),
							U256::one(),
						)
					} else {
						(
							U256::zero(),
							U256::one(),
							U256::from(exec.amount_out),
							U256::from(exec.amount_in),
						)
					};
					return Some(PairClearing {
						forward_n: fwd_n,
						forward_d: fwd_d,
						backward_n: bwd_n,
						backward_d: bwd_d,
					});
				}
				Err(_) => return None,
			}
		}

		// Both directions: compute net imbalance and per-direction rates.
		// Convert volumes to common denomination to compare values.
		// Use calc_amount_out to avoid overflow with large Ratio values.
		let a_as_b = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);
		// a_as_b = how much B the A sellers' volume is worth at spot

		if a_as_b > total_b_sold {
			// Excess A: more A value than B value
			// B→A sellers (scarce): matched at spot rate
			// net_a_to_amm: excess A that must go through AMM
			let matched_b_for_a = total_b_sold; // all B sellers' volume goes to direct match
			let matched_a_for_b = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
			let net_a = total_a_sold.saturating_sub(matched_a_for_b);

			let backward_n = U256::from(matched_a_for_b);
			let backward_d = U256::from(total_b_sold);

			if net_a == 0 {
				// Perfect cancel: A→B sellers also get spot
				let forward_out = a_as_b;
				return Some(PairClearing {
					forward_n: U256::from(forward_out),
					forward_d: U256::from(total_a_sold),
					backward_n,
					backward_d,
				});
			}

			match A::sell(asset_a, asset_b, net_a, None, state) {
				Ok((_new_state, exec)) => {
					let total_b_for_a = matched_b_for_a.saturating_add(exec.amount_out);
					Some(PairClearing {
						forward_n: U256::from(total_b_for_a),
						forward_d: U256::from(total_a_sold),
						backward_n,
						backward_d,
					})
				}
				Err(_) => None,
			}
		} else if total_b_sold > a_as_b || a_as_b == 0 {
			// Excess B (or can't compute): more B value than A value
			let b_as_a = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
			let matched_a_for_b = total_a_sold; // all A sellers' volume goes to direct match
			let matched_b_for_a = common::calc_amount_out(total_a_sold, pa, pb).unwrap_or(0);
			let net_b = total_b_sold.saturating_sub(matched_b_for_a);

			let forward_n = U256::from(matched_b_for_a);
			let forward_d = U256::from(total_a_sold);

			if net_b == 0 {
				let backward_out = b_as_a;
				return Some(PairClearing {
					forward_n,
					forward_d,
					backward_n: U256::from(backward_out),
					backward_d: U256::from(total_b_sold),
				});
			}

			match A::sell(asset_b, asset_a, net_b, None, state) {
				Ok((_new_state, exec)) => {
					let total_a_for_b = matched_a_for_b.saturating_add(exec.amount_out);
					Some(PairClearing {
						forward_n,
						forward_d,
						backward_n: U256::from(total_a_for_b),
						backward_d: U256::from(total_b_sold),
					})
				}
				Err(_) => None,
			}
		} else {
			// Perfect cancel — both at spot (a_as_b == total_b_sold)
			let b_as_a = common::calc_amount_out(total_b_sold, pb, pa).unwrap_or(0);
			Some(PairClearing {
				forward_n: U256::from(a_as_b),
				forward_d: U256::from(total_a_sold),
				backward_n: U256::from(b_as_a),
				backward_d: U256::from(total_b_sold),
			})
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
}
