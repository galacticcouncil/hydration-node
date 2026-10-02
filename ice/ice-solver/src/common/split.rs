//! Splitting one AMM transfer across several routes of the same pair.
//!
//! The single best route is the baseline. The other routes are tried as extra
//! legs, sized on a grid plus each one's probed capacity, and every candidate is
//! simulated leg by leg in emission order — the order `submit_solution`
//! executes the trades in — so legs through a shared pool are priced exactly as
//! they will execute. A split is kept only when it beats the single route by a
//! margin that pays for the extra legs.

use crate::common::RouteCache;
use hydradx_traits::amm::AMMInterface;
use hydradx_traits::router::{PoolType, Route};
use ice_support::{AssetId, Balance};
use sp_core::U256;
use sp_std::vec;
use sp_std::vec::Vec;

const LOG_TARGET: &str = "solver::split";

/// AMM outputs are haircut by 1 bps so the on-chain execution can never
/// undershoot the solver's claim.
const AMM_SIMULATION_TOLERANCE_BPS: Balance = 1;

/// Candidate sizes per extra route in trade building (5 % steps).
pub const SPLIT_GRID: u32 = 20;
/// Coarser grid for fitting quotes, which the crossing requests up to 128 times
/// per trimmed intent, and for three-leg searches, whose candidates multiply.
pub const LIGHT_SPLIT_GRID: u32 = 10;
/// Beyond three legs the candidate product calls for a different search.
pub const MAX_SPLIT_LEGS: u8 = 3;
const MIN_SPLIT_GAIN_BPS: u32 = 10;
/// A v3 leg declares ~725 KB of proof size, so adding one must earn more.
const V3_LEG_SURCHARGE_BPS: u32 = 25;
/// Halvings of one grid step tried before an extra route is given up; a route
/// that cannot take 1/256 of a step cannot move the result past the threshold.
const MAX_PROBE_HALVINGS: u32 = 8;
const CAPACITY_BISECTION_STEPS: u32 = 8;
/// Extra routes searched per transfer, best rate first.
const MAX_CANDIDATES: usize = 4;
/// Left in the holding pot per extra leg. Every leg is its own transfer out of
/// the pot, and an aToken transfer rounds its scaled amount up, so two legs of
/// an aToken can overdraw by a wei where one would not.
const ROUNDING_DUST: Balance = 2;

pub fn adjust_amm_output(simulated_out: Balance) -> Balance {
	simulated_out.saturating_sub(simulated_out * AMM_SIMULATION_TOLERANCE_BPS / 10_000)
}

/// [`adjust_amm_output`] with the tolerance rounded up, for deciding whether a
/// trade clears a limit. Rounded down, the tolerance vanishes on a small enough
/// trade, and a limit the market misses by less than the tolerance is then filled
/// in slivers no larger trade at the same rate could clear.
pub fn adjust_amm_output_strict(simulated_out: Balance) -> Balance {
	simulated_out.saturating_sub(
		simulated_out
			.saturating_mul(AMM_SIMULATION_TOLERANCE_BPS)
			.div_ceil(10_000),
	)
}

/// One `ExactIn` trade of a transfer; `amount_out` is the raw simulated output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leg {
	pub route: Route<AssetId>,
	pub amount_in: Balance,
	pub amount_out: Balance,
}

impl Leg {
	/// The trade's on-chain `min_amount_out`.
	pub fn claimed_out(&self) -> Balance {
		adjust_amm_output(self.amount_out)
	}
}

pub fn amount_in(legs: &[Leg]) -> Balance {
	legs.iter().fold(0, |acc, l| acc.saturating_add(l.amount_in))
}

pub fn raw_out(legs: &[Leg]) -> Balance {
	legs.iter().fold(0, |acc, l| acc.saturating_add(l.amount_out))
}

pub fn claimed_out(legs: &[Leg]) -> Balance {
	legs.iter().fold(0, |acc, l| acc.saturating_add(l.claimed_out()))
}

/// A transfer's legs in emission order and the state after all of them.
pub struct Routed<S> {
	pub legs: Vec<Leg>,
	pub state: S,
}

/// Extra `sell` simulations one phase may spend on split searches. Once spent,
/// every further transfer takes its single best route.
#[derive(Clone, Copy, Debug)]
pub struct SellBudget {
	left: u32,
	spent: u32,
}

struct Exhausted;

impl SellBudget {
	pub fn new(sells: u32) -> Self {
		Self { left: sells, spent: 0 }
	}

	pub fn spent(&self) -> u32 {
		self.spent
	}

	fn take(&mut self) -> Result<(), Exhausted> {
		self.left = self.left.checked_sub(1).ok_or(Exhausted)?;
		self.spent = self.spent.saturating_add(1);
		Ok(())
	}
}

#[derive(Clone, Copy)]
struct Pair {
	asset_in: AssetId,
	asset_out: AssetId,
	ed_in: Balance,
	ed_out: Balance,
}

/// One budgeted simulation, `None` unless `submit_solution` would execute it:
/// the pallet skips a trade with either end below the ED, and a skipped trade
/// never pays into the holding pot.
fn leg<A: AMMInterface>(
	pair: Pair,
	route: &Route<AssetId>,
	amount_in: Balance,
	state: &A::State,
	budget: &mut SellBudget,
) -> Result<Option<(A::State, Balance)>, Exhausted> {
	if amount_in < pair.ed_in {
		return Ok(None);
	}
	budget.take()?;
	let Ok((next, exec)) = A::sell(pair.asset_in, pair.asset_out, amount_in, route.clone(), state) else {
		return Ok(None);
	};
	if adjust_amm_output(exec.amount_out) < pair.ed_out {
		return Ok(None);
	}
	Ok(Some((next, exec.amount_out)))
}

/// Largest input up to `limit` the route still executes, probed upward from a
/// known-good `from`; `true` when it fails somewhere below the limit.
///
/// Every probe starts from `state`, never from a previous probe's result — a
/// v3 pool refuses a second trade, so a threaded probe would poison the rest.
fn capacity<A: AMMInterface>(
	pair: Pair,
	route: &Route<AssetId>,
	from: Balance,
	limit: Balance,
	state: &A::State,
	budget: &mut SellBudget,
) -> Result<(Balance, bool), Exhausted> {
	let mut ok = from.min(limit);
	let mut failed = None;
	while ok < limit {
		let next = ok.saturating_mul(2).min(limit);
		if leg::<A>(pair, route, next, state, budget)?.is_none() {
			failed = Some(next);
			break;
		}
		ok = next;
	}
	let Some(mut bad) = failed else {
		return Ok((ok, false));
	};
	for _ in 0..CAPACITY_BISECTION_STEPS {
		let mid = ok + (bad - ok) / 2;
		if mid == ok {
			break;
		}
		if leg::<A>(pair, route, mid, state, budget)?.is_some() {
			ok = mid;
		} else {
			bad = mid;
		}
	}
	Ok((ok, true))
}

/// Simulate `plan` leg by leg, each against the state the previous one left.
fn materialise<A: AMMInterface>(
	pair: Pair,
	routes: &[Route<AssetId>],
	plan: &[(usize, Balance)],
	state: &A::State,
	budget: &mut SellBudget,
) -> Result<Option<Routed<A::State>>, Exhausted> {
	let mut current: Option<A::State> = None;
	let mut legs = Vec::with_capacity(plan.len());
	for &(i, amount_in) in plan {
		let Some((next, amount_out)) =
			leg::<A>(pair, &routes[i], amount_in, current.as_ref().unwrap_or(state), budget)?
		else {
			return Ok(None);
		};
		legs.push(Leg {
			route: routes[i].clone(),
			amount_in,
			amount_out,
		});
		current = Some(next);
	}
	Ok(current.map(|state| Routed { legs, state }))
}

/// Whether `out` beats the single route's `base` by enough to pay for the
/// legs it adds: a flat margin, plus a surcharge for every added v3 leg.
fn clears_margin(out: Balance, base: Balance, legs: &[Leg], single: &Route<AssetId>) -> bool {
	let added_v3 = legs
		.iter()
		.filter(|l| l.route != *single && l.route.iter().any(|t| matches!(t.pool, PoolType::UniswapV3(_))))
		.count() as u32;
	let margin_bps = MIN_SPLIT_GAIN_BPS.saturating_add(added_v3.saturating_mul(V3_LEG_SURCHARGE_BPS));
	U256::from(out) * U256::from(10_000u32) >= U256::from(base) * U256::from(10_000u32.saturating_add(margin_bps))
}

/// Advance a mixed-radix counter; `false` once every combination was visited.
fn advance(idx: &mut [usize], sizes: &[Vec<Balance>]) -> bool {
	for k in (0..idx.len()).rev() {
		idx[k] += 1;
		if idx[k] < sizes[k].len() {
			return true;
		}
		idx[k] = 0;
	}
	false
}

struct Satellite {
	index: usize,
	/// Largest of one grid step, a half, a quarter, … the route executes.
	probe: Balance,
	out: Balance,
}

impl<A: AMMInterface> RouteCache<A> {
	/// Best allocation of `amount_in` over at most `max_legs` of the pair's
	/// routes, simulated against `state`.
	///
	/// `None` exactly when [`Self::best_sell`] finds no route. With
	/// `max_legs < 2`, or when no split clears the gain threshold, the result is
	/// `best_sell`'s single route as one leg.
	#[allow(clippy::too_many_arguments)]
	pub fn best_split_sell(
		&mut self,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		state: &A::State,
		max_legs: u8,
		grid: u32,
		budget: &mut SellBudget,
	) -> Option<Routed<A::State>> {
		let (route, amount_out, after) = self.best_sell(asset_in, asset_out, amount_in, state)?;
		let single = Leg {
			route,
			amount_in,
			amount_out,
		};
		if max_legs >= 2 && self.routes(asset_in, asset_out, state).len() >= 2 {
			let pair = Pair {
				asset_in,
				asset_out,
				ed_in: self.ed(asset_in).max(1),
				ed_out: self.ed(asset_out).max(1),
			};
			let routes = self.routes(asset_in, asset_out, state).to_vec();
			let legs = max_legs.min(MAX_SPLIT_LEGS);
			if let Ok(Some(split)) = split::<A>(pair, &routes, &single, state, legs, grid.max(2), budget) {
				return Some(split);
			}
		}
		Some(Routed {
			legs: vec![single],
			state: after,
		})
	}
}

fn split<A: AMMInterface>(
	pair: Pair,
	routes: &[Route<AssetId>],
	single: &Leg,
	state: &A::State,
	legs: u8,
	grid: u32,
	budget: &mut SellBudget,
) -> Result<Option<Routed<A::State>>, Exhausted> {
	let amount = single.amount_in;
	let Some(anchor) = routes.iter().rposition(|r| *r == single.route) else {
		return Ok(None);
	};
	// Legs may share pools: they are simulated in sequence, so a shared pool is
	// priced with the earlier leg's impact, and excluding it would drop routes
	// that only share a hop with spare capacity (USDT -> SKY on mainnet: the one
	// route that can take the amount ends in the same Omnipool hop as the one a
	// saturated stableswap caps).
	let others: Vec<usize> = (0..routes.len()).filter(|&i| i != anchor).collect();
	if others.is_empty() || amount < pair.ed_in.saturating_mul(2) {
		return Ok(None);
	}
	let step = (amount / Balance::from(grid)).max(pair.ed_in);

	// An extra route can only help if its first step pays more than the
	// anchor's last one.
	let Some((_, below)) = leg::<A>(pair, &routes[anchor], amount - step, state, budget)? else {
		return Ok(None);
	};
	let anchor_last_step = single.amount_out.saturating_sub(below);

	let mut candidates: Vec<Satellite> = Vec::new();
	for index in others {
		let route = &routes[index];
		let mut probe = step;
		for _ in 0..=MAX_PROBE_HALVINGS {
			if let Some((_, out)) = leg::<A>(pair, route, probe, state, budget)? {
				if U256::from(out) * U256::from(step) > U256::from(anchor_last_step) * U256::from(probe) {
					candidates.push(Satellite { index, probe, out });
				}
				break;
			}
			probe /= 2;
			if probe < pair.ed_in {
				break;
			}
		}
	}
	// Best rate first, route order on ties. The best rate can come with the
	// least capacity, so each candidate gets its own search below.
	candidates.sort_by(|x, y| {
		(U256::from(y.out) * U256::from(x.probe))
			.cmp(&(U256::from(x.out) * U256::from(y.probe)))
			.then(x.index.cmp(&y.index))
	});
	candidates.truncate(MAX_CANDIDATES);
	if candidates.is_empty() {
		return Ok(None);
	}

	// Sizes per candidate: none, every grid step below its capacity, and the
	// capacity itself — the point a fixed grid misses when a capped route is
	// worth less than one step. A capacity that ends at a cliff is backed off
	// by 1/16 so a pool that moves before execution does not revert the leg.
	let mut sizes: Vec<Vec<Balance>> = Vec::with_capacity(candidates.len());
	for c in &candidates {
		let (cap, cliff) = capacity::<A>(pair, &routes[c.index], c.probe, amount - pair.ed_in, state, budget)?;
		let cap = if cliff { (cap - cap / 16).max(pair.ed_in) } else { cap };
		let mut options = vec![0];
		options.extend(
			(1..=Balance::from(grid))
				.map(|j| step.saturating_mul(j))
				.take_while(|x| *x < cap),
		);
		options.push(cap);
		sizes.push(options);
	}

	let mut sets: Vec<Vec<usize>> = (0..candidates.len()).map(|c| vec![c]).collect();
	if legs >= 3 {
		for x in 0..candidates.len() {
			sets.extend((x + 1..candidates.len()).map(|y| vec![x, y]));
		}
	}

	let base = single.claimed_out();
	let mut best: Option<(Balance, Routed<A::State>)> = None;
	for set in sets {
		let set_sizes: Vec<Vec<Balance>> = set.iter().map(|&c| sizes[c].clone()).collect();
		let mut idx = vec![0usize; set.len()];
		while advance(&mut idx, &set_sizes) {
			let alloc: Vec<Balance> = idx.iter().zip(&set_sizes).map(|(&i, s)| s[i]).collect();
			let used = alloc.iter().fold(0, |acc: Balance, x| acc.saturating_add(*x));
			let Some(rest) = amount.checked_sub(used) else {
				continue;
			};
			if rest > 0 && rest < pair.ed_in {
				continue;
			}
			let mut plan: Vec<(usize, Balance)> = set
				.iter()
				.zip(&alloc)
				.filter(|(_, x)| **x > 0)
				.map(|(&c, x)| (candidates[c].index, *x))
				.collect();
			if rest > 0 {
				plan.push((anchor, rest));
			}
			let dust = ROUNDING_DUST.saturating_mul(plan.len().saturating_sub(1) as Balance);
			if let Some(largest) = plan.iter_mut().max_by_key(|(_, x)| *x) {
				largest.1 = largest.1.saturating_sub(dust);
			}
			plan.sort_by_key(|(i, _)| *i);
			let Some(routed) = materialise::<A>(pair, routes, &plan, state, budget)? else {
				continue;
			};
			let out = claimed_out(&routed.legs);
			if !clears_margin(out, base, &routed.legs, &single.route) {
				continue;
			}
			// Ties go to fewer legs, then to the first allocation visited.
			let better = best.as_ref().is_none_or(|(best_out, best_routed)| {
				out > *best_out || (out == *best_out && routed.legs.len() < best_routed.legs.len())
			});
			if better {
				best = Some((out, routed));
			}
		}
	}

	let Some((out, routed)) = best else {
		return Ok(None);
	};
	log::debug!(
		target: LOG_TARGET,
		"{} -> {}: {amount} split into {} legs, {out} vs {base} on the single route",
		pair.asset_in,
		pair.asset_out,
		routed.legs.len(),
	);
	Ok(Some(routed))
}
