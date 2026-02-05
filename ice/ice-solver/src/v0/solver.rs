use hydradx_traits::amm::AMMInterface;
use ice_support::{
	Intent, IntentData, PoolTrade, ResolvedIntent, ResolvedIntents, Solution, SolutionTrades, SwapData, SwapType,
};
use sp_std::collections::btree_map::BTreeMap;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

pub struct SolverV0<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> SolverV0<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State) -> Result<Solution, A::Error> {
		if intents.is_empty() {
			return Ok(Solution {
				resolved_intents: ResolvedIntents::truncate_from(Vec::new()),
				trades: SolutionTrades::truncate_from(Vec::new()),
				clearing_prices: BTreeMap::new(),
				score: 0,
			});
		}

		let mut resolved_intents = Vec::new();
		let mut executed_trades = Vec::new();

		let mut state = initial_state;

		for intent in intents {
			match &intent.data {
				IntentData::Swap(swap_data) => {
					let trade_result = match swap_data.swap_type {
						SwapType::ExactIn => A::sell(
							swap_data.asset_in,
							swap_data.asset_out,
							swap_data.amount_in,
							None,
							&state,
						),
						SwapType::ExactOut => A::buy(
							swap_data.asset_in,
							swap_data.asset_out,
							swap_data.amount_out,
							None,
							&state,
						),
					};

					let (new_state, trade_execution) = match trade_result {
						Ok(r) => r,
						Err(_) => continue,
					};

					let limits_satisfied = match swap_data.swap_type {
						SwapType::ExactIn => trade_execution.amount_out >= swap_data.amount_out,
						SwapType::ExactOut => trade_execution.amount_in <= swap_data.amount_in,
					};

					if !limits_satisfied {
						continue;
					}

					resolved_intents.push(ResolvedIntent {
						id: intent.id,
						data: IntentData::Swap(SwapData {
							asset_in: swap_data.asset_in,
							asset_out: swap_data.asset_out,
							amount_in: trade_execution.amount_in,
							amount_out: trade_execution.amount_out,
							swap_type: swap_data.swap_type,
							partial: false,
						}),
					});

					executed_trades.push(PoolTrade {
						direction: swap_data.swap_type,
						amount_in: trade_execution.amount_in,
						amount_out: trade_execution.amount_out,
						route: trade_execution.route,
					});

					state = new_state;
				}
			}
		}

		let solution = Solution {
			resolved_intents: ResolvedIntents::truncate_from(resolved_intents),
			trades: SolutionTrades::truncate_from(executed_trades),
			clearing_prices: BTreeMap::new(),
			score: 0,
		};

		Ok(solution)
	}
}
