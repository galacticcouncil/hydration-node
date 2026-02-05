//! V1 Solver
//!
//! Algorithm:
//! 1. Get spot prices for all assets involved in intents
//! 2. Filter intents that can be satisfied at spot price
//! 3. Calculate net flows per asset (surplus/deficit)
//! 4. Execute only net trades through AMM
//! 5. Distribute at uniform clearing price

use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::AMMInterface;
use ice_support::{
	AssetId, Balance, Intent, IntentData, PoolTrade, ResolvedIntent, ResolvedIntents, Solution, SolutionTrades,
	SwapData, SwapType,
};
use sp_core::{U256, U512};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

pub struct SolverV1<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

#[derive(Default, Debug, Clone)]
struct AssetFlow {
	total_in: Balance,
	total_out: Balance,
}

impl<A: AMMInterface> SolverV1<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(Solution {
				resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
				trades: SolutionTrades::truncate_from(Vec::new()),
				clearing_prices: BTreeMap::new(),
				score: 0,
			});
		}

		let denominator = A::price_denominator();

		let unique_assets = Self::collect_unique_assets(&intents);
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
						log::warn!(target: "solver", "Failed to get spot price for asset {}. Skipping.", asset);
						continue;
					}
				}
			}
		}

		let satisfiable_intents: Vec<&Intent> = intents
			.iter()
			.filter(|intent| Self::is_satisfiable(intent, &spot_prices))
			.collect();

		if satisfiable_intents.is_empty() {
			return Ok(Solution {
				resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
				trades: SolutionTrades::truncate_from(Vec::new()),
				clearing_prices: BTreeMap::new(),
				score: 0,
			});
		}

		let mut state = initial_state;
		let mut executed_trades: Vec<PoolTrade> = Vec::new();
		let mut actual_prices = spot_prices.clone();

		if satisfiable_intents.len() == 1 {
			// Single intent: execute direct trade without going through denominator
			let intent = satisfiable_intents[0];
			let IntentData::Swap(swap) = &intent.data;

			let trade_result = match swap.swap_type {
				SwapType::ExactIn => A::sell(swap.asset_in, swap.asset_out, swap.amount_in, None, &state),
				SwapType::ExactOut => A::buy(swap.asset_in, swap.asset_out, swap.amount_out, None, &state),
			};

			match trade_result {
				Ok((new_state, trade_execution)) => {
					let price_ratio = Ratio::new(trade_execution.amount_out, trade_execution.amount_in);
					actual_prices.insert(swap.asset_in, price_ratio);
					let inverse_ratio = Ratio::new(trade_execution.amount_in, trade_execution.amount_out);
					actual_prices.insert(swap.asset_out, inverse_ratio);

					executed_trades.push(PoolTrade {
						direction: swap.swap_type,
						amount_in: trade_execution.amount_in,
						amount_out: trade_execution.amount_out,
						route: trade_execution.route,
					});

					state = new_state;
				}
				Err(_) => {
					return Ok(Solution {
						resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
						trades: SolutionTrades::truncate_from(Vec::new()),
						clearing_prices: BTreeMap::new(),
						score: 0,
					});
				}
			}
		} else {
			// Multiple intents: match through denominator
			let flows = Self::calculate_flows(&satisfiable_intents, &spot_prices);

			// Track actual denominator balance as we execute trades
			// This accounts for price impact and execution differences
			let mut actual_denominator_balance: Balance = 0;

			// First pass: sell surplus non-denominator assets to get denominator
			for (asset, flow) in &flows {
				let net = flow.total_in as i128 - flow.total_out as i128;

				if net > 0 && *asset != denominator {
					let sell_amount = net as Balance;

					match A::sell(*asset, denominator, sell_amount, None, &state) {
						Ok((new_state, trade_execution)) => {
							let effective_price = Ratio::new(trade_execution.amount_out, trade_execution.amount_in);
							actual_prices.insert(*asset, effective_price);

							// Track the actual denominator received
							actual_denominator_balance =
								actual_denominator_balance.saturating_add(trade_execution.amount_out);

							executed_trades.push(PoolTrade {
								direction: SwapType::ExactIn,
								amount_in: trade_execution.amount_in,
								amount_out: trade_execution.amount_out,
								route: trade_execution.route,
							});

							state = new_state;
						}
						Err(_) => {
							continue;
						}
					}
				}
			}

			// Second pass: handle deficit non-denominator assets
			// Use actual denominator balance from first pass, not theoretical surplus
			for (asset, flow) in &flows {
				let net = flow.total_in as i128 - flow.total_out as i128;

				if net < 0 && *asset != denominator {
					if actual_denominator_balance > 0 {
						// Sell the actual denominator we have for the deficit asset
						let sell_amount = actual_denominator_balance;

						match A::sell(denominator, *asset, sell_amount, None, &state) {
							Ok((new_state, trade_execution)) => {
								let asset_price = Ratio::new(trade_execution.amount_in, trade_execution.amount_out);
								actual_prices.insert(*asset, asset_price);

								// Use what we actually spent
								actual_denominator_balance =
									actual_denominator_balance.saturating_sub(trade_execution.amount_in);

								executed_trades.push(PoolTrade {
									direction: SwapType::ExactIn,
									amount_in: trade_execution.amount_in,
									amount_out: trade_execution.amount_out,
									route: trade_execution.route,
								});

								state = new_state;
							}
							Err(_) => {
								continue;
							}
						}
					} else {
						let buy_amount = (-net) as Balance;

						match A::buy(denominator, *asset, buy_amount, None, &state) {
							Ok((new_state, trade_execution)) => {
								let effective_price = Ratio::new(trade_execution.amount_in, trade_execution.amount_out);
								actual_prices.insert(*asset, effective_price);

								executed_trades.push(PoolTrade {
									direction: SwapType::ExactOut,
									amount_in: trade_execution.amount_in,
									amount_out: trade_execution.amount_out,
									route: trade_execution.route,
								});

								state = new_state;
							}
							Err(_) => {
								continue;
							}
						}
					}
				}
			}
		}

		let mut resolved_intents: Vec<ResolvedIntent> = Vec::new();
		let mut total_score: Balance = 0;

		// For single intent with direct trade, use actual trade execution amounts
		if satisfiable_intents.len() == 1 && executed_trades.len() == 1 {
			let intent = satisfiable_intents[0];
			let IntentData::Swap(swap) = &intent.data;
			let trade = &executed_trades[0];

			let limits_ok = match swap.swap_type {
				SwapType::ExactIn => trade.amount_out >= swap.amount_out,
				SwapType::ExactOut => trade.amount_in <= swap.amount_in,
			};

			if limits_ok {
				let surplus = match swap.swap_type {
					SwapType::ExactIn => trade.amount_out.saturating_sub(swap.amount_out),
					SwapType::ExactOut => swap.amount_in.saturating_sub(trade.amount_in),
				};
				total_score = surplus;

				resolved_intents.push(ResolvedIntent {
					id: intent.id,
					data: IntentData::Swap(SwapData {
						asset_in: swap.asset_in,
						asset_out: swap.asset_out,
						amount_in: trade.amount_in,
						amount_out: trade.amount_out,
						swap_type: swap.swap_type,
						partial: false,
					}),
				});
			}
		} else {
			// Multiple intents: use price-based resolution with conservation checks
			let mut available: BTreeMap<AssetId, Balance> = BTreeMap::new();

			for intent in &satisfiable_intents {
				let IntentData::Swap(swap) = &intent.data;
				let amount_in = match swap.swap_type {
					SwapType::ExactIn => swap.amount_in,
					SwapType::ExactOut => {
						if let (Some(price_in), Some(price_out)) =
							(actual_prices.get(&swap.asset_in), actual_prices.get(&swap.asset_out))
						{
							Self::calc_amount_in(swap.amount_out, price_in, price_out).unwrap_or(swap.amount_in)
						} else {
							swap.amount_in
						}
					}
				};
				*available.entry(swap.asset_in).or_default() += amount_in;
			}

			for trade in &executed_trades {
				let asset_in = trade.route.first().map(|t| t.asset_in).unwrap_or(0);
				let asset_out = trade.route.last().map(|t| t.asset_out).unwrap_or(0);

				if let Some(bal) = available.get_mut(&asset_in) {
					*bal = bal.saturating_sub(trade.amount_in);
				}
				*available.entry(asset_out).or_default() += trade.amount_out;
			}

			let mut ideal_resolutions: Vec<(usize, ResolvedIntent)> = Vec::new();
			let mut total_output_per_asset: BTreeMap<AssetId, Balance> = BTreeMap::new();

			for (idx, intent) in satisfiable_intents.iter().enumerate() {
				if let Some(resolved) = Self::resolve_intent(intent, &actual_prices) {
					let amount_out = resolved.data.amount_out();
					let asset_out = resolved.data.asset_out();
					*total_output_per_asset.entry(asset_out).or_default() += amount_out;
					ideal_resolutions.push((idx, resolved));
				}
			}

			// Process ExactOut first (they need exact amounts), then ExactIn (can be scaled)
			let mut committed_output: BTreeMap<AssetId, Balance> = BTreeMap::new();

			for (idx, resolved) in ideal_resolutions.iter() {
				let intent = satisfiable_intents[*idx];
				let IntentData::Swap(swap) = &intent.data;

				if swap.swap_type != SwapType::ExactOut {
					continue;
				}

				let asset_out = resolved.data.asset_out();
				let amount_out = swap.amount_out;
				let avail = available.get(&asset_out).copied().unwrap_or(0);
				let already_committed = committed_output.get(&asset_out).copied().unwrap_or(0);

				if already_committed + amount_out > avail {
					continue;
				}

				let (Some(price_in), Some(price_out)) =
					(actual_prices.get(&swap.asset_in), actual_prices.get(&swap.asset_out))
				else {
					continue;
				};

				let Some(actual_in) = Self::calc_amount_in(amount_out, price_in, price_out) else {
					continue;
				};

				if actual_in > swap.amount_in {
					continue;
				}

				*committed_output.entry(asset_out).or_default() += amount_out;

				let surplus = swap.amount_in.saturating_sub(actual_in);
				total_score = total_score.saturating_add(surplus);

				resolved_intents.push(ResolvedIntent {
					id: intent.id,
					data: IntentData::Swap(SwapData {
						asset_in: swap.asset_in,
						asset_out: swap.asset_out,
						amount_in: actual_in,
						amount_out,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				});
			}

			// Process ExactIn intents with remaining availability
			let mut remaining_avail: BTreeMap<AssetId, Balance> = BTreeMap::new();
			for (asset, &avail) in &available {
				let committed = committed_output.get(asset).copied().unwrap_or(0);
				remaining_avail.insert(*asset, avail.saturating_sub(committed));
			}

			let mut exactin_demand: BTreeMap<AssetId, Balance> = BTreeMap::new();
			for (idx, resolved) in ideal_resolutions.iter() {
				let intent = satisfiable_intents[*idx];
				let IntentData::Swap(swap) = &intent.data;

				if swap.swap_type != SwapType::ExactIn {
					continue;
				}

				let asset_out = resolved.data.asset_out();
				let ideal_amount = resolved.data.amount_out();
				*exactin_demand.entry(asset_out).or_default() += ideal_amount;
			}

			for (idx, resolved) in ideal_resolutions {
				let intent = satisfiable_intents[idx];
				let IntentData::Swap(swap) = &intent.data;

				if swap.swap_type != SwapType::ExactIn {
					continue;
				}

				let asset_out = resolved.data.asset_out();
				let ideal_amount = resolved.data.amount_out();
				let remaining = remaining_avail.get(&asset_out).copied().unwrap_or(0);
				let total_demand = exactin_demand.get(&asset_out).copied().unwrap_or(0);

				// Scale down proportionally if total ExactIn demand exceeds remaining availability
				let actual_out = if total_demand > remaining && total_demand > 0 {
					U256::from(ideal_amount)
						.checked_mul(U256::from(remaining))
						.and_then(|n| n.checked_div(U256::from(total_demand)))
						.map(|r| r.as_u128())
						.unwrap_or(ideal_amount)
				} else {
					ideal_amount
				};

				if actual_out < swap.amount_out {
					continue;
				}

				let surplus = actual_out.saturating_sub(swap.amount_out);
				total_score = total_score.saturating_add(surplus);

				resolved_intents.push(ResolvedIntent {
					id: intent.id,
					data: IntentData::Swap(SwapData {
						asset_in: swap.asset_in,
						asset_out: swap.asset_out,
						amount_in: swap.amount_in,
						amount_out: actual_out,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				});
			}
		}

		let clearing_prices: BTreeMap<AssetId, Ratio> = actual_prices;

		Ok(Solution {
			resolved_intents: ResolvedIntents::truncate_from(resolved_intents),
			trades: SolutionTrades::truncate_from(executed_trades),
			clearing_prices,
			score: total_score,
		})
	}

	fn collect_unique_assets(intents: &[Intent]) -> Vec<AssetId> {
		let mut assets: Vec<AssetId> = Vec::new();
		for intent in intents {
			match &intent.data {
				IntentData::Swap(swap) => {
					if !assets.contains(&swap.asset_in) {
						assets.push(swap.asset_in);
					}
					if !assets.contains(&swap.asset_out) {
						assets.push(swap.asset_out);
					}
				}
			}
		}
		assets
	}

	fn is_satisfiable(intent: &Intent, spot_prices: &BTreeMap<AssetId, Ratio>) -> bool {
		match &intent.data {
			IntentData::Swap(swap) => {
				let Some(price_in) = spot_prices.get(&swap.asset_in) else {
					return false;
				};
				let Some(price_out) = spot_prices.get(&swap.asset_out) else {
					return false;
				};

				match swap.swap_type {
					SwapType::ExactIn => {
						let Some(calculated_out) = Self::calc_amount_out(swap.amount_in, price_in, price_out) else {
							return false;
						};
						calculated_out >= swap.amount_out
					}
					SwapType::ExactOut => {
						let Some(calculated_in) = Self::calc_amount_in(swap.amount_out, price_in, price_out) else {
							return false;
						};
						calculated_in <= swap.amount_in
					}
				}
			}
		}
	}

	/// in = amount_out × (price_out / price_in)
	fn calc_amount_in(amount_out: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
		let n = U512::from(price_out.n) * U512::from(price_in.d);
		let d = U512::from(price_out.d) * U512::from(price_in.n);
		let result = U512::from(amount_out).checked_mul(n)?.checked_div(d)?;
		result.try_into().ok()
	}

	fn calculate_flows(intents: &[&Intent], spot_prices: &BTreeMap<AssetId, Ratio>) -> BTreeMap<AssetId, AssetFlow> {
		let mut flows: BTreeMap<AssetId, AssetFlow> = BTreeMap::new();

		for intent in intents {
			match &intent.data {
				IntentData::Swap(swap) => {
					if let (Some(price_in), Some(price_out)) =
						(spot_prices.get(&swap.asset_in), spot_prices.get(&swap.asset_out))
					{
						match swap.swap_type {
							SwapType::ExactIn => {
								flows.entry(swap.asset_in).or_default().total_in += swap.amount_in;
								if let Some(amount_out) = Self::calc_amount_out(swap.amount_in, price_in, price_out) {
									flows.entry(swap.asset_out).or_default().total_out += amount_out;
								}
							}
							SwapType::ExactOut => {
								flows.entry(swap.asset_out).or_default().total_out += swap.amount_out;
								if let Some(amount_in) = Self::calc_amount_in(swap.amount_out, price_in, price_out) {
									flows.entry(swap.asset_in).or_default().total_in += amount_in;
								}
							}
						}
					}
				}
			}
		}

		flows
	}

	fn resolve_intent(intent: &Intent, prices: &BTreeMap<AssetId, Ratio>) -> Option<ResolvedIntent> {
		match &intent.data {
			IntentData::Swap(swap) => {
				let price_in = prices.get(&swap.asset_in)?;
				let price_out = prices.get(&swap.asset_out)?;

				match swap.swap_type {
					SwapType::ExactIn => {
						let amount_out = Self::calc_amount_out(swap.amount_in, price_in, price_out)?;

						if amount_out < swap.amount_out {
							return None;
						}

						Some(ResolvedIntent {
							id: intent.id,
							data: IntentData::Swap(SwapData {
								asset_in: swap.asset_in,
								asset_out: swap.asset_out,
								amount_in: swap.amount_in,
								amount_out,
								swap_type: SwapType::ExactIn,
								partial: false,
							}),
						})
					}
					SwapType::ExactOut => {
						let amount_in = Self::calc_amount_in(swap.amount_out, price_in, price_out)?;

						if amount_in > swap.amount_in {
							return None;
						}

						Some(ResolvedIntent {
							id: intent.id,
							data: IntentData::Swap(SwapData {
								asset_in: swap.asset_in,
								asset_out: swap.asset_out,
								amount_in,
								amount_out: swap.amount_out,
								swap_type: SwapType::ExactOut,
								partial: false,
							}),
						})
					}
				}
			}
		}
	}

	/// out = amount_in × (price_in / price_out)
	fn calc_amount_out(amount_in: Balance, price_in: &Ratio, price_out: &Ratio) -> Option<Balance> {
		let n = U512::from(price_in.n) * U512::from(price_out.d);
		let d = U512::from(price_in.d) * U512::from(price_out.n);
		let result = U512::from(amount_in).checked_mul(n)?.checked_div(d)?;
		result.try_into().ok()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use ice_support::IntentId;

	fn make_sell_intent(
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
				swap_type: SwapType::ExactIn,
				partial: false,
			}),
		}
	}

	#[test]
	fn test_is_satisfiable_at_spot_price() {
		let mut prices = BTreeMap::new();
		prices.insert(1u32, Ratio::new(1, 100));
		prices.insert(2u32, Ratio::new(2, 100));

		let intent = make_sell_intent(1, 1, 2, 100, 40);
		assert!(SolverV1::<MockAMM>::is_satisfiable(&intent, &prices));

		let intent2 = make_sell_intent(2, 1, 2, 100, 60);
		assert!(!SolverV1::<MockAMM>::is_satisfiable(&intent2, &prices));
	}

	#[test]
	fn test_calc_amount_out() {
		let price_in = Ratio::new(1, 100);
		let price_out = Ratio::new(2, 100);

		let result = SolverV1::<MockAMM>::calc_amount_out(100, &price_in, &price_out);
		assert_eq!(result, Some(50));
	}

	#[test]
	fn test_calculate_flows() {
		let mut prices = BTreeMap::new();
		prices.insert(1u32, Ratio::new(1, 100));
		prices.insert(2u32, Ratio::new(2, 100));

		let intents = [
			make_sell_intent(1, 1, 2, 100, 40),
			make_sell_intent(2, 2, 1, 60, 100),
			make_sell_intent(3, 1, 2, 50, 20),
		];

		let intent_refs: Vec<&Intent> = intents.iter().collect();
		let flows = SolverV1::<MockAMM>::calculate_flows(&intent_refs, &prices);

		assert_eq!(flows.get(&1u32).map(|f| f.total_in), Some(150));
		assert_eq!(flows.get(&1u32).map(|f| f.total_out), Some(120));

		assert_eq!(flows.get(&2u32).map(|f| f.total_in), Some(60));
		assert_eq!(flows.get(&2u32).map(|f| f.total_out), Some(75));
	}

	struct MockAMM;

	impl AMMInterface for MockAMM {
		type Error = ();
		type State = ();

		fn sell(
			_asset_in: u32,
			_asset_out: u32,
			_amount_in: u128,
			_route: Option<hydradx_traits::router::Route<u32>>,
			_state: &Self::State,
		) -> Result<(Self::State, hydradx_traits::amm::TradeExecution), Self::Error> {
			unimplemented!()
		}

		fn buy(
			_asset_in: u32,
			_asset_out: u32,
			_amount_out: u128,
			_route: Option<hydradx_traits::router::Route<u32>>,
			_state: &Self::State,
		) -> Result<(Self::State, hydradx_traits::amm::TradeExecution), Self::Error> {
			unimplemented!()
		}

		fn get_spot_price(_asset_in: u32, _asset_out: u32, _state: &Self::State) -> Result<Ratio, Self::Error> {
			unimplemented!()
		}

		fn price_denominator() -> u32 {
			0
		}
	}
}
