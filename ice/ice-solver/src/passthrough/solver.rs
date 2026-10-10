//! Pass-through solve — deliberately dumb admission, no matching.
//!
//! Intents are walked in ascending id order (id = `deadline << 64 | seq`, so
//! ascending is roughly oldest-deadline-first) and each is quoted as a single
//! `ExactIn` sell of its full remaining amount against a state that carries
//! every accepted trade — two intents on the same pair legitimately clear at
//! different prices, which is exactly what the mode gives up the price
//! consistency check for.
//!
//! The only job here is admission: "will this route plausibly clear the limit
//! right now". Resolve amounts and the score are advisory — under
//! `SolverMode::Passthrough` the chain derives the binding amounts from intent
//! storage and pays the router's actual output — so a wrong quote costs one
//! skipped intent, never a reverted solution.

use crate::common::RouteCache;
use crate::{IceSolver, MinOuts};
use frame_support::sp_runtime::Permill;
use hydradx_traits::amm::AMMInterface;
use ice_support::{
	Balance, Intent, IntentData, PoolTrade, ResolvedIntent, ResolvedIntents, Solution, SolutionTrades, SwapData,
	SwapType, MAX_NUMBER_OF_RESOLVED_INTENTS, MAX_NUMBER_OF_SOLUTION_TRADES,
};
use sp_core::U256;
use sp_std::marker::PhantomData;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "solver::passthrough";

/// Every admitted intent emits exactly one trade, and the chain rejects a
/// pass-through solution whose two lists differ in length — so the binding limit
/// is whichever cap is lower, never `MAX_NUMBER_OF_RESOLVED_INTENTS` alone.
const MAX_ADMITTED: usize = if MAX_NUMBER_OF_RESOLVED_INTENTS < MAX_NUMBER_OF_SOLUTION_TRADES {
	MAX_NUMBER_OF_RESOLVED_INTENTS as usize
} else {
	MAX_NUMBER_OF_SOLUTION_TRADES as usize
};

/// The limit the chain applies to a full fill of `amount` — `IntentData::pro_rata`'s
/// floor division, reproduced exactly. `None` when it is not computable, which
/// every caller reads as "this intent cannot be admitted".
fn pro_rata(amount: Balance, swap: &SwapData) -> Option<Balance> {
	let value = U256::from(amount)
		.checked_mul(U256::from(swap.amount_out))?
		.checked_div(U256::from(swap.amount_in))?;
	Balance::try_from(value).ok()
}

pub struct Solver<A: AMMInterface> {
	_phantom: PhantomData<A>,
}

impl<A: AMMInterface> Solver<A> {
	pub fn solve(intents: Vec<Intent>, initial_state: A::State, matched_fee: Permill) -> Result<Solution, A::Error> {
		<Self as IceSolver<A>>::solve(intents, initial_state, matched_fee)
	}

	/// As `solve`, with per-intent admission floors (see `MinOuts`): an intent
	/// is admitted only when its quote clears `max(own limit, floor)`.
	pub fn solve_with_limits(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		matched_fee: Permill,
	) -> Result<Solution, A::Error> {
		<Self as IceSolver<A>>::solve_with_limits(intents, min_outs, initial_state, matched_fee)
	}
}

impl<A: AMMInterface> IceSolver<A> for Solver<A> {
	/// `matched_fee` is ignored: nothing is matched in this mode, so there is no
	/// matched volume to charge it on.
	fn solve_with_limits(
		intents: Vec<Intent>,
		min_outs: MinOuts,
		initial_state: A::State,
		_matched_fee: Permill,
	) -> Result<Solution, A::Error> {
		let mut ordered: Vec<&Intent> = intents.iter().collect();
		ordered.sort_unstable_by_key(|intent| intent.id);

		let mut cache = RouteCache::<A>::new();
		let mut state = initial_state;
		let mut resolved: Vec<ResolvedIntent> = Vec::new();
		let mut trades: Vec<PoolTrade> = Vec::new();
		let mut score: Balance = 0;

		for intent in ordered {
			if resolved.len() >= MAX_ADMITTED {
				log::debug!(target: LOG_TARGET, "solution cap of {MAX_ADMITTED} reached; the rest of the batch waits for a later block");
				break;
			}

			// `solver_intents()` transforms DCA intents into swaps before handing
			// them over; anything else cannot be quoted here.
			let IntentData::Swap(swap) = &intent.data else {
				log::debug!(target: LOG_TARGET, "intent {}: not a swap, skipping", intent.id);
				continue;
			};

			let amount = swap.remaining();
			let limit = if swap.partial.is_partial() {
				match pro_rata(amount, swap) {
					Some(limit) => limit,
					None => {
						log::debug!(target: LOG_TARGET, "intent {}: pro-rata limit not computable, skipping", intent.id);
						continue;
					}
				}
			} else {
				swap.amount_out
			};
			let floor = limit.max(min_outs.get(&intent.id).copied().unwrap_or(0));

			// The router rejects dust on either end, so an intent the chain
			// could not execute must not enter the solution at all.
			let ed_in = cache.ed(swap.asset_in).max(1);
			if amount < ed_in {
				log::debug!(target: LOG_TARGET, "intent {}: remaining {amount} below ED {ed_in}, skipping", intent.id);
				continue;
			}

			let Some((route, quote, next_state)) = cache.best_sell(swap.asset_in, swap.asset_out, amount, &state)
			else {
				log::debug!(target: LOG_TARGET, "intent {}: no usable route {} -> {}, skipping",
					intent.id, swap.asset_in, swap.asset_out);
				continue;
			};

			let ed_out = cache.ed(swap.asset_out).max(1);
			if quote < ed_out {
				log::debug!(target: LOG_TARGET, "intent {}: quote {quote} below ED {ed_out}, skipping", intent.id);
				continue;
			}
			if quote < floor {
				log::debug!(target: LOG_TARGET, "intent {}: quote {quote} below floor {floor}, skipping", intent.id);
				continue;
			}

			// Score stays on the intent's own limit — the floor is admission only.
			score = score.saturating_add(quote.saturating_sub(limit));
			resolved.push(ResolvedIntent {
				id: intent.id,
				data: IntentData::Swap(SwapData {
					asset_in: swap.asset_in,
					asset_out: swap.asset_out,
					amount_in: amount,
					amount_out: quote,
					// The pallet matches this against the stored intent, so a
					// partial must stay a partial with its filled amount intact.
					partial: swap.partial,
				}),
			});
			trades.push(PoolTrade {
				direction: SwapType::ExactIn,
				amount_in: amount,
				amount_out: quote,
				route,
			});
			state = next_state;
		}

		log::info!(
			target: LOG_TARGET,
			"passthrough solve: resolved={}/{} trades={} score={score}",
			resolved.len(),
			intents.len(),
			trades.len(),
		);

		Ok(Solution::new(
			ResolvedIntents::truncate_from(resolved),
			SolutionTrades::truncate_from(trades),
			score,
		))
	}
}
