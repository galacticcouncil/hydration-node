use crate::v4::Solver;
use frame_support::sp_runtime::Permill;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{AMMInterface, TradeExecution};
use hydradx_traits::router::{PoolEdge, Route, Trade};
use ice_support::{AssetId, Balance, Intent, IntentData, IntentId, Partial, ResolvedIntent, SwapData};
use sp_core::U256;
use sp_std::collections::btree_set::BTreeSet;

// ---------- fixtures ----------

fn make_intent(id: IntentId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial: Partial::No,
		}),
	}
}

fn make_partial(id: IntentId, asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> Intent {
	make_partial_filled(id, asset_in, asset_out, amount_in, min_out, 0)
}

fn make_partial_filled(
	id: IntentId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
	already_filled: Balance,
) -> Intent {
	Intent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial: Partial::Yes(already_filled),
		}),
	}
}

fn dummy_route(asset_in: u32, asset_out: u32) -> Route<u32> {
	Route::try_from(vec![Trade {
		pool: hydradx_traits::router::PoolType::Omnipool,
		asset_in,
		asset_out,
	}])
	.unwrap()
}

/// Mirrors the on-chain `validate_price_consistency` predicate (±1 rounding).
fn same_rate_within(a: &ResolvedIntent, b: &ResolvedIntent, tol: u128) -> bool {
	let a_in = a.data.amount_in();
	let a_out = a.data.amount_out();
	let b_in = b.data.amount_in();
	let b_out = b.data.amount_out();
	let lhs = U256::from(a_out) * U256::from(b_in);
	let rhs = U256::from(b_out) * U256::from(a_in);
	let diff = if lhs >= rhs { lhs - rhs } else { rhs - lhs };
	let tol_scaled = U256::from(a_in.max(b_in)) * U256::from(tol);
	diff <= tol_scaled
}

/// Sum of `IntentData::surplus` — the formula the pallet uses to recompute score.
fn pallet_score(originals: &[Intent], resolved: &[ResolvedIntent]) -> Balance {
	let mut total: Balance = 0;
	for r in resolved {
		let original = originals.iter().find(|i| i.id == r.id).unwrap();
		let surplus = original.data.surplus(&r.data).expect("surplus should be computable");
		total = total.saturating_add(surplus);
	}
	total
}

fn find_resolved(resolved: &[ResolvedIntent], id: IntentId) -> &ResolvedIntent {
	resolved.iter().find(|r| r.id == id).expect("intent should be resolved")
}

// ---------- mocks ----------

/// 1:1 price, no slippage, zero existential deposit.
struct MockAMMOneToOne;

impl AMMInterface for MockAMMOneToOne {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out: amount_in,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in: amount_out,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(1, 1))
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
}

/// Asset 1 is worth 2× asset 2; 1% slippage on every sell.
struct MockAMMWithSlippage;

impl AMMInterface for MockAMMWithSlippage {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
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
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
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
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(asset_in: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		match asset_in {
			1 => Ok(Ratio::new(2, 1)),
			_ => Ok(Ratio::new(1, 1)),
		}
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
}

/// Constant-product style pool with volume-dependent slippage:
/// `out = in * depth / (depth + in)`, spot 1:1.
fn cp_out(amount_in: u128, depth: u128) -> u128 {
	amount_in * depth / (depth + amount_in)
}

macro_rules! depth_mock {
	($name:ident, $depth:expr, $ed:expr) => {
		struct $name;

		impl AMMInterface for $name {
			type Error = ();
			type State = ();

			fn discover_routes(
				asset_in: u32,
				asset_out: u32,
				_s: &Self::State,
			) -> Result<Vec<Route<u32>>, Self::Error> {
				Ok(vec![dummy_route(asset_in, asset_out)])
			}

			fn sell(
				asset_in: u32,
				asset_out: u32,
				amount_in: u128,
				_route: Route<u32>,
				_state: &Self::State,
			) -> Result<(Self::State, TradeExecution), Self::Error> {
				Ok((
					(),
					TradeExecution {
						amount_in,
						amount_out: cp_out(amount_in, $depth),
						route: dummy_route(asset_in, asset_out),
					},
				))
			}

			fn buy(
				asset_in: u32,
				asset_out: u32,
				amount_out: u128,
				_route: Route<u32>,
				_state: &Self::State,
			) -> Result<(Self::State, TradeExecution), Self::Error> {
				Ok((
					(),
					TradeExecution {
						amount_in: amount_out + 1,
						amount_out,
						route: dummy_route(asset_in, asset_out),
					},
				))
			}

			fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
				Ok(Ratio::new(1, 1))
			}
			fn price_denominator() -> u32 {
				0
			}
			fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
				Vec::new()
			}
			fn existential_deposit(_asset_id: AssetId) -> Balance {
				$ed
			}
		}
	};
}

depth_mock!(MockAMMDepth, 10_000u128, 0u128);
depth_mock!(MockAMMDepthWithED, 10_000u128, 300u128);
depth_mock!(MockAMMDepth12, 12_000u128, 0u128);

/// 1:1, no slippage, existential deposit of 1000 on every asset.
struct MockAMMWithED;

impl AMMInterface for MockAMMWithED {
	type Error = ();
	type State = ();

	fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
		Ok(vec![dummy_route(asset_in, asset_out)])
	}

	fn sell(
		asset_in: u32,
		asset_out: u32,
		amount_in: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in,
				amount_out: amount_in,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn buy(
		asset_in: u32,
		asset_out: u32,
		amount_out: u128,
		_route: Route<u32>,
		_state: &Self::State,
	) -> Result<(Self::State, TradeExecution), Self::Error> {
		Ok((
			(),
			TradeExecution {
				amount_in: amount_out,
				amount_out,
				route: dummy_route(asset_in, asset_out),
			},
		))
	}

	fn get_spot_price(_: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
		Ok(Ratio::new(1, 1))
	}
	fn price_denominator() -> u32 {
		0
	}
	fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
		Vec::new()
	}
	fn existential_deposit(_asset_id: AssetId) -> Balance {
		1_000
	}
}

/// Fully scriptable AMM for the targeted regression tests: per-asset
/// existential deposits and spot prices, per-directed-pair sell behaviour.
///
/// Everything the solver can observe about the market is configured up front,
/// so a test can put the solver in exactly the state a review finding describes
/// (a residual trade whose output is dust, a route that fails only for one
/// pair, an asset with no spot price at all).
pub mod scripted {
	use super::*;
	use std::cell::RefCell;
	use std::collections::BTreeMap;

	/// What `sell` does for one directed pair.
	#[derive(Clone, Copy, Debug)]
	pub enum SellRule {
		/// `amount_out = amount_in * n / d`.
		Rate { n: Balance, d: Balance },
		/// Constant-product style: `out = in * depth / (depth + in)`.
		Depth(Balance),
		/// `amount_out = amount_in * n / d`, but only while `amount_in <= cap`.
		Capped { n: Balance, d: Balance, cap: Balance },
		/// The AMM cannot serve this pair at all — no route is discovered.
		Fail,
		/// A route exists and prices, but every sell against it errors.
		SellFails,
	}

	#[derive(Default)]
	pub struct Script {
		pub eds: BTreeMap<AssetId, Balance>,
		/// `None` entry = `get_spot_price` fails for that asset.
		pub prices: BTreeMap<AssetId, Option<Ratio>>,
		pub sells: BTreeMap<(AssetId, AssetId), SellRule>,
		pub default_sell: Option<SellRule>,
		pub denominator: AssetId,
	}

	thread_local! {
		static SCRIPT: RefCell<Script> = RefCell::new(Script::default());
	}

	pub struct Builder(Script);

	pub fn script() -> Builder {
		Builder(Script {
			default_sell: Some(SellRule::Rate { n: 1, d: 1 }),
			..Script::default()
		})
	}

	impl Builder {
		pub fn ed(mut self, asset: AssetId, ed: Balance) -> Self {
			self.0.eds.insert(asset, ed);
			self
		}
		pub fn price(mut self, asset: AssetId, price: Option<Ratio>) -> Self {
			self.0.prices.insert(asset, price);
			self
		}
		pub fn sell(mut self, asset_in: AssetId, asset_out: AssetId, rule: SellRule) -> Self {
			self.0.sells.insert((asset_in, asset_out), rule);
			self
		}
		pub fn default_sell(mut self, rule: SellRule) -> Self {
			self.0.default_sell = Some(rule);
			self
		}
		/// Install the script and run `f` against [`ScriptedAmm`].
		pub fn run<R>(self, f: impl FnOnce() -> R) -> R {
			SCRIPT.with(|s| *s.borrow_mut() = self.0);
			f()
		}
	}

	pub struct ScriptedAmm;

	fn rule(asset_in: AssetId, asset_out: AssetId) -> Option<SellRule> {
		SCRIPT.with(|s| {
			let s = s.borrow();
			s.sells.get(&(asset_in, asset_out)).copied().or(s.default_sell)
		})
	}

	impl AMMInterface for ScriptedAmm {
		type Error = ();
		type State = ();

		fn discover_routes(asset_in: u32, asset_out: u32, _s: &Self::State) -> Result<Vec<Route<u32>>, Self::Error> {
			match rule(asset_in, asset_out) {
				Some(SellRule::Fail) | None => Ok(Vec::new()),
				Some(_) => Ok(vec![dummy_route(asset_in, asset_out)]),
			}
		}

		fn sell(
			asset_in: u32,
			asset_out: u32,
			amount_in: u128,
			_route: Route<u32>,
			_state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			let amount_out = match rule(asset_in, asset_out) {
				Some(SellRule::Rate { n, d }) => amount_in.checked_mul(n).map(|v| v / d).ok_or(())?,
				Some(SellRule::Depth(depth)) => amount_in
					.checked_mul(depth)
					.map(|v| v / depth.saturating_add(amount_in))
					.ok_or(())?,
				Some(SellRule::Capped { n, d, cap }) => {
					if amount_in > cap {
						return Err(());
					}
					amount_in.checked_mul(n).map(|v| v / d).ok_or(())?
				}
				Some(SellRule::Fail) | Some(SellRule::SellFails) | None => return Err(()),
			};
			Ok((
				(),
				TradeExecution {
					amount_in,
					amount_out,
					route: dummy_route(asset_in, asset_out),
				},
			))
		}

		fn buy(
			asset_in: u32,
			asset_out: u32,
			amount_out: u128,
			_route: Route<u32>,
			_state: &Self::State,
		) -> Result<(Self::State, TradeExecution), Self::Error> {
			Ok((
				(),
				TradeExecution {
					amount_in: amount_out,
					amount_out,
					route: dummy_route(asset_in, asset_out),
				},
			))
		}

		fn get_spot_price(asset_in: u32, _: u32, _: Route<u32>, _: &Self::State) -> Result<Ratio, Self::Error> {
			SCRIPT.with(|s| match s.borrow().prices.get(&asset_in) {
				Some(Some(p)) => Ok(*p),
				Some(None) => Err(()),
				None => Ok(Ratio::new(1, 1)),
			})
		}

		fn price_denominator() -> u32 {
			SCRIPT.with(|s| s.borrow().denominator)
		}

		fn pool_edges(_: &Self::State) -> Vec<PoolEdge<u32>> {
			Vec::new()
		}

		fn existential_deposit(asset_id: AssetId) -> Balance {
			SCRIPT.with(|s| s.borrow().eds.get(&asset_id).copied().unwrap_or(0))
		}
	}
}

// ---------- invariant helpers ----------

/// Every emitted trade must clear the existential deposit on both ends. The
/// pallet silently *skips* a trade that does not, which would leave the holding
/// pot short of an output this solution already promised to a user.
fn assert_trades_are_executable(solution: &ice_support::Solution, ed: impl Fn(AssetId) -> Balance) {
	for trade in solution.trades.iter() {
		let asset_in = trade.route.first().expect("trade route is never empty").asset_in;
		let asset_out = trade.route.last().expect("trade route is never empty").asset_out;
		assert!(
			trade.amount_in >= ed(asset_in).max(1),
			"trade {asset_in} -> {asset_out} has input {} below ED {}",
			trade.amount_in,
			ed(asset_in),
		);
		assert!(
			trade.amount_out >= ed(asset_out).max(1),
			"trade {asset_in} -> {asset_out} has output {} below ED {}",
			trade.amount_out,
			ed(asset_out),
		);
	}
}

/// Per-asset conservation, as the pallet re-checks it: for every asset the
/// holding pot must end non-negative, i.e.
/// `intent_in + pool_out >= intent_out + pool_in`.
fn assert_conserves(solution: &ice_support::Solution) {
	let mut credit: std::collections::BTreeMap<AssetId, i128> = std::collections::BTreeMap::new();
	for r in solution.resolved_intents.iter() {
		*credit.entry(r.data.asset_in()).or_default() += r.data.amount_in() as i128;
		*credit.entry(r.data.asset_out()).or_default() -= r.data.amount_out() as i128;
	}
	for t in solution.trades.iter() {
		let asset_in = t.route.first().expect("trade route is never empty").asset_in;
		let asset_out = t.route.last().expect("trade route is never empty").asset_out;
		*credit.entry(asset_in).or_default() -= t.amount_in as i128;
		*credit.entry(asset_out).or_default() += t.amount_out as i128;
	}
	for (asset, balance) in credit {
		assert!(balance >= 0, "asset {asset} is over-paid by {} units", -balance);
	}
}

// ---------- tests ----------

#[test]
fn solve_should_return_empty_solution_when_no_intents() {
	let solution = Solver::<MockAMMOneToOne>::solve(vec![], (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 0);
}

#[test]
fn solve_should_resolve_single_intent_when_route_meets_limit() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 90_000)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 100_000);
	// AMM output 100_000 minus the 1 bps haircut (10).
	assert_eq!(r.data.amount_out(), 99_990);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 100_000);
	assert_eq!(solution.trades[0].amount_out, 99_990);
	assert_eq!(solution.score, 9_990);
}

#[test]
fn solve_should_return_empty_solution_when_limit_unreachable() {
	let intents = vec![make_intent(1, 1, 2, 100_000, 200_000)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
}

#[test]
fn opposing_intents_should_settle_without_amm_trade_when_volumes_cancel() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// Direct matching at the 1:1 reference price, no fee — full output, no trades.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 100_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 20_000);
}

#[test]
fn matched_volume_should_pay_fee_when_intents_cancel() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::from_percent(1)).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// 100% matched volume — both sides pay the 1% matched fee.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 99_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 99_000);
	assert!(solution.trades.is_empty());
	assert_eq!(solution.score, 18_000);
}

#[test]
fn scarce_side_should_get_spot_rate_when_opposing_flow_is_excess() {
	// Asset 1 is worth 2× asset 2. id 1 sells 100 of asset 1 (200 in asset-2 value),
	// id 2 sells 100 of asset 2 — excess on the forward side, net 50 of asset 1
	// goes through the AMM at 1% slippage.
	let intents = vec![make_intent(1, 1, 2, 100, 150), make_intent(2, 2, 1, 100, 40)];
	let solution = Solver::<MockAMMWithSlippage>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	// Forward: 100 matched at spot (100 of asset 2 from the scarce side... valued
	// as direct_match=100) + AMM output 99 for the net 50 → 199 total.
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 199);
	// Scarce side gets the spot rate: 100 of asset 2 → 50 of asset 1.
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 50);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 50);
	assert_eq!(solution.trades[0].amount_out, 99);
	assert_eq!(solution.score, 59);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn tight_partial_should_not_throttle_loose_partial_when_sharing_direction() {
	// Depth-10_000 pool. Combined volume 2_000 clears at ~0.83 — below the tight
	// partial's 0.9 limit. Price priority: the loose partial fills fully, the
	// tight partial is trimmed to a fill that keeps the uniform rate at its
	// limit (the bisection lands on 105 — integer floor jitter makes the exact
	// feasibility boundary non-monotone, so the result is conservative).
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 900)];
	let solution = Solver::<MockAMMDepth>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	let loose = find_resolved(&solution.resolved_intents, 1);
	let tight = find_resolved(&solution.resolved_intents, 2);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 900);
	assert_eq!(tight.data.amount_in(), 105);
	assert_eq!(tight.data.amount_out(), 94);
	assert!(same_rate_within(loose, tight, 1));
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 1_105);
	assert_eq!(solution.trades[0].amount_out, 995);
	assert_eq!(solution.score, 400);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn partial_fills_should_be_input_order_independent_when_solved() {
	let a = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 900)];
	let b = vec![make_partial(2, 1, 2, 1_000, 900), make_partial(1, 1, 2, 1_000, 500)];

	let sol_a = Solver::<MockAMMDepth>::solve(a, (), Permill::zero()).unwrap();
	let sol_b = Solver::<MockAMMDepth>::solve(b, (), Permill::zero()).unwrap();

	let amounts = |s: &ice_support::Solution| -> Vec<(IntentId, Balance, Balance)> {
		let mut v: Vec<(IntentId, Balance, Balance)> = s
			.resolved_intents
			.iter()
			.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
			.collect();
		v.sort();
		v
	};
	assert_eq!(amounts(&sol_a), amounts(&sol_b));
	assert_eq!(sol_a.score, sol_b.score);
}

#[test]
fn infeasible_partial_should_be_dropped_when_no_fill_meets_limit() {
	// The tight partial demands 1.5× — unreachable at any volume. It must be
	// dropped without affecting the loose intent.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 1_500)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let loose = find_resolved(&solution.resolved_intents, 1);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 909);
	assert_eq!(solution.score, 409);
}

#[test]
fn nonpartial_should_be_dropped_when_clearing_rate_below_limit() {
	// Non-partial demanding 0.95 at a combined volume that clears at ~0.83:
	// all-or-nothing, so it is excluded entirely; the loose intent still fills.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_intent(2, 1, 2, 1_000, 950)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let loose = find_resolved(&solution.resolved_intents, 1);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 909);
	assert_eq!(solution.score, 409);
}

#[test]
fn partial_should_leave_no_dust_remainder_when_trim_lands_below_ed() {
	// ED 300. The tight partial's best feasible fill is 762, which would leave
	// an untradeable remainder of 238 — the fill is reduced to 700 so the
	// remainder (300) stays at the ED.
	let intents = vec![make_partial(1, 1, 2, 1_000, 500), make_partial(2, 1, 2, 1_000, 850)];
	let solution = Solver::<MockAMMDepthWithED>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	let loose = find_resolved(&solution.resolved_intents, 1);
	let tight = find_resolved(&solution.resolved_intents, 2);
	assert_eq!(loose.data.amount_in(), 1_000);
	assert_eq!(loose.data.amount_out(), 854);
	assert_eq!(tight.data.amount_in(), 700);
	assert_eq!(tight.data.amount_out(), 597);
	assert!(same_rate_within(loose, tight, 1));
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 1_700);
	assert_eq!(solution.trades[0].amount_out, 1_452);
	assert_eq!(solution.score, 356);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn intent_should_be_excluded_when_amount_below_existential_deposit() {
	let intents = vec![make_intent(1, 1, 2, 500, 400)];
	let solution = Solver::<MockAMMWithED>::solve(intents, (), Permill::zero()).unwrap();
	assert!(solution.resolved_intents.is_empty());
	assert!(solution.trades.is_empty());
}

#[test]
fn resolved_intents_should_be_capped_by_surplus_when_exceeding_max() {
	// 105 same-direction intents; min_out = 1_000 − (id − 1), so higher ids have
	// looser limits and more surplus. id 1 (limit 1.0) is dropped by the
	// crossing (the 1 bps haircut makes 1.0 unreachable); ids 2..=5 are the
	// lowest-surplus survivors and get cut by the cap.
	let intents: Vec<Intent> = (1..=105u128)
		.map(|id| make_intent(id, 1, 2, 1_000, 1_000 - (id - 1)))
		.collect();
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 100);
	let resolved_ids: BTreeSet<IntentId> = solution.resolved_intents.iter().map(|r| r.id).collect();
	let expected_ids: BTreeSet<IntentId> = (6..=105u128).collect();
	assert_eq!(resolved_ids, expected_ids);
	for r in solution.resolved_intents.iter() {
		assert_eq!(r.data.amount_in(), 1_000);
		assert_eq!(r.data.amount_out(), 999);
	}
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 100_000);
	assert_eq!(solution.trades[0].amount_out, 99_990);
	// Σ_{id=6}^{105} (999 − (1_000 − (id−1))) = Σ_{k=4}^{103} k = 5_350.
	assert_eq!(solution.score, 5_350);
}

#[test]
fn cumulative_partial_should_resolve_remaining_when_partially_filled() {
	// 400 of 1_000 already filled — only the remaining 600 may be spent.
	let intents = vec![make_partial_filled(1, 1, 2, 1_000, 500, 400)];
	let solution = Solver::<MockAMMDepth>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 600);
	assert_eq!(r.data.amount_out(), 566);
	let IntentData::Swap(swap) = &r.data else {
		panic!("expected swap data");
	};
	assert_eq!(swap.partial, Partial::Yes(400));
	// Pro-rata minimum for 600/1_000 of the original 500 limit is 300.
	assert_eq!(solution.score, 266);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn partial_should_fill_maximum_when_full_amount_infeasible() {
	// Depth-12_000 pool, limit 0.75. The full 10_000 only yields 5_454 (rate
	// 0.55) — the bisection finds the largest feasible fill instead.
	let intents = vec![make_partial(1, 1, 2, 10_000, 7_500)];
	let solution = Solver::<MockAMMDepth12>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_in(), 4_002);
	assert_eq!(r.data.amount_out(), 3_001);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, 4_002);
	assert_eq!(solution.trades[0].amount_out, 3_001);
	assert_eq!(solution.score, 0);
}

#[test]
fn zero_limit_intent_should_receive_market_rate_when_resolved() {
	// A "don't care" minimum of 1 must not be paid out as the limit — the user
	// receives the full market-rate output.
	let intents = vec![make_intent(1, 1, 2, 1_000, 1)];
	let solution = Solver::<MockAMMDepth>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.data.amount_out(), 909);
	assert_eq!(solution.score, 908);
}

#[test]
fn zero_limit_intent_should_share_uniform_rate_when_matched_against_opposing_flow() {
	// Two zero-ish-limit intents matched directly: both must settle at the
	// reference (spot) rate, not at each other's limit.
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 2, 1, 100_000, 1)];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 100_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert!(solution.trades.is_empty());
}

// ---------- review-fix regressions ----------

#[test]
fn netting_should_not_emit_a_trade_when_its_output_is_below_ed() {
	use scripted::{script, ScriptedAmm, SellRule};

	// Asset 2 has a 10_000 ED, so the 500-unit residual imbalance between the
	// two intents would route into a 500-unit output the pallet *skips*. The
	// solver must leave that imbalance unrouted and pay only what the pot holds.
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 2, 1, 99_500, 1)];
	let solution = script()
		.ed(1, 1)
		.ed(2, 10_000)
		.default_sell(SellRule::Rate { n: 1, d: 1 })
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	assert!(
		solution.trades.is_empty(),
		"a sub-ED residual trade must not be emitted"
	);
	assert_eq!(solution.resolved_intents.len(), 2);
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 99_500);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert_eq!(solution.score, 199_498);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
	assert_conserves(&solution);
}

#[test]
fn pairwise_round_should_not_emit_a_trade_when_its_output_is_below_ed() {
	use scripted::{script, ScriptedAmm, SellRule};

	// Intent 3 sells an asset with no spot price, which forces the whole batch
	// through the pairwise fallback. There the 1→2 pair nets to a 500-unit AMM
	// sell whose 500-unit output is below asset 2's ED — the pallet would skip
	// that trade, so intent 1 must be excluded instead of paid from it.
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 1),
		make_intent(2, 2, 1, 99_500, 1),
		make_intent(3, 3, 1, 50_000, 1),
	];
	let solution = script()
		.ed(1, 1)
		.ed(2, 10_000)
		.ed(3, 1)
		.price(3, None)
		.default_sell(SellRule::Rate { n: 1, d: 1 })
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	let ids: Vec<IntentId> = solution.resolved_intents.iter().map(|r| r.id).collect();
	assert_eq!(ids, vec![2, 3]);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 99_491);
	assert_eq!(find_resolved(&solution.resolved_intents, 3).data.amount_out(), 49_995);
	assert_eq!(solution.trades.len(), 2);
	assert_eq!(
		(solution.trades[0].amount_in, solution.trades[0].amount_out),
		(99_500, 99_491)
	);
	assert_eq!(
		(solution.trades[1].amount_in, solution.trades[1].amount_out),
		(50_000, 49_995)
	);
	assert_eq!(solution.score, 149_484);
	assert_trades_are_executable(&solution, |a| if a == 2 { 10_000 } else { 1 });
	assert_conserves(&solution);
}

#[test]
fn netting_should_route_surplus_to_the_next_deficit_when_a_route_fails() {
	use scripted::{script, ScriptedAmm, SellRule};

	// Asset 1 is 50_000 in surplus, assets 2 and 3 are in deficit. Asset 2 is
	// tried first and has no route from asset 1 — that failed attempt must not
	// consume the surplus, or asset 3's buyer is paid from asset 4's smaller
	// share and receives roughly half the correct rate.
	let intents = vec![make_intent(1, 1, 3, 50_000, 1), make_intent(2, 4, 2, 100_000, 1)];
	let solution = script()
		.sell(1, 2, SellRule::Fail)
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	assert_eq!(solution.trades.len(), 2);
	let legs: Vec<(AssetId, AssetId, Balance, Balance)> = solution
		.trades
		.iter()
		.map(|t| {
			let route = &t.route;
			(
				route.first().expect("route").asset_in,
				route.last().expect("route").asset_out,
				t.amount_in,
				t.amount_out,
			)
		})
		.collect();
	assert_eq!(legs, vec![(1, 3, 50_000, 49_995), (4, 2, 100_000, 99_990)]);
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 49_995);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 99_990);
	assert_eq!(solution.score, 149_983);
	assert_conserves(&solution);
}

#[test]
fn partial_search_should_find_the_exact_largest_fill_when_amounts_approach_u128_max() {
	use scripted::{script, ScriptedAmm, SellRule};

	// The AMM serves any amount up to `cap` and fails above it, so the largest
	// feasible fill is exactly `cap`. A `(lo + hi) / 2` midpoint saturates once
	// `lo + hi` passes `u128::MAX` and the bisection then stalls at `u128::MAX / 2`.
	let cap = (u128::MAX / 5) * 4;
	let amount_in = u128::MAX - 1;
	let intents = vec![make_partial(1, 1, 2, amount_in, amount_in / 2)];
	let solution = script()
		.default_sell(SellRule::Capped { n: 1, d: 1, cap })
		.run(|| Solver::<ScriptedAmm>::solve(intents, (), Permill::zero()).unwrap());

	assert_eq!(solution.resolved_intents.len(), 1);
	assert_eq!(solution.resolved_intents[0].data.amount_in(), cap);
	// 1 bps haircut on the simulated output.
	assert_eq!(solution.resolved_intents[0].data.amount_out(), cap - cap / 10_000);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(solution.trades[0].amount_in, cap);
}

#[test]
fn crossing_should_refit_the_partial_when_ed_remainder_adjustment_lowers_its_fill() {
	use scripted::{script, ScriptedAmm, SellRule};

	// The crossing trims the partial, the ED-remainder rule then lowers that
	// fill further, and the lower volume no longer clears the limit the trim was
	// proven against. Re-fitting after the adjustment lands on 251 (which clears
	// its pro-rata minimum with a unit to spare) instead of keeping the stale
	// 271 that only just met it.
	let intents = vec![make_partial(1, 1, 2, 1_000, 950), make_intent(2, 2, 1, 900, 810)];
	let solution = script()
		.ed(1, 200)
		.ed(2, 200)
		.default_sell(SellRule::Depth(5_000))
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.id, 1);
	assert_eq!(r.data.amount_in(), 251);
	assert_eq!(r.data.amount_out(), 239);
	assert_eq!(solution.score, 1);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
	// The untouched remainder stays tradeable in a later block.
	assert!(1_000 - r.data.amount_in() >= 200);
}

#[test]
fn crossing_should_keep_both_directions_when_pair_cannot_be_valued_at_spot() {
	use scripted::{script, ScriptedAmm, SellRule};

	// Reference prices that are individually valid but whose cross-conversion
	// does not fit 128 bits. Collapsing that arithmetic failure to a zero-valued
	// flow classifies the pair as one-sided excess and hands the forward
	// direction a zero rate, which silently drops the forward intent; the pair
	// must be quoted through the AMM in both directions instead.
	let intents = vec![make_intent(1, 1, 2, 100_000, 1), make_intent(2, 2, 1, 100_000, 1)];
	let solution = script()
		.price(1, Some(Ratio::new(u128::MAX, 1)))
		.price(2, Some(Ratio::new(1, u128::MAX)))
		.default_sell(SellRule::Rate { n: 1, d: 1 })
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	// Intent 1 survives on an AMM quote of its own direction. Under the
	// error-to-zero reading it was the one dropped and intent 2 took its place.
	assert_eq!(solution.resolved_intents.len(), 1);
	let r = &solution.resolved_intents[0];
	assert_eq!(r.id, 1);
	assert_eq!(r.data.amount_in(), 100_000);
	assert_eq!(r.data.amount_out(), 99_990);
	assert_eq!(solution.trades.len(), 1);
	assert_eq!(
		(solution.trades[0].amount_in, solution.trades[0].amount_out),
		(100_000, 99_990)
	);
	assert_conserves(&solution);
}

#[test]
fn every_trade_should_be_executable_when_batch_mixes_matched_and_routed_volume() {
	use scripted::{script, ScriptedAmm, SellRule};

	let ed = |asset: AssetId| -> Balance {
		match asset {
			2 => 5_000,
			3 => 2_000,
			_ => 1,
		}
	};
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 1),
		make_intent(2, 2, 1, 97_000, 1),
		make_partial(3, 1, 3, 60_000, 1),
		make_intent(4, 3, 1, 59_000, 1),
		make_intent(5, 2, 3, 3_000, 1),
	];
	let solution = script()
		.ed(1, 1)
		.ed(2, 5_000)
		.ed(3, 2_000)
		.default_sell(SellRule::Depth(50_000_000))
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::from_percent(1)).unwrap());

	assert!(!solution.resolved_intents.is_empty());
	assert_trades_are_executable(&solution, ed);
	assert_conserves(&solution);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn opposing_intents_should_still_match_when_every_amm_sell_fails() {
	use scripted::{script, ScriptedAmm, SellRule};

	// Routes exist and price, but the pool refuses every sell. The two intents
	// cancel exactly, so matching alone settles them — a dead AMM must not take
	// the whole batch down with it.
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = script()
		.default_sell(SellRule::SellFails)
		.run(|| Solver::<ScriptedAmm>::solve(intents.clone(), (), Permill::zero()).unwrap());

	assert_eq!(solution.resolved_intents.len(), 2);
	assert!(solution.trades.is_empty());
	assert_eq!(find_resolved(&solution.resolved_intents, 1).data.amount_out(), 100_000);
	assert_eq!(find_resolved(&solution.resolved_intents, 2).data.amount_out(), 100_000);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
	assert_conserves(&solution);
}

// ---------- properties migrated from the retired solver suites ----------

#[test]
fn three_asset_ring_should_settle_without_an_amm_trade_when_volumes_match() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 2, 3, 100, 90),
		make_intent(3, 3, 1, 100, 90),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents.clone(), (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 3);
	assert!(solution.trades.is_empty(), "a closed ring must not touch the AMM");
	for r in solution.resolved_intents.iter() {
		assert_eq!(r.data.amount_in(), 100);
		assert_eq!(r.data.amount_out(), 100);
	}
	assert_eq!(solution.score, 30);
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
}

#[test]
fn matched_volumes_should_settle_exactly_when_amounts_are_large() {
	let unit: Balance = 1_000_000_000_000;
	let intents = vec![
		make_intent(1, 1, 2, 1_000_000 * unit, 900_000 * unit),
		make_intent(2, 2, 1, 1_000_000 * unit, 900_000 * unit),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();

	assert_eq!(solution.resolved_intents.len(), 2);
	assert!(solution.trades.is_empty());
	for r in solution.resolved_intents.iter() {
		assert_eq!(r.data.amount_in(), 1_000_000 * unit);
		assert_eq!(r.data.amount_out(), 1_000_000 * unit);
	}
}

#[test]
fn solve_should_not_panic_when_many_intents_carry_near_maximum_amounts() {
	let per_intent: Balance = u128::MAX / 1_000;
	let mut intents = Vec::new();
	for i in 0..50u128 {
		intents.push(make_intent(i * 2 + 1, 1, 2, per_intent, per_intent / 2));
		intents.push(make_intent(i * 2 + 2, 2, 1, per_intent, per_intent / 2));
	}
	let solution = Solver::<MockAMMOneToOne>::solve(intents.clone(), (), Permill::zero()).unwrap();
	assert_eq!(solution.score, pallet_score(&intents, &solution.resolved_intents));
	assert_conserves(&solution);
}

#[test]
fn score_should_equal_the_pallet_recompute_when_batch_mixes_fill_kinds() {
	let intents = vec![
		make_intent(1, 1, 2, 100, 90),
		make_intent(2, 2, 1, 100, 90),
		make_partial(3, 1, 2, 200, 180),
		make_intent(4, 1, 2, 50, 40),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents.clone(), (), Permill::zero()).unwrap();
	assert_eq!(
		solution.score,
		pallet_score(&intents, solution.resolved_intents.as_slice()),
		"solver score diverges from the pallet recompute",
	);
}

#[test]
fn resolved_amounts_should_clear_existential_deposits_when_ed_is_nonzero() {
	let intents = vec![
		make_intent(1, 1, 2, 100_000, 90_000),
		make_intent(2, 2, 1, 100_000, 90_000),
		make_partial(3, 1, 2, 200_000, 180_000),
	];
	let solution = Solver::<MockAMMWithED>::solve(intents, (), Permill::zero()).unwrap();
	assert!(!solution.resolved_intents.is_empty());
	for r in solution.resolved_intents.iter() {
		assert!(r.data.amount_in() >= 1_000, "intent {} input below ED", r.id);
		assert!(r.data.amount_out() >= 1_000, "intent {} output below ED", r.id);
	}
	assert_trades_are_executable(&solution, |_| 1_000);
}

#[test]
fn partial_should_be_excluded_when_remaining_is_below_existential_deposit() {
	// remaining = 100_000 − 99_500 = 500, below the 1_000 ED.
	let intents = vec![
		make_partial_filled(1, 1, 2, 100_000, 90_000, 99_500),
		make_intent(2, 2, 1, 100_000, 90_000),
	];
	let solution = Solver::<MockAMMWithED>::solve(intents, (), Permill::zero()).unwrap();
	assert!(
		solution.resolved_intents.iter().all(|r| r.id != 1),
		"a partial whose remaining is below ED must not be resolved",
	);
}

#[test]
fn partial_fill_should_leave_no_untradeable_dust_when_ed_is_nonzero() {
	let intents = vec![make_partial(1, 1, 2, 100_000, 90_000)];
	let solution = Solver::<MockAMMWithED>::solve(intents, (), Permill::zero()).unwrap();
	for r in solution.resolved_intents.iter() {
		let remaining_after = 100_000 - r.data.amount_in();
		assert!(
			remaining_after == 0 || remaining_after >= 1_000,
			"fill {} left an untradeable remainder of {remaining_after}",
			r.data.amount_in(),
		);
	}
}

#[test]
fn partial_should_not_exceed_remaining_when_solved_across_two_rounds() {
	let original: Balance = 200;
	let first = Solver::<MockAMMWithSlippage>::solve(
		vec![make_partial(1, 1, 2, original, 150), make_intent(2, 2, 1, 100, 90)],
		(),
		Permill::zero(),
	)
	.unwrap();
	let first_fill = first
		.resolved_intents
		.iter()
		.find(|r| r.id == 1)
		.map(|r| r.data.amount_in())
		.unwrap_or(0);
	assert!(first_fill > 0 && first_fill <= original);

	if first_fill < original {
		let second = Solver::<MockAMMWithSlippage>::solve(
			vec![
				make_partial_filled(1, 1, 2, original, 150, first_fill),
				make_intent(2, 2, 1, 100, 90),
			],
			(),
			Permill::zero(),
		)
		.unwrap();
		let second_fill = second
			.resolved_intents
			.iter()
			.find(|r| r.id == 1)
			.map(|r| r.data.amount_in())
			.unwrap_or(0);
		assert!(
			first_fill.saturating_add(second_fill) <= original,
			"cumulative fill {first_fill} + {second_fill} exceeds the original {original}",
		);
	}
}

#[test]
fn cap_should_keep_the_highest_surplus_intent_when_batch_exceeds_the_maximum() {
	let mut intents: Vec<Intent> = (0..ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS as u128)
		.map(|id| make_intent(id + 1, 1, 2, 100_000, 99_000))
		.collect();
	// A far looser opposite-direction intent added last — it must survive the cap.
	intents.push(make_intent(u128::MAX, 2, 1, 100_000, 10_000));
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();
	assert_eq!(
		solution.resolved_intents.len(),
		ice_support::MAX_NUMBER_OF_RESOLVED_INTENTS as usize
	);
	assert!(
		solution.resolved_intents.iter().any(|r| r.id == u128::MAX),
		"the highest-surplus intent was dropped by a first-N cap",
	);
}

#[test]
fn ring_should_not_fill_more_than_remaining_when_partial_is_already_filled() {
	// A→B is a partial with only 40 of its 100 left; the B→C and C→A legs are
	// full. The cycle must be capped at the partial's remaining volume.
	let intents = vec![
		make_partial_filled(1, 1, 2, 100, 90, 60),
		make_intent(2, 2, 3, 100, 90),
		make_intent(3, 3, 1, 100, 90),
	];
	let solution = Solver::<MockAMMOneToOne>::solve(intents, (), Permill::zero()).unwrap();
	if let Some(r) = solution.resolved_intents.iter().find(|r| r.id == 1) {
		assert!(
			r.data.amount_in() <= 40,
			"cycle filled {} of a partial with 40 remaining",
			r.data.amount_in(),
		);
		assert!(
			r.data.amount_out() <= r.data.amount_in(),
			"output {} exceeds input {} at a 1:1 reference price",
			r.data.amount_out(),
			r.data.amount_in(),
		);
	}
	assert_conserves(&solution);
}
