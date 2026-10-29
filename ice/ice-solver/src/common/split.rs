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

/// Haircut on one hop of a given venue.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HopTolerance {
	pub ppm: u32,
	/// Flat, for venues whose error is rounding rather than drift.
	pub units: Balance,
}

/// How far below the simulation a trade claims, so on-chain execution never
/// undershoots it: `route_ppm` plus each hop's `ppm` of the output, plus each
/// hop's `units` carried through the hops after it. Hops add up because their
/// errors compound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tolerance {
	pub route_ppm: u32,
	pub omnipool: HopTolerance,
	pub stableswap: HopTolerance,
	pub xyk: HopTolerance,
	pub lbp: HopTolerance,
	pub aave: HopTolerance,
	pub hsm: HopTolerance,
	pub uniswap_v3: HopTolerance,
}

impl Default for Tolerance {
	/// Per-venue table measured on mainnet: Omnipool drifts up to 1 bps between
	/// build and execution, Stableswap and Aave only round, Uniswap v3 always
	/// underquotes. XYK, LBP and HSM keep 1 bps because they are unmeasured.
	fn default() -> Self {
		Self {
			route_ppm: 0,
			omnipool: HopTolerance { ppm: 100, units: 0 },
			stableswap: HopTolerance { ppm: 1, units: 2 },
			xyk: HopTolerance { ppm: 100, units: 0 },
			lbp: HopTolerance { ppm: 100, units: 0 },
			aave: HopTolerance { ppm: 0, units: 2 },
			hsm: HopTolerance { ppm: 100, units: 0 },
			uniswap_v3: HopTolerance::default(),
		}
	}
}

impl Tolerance {
	fn hop(&self, pool: &PoolType<AssetId>) -> HopTolerance {
		match pool {
			PoolType::Omnipool => self.omnipool,
			PoolType::Stableswap(_) => self.stableswap,
			PoolType::XYK => self.xyk,
			PoolType::LBP => self.lbp,
			PoolType::Aave => self.aave,
			PoolType::HSM => self.hsm,
			PoolType::UniswapV3(_) => self.uniswap_v3,
		}
	}

	fn claim(&self, leg: &Leg, round_up: bool) -> Balance {
		let ppm = leg
			.route
			.iter()
			.fold(self.route_ppm, |p, t| p.saturating_add(self.hop(&t.pool).ppm));
		let scaled = U256::from(leg.amount_out) * U256::from(ppm);
		let share = if round_up {
			scaled.saturating_add(U256::from(999_999u32)) / 1_000_000u32
		} else {
			scaled / 1_000_000u32
		};
		let haircut = Balance::try_from(share)
			.unwrap_or(Balance::MAX)
			.saturating_add(leg.units);
		leg.amount_out.saturating_sub(haircut)
	}

	/// Every hop's `units` in the route's output asset, given each hop's output.
	/// A hop that delivers `units` less shrinks the hops after it by at most their
	/// average rate — a marginal rate never beats it — so each is scaled by
	/// `out / hop_out`, rounded up.
	pub fn units_out(&self, route: &Route<AssetId>, hop_outs: &[Balance]) -> Balance {
		let out = hop_outs.last().copied().unwrap_or_default();
		route.iter().zip(hop_outs).fold(0, |acc: Balance, (t, &hop_out)| {
			let units = self.hop(&t.pool).units;
			let scaled = if units == 0 || hop_out == out {
				units
			} else if hop_out == 0 {
				Balance::MAX
			} else {
				let n = U256::from(units) * U256::from(out);
				let d = U256::from(hop_out);
				Balance::try_from((n + d - 1) / d).unwrap_or(Balance::MAX)
			};
			acc.saturating_add(scaled)
		})
	}

	/// `key=ppm[+units],...` over the default, keys `route`, `omnipool`,
	/// `stableswap`, `xyk`, `lbp`, `aave`, `hsm`, `uniswapv3`; `route` takes no units.
	pub fn parse(spec: &str) -> Result<Self, &'static str> {
		let mut t = Self::default();
		for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
			let (key, value) = item.split_once('=').ok_or("expected key=ppm[+units]")?;
			let (ppm, units) = value.split_once('+').unwrap_or((value, "0"));
			let hop = HopTolerance {
				ppm: ppm.trim().parse().map_err(|_| "ppm must be a u32")?,
				units: units.trim().parse().map_err(|_| "units must be a u128")?,
			};
			let slot = match key.trim() {
				"route" if hop.units == 0 => {
					t.route_ppm = hop.ppm;
					continue;
				}
				"route" => return Err("route takes no units"),
				"omnipool" => &mut t.omnipool,
				"stableswap" => &mut t.stableswap,
				"xyk" => &mut t.xyk,
				"lbp" => &mut t.lbp,
				"aave" => &mut t.aave,
				"hsm" => &mut t.hsm,
				"uniswapv3" => &mut t.uniswap_v3,
				_ => return Err("unknown venue"),
			};
			*slot = hop;
		}
		Ok(t)
	}
}

/// One `ExactIn` trade of a transfer; `amount_out` is the raw simulated output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Leg {
	pub route: Route<AssetId>,
	pub amount_in: Balance,
	pub amount_out: Balance,
	/// The hops' fixed tolerance in the output asset, see [`Tolerance::units_out`].
	pub units: Balance,
}

impl Leg {
	/// `A::sell` as a leg. Hop by hop — the sequence `A::sell` runs — only when an
	/// intermediate hop has fixed units, the one case its amount is needed for.
	pub fn sell<A: AMMInterface>(
		tolerance: &Tolerance,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		route: Route<AssetId>,
		state: &A::State,
	) -> Result<(A::State, Self), A::Error> {
		let intermediate = route.len().saturating_sub(1);
		if route
			.iter()
			.take(intermediate)
			.all(|t| tolerance.hop(&t.pool).units == 0)
		{
			let (next, exec) = A::sell(asset_in, asset_out, amount_in, route.clone(), state)?;
			let units = route.last().map_or(0, |t| tolerance.hop(&t.pool).units);
			return Ok((
				next,
				Self {
					route,
					amount_in,
					amount_out: exec.amount_out,
					units,
				},
			));
		}
		let mut current = state.clone();
		let mut amount = amount_in;
		let mut hop_outs = Vec::with_capacity(route.len());
		for t in route.iter() {
			let (next, exec) = A::sell(
				t.asset_in,
				t.asset_out,
				amount,
				Route::truncate_from(vec![*t]),
				&current,
			)?;
			current = next;
			amount = exec.amount_out;
			hop_outs.push(amount);
		}
		let units = tolerance.units_out(&route, &hop_outs);
		Ok((
			current,
			Self {
				route,
				amount_in,
				amount_out: amount,
				units,
			},
		))
	}

	/// The trade's on-chain `min_amount_out`.
	pub fn claimed_out(&self, tolerance: &Tolerance) -> Balance {
		tolerance.claim(self, false)
	}

	/// [`Self::claimed_out`] with the tolerance rounded up, for deciding whether a
	/// trade clears a limit. Rounded down, the tolerance vanishes on a small enough
	/// trade, and a limit the market misses by less than the tolerance is then filled
	/// in slivers no larger trade at the same rate could clear.
	pub fn claimed_out_strict(&self, tolerance: &Tolerance) -> Balance {
		tolerance.claim(self, true)
	}
}

pub fn amount_in(legs: &[Leg]) -> Balance {
	legs.iter().fold(0, |acc, l| acc.saturating_add(l.amount_in))
}

pub fn raw_out(legs: &[Leg]) -> Balance {
	legs.iter().fold(0, |acc, l| acc.saturating_add(l.amount_out))
}

pub fn claimed_out(legs: &[Leg], tolerance: &Tolerance) -> Balance {
	legs.iter()
		.fold(0, |acc, l| acc.saturating_add(l.claimed_out(tolerance)))
}

pub fn claimed_out_strict(legs: &[Leg], tolerance: &Tolerance) -> Balance {
	legs.iter()
		.fold(0, |acc, l| acc.saturating_add(l.claimed_out_strict(tolerance)))
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
	tolerance: Tolerance,
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
) -> Result<Option<(A::State, Leg)>, Exhausted> {
	if amount_in < pair.ed_in {
		return Ok(None);
	}
	budget.take()?;
	let Ok((next, leg)) = Leg::sell::<A>(
		&pair.tolerance,
		pair.asset_in,
		pair.asset_out,
		amount_in,
		route.clone(),
		state,
	) else {
		return Ok(None);
	};
	if leg.claimed_out(&pair.tolerance) < pair.ed_out {
		return Ok(None);
	}
	Ok(Some((next, leg)))
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
		let Some((next, leg)) = leg::<A>(pair, &routes[i], amount_in, current.as_ref().unwrap_or(state), budget)?
		else {
			return Ok(None);
		};
		legs.push(leg);
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
	claimed: Balance,
	raw: Balance,
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
		let (single, after) = self.best_sell(asset_in, asset_out, amount_in, state)?;
		if max_legs >= 2 && self.routes(asset_in, asset_out, state).len() >= 2 {
			let pair = Pair {
				asset_in,
				asset_out,
				ed_in: self.ed(asset_in).max(1),
				ed_out: self.ed(asset_out).max(1),
				tolerance: self.tolerance,
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

	// An extra route can only help if its first step claims more than the
	// anchor's last one.
	let Some((_, below)) = leg::<A>(pair, &routes[anchor], amount - step, state, budget)? else {
		return Ok(None);
	};
	let anchor_last_step = single
		.claimed_out(&pair.tolerance)
		.saturating_sub(below.claimed_out(&pair.tolerance));

	let mut candidates: Vec<Satellite> = Vec::new();
	for index in others {
		let route = &routes[index];
		let mut probe = step;
		for _ in 0..=MAX_PROBE_HALVINGS {
			if let Some((_, l)) = leg::<A>(pair, route, probe, state, budget)? {
				let claimed = l.claimed_out(&pair.tolerance);
				if U256::from(claimed) * U256::from(step) > U256::from(anchor_last_step) * U256::from(probe) {
					candidates.push(Satellite {
						index,
						probe,
						claimed,
						raw: l.amount_out,
					});
				}
				break;
			}
			probe /= 2;
			if probe < pair.ed_in {
				break;
			}
		}
	}
	// Best claimed rate first, then raw rate, then route order. The best rate
	// can come with the least capacity, so each candidate gets its own search below.
	candidates.sort_by(|x, y| {
		let by = |xo: Balance, yo: Balance| {
			(U256::from(yo) * U256::from(x.probe)).cmp(&(U256::from(xo) * U256::from(y.probe)))
		};
		by(x.claimed, y.claimed)
			.then(by(x.raw, y.raw))
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

	let base = single.claimed_out(&pair.tolerance);
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
			let out = claimed_out(&routed.legs, &pair.tolerance);
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
