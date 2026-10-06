//! Route splitting, on a mock market where every directed pair is served by the
//! same venues — each with its own price model and its own state, so legs that
//! share a venue see each other's impact.

use crate::passthrough;
use crate::v4::Solver;
use crate::{IceSolver, MinOuts, SolverOptions, SplitConfig};
use codec::Encode;
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, PoolType, Route, Trade};
use ice_support::{AssetId, Balance, Intent, IntentData, Partial, Solution, SwapData};
use std::cell::Cell;
use std::collections::BTreeMap;

const UNIT: Balance = 1_000_000_000_000;
const A: AssetId = 1;
const B: AssetId = 2;

#[derive(Clone, Copy, Debug)]
enum Model {
	/// Zero-fee constant product holding `depth` of each asset.
	Curve { depth: Balance },
	/// `out = in · num / den` until `cap` input has gone through it; above that
	/// the venue refuses — a capacity cliff.
	Capped { num: Balance, den: Balance, cap: Balance },
}

#[derive(Clone, Copy, Debug)]
struct Venue {
	pool: PoolType<AssetId>,
	/// Second hop through an intermediate asset, so two routes can share a pool.
	via: Option<(AssetId, PoolType<AssetId>)>,
	model: Model,
}

impl Venue {
	fn route(&self, asset_in: AssetId, asset_out: AssetId) -> Route<AssetId> {
		let hops = match self.via {
			None => vec![Trade {
				pool: self.pool,
				asset_in,
				asset_out,
			}],
			Some((via, second)) => vec![
				Trade {
					pool: self.pool,
					asset_in,
					asset_out: via,
				},
				Trade {
					pool: second,
					asset_in: via,
					asset_out,
				},
			],
		};
		Route::try_from(hops).unwrap()
	}
}

/// The venues plus what each pool has absorbed so far: curve reserves as
/// `(lower asset, higher asset)`, or the capped input used. Venues whose first
/// hop is the same pool share that state — a stableswap pool across all its
/// assets, any other pool per unordered pair.
#[derive(Clone, Debug)]
struct Book {
	venues: Vec<Venue>,
	reserves: BTreeMap<PoolKey, (Balance, Balance)>,
	/// Curve depth per unordered pair, where it differs from the venue's own.
	depths: BTreeMap<(AssetId, AssetId), Balance>,
}

type PoolKey = ((u8, u32), AssetId, AssetId);

fn pool_key(pool: PoolType<AssetId>, a: AssetId, b: AssetId) -> PoolKey {
	let (lo, hi) = (a.min(b), a.max(b));
	match pool {
		PoolType::Stableswap(id) => ((1, id), 0, 0),
		PoolType::Omnipool => ((0, 0), lo, hi),
		PoolType::XYK => ((2, 0), lo, hi),
		PoolType::LBP => ((3, 0), lo, hi),
		PoolType::Aave => ((4, 0), lo, hi),
		PoolType::HSM => ((5, 0), lo, hi),
		PoolType::UniswapV3(fee) => ((6, fee), lo, hi),
	}
}

thread_local! {
	static ED: Cell<Balance> = const { Cell::new(1) };
}

struct SplitMock;

impl AMMInterface for SplitMock {
	type Error = ();
	type State = Book;

	fn discover_routes(asset_in: AssetId, asset_out: AssetId, state: &Book) -> Result<Vec<Route<AssetId>>, ()> {
		Ok(state.venues.iter().map(|v| v.route(asset_in, asset_out)).collect())
	}

	fn sell(
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		route: Route<AssetId>,
		state: &Book,
	) -> Result<(Book, TradeExecution), ()> {
		let i = state
			.venues
			.iter()
			.position(|v| v.route(asset_in, asset_out) == route)
			.ok_or(())?;
		let key = pool_key(state.venues[i].pool, asset_in, asset_out);
		let mut next = state.clone();
		let amount_out = match state.venues[i].model {
			Model::Curve { depth } => {
				let depth = state
					.depths
					.get(&(asset_in.min(asset_out), asset_in.max(asset_out)))
					.copied()
					.unwrap_or(depth);
				let (lo, hi) = state.reserves.get(&key).copied().unwrap_or((depth, depth));
				let (r_in, r_out) = if asset_in < asset_out { (lo, hi) } else { (hi, lo) };
				let out = r_out * amount_in / (r_in + amount_in);
				let (r_in, r_out) = (r_in + amount_in, r_out - out);
				next.reserves.insert(
					key,
					if asset_in < asset_out {
						(r_in, r_out)
					} else {
						(r_out, r_in)
					},
				);
				out
			}
			Model::Capped { num, den, cap } => {
				let used = state.reserves.get(&key).map(|r| r.0).unwrap_or(0);
				if used + amount_in > cap {
					return Err(());
				}
				next.reserves.insert(key, (used + amount_in, 0));
				amount_in * num / den
			}
		};
		Ok((
			next,
			TradeExecution {
				amount_in,
				amount_out,
				route,
			},
		))
	}

	fn buy(_: AssetId, _: AssetId, _: Balance, _: Route<AssetId>, _: &Book) -> Result<(Book, TradeExecution), ()> {
		Err(())
	}

	fn get_spot_price(_: AssetId, _: AssetId, _: Route<AssetId>, _: &Book) -> Result<Ratio, ()> {
		Ok(Ratio::new(1, 1))
	}

	fn price_denominator() -> AssetId {
		0
	}

	fn pool_edges(_: &Book) -> Vec<PoolEdge<AssetId>> {
		Vec::new()
	}

	fn existential_deposit(_: AssetId) -> Balance {
		ED.with(|e| e.get())
	}
}

// ---------- scenario builders ----------

fn deep() -> Venue {
	Venue {
		pool: PoolType::Omnipool,
		via: None,
		model: Model::Curve { depth: 100_000 * UNIT },
	}
}

/// A 1:1 venue with no price impact that refuses anything above `cap`.
fn capped(pool: PoolType<AssetId>, cap: Balance) -> Venue {
	Venue {
		pool,
		via: None,
		model: Model::Capped { num: 1, den: 1, cap },
	}
}

fn book(venues: Vec<Venue>) -> Book {
	Book {
		venues,
		reserves: BTreeMap::new(),
		depths: BTreeMap::new(),
	}
}

fn swap(id: u128, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: Partial::No,
		}),
	}
}

fn partial(id: u128, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: Partial::Yes(0),
		}),
	}
}

/// Deep constant-product route first, a 400-unit capped route at a better
/// price second.
fn two_venue_market() -> Book {
	book(vec![deep(), capped(PoolType::Stableswap(7), 400 * UNIT)])
}

/// Sellers of distinct assets into `B` — one transfer each through netting.
fn many_sellers(count: u32) -> Vec<Intent> {
	(0..count)
		.map(|i| swap(i as u128 + 1, 10 + i, B, 1_000 * UNIT, 900 * UNIT))
		.collect()
}

fn shared_pool_market() -> Book {
	book(vec![
		Venue {
			pool: PoolType::Stableswap(7),
			via: None,
			model: Model::Curve { depth: 100_000 * UNIT },
		},
		Venue {
			pool: PoolType::Stableswap(7),
			via: Some((9, PoolType::XYK)),
			model: Model::Capped {
				num: 1,
				den: 1,
				cap: 400 * UNIT,
			},
		},
	])
}

type Trades = Vec<(Balance, Balance, Vec<PoolType<AssetId>>)>;
type Shape = (Vec<(u128, Balance, Balance)>, Trades, Balance);

fn shape(solution: &Solution) -> Shape {
	(
		solution
			.resolved_intents
			.iter()
			.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
			.collect(),
		solution
			.trades
			.iter()
			.map(|t| (t.amount_in, t.amount_out, t.route.iter().map(|h| h.pool).collect()))
			.collect(),
		solution.score,
	)
}

fn solve_with(intents: Vec<Intent>, state: Book, split: SplitConfig) -> Solution {
	Solver::<SplitMock>::solve_with_options(
		intents,
		MinOuts::new(),
		state,
		Permill::zero(),
		&SolverOptions {
			split,
			..SolverOptions::default()
		},
	)
	.expect("solver should succeed")
}

fn solve(intents: Vec<Intent>, state: Book) -> Solution {
	solve_with(intents, state, SplitConfig::default())
}

/// Per asset, what the pot receives covers what it pays out — `submit_solution`'s
/// conservation check on the trades' claimed minimums.
fn assert_conserves(solution: &Solution) {
	let mut credit: BTreeMap<AssetId, i128> = BTreeMap::new();
	for r in solution.resolved_intents.iter() {
		*credit.entry(r.data.asset_in()).or_default() += r.data.amount_in() as i128;
		*credit.entry(r.data.asset_out()).or_default() -= r.data.amount_out() as i128;
	}
	for t in solution.trades.iter() {
		*credit.entry(t.route.first().unwrap().asset_in).or_default() -= t.amount_in as i128;
		*credit.entry(t.route.last().unwrap().asset_out).or_default() += t.amount_out as i128;
	}
	for (asset, balance) in credit {
		assert!(balance >= 0, "asset {asset} is over-paid by {}", -balance);
	}
}

fn one_route(amount_out: Balance, pool: PoolType<AssetId>) -> Shape {
	(
		vec![(1, 1_000 * UNIT, amount_out)],
		vec![(1_000 * UNIT, amount_out, vec![pool])],
		amount_out - 900 * UNIT,
	)
}

fn sellers_single_route(count: u128) -> Shape {
	(
		(1..=count).map(|id| (id, 1_000 * UNIT, 990 * UNIT)).collect(),
		(1..=count)
			.map(|_| (1_000 * UNIT, 990 * UNIT, vec![PoolType::Omnipool]))
			.collect(),
		count * 90 * UNIT,
	)
}

/// The deep route's 625 (less the 2 wei of rounding dust a second leg leaves in
/// the pot) plus the capped route at 15/16 of its 400 cliff.
fn split_legs_of_1000() -> Trades {
	vec![
		(624_999_999_999_998, 621_055_900_621_116, vec![PoolType::Omnipool]),
		(375 * UNIT, 374_962_500_000_000, vec![PoolType::Stableswap(7)]),
	]
}

#[test]
fn solve_should_match_the_single_route_solver_when_splitting_is_disabled() {
	// Pinned from the solver before splitting existed, on the same markets; the
	// partial fill is sized by the strict limit check since.
	let off = SplitConfig::disabled();
	assert_eq!(
		shape(&solve_with(
			vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)],
			two_venue_market(),
			off
		)),
		one_route(990 * UNIT, PoolType::Omnipool),
	);
	assert_eq!(
		shape(&solve_with(
			vec![swap(1, A, B, 1_000 * UNIT, 993 * UNIT)],
			two_venue_market(),
			off
		)),
		(vec![], vec![], 0),
	);
	assert_eq!(
		shape(&solve_with(
			vec![partial(1, A, B, 1_000 * UNIT, 993 * UNIT)],
			two_venue_market(),
			off
		)),
		(
			vec![(1, 694_864_048_338_113, 689_999_999_999_748)],
			vec![(694_864_048_338_113, 689_999_999_999_748, vec![PoolType::Omnipool])],
			2,
		),
	);
	assert_eq!(
		shape(&solve_with(
			vec![
				swap(1, A, B, 1_500 * UNIT, 1_000 * UNIT),
				swap(2, B, A, 500 * UNIT, 400 * UNIT),
			],
			two_venue_market(),
			off,
		)),
		(
			vec![(1, 1_500 * UNIT, 1_490 * UNIT), (2, 500 * UNIT, 500 * UNIT)],
			vec![(1_000 * UNIT, 990 * UNIT, vec![PoolType::Omnipool])],
			590 * UNIT,
		),
	);
	assert_eq!(
		shape(&solve_with(many_sellers(29), two_venue_market(), off)),
		sellers_single_route(29)
	);
	assert_eq!(
		shape(&solve_with(
			vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)],
			shared_pool_market(),
			off
		)),
		one_route(990 * UNIT, PoolType::Stableswap(7)),
	);
}

#[test]
fn single_intent_should_split_when_a_capped_route_prices_better() {
	let solution = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], two_venue_market());

	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 996_018_400_621_116)],
			split_legs_of_1000(),
			96_018_400_621_116,
		),
	);
	assert_conserves(&solution);
}

#[test]
fn split_should_not_be_emitted_when_gain_is_below_threshold() {
	// A 10-unit cap is worth ~2 bps on a 1 000 sale, under the 10 bps margin.
	let market = book(vec![deep(), capped(PoolType::Stableswap(7), 10 * UNIT)]);

	let solution = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market);

	assert_eq!(shape(&solution), one_route(990 * UNIT, PoolType::Omnipool));
}

#[test]
fn split_should_need_a_larger_gain_when_the_extra_leg_is_uniswap_v3() {
	// A 100-unit cap is worth ~18 bps: enough for a stableswap leg, not for a
	// v3 leg that must also cover its 25 bps surcharge.
	let intent = || vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)];

	let stableswap = solve(
		intent(),
		book(vec![deep(), capped(PoolType::Stableswap(7), 100 * UNIT)]),
	);
	let uniswap = solve(
		intent(),
		book(vec![deep(), capped(PoolType::UniswapV3(3000), 100 * UNIT)]),
	);

	assert_eq!(
		shape(&stableswap),
		(
			vec![(1, 1_000 * UNIT, 991_761_684_151_438)],
			vec![
				(906_249_999_999_998, 898_021_059_151_438, vec![PoolType::Omnipool]),
				(93_750_000_000_000, 93_740_625_000_000, vec![PoolType::Stableswap(7)]),
			],
			91_761_684_151_438,
		),
	);
	assert_eq!(shape(&uniswap), one_route(990 * UNIT, PoolType::Omnipool));
}

#[test]
fn split_should_price_legs_through_a_shared_pool_in_sequence() {
	// Two routes through the same 400-unit stableswap: whatever the split puts
	// through one is gone for the other.
	let market = book(vec![
		deep(),
		capped(PoolType::Stableswap(7), 400 * UNIT),
		Venue {
			pool: PoolType::Stableswap(7),
			via: Some((9, PoolType::XYK)),
			model: Model::Capped {
				num: 1,
				den: 1,
				cap: 400 * UNIT,
			},
		},
	]);
	let three = SplitConfig {
		max_legs: 3,
		..SplitConfig::default()
	};

	let solution = solve_with(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market, three);

	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 996_321_829_025_841)],
			vec![
				(599_999_999_999_996, 596_361_829_025_841, vec![PoolType::Omnipool]),
				(100 * UNIT, 99_990_000_000_000, vec![PoolType::Stableswap(7)]),
				(
					300 * UNIT,
					299_970_000_000_000,
					vec![PoolType::Stableswap(7), PoolType::XYK]
				),
			],
			96_321_829_025_841,
		),
	);
	assert_conserves(&solution);
}

#[test]
fn split_should_not_emit_a_leg_below_the_existential_deposit() {
	// The capped leg would be 375, below a 400 ED on either end.
	ED.with(|e| e.set(400 * UNIT));

	let solution = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], two_venue_market());

	assert_eq!(shape(&solution), one_route(990 * UNIT, PoolType::Omnipool));
}

#[test]
fn netting_should_split_only_the_residual_when_opposing_flow_nets() {
	let solution = solve(
		vec![
			swap(1, A, B, 1_500 * UNIT, 1_000 * UNIT),
			swap(2, B, A, 500 * UNIT, 400 * UNIT),
		],
		two_venue_market(),
	);

	assert_eq!(
		shape(&solution),
		(
			vec![
				(1, 1_500 * UNIT, 1_496_018_400_621_116),
				(2, 500 * UNIT, 500_000_000_000_002)
			],
			split_legs_of_1000(),
			596_018_400_621_118,
		),
	);
	assert_conserves(&solution);
}

#[test]
fn netting_should_not_split_when_transfers_fill_the_trade_cap() {
	let solution = solve(many_sellers(30), two_venue_market());

	assert_eq!(shape(&solution), sellers_single_route(30));
}

#[test]
fn netting_should_split_only_within_the_trade_cap_the_other_transfers_leave() {
	// 29 transfers leave room for one extra leg: the first transfer splits and
	// every seller of B shares the gain through B's pot.
	let solution = solve(many_sellers(29), two_venue_market());

	let mut trades = split_legs_of_1000();
	trades.extend((0..28).map(|_| (1_000 * UNIT, 990 * UNIT, vec![PoolType::Omnipool])));
	assert_eq!(
		shape(&solution),
		(
			(1..=29).map(|id| (id, 1_000 * UNIT, 990_207_531_055_900)).collect(),
			trades,
			2_616_018_400_621_100,
		),
	);
	assert_conserves(&solution);
}

#[test]
fn fitting_should_admit_an_intent_only_the_split_can_pay() {
	// 993 per 1 000 sits between the single route (990) and the split (996).
	let intent = || vec![swap(1, A, B, 1_000 * UNIT, 993 * UNIT)];
	let without_fitting = SplitConfig {
		fitting: false,
		..SplitConfig::default()
	};

	let admitted = solve(intent(), two_venue_market());
	let rejected = solve_with(intent(), two_venue_market(), without_fitting);

	assert_eq!(
		shape(&admitted),
		(
			vec![(1, 1_000 * UNIT, 996_018_400_621_116)],
			split_legs_of_1000(),
			3_018_400_621_116,
		),
	);
	assert_eq!(shape(&rejected), (vec![], vec![], 0));
}

#[test]
fn fitting_should_fill_a_partial_in_full_when_only_the_split_pays_its_limit() {
	let intent = || vec![partial(1, A, B, 1_000 * UNIT, 993 * UNIT)];
	let without_fitting = SplitConfig {
		fitting: false,
		..SplitConfig::default()
	};

	let full = solve(intent(), two_venue_market());
	let trimmed = solve_with(intent(), two_venue_market(), without_fitting);

	assert_eq!(
		shape(&full),
		(
			vec![(1, 1_000 * UNIT, 996_018_400_621_116)],
			split_legs_of_1000(),
			3_018_400_621_116,
		),
	);
	// Sized on single-route quotes, yet still settled through the split and
	// paid what its legs claim.
	assert_eq!(
		shape(&trimmed),
		(
			vec![(1, 694_864_048_338_113, 693_772_089_606_383)],
			vec![
				(320_288_897_280_856, 319_234_396_064_233, vec![PoolType::Omnipool]),
				(374_575_151_057_255, 374_537_693_542_150, vec![PoolType::Stableswap(7)]),
			],
			3_772_089_606_637,
		),
	);
	assert_conserves(&trimmed);
}

#[test]
fn split_should_fall_back_to_the_single_route_when_the_budget_is_spent() {
	let starved = SplitConfig {
		sell_budget: 3,
		..SplitConfig::default()
	};

	let solution = solve_with(
		vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)],
		two_venue_market(),
		starved,
	);

	assert_eq!(shape(&solution), one_route(990 * UNIT, PoolType::Omnipool));
}

#[test]
fn split_should_use_two_extra_routes_when_three_legs_are_allowed() {
	let market = || {
		book(vec![
			deep(),
			capped(PoolType::Stableswap(7), 400 * UNIT),
			capped(PoolType::Stableswap(8), 300 * UNIT),
		])
	};
	let three = SplitConfig {
		max_legs: 3,
		..SplitConfig::default()
	};

	let solution = solve_with(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market(), three);
	let two = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market());

	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 998_722_525_108_998)],
			vec![
				(343_750_000_000_000, 342_538_150_109_001, vec![PoolType::Omnipool]),
				(374_999_999_999_996, 374_962_499_999_997, vec![PoolType::Stableswap(7)]),
				(281_250_000_000_000, 281_221_875_000_000, vec![PoolType::Stableswap(8)]),
			],
			98_722_525_108_998,
		),
	);
	assert_eq!(
		shape(&two),
		(
			vec![(1, 1_000 * UNIT, 996_018_400_621_116)],
			split_legs_of_1000(),
			96_018_400_621_116,
		),
	);
	assert_conserves(&solution);
}

#[test]
fn split_should_share_volume_between_two_curves_when_neither_has_a_cliff() {
	let market = book(vec![
		deep(),
		Venue {
			pool: PoolType::Stableswap(7),
			via: None,
			model: Model::Curve { depth: 20_000 * UNIT },
		},
	]);

	let solution = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market);

	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 991_620_097_656_273)],
			vec![
				(849_999_999_999_998, 842_751_611_303_915, vec![PoolType::Omnipool]),
				(150 * UNIT, 148_868_486_352_358, vec![PoolType::Stableswap(7)]),
			],
			91_620_097_656_273,
		),
	);
}

#[test]
fn solve_should_be_deterministic_when_splitting() {
	let intents = vec![
		swap(1, A, B, 1_500 * UNIT, 1_000 * UNIT),
		swap(2, B, A, 500 * UNIT, 400 * UNIT),
	];

	let first = solve(intents.clone(), two_venue_market());
	let second = solve(intents, two_venue_market());

	assert_eq!(first.encode(), second.encode());
}

#[test]
fn passthrough_should_never_split() {
	let solution = passthrough::Solver::<SplitMock>::solve(
		vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)],
		two_venue_market(),
		Permill::zero(),
	)
	.expect("solver should succeed");

	// Pass-through claims the raw quote; the chain pays the router's output.
	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 990_099_009_900_990)],
			vec![(1_000 * UNIT, 990_099_009_900_990, vec![PoolType::Omnipool])],
			90_099_009_900_990,
		),
	);
}

#[test]
fn split_should_use_a_deeper_route_when_the_best_rate_route_is_tiny() {
	// The 50-unit route pays the best rate but is worth under the margin; the
	// 600-unit route pays 0.1 % less and carries the split.
	let market = book(vec![
		deep(),
		capped(PoolType::Stableswap(7), 50 * UNIT),
		Venue {
			pool: PoolType::Stableswap(8),
			via: None,
			model: Model::Capped {
				num: 999,
				den: 1_000,
				cap: 600 * UNIT,
			},
		},
	]);

	let solution = solve(vec![swap(1, A, B, 1_000 * UNIT, 900 * UNIT)], market);

	assert_eq!(
		shape(&solution),
		(
			vec![(1, 1_000 * UNIT, 997_432_021_869_165)],
			vec![
				(437_500_000_000_000, 435_550_715_619_166, vec![PoolType::Omnipool]),
				(562_499_999_999_998, 561_881_306_249_999, vec![PoolType::Stableswap(8)]),
			],
			97_432_021_869_165,
		),
	);
}

#[test]
fn solve_should_keep_single_routes_when_splitting_leaves_the_batch_worse() {
	// Both sellers of B can use one 400-unit stableswap. Splitting the first
	// transfer spends it on a deep pair; the second, shallow pair then loses far
	// more than the first gained.
	let mut market = two_venue_market();
	market.depths.insert((B, 10), 20_000 * UNIT);
	market.depths.insert((B, 11), 2_000 * UNIT);
	let batch = || vec![swap(1, 10, B, 1_000 * UNIT, UNIT), swap(2, 11, B, 390 * UNIT, UNIT)];

	let guarded = solve(batch(), market.clone());
	let single = solve_with(batch(), market.clone(), SplitConfig::disabled());
	let first_alone = solve(vec![swap(1, 10, B, 1_000 * UNIT, UNIT)], market);

	// Splitting both transfers would score 1_312_147_253_358_425.
	let single_routes = (
		vec![
			(1, 1_000 * UNIT, 965_645_118_191_161),
			(2, 390 * UNIT, 376_601_596_094_552),
		],
		vec![
			(1_000 * UNIT, 952_285_714_285_714, vec![PoolType::Omnipool]),
			(390 * UNIT, 389_961_000_000_000, vec![PoolType::Stableswap(7)]),
		],
		1_340_246_714_285_713,
	);
	assert_eq!(shape(&single), single_routes);
	assert_eq!(shape(&guarded), single_routes);
	assert_eq!(
		shape(&first_alone),
		(
			vec![(1, 1_000 * UNIT, 980_962_499_999_998)],
			vec![
				(624_999_999_999_998, 605_999_999_999_998, vec![PoolType::Omnipool]),
				(375 * UNIT, 374_962_500_000_000, vec![PoolType::Stableswap(7)]),
			],
			979_962_499_999_998,
		),
	);
}
