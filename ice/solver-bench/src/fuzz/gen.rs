//! Scenario generation. Pure random intents almost never produce a
//! coincidence-of-wants — they just route independently through the AMM. The
//! interesting solver logic (matching, netting, ring detection, partial
//! throttling) only fires on *structured* inputs, so generation is a weighted
//! grammar of archetypes that deliberately build structure, then layer noise.

use super::rng::Rng;
use super::{asset_spec, AssetSpec, ASSETS};
use crate::SolverIntent;
use ice_support::{IntentData, Partial, SwapData};
use primitives::{AssetId, Balance};

#[derive(Clone, Copy, Debug)]
pub enum LimitPolicy {
	/// Min ≈ a small fraction of achievable — solver fills at best price.
	Loose,
	/// Min == achievable spot output — sits exactly on the feasibility edge.
	AtSpot,
	/// Min just below spot by `permille` — feasible but tight.
	Tight(u32),
	/// Min far above any achievable output — must be dropped.
	Impossible,
}

#[derive(Clone, Debug)]
pub struct IntentSpec {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub limit: LimitPolicy,
	pub partial: bool,
}

#[derive(Clone, Debug)]
pub struct Scenario {
	pub archetype: &'static str,
	pub specs: Vec<IntentSpec>,
}

/// A spec with its limit resolved to a concrete `amount_out` (min receive).
#[derive(Clone, Debug)]
pub struct RealizedIntent {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub amount_out: Balance,
	pub partial: bool,
}

fn amount_for(rng: &mut Rng, a: &AssetSpec) -> Balance {
	rng.log_amount(a.min_amount, a.max_amount)
}

fn random_limit(rng: &mut Rng) -> LimitPolicy {
	match rng.range_usize(0, 10) {
		0..=5 => LimitPolicy::Loose,
		6 => LimitPolicy::AtSpot,
		7 | 8 => LimitPolicy::Tight(rng.range_usize(1, 50) as u32),
		_ => LimitPolicy::Impossible,
	}
}

fn distinct_pair(rng: &mut Rng) -> (&'static AssetSpec, &'static AssetSpec) {
	let a = rng.choose(ASSETS);
	loop {
		let b = rng.choose(ASSETS);
		if b.id != a.id {
			return (a, b);
		}
	}
}

/// A random cycle/chain over `len` distinct assets (partial Fisher-Yates).
fn distinct_path(rng: &mut Rng, len: usize) -> Vec<&'static AssetSpec> {
	let mut pool: Vec<&'static AssetSpec> = ASSETS.iter().collect();
	rng.shuffle(&mut pool);
	pool.truncate(len.min(pool.len()));
	pool
}

fn independent(rng: &mut Rng, max_intents: usize) -> Vec<IntentSpec> {
	let n = rng.range_usize(1, max_intents.max(2));
	(0..n)
		.map(|_| {
			let (a, b) = distinct_pair(rng);
			IntentSpec {
				asset_in: a.id,
				asset_out: b.id,
				amount_in: amount_for(rng, a),
				limit: random_limit(rng),
				partial: rng.chance(1, 3),
			}
		})
		.collect()
}

fn opposing_pair(rng: &mut Rng, max_intents: usize) -> Vec<IntentSpec> {
	let (a, b) = distinct_pair(rng);
	let fwd = rng.range_usize(1, (max_intents / 2).max(2));
	let bwd = rng.range_usize(1, (max_intents / 2).max(2));
	let mut specs = Vec::new();
	for _ in 0..fwd {
		specs.push(IntentSpec {
			asset_in: a.id,
			asset_out: b.id,
			amount_in: amount_for(rng, a),
			limit: if rng.chance(3, 4) {
				LimitPolicy::Loose
			} else {
				random_limit(rng)
			},
			partial: rng.chance(1, 2),
		});
	}
	for _ in 0..bwd {
		specs.push(IntentSpec {
			asset_in: b.id,
			asset_out: a.id,
			amount_in: amount_for(rng, b),
			limit: if rng.chance(3, 4) {
				LimitPolicy::Loose
			} else {
				random_limit(rng)
			},
			partial: rng.chance(1, 2),
		});
	}
	specs
}

fn ring(rng: &mut Rng) -> Vec<IntentSpec> {
	let len = rng.range_usize(3, ASSETS.len() + 1);
	let path = distinct_path(rng, len);
	(0..path.len())
		.map(|i| {
			let a = path[i];
			let b = path[(i + 1) % path.len()];
			IntentSpec {
				asset_in: a.id,
				asset_out: b.id,
				amount_in: amount_for(rng, a),
				limit: LimitPolicy::Loose,
				partial: rng.chance(1, 3),
			}
		})
		.collect()
}

fn chain(rng: &mut Rng) -> Vec<IntentSpec> {
	let len = rng.range_usize(3, ASSETS.len() + 1);
	let path = distinct_path(rng, len);
	(0..path.len().saturating_sub(1))
		.map(|i| IntentSpec {
			asset_in: path[i].id,
			asset_out: path[i + 1].id,
			amount_in: amount_for(rng, path[i]),
			limit: LimitPolicy::Loose,
			partial: rng.chance(1, 3),
		})
		.collect()
}

fn whale_dust(rng: &mut Rng, max_intents: usize) -> Vec<IntentSpec> {
	let (a, b) = distinct_pair(rng);
	let mut specs = vec![IntentSpec {
		asset_in: a.id,
		asset_out: b.id,
		amount_in: a.max_amount,
		limit: LimitPolicy::Loose,
		partial: rng.chance(1, 2),
	}];
	let dust = rng.range_usize(1, max_intents.max(2));
	for _ in 0..dust {
		specs.push(IntentSpec {
			asset_in: b.id,
			asset_out: a.id,
			amount_in: b.min_amount,
			limit: random_limit(rng),
			partial: rng.chance(1, 2),
		});
	}
	specs
}

fn boundary(rng: &mut Rng) -> Vec<IntentSpec> {
	let n = rng.range_usize(1, 5);
	(0..n)
		.map(|_| {
			let (a, b) = distinct_pair(rng);
			IntentSpec {
				asset_in: a.id,
				asset_out: b.id,
				amount_in: amount_for(rng, a),
				limit: match rng.range_usize(0, 3) {
					0 => LimitPolicy::AtSpot,
					1 => LimitPolicy::Tight(rng.range_usize(1, 5) as u32),
					_ => LimitPolicy::Impossible,
				},
				partial: rng.chance(1, 2),
			}
		})
		.collect()
}

/// Degenerate / robustness inputs — including self-pairs that the pallet would
/// reject (Tier 1 feeds them straight to the solver to prove it never panics;
/// Tier 2 filters them).
fn degenerate(rng: &mut Rng, max_intents: usize) -> Vec<IntentSpec> {
	match rng.range_usize(0, 5) {
		0 => vec![],
		1 => {
			let (a, b) = distinct_pair(rng);
			vec![IntentSpec {
				asset_in: a.id,
				asset_out: b.id,
				amount_in: amount_for(rng, a),
				limit: LimitPolicy::Loose,
				partial: false,
			}]
		}
		2 => {
			let (a, b) = distinct_pair(rng);
			let s = IntentSpec {
				asset_in: a.id,
				asset_out: b.id,
				amount_in: amount_for(rng, a),
				limit: LimitPolicy::Loose,
				partial: rng.chance(1, 2),
			};
			vec![s.clone(), s]
		}
		3 => {
			let a = rng.choose(ASSETS);
			vec![IntentSpec {
				asset_in: a.id,
				asset_out: a.id,
				amount_in: amount_for(rng, a),
				limit: LimitPolicy::Loose,
				partial: false,
			}]
		}
		_ => independent(rng, max_intents.max(80)),
	}
}

/// Pick an archetype by weight and generate its specs.
pub fn scenario(rng: &mut Rng, max_intents: usize) -> Scenario {
	let (archetype, specs) = match rng.range_usize(0, 20) {
		0..=4 => ("independent", independent(rng, max_intents)),
		5..=8 => ("opposing_pair", opposing_pair(rng, max_intents)),
		9..=11 => ("ring", ring(rng)),
		12 | 13 => ("chain", chain(rng)),
		14 | 15 => ("whale_dust", whale_dust(rng, max_intents)),
		16 | 17 => ("boundary", boundary(rng)),
		_ => ("degenerate", degenerate(rng, max_intents)),
	};
	Scenario { archetype, specs }
}

/// Resolve each spec's limit policy to a concrete `amount_out` using a spot
/// `quote`. When `min_ge_ed` (Tier 2 submission), the min must clear the
/// asset's existential deposit, so `Loose` becomes half the achievable output
/// and intents whose route can't be quoted are dropped.
pub fn realize(
	specs: &[IntentSpec],
	min_ge_ed: bool,
	quote: &dyn Fn(AssetId, AssetId, Balance) -> Option<Balance>,
) -> Vec<RealizedIntent> {
	let mut out = Vec::with_capacity(specs.len());
	for s in specs {
		if min_ge_ed && (s.asset_in == s.asset_out || asset_spec(s.asset_out).is_none()) {
			continue;
		}
		let spot = quote(s.asset_in, s.asset_out, s.amount_in);
		let amount_out = match s.limit {
			LimitPolicy::Loose => match spot {
				Some(q) => (q / 2).max(1),
				None if min_ge_ed => continue,
				None => 1,
			},
			LimitPolicy::AtSpot => match spot {
				Some(q) => q.max(1),
				None if min_ge_ed => continue,
				None => 1,
			},
			LimitPolicy::Tight(pm) => match spot {
				Some(q) => (q.saturating_mul(1000u128.saturating_sub(pm as u128)) / 1000).max(1),
				None if min_ge_ed => continue,
				None => 1,
			},
			LimitPolicy::Impossible => match spot {
				Some(q) => q.saturating_mul(2).max(1),
				None if min_ge_ed => continue,
				None => u128::MAX / 4,
			},
		};
		out.push(RealizedIntent {
			asset_in: s.asset_in,
			asset_out: s.asset_out,
			amount_in: s.amount_in,
			amount_out,
			partial: s.partial,
		});
	}
	out
}

pub fn to_solver_intents(realized: &[RealizedIntent]) -> Vec<SolverIntent> {
	realized
		.iter()
		.enumerate()
		.map(|(i, r)| SolverIntent {
			id: (i as u128) + 1,
			data: IntentData::Swap(SwapData {
				asset_in: r.asset_in,
				asset_out: r.asset_out,
				amount_in: r.amount_in,
				amount_out: r.amount_out,
				partial: if r.partial { Partial::Yes(0) } else { Partial::No },
			}),
		})
		.collect()
}
