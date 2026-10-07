//! Route splitting on real `mainnet_apr` state: a shallow, low-fee WETH/ETH
//! stableswap pool next to the Omnipool gives the pair a second venue that
//! prices the first units better, so the solver splits a large enough transfer
//! between the two. Every scenario submits its solution, so the pallet's own
//! conservation and score checks are the oracle.

use crate::polkadot_test_net::{TestNet, ALICE};
use frame_support::assert_ok;
use frame_support::storage::{with_transaction, TransactionOutcome};
use frame_support::BoundedVec;
use hydradx_runtime::{AssetRegistry, Currencies, RuntimeOrigin, Stableswap};
use hydradx_traits::amm::AMMInterface;
use hydradx_traits::router::PoolType;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::{AssetKind, Create};
use ice_support::{AssetId, Balance, Solution, SolverMode};
use orml_traits::MultiCurrency;
use primitives::AccountId;
use sp_runtime::Permill;
use xcm_emulator::Network;

use super::harness::{
	enable_slip_fees, run_and_submit_as, solve_as, CombinedSimulatorState, TestSimulator, V4NoSplit, V4Solver,
};
use super::PATH_TO_SNAPSHOT;
use hydradx_traits::amm::SimulatorSet;
use ice_solver::common::split::{adjust_amm_output, claimed_out, SellBudget, SPLIT_GRID};
use ice_solver::common::RouteCache;
use sp_core::U256;
use std::collections::{BTreeMap, BTreeSet};

type Shape = Vec<(Vec<PoolType<AssetId>>, Balance, Balance)>;

const WETH: AssetId = 20;
const ETH: AssetId = 34;
const UNIT: Balance = 1_000_000_000_000_000_000;

/// A 1 bps, amplification-100 stableswap pool of WETH and ETH holding
/// `liquidity` of each: a second route for the pair next to its existing one
/// through stableswap 104 and Aave. Returns the pool id.
fn create_two_venue_pool(liquidity: Balance) -> AssetId {
	let pool = with_transaction(|| {
		TransactionOutcome::Commit(AssetRegistry::register_sufficient_asset(
			None,
			Some(b"two-venue".to_vec().try_into().unwrap()),
			AssetKind::StableSwap,
			1u128,
			None,
			Some(18),
			None,
			None,
		))
	})
	.unwrap();
	assert_ok!(Stableswap::create_pool(
		RuntimeOrigin::root(),
		pool,
		BoundedVec::truncate_from(vec![WETH, ETH]),
		100,
		Permill::from_rational(1u32, 10_000u32),
	));
	let lp = AccountId::from([77u8; 32]);
	for asset in [WETH, ETH] {
		assert_ok!(Currencies::update_balance(
			RuntimeOrigin::root(),
			lp.clone(),
			asset,
			liquidity as i128,
		));
	}
	assert_ok!(Stableswap::add_assets_liquidity(
		RuntimeOrigin::signed(lp),
		pool,
		BoundedVec::truncate_from(vec![
			AssetAmount::new(WETH, liquidity),
			AssetAmount::new(ETH, liquidity)
		]),
		0,
	));
	pool
}

fn pools(solution: &Solution) -> Shape {
	solution
		.trades
		.iter()
		.map(|t| (t.route.iter().map(|h| h.pool).collect(), t.amount_in, t.amount_out))
		.collect()
}

#[test]
#[ignore]
fn probe_two_venue_pool() {
	TestNet::reset();
	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT).execute(|| {
		enable_slip_fees();
		for liquidity in [5 * UNIT] {
			let pool = create_two_venue_pool(liquidity);
			let state = <hydradx_runtime::HydrationSimulators as hydradx_traits::amm::SimulatorSet>::initial_state();
			for (asset_in, asset_out) in [(ETH, WETH)] {
				let routes = TestSimulator::discover_routes(asset_in, asset_out, &state).unwrap_or_default();
				for size in [10u128, 20, 25, 30, 35, 40, 50] {
					let amount = size * UNIT;
					for route in &routes {
						let out = TestSimulator::sell(asset_in, asset_out, amount, route.clone(), &state)
							.map(|(_, e)| e.amount_out);
						let hops: Vec<_> = route.iter().map(|t| t.pool).collect();
						println!("pool {pool} liq {liquidity} {asset_in}->{asset_out} {size}: {hops:?} -> {out:?}");
					}
					let split = ice_solver::common::RouteCache::<TestSimulator>::new().best_split_sell(
						asset_in,
						asset_out,
						amount,
						&state,
						2,
						ice_solver::common::split::SPLIT_GRID,
						&mut ice_solver::common::split::SellBudget::new(10_000),
					);
					let legs: Vec<_> = split
						.map(|r| {
							r.legs
								.iter()
								.map(|l| {
									(
										l.route.iter().map(|t| t.pool).collect::<Vec<_>>(),
										l.amount_in,
										l.amount_out,
									)
								})
								.collect()
						})
						.unwrap_or_default();
					println!("  split: {legs:?}");
				}
			}
		}
	});
}

const ALICE_ID: u128 = 32752052247409382067756072960000;
const BOB_ID: u128 = 32752052247409382067756072960001;

fn resolved(solution: &Solution) -> Vec<(u128, Balance, Balance)> {
	solution
		.resolved_intents
		.iter()
		.map(|r| (r.id, r.data.amount_in(), r.data.amount_out()))
		.collect()
}

fn existing_route() -> Vec<PoolType<AssetId>> {
	vec![PoolType::Aave, PoolType::Stableswap(104)]
}

#[test]
fn intent_should_settle_through_both_routes_when_the_new_pool_prices_the_first_chunk_better() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let sale = 40 * UNIT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), ETH, sale)
		.submit_swap_intent(alice.clone(), ETH, WETH, sale, 30 * UNIT, Some(10))
		.execute(|| {
			enable_slip_fees();
			let pool = create_two_venue_pool(5 * UNIT);

			let single = solve_as::<V4NoSplit>().expect("the existing route alone clears the limit");
			let before = Currencies::free_balance(WETH, &alice);
			let solution = run_and_submit_as::<V4Solver>(SolverMode::V4, "stableswap_split");
			let received = Currencies::free_balance(WETH, &alice) - before;

			let single_route: Shape = vec![(existing_route(), sale, 39_316_158_329_486_508_810)];
			assert_eq!(pools(&single), single_route);
			assert_eq!(resolved(&single), vec![(ALICE_ID, sale, 39_316_158_329_486_508_810)]);
			let split: Shape = vec![
				(vec![PoolType::Stableswap(pool)], 4 * UNIT, 3_920_465_932_961_080_778),
				(existing_route(), 35_999_999_999_999_999_998, 35_552_341_506_643_445_547),
			];
			assert_eq!(pools(&solution), split);
			assert_eq!(resolved(&solution), vec![(ALICE_ID, sale, 39_472_807_439_604_526_325)]);
			assert_eq!(received, 39_472_807_439_604_526_325);
		});
}

#[test]
fn intent_should_stay_on_one_route_when_split_gain_is_below_threshold() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let sale = 20 * UNIT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), ETH, sale)
		.submit_swap_intent(alice.clone(), ETH, WETH, sale, 15 * UNIT, Some(10))
		.execute(|| {
			enable_slip_fees();
			create_two_venue_pool(5 * UNIT);

			let single = solve_as::<V4NoSplit>().expect("resolves");
			let solution = run_and_submit_as::<V4Solver>(SolverMode::V4, "stableswap_below_threshold");

			let one_route: Shape = vec![(existing_route(), sale, 19_913_168_068_155_459_090)];
			assert_eq!(pools(&single), one_route);
			assert_eq!(pools(&solution), one_route);
			assert_eq!(resolved(&solution), vec![(ALICE_ID, sale, 19_913_168_068_155_459_090)]);
		});
}

#[test]
fn netting_should_split_only_the_residual_when_opposing_flow_leaves_one() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let bob: AccountId = crate::polkadot_test_net::BOB.into();

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), ETH, 50 * UNIT)
		.endow_account(bob.clone(), WETH, 8 * UNIT)
		.submit_swap_intent(alice.clone(), ETH, WETH, 50 * UNIT, 40 * UNIT, Some(10))
		.submit_swap_intent(bob.clone(), WETH, ETH, 8 * UNIT, 7 * UNIT, Some(10))
		.execute(|| {
			enable_slip_fees();
			let pool = create_two_venue_pool(5 * UNIT);

			let single = solve_as::<V4NoSplit>().expect("resolves");
			let solution = run_and_submit_as::<V4Solver>(SolverMode::V4, "stableswap_netting");

			// Bob's side is matched against Alice's and never reaches a pool; only
			// Alice's residual is routed, and only it is split.
			let residual = 42_000_800_000_000_000_000;
			assert_eq!(
				pools(&single),
				vec![(existing_route(), residual, 41_140_146_721_530_196_571)]
			);
			assert_eq!(
				pools(&solution),
				vec![
					(
						vec![PoolType::Stableswap(pool)],
						4_200_080_000_000_000_000,
						4_098_422_336_583_598_486
					),
					(existing_route(), 37_800_719_999_999_999_998, 37_261_524_673_764_943_420),
				]
			);
			assert_eq!(
				resolved(&single),
				vec![
					(BOB_ID, 8 * UNIT, 7_997_600_160_000_000_000),
					(ALICE_ID, 50 * UNIT, 49_138_546_721_530_196_571),
				]
			);
			assert_eq!(
				resolved(&solution),
				vec![
					(BOB_ID, 8 * UNIT, 7_997_600_160_000_000_002),
					(ALICE_ID, 50 * UNIT, 49_358_347_010_348_541_906),
				]
			);
		});
}

#[test]
fn passthrough_should_never_split() {
	TestNet::reset();
	let alice: AccountId = ALICE.into();
	let sale = 40 * UNIT;

	crate::driver::HydrationTestDriver::with_snapshot(PATH_TO_SNAPSHOT)
		.endow_account(alice.clone(), ETH, sale)
		.submit_swap_intent(alice.clone(), ETH, WETH, sale, 30 * UNIT, Some(10))
		.execute(|| {
			enable_slip_fees();
			create_two_venue_pool(5 * UNIT);

			let solution = run_and_submit_as::<super::harness::PassthroughSolver>(
				SolverMode::Passthrough,
				"stableswap_passthrough",
			);

			// Pass-through claims the raw quote of the single best route.
			assert_eq!(
				pools(&solution),
				vec![(existing_route(), sale, 39_320_090_338_520_360_846)]
			);
		});
}

type Price = (U256, U256);

/// The Uniswap snapshot with its aDOT / HOLLAR v3 pool registered: the Omnipool
/// and the v3 pool both serve aDOT, the case splitting is for. v3 is opt-in, so
/// it is registered wherever the snapshot has it deployed.
fn with_uniswap_pool(path: &str, f: impl FnOnce()) {
	TestNet::reset();
	crate::polkadot_test_net::hydra_live_ext(path).execute_with(|| {
		crate::uniswap_v3_router::reset_consensus_slots();
		if hydradx_runtime::Parameters::uniswap_v3_factory().is_none() {
			assert_ok!(hydradx_runtime::Parameters::set_uniswap_v3_addresses(
				RuntimeOrigin::root(),
				crate::uniswap_v3_router::UNISWAP_V3_FACTORY,
				crate::uniswap_v3_router::UNISWAP_V3_SWAP_ROUTER,
				crate::uniswap_v3_router::UNISWAP_V3_QUOTER,
			));
		}
		if let Ok(Some(pool)) = hydradx_runtime::evm::uniswap_v3_trade_executor::UniswapV3::find_pool(1001, 222, 3000) {
			assert_ok!(hydradx_runtime::ICE::update_routing(
				RuntimeOrigin::root(),
				ice_support::RoutingTarget::UniswapV3Pool(pool),
				Some(ice_support::RoutingState::Included),
			));
		}
		f();
	});
}

/// USDT per unit of every asset in the routing graph (raw / raw): its best spot
/// price over its routes to USDT.
fn usdt_prices(state: &CombinedSimulatorState, cache: &mut RouteCache<TestSimulator>) -> BTreeMap<AssetId, Price> {
	const USDT: AssetId = 10;
	let assets: BTreeSet<AssetId> = <hydradx_runtime::HydrationSimulators as SimulatorSet>::pool_edges(state)
		.into_iter()
		.flat_map(|e| e.assets)
		.collect();
	let mut prices = BTreeMap::new();
	prices.insert(USDT, (U256::one(), U256::one()));
	for &asset in assets.iter().filter(|a| **a != USDT) {
		let routes = cache.routes(asset, USDT, state).to_vec();
		let best = routes
			.into_iter()
			.filter_map(|r| TestSimulator::get_spot_price(asset, USDT, r, state).ok())
			.filter(|p| p.n > 0 && p.d > 0)
			.max_by(|x, y| (U256::from(x.n) * U256::from(y.d)).cmp(&(U256::from(y.n) * U256::from(x.d))));
		if let Some(p) = best {
			prices.insert(asset, (U256::from(p.n), U256::from(p.d)));
		}
	}
	prices
}

fn usd_amount(usd: u128, (n, d): Price) -> Option<Balance> {
	u128::try_from(U256::from(usd) * U256::from(1_000_000u128) * d / n).ok()
}

fn cents(raw: Balance, (n, d): Price) -> u128 {
	u128::try_from(U256::from(raw) * n / d / U256::from(10_000u128)).unwrap_or(u128::MAX)
}

/// Where splitting pays on real state: every directed pair among the routing
/// graph's assets that have a USDT price, at $1k / $10k / $100k, the single best
/// route against the split, each split settled on chain both ways.
/// `SPLIT_PROBE_SNAPSHOT` overrides the snapshot (default: the Uniswap one).
#[test]
#[ignore]
fn probe_split_gains_on_real_state() {
	let path =
		std::env::var("SPLIT_PROBE_SNAPSHOT").unwrap_or_else(|_| crate::uniswap_v3_router::PATH_TO_SNAPSHOT.into());
	with_uniswap_pool(&path, || {
		let state = <hydradx_runtime::HydrationSimulators as SimulatorSet>::initial_state();
		println!(
			"{path}: {} omnipool assets, {} stableswap pools, {} aave pairs, {} uniswap v3 pools",
			state.0.assets.len(),
			state.1.pools.len(),
			state.2.pairs.len(),
			state.3.pools.len(),
		);
		let mut cache = RouteCache::<TestSimulator>::new();
		let prices = usdt_prices(&state, &mut cache);

		for usd in [1_000u128, 10_000, 100_000] {
			let started = std::time::Instant::now();
			let (mut routable, mut multi_route) = (0u32, 0u32);
			let mut splits = Vec::new();
			for (&a, &price_a) in &prices {
				let Some(amount) = usd_amount(usd, price_a) else {
					continue;
				};
				for (&b, &price_b) in &prices {
					if a == b {
						continue;
					}
					let routes = cache.routes(a, b, &state).len();
					let Some((_, single_out, _)) = cache.best_sell(a, b, amount, &state) else {
						continue;
					};
					routable += 1;
					if routes < 2 {
						continue;
					}
					multi_route += 1;
					let Some(routed) =
						cache.best_split_sell(a, b, amount, &state, 2, SPLIT_GRID, &mut SellBudget::new(u32::MAX))
					else {
						continue;
					};
					if routed.legs.len() < 2 {
						continue;
					}
					let gain = claimed_out(&routed.legs).saturating_sub(adjust_amm_output(single_out));
					let legs: Vec<(Vec<PoolType<AssetId>>, u128)> = routed
						.legs
						.iter()
						.map(|l| (pools_of(&l.route), l.amount_in * 100 / amount))
						.collect();
					splits.push((a, b, amount, single_out, gain, price_b, legs));
				}
			}
			// Trades a user would place keep most of their value on the single route;
			// the rest run through depleted pools, where any split looks huge.
			let (mut sane, mut degenerate) = (Vec::new(), 0u32);
			for (a, b, amount, single_out, simulated, price_b, legs) in splits {
				let quality = cents(single_out, price_b) / usd;
				if quality < 90 {
					degenerate += 1;
					continue;
				}
				let order = Order {
					owner: ALICE.into(),
					asset_in: a,
					asset_out: b,
					amount_in: amount,
					min_out: single_out / 2,
				};
				let chain = match settle_both_ways(&[order]) {
					(_, skipped, _) if !skipped.is_empty() => Err(skipped.join(", ")),
					(_, _, [Ok(single), Ok(split)]) => {
						let (x, y) = (single.received[0], split.received[0]);
						Ok(if y >= x {
							cents(y - x, price_b) as i128
						} else {
							-(cents(x - y, price_b) as i128)
						})
					}
					(_, _, [Ok(_), Err(e)]) => Err(format!("split not settled: {e}")),
					(_, _, [Err(e), _]) => Err(format!("single route not settled: {e}")),
				};
				let bps = simulated * 10_000 / single_out.max(1);
				sane.push((cents(simulated, price_b), bps, quality, a, b, chain, legs));
			}
			sane.sort_by(|x, y| y.0.cmp(&x.0));
			let verified: Vec<i128> = sane.iter().filter_map(|r| r.5.as_ref().ok().copied()).collect();
			println!(
				"${usd}: {routable} routable pairs, {multi_route} with 2+ routes; splits: {degenerate} on depleted paths \
				 (single route keeps < 90 % of value), {} on sane trades — settled both ways {}, split pays more {}, \
				 total ${:.2} ({:?})",
				sane.len(),
				verified.len(),
				verified.iter().filter(|g| **g > 0).count(),
				verified.iter().sum::<i128>() as f64 / 100.0,
				started.elapsed(),
			);
			for (sim, bps, quality, a, b, chain, legs) in &sane {
				let chain = match chain {
					Ok(g) => format!("{:+.2} $ on chain", *g as f64 / 100.0),
					Err(e) => e.clone(),
				};
				println!(
					"  {:>6} -> {:<6} single keeps {quality} % | simulated +{bps} bps +${:.2} | {chain} | {legs:?}",
					symbol(*a),
					symbol(*b),
					*sim as f64 / 100.0
				);
			}
		}
	});
}

/// Batches on real state: random mixes of the pairs that split at $10k — two to
/// six owners, $2k–$20k each, some trading against each other so part of the
/// flow matches — settled on chain with the solver as shipped (best-of-both
/// guard included) and with splitting off, both rolled back. Does a batch ever
/// come out worse, and do splits survive the guard? `SPLIT_BATCHES` sets how
/// many (default 40).
#[test]
#[ignore]
fn probe_split_batches_on_real_state() {
	let batches: u64 = std::env::var("SPLIT_BATCHES")
		.ok()
		.and_then(|v| v.parse().ok())
		.unwrap_or(40);
	with_uniswap_pool(crate::uniswap_v3_router::PATH_TO_SNAPSHOT, || {
		let state = <hydradx_runtime::HydrationSimulators as SimulatorSet>::initial_state();
		let mut cache = RouteCache::<TestSimulator>::new();
		let prices = usdt_prices(&state, &mut cache);
		let mut pairs = Vec::new();
		for (&a, &price_a) in &prices {
			let Some(amount) = usd_amount(10_000, price_a) else {
				continue;
			};
			for (&b, &price_b) in &prices {
				if a == b || cache.routes(a, b, &state).len() < 2 {
					continue;
				}
				let Some((_, single_out, _)) = cache.best_sell(a, b, amount, &state) else {
					continue;
				};
				if cents(single_out, price_b) < 900_000 {
					continue;
				}
				if cache
					.best_split_sell(a, b, amount, &state, 2, SPLIT_GRID, &mut SellBudget::new(u32::MAX))
					.is_some_and(|r| r.legs.len() > 1)
				{
					pairs.push((a, b));
				}
			}
		}
		println!("{} pairs split at $10k", pairs.len());

		let mut seed = 0x2545_f491_4f6c_dd1du64;
		let mut next = move || {
			seed ^= seed << 13;
			seed ^= seed >> 7;
			seed ^= seed << 17;
			seed
		};
		let mut order = |a: AssetId, b: AssetId, usd: u128, owner: usize| -> Option<Order> {
			let amount_in = usd_amount(usd, prices[&a])?;
			let (_, quote, _) = cache.best_sell(a, b, amount_in, &state)?;
			Some(Order {
				owner: AccountId::from([100 + owner as u8; 32]),
				asset_in: a,
				asset_out: b,
				amount_in,
				min_out: quote / 2,
			})
		};
		let (mut compared, mut better, mut same, mut worse, mut kept_split) = (0u32, 0u32, 0u32, 0u32, 0u32);
		let (mut value_single, mut value_split, mut fills_single, mut fills_split) = (0u128, 0u128, 0usize, 0usize);
		for batch in 0..batches {
			let mut orders = Vec::new();
			for _ in 0..2 + next() % 5 {
				let (a, b) = pairs[(next() % pairs.len() as u64) as usize];
				let usd = [2_000u128, 5_000, 10_000, 20_000][(next() % 4) as usize];
				orders.extend(order(a, b, usd, orders.len()));
				if next() % 3 == 0 {
					orders.extend(order(b, a, usd / 2, orders.len()));
				}
			}
			let (submitted, _, outcome) = settle_both_ways(&orders);
			let [Ok(single), Ok(split)] = outcome else {
				let shown: Vec<_> = orders
					.iter()
					.map(|o| format!("{} {} -> {}", o.amount_in, symbol(o.asset_in), symbol(o.asset_out)))
					.collect();
				println!(
					"batch {batch:>3}: {} orders — not settled both ways: {outcome:?} | {shown:?}",
					submitted.len()
				);
				continue;
			};
			let value = |s: &Settlement| -> u128 {
				s.received
					.iter()
					.zip(&submitted)
					.map(|(r, &i)| cents(*r, prices[&orders[i].asset_out]))
					.sum()
			};
			let fills = |s: &Settlement| s.received.iter().filter(|r| **r > 0).count();
			let (v0, v1) = (value(&single), value(&split));
			compared += 1;
			match v1.cmp(&v0) {
				std::cmp::Ordering::Greater => better += 1,
				std::cmp::Ordering::Equal => same += 1,
				std::cmp::Ordering::Less => worse += 1,
			}
			kept_split += u32::from(split.split);
			value_single += v0;
			value_split += v1;
			fills_single += fills(&single);
			fills_split += fills(&split);
			println!(
				"batch {batch:>3}: {} orders | single: {} filled, ${:.2}, score {} | shipped: {} filled, ${:.2}, score {}{} | {:+.2} $",
				submitted.len(),
				fills(&single),
				v0 as f64 / 100.0,
				single.score,
				fills(&split),
				v1 as f64 / 100.0,
				split.score,
				if split.split { ", split legs" } else { "" },
				(v1 as f64 - v0 as f64) / 100.0,
			);
		}
		println!(
			"{compared} batches settled both ways: shipped solver better in {better}, equal in {same}, worse in {worse}; \
			 split legs kept in {kept_split}; filled {fills_single} -> {fills_split}; received ${:.2} -> ${:.2}",
			value_single as f64 / 100.0,
			value_split as f64 / 100.0,
		);
	});
}

/// One split candidate from `probe_split_gains_on_real_state`, end to end on the
/// Uniswap snapshot: every route's output at 25–100 % of the amount, the single
/// route and the split replayed hop by hop, then the intent settled on chain with
/// and without splitting.
/// `SPLIT_CASE="asset_in,asset_out,usd"`.
#[test]
#[ignore]
fn probe_split_case_on_chain() {
	let case = std::env::var("SPLIT_CASE").expect("SPLIT_CASE=asset_in,asset_out,usd");
	let parts: Vec<u128> = case.split(',').map(|p| p.trim().parse().expect("number")).collect();
	let (asset_in, asset_out, usd) = (parts[0] as AssetId, parts[1] as AssetId, parts[2]);

	with_uniswap_pool(crate::uniswap_v3_router::PATH_TO_SNAPSHOT, || {
		println!(
			"omnipool slip fee config: {:?}",
			pallet_omnipool::SlipFee::<hydradx_runtime::Runtime>::get()
		);
		for asset in [asset_in, asset_out] {
			let meta = AssetRegistry::assets(asset);
			println!(
				"asset {asset}: {:?}",
				meta.map(|m| (m.symbol, m.decimals, m.asset_type))
			);
		}
		let state = <hydradx_runtime::HydrationSimulators as SimulatorSet>::initial_state();
		let mut cache = RouteCache::<TestSimulator>::new();
		let prices = usdt_prices(&state, &mut cache);
		let amount = usd_amount(usd, prices[&asset_in]).expect("amount fits");
		println!("{usd} USD of {asset_in} = {amount}");
		for route in cache.routes(asset_in, asset_out, &state).to_vec() {
			let outs: Vec<_> = [25u128, 50, 75, 97, 100]
				.iter()
				.map(|pct| {
					TestSimulator::sell(asset_in, asset_out, amount * pct / 100, route.clone(), &state)
						.map(|(_, e)| e.amount_out)
						.ok()
				})
				.collect();
			println!("  {:?}: at 25/50/75/97/100 % -> {outs:?}", pools_of(&route));
		}
		let (route, quote, _) = cache
			.best_sell(asset_in, asset_out, amount, &state)
			.expect("pair is routable");
		let split = cache
			.best_split_sell(
				asset_in,
				asset_out,
				amount,
				&state,
				2,
				SPLIT_GRID,
				&mut SellBudget::new(u32::MAX),
			)
			.expect("pair is routable");
		let split_legs: Vec<_> = split.legs.iter().map(|l| (l.route.clone(), l.amount_in)).collect();
		for (label, legs) in [("single", vec![(route, amount)]), ("split", split_legs)] {
			let mut hop_state = state.clone();
			for (route, amount_in) in legs {
				let mut amount = amount_in;
				let mut hops = Vec::new();
				for hop in route.iter() {
					let (next, trade) = <hydradx_runtime::HydrationSimulators as SimulatorSet>::simulate_sell(
						hop.pool,
						hop.asset_in,
						hop.asset_out,
						amount,
						0,
						&hop_state,
					)
					.expect("route simulates");
					hop_state = next;
					amount = trade.amount_out;
					hops.push(format!(
						"{:?} {} -> {} {amount}",
						hop.pool,
						symbol(hop.asset_in),
						symbol(hop.asset_out)
					));
				}
				println!("  {label} leg {amount_in}: {}", hops.join(" | "));
			}
		}
		let order = Order {
			owner: ALICE.into(),
			asset_in,
			asset_out,
			amount_in: amount,
			min_out: quote / 2,
		};
		let (_, skipped, [single, split]) = settle_both_ways(&[order]);
		println!("skipped: {skipped:?}");
		println!("single: {:?}", single.map(|s| s.received));
		println!("split: {:?}", split.map(|s| (s.received, s.split)));
	});
}

struct Order {
	owner: AccountId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
}

#[derive(Debug)]
struct Settlement {
	/// Per submitted order, in `settle_both_ways`' order.
	received: Vec<Balance>,
	score: Balance,
	/// The solution routes a directed pair through more than one trade.
	split: bool,
}

/// Fund `who` with `amount` of `asset`: minted where the asset allows it, else
/// taken from the treasury — never by minting stableswap shares, which the pool
/// tracks and would refuse to burn.
fn fund(who: &AccountId, asset: AssetId, amount: Balance) -> bool {
	if !pallet_stableswap::Pools::<hydradx_runtime::Runtime>::contains_key(asset) {
		let _ = Currencies::update_balance(RuntimeOrigin::root(), who.clone(), asset, amount as i128);
	}
	if Currencies::free_balance(asset, who) < amount {
		let _ = hydradx_runtime::Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(hydradx_runtime::Treasury::account_id()),
			Box::new(hydradx_runtime::RuntimeCall::Currencies(
				pallet_currencies::Call::transfer {
					dest: who.clone(),
					currency_id: asset,
					amount,
				},
			)),
			1_000_000,
		);
	}
	Currencies::free_balance(asset, who) >= amount
}

/// `orders` submitted as intents and settled on chain with splitting off and
/// with the solver as shipped: the submitted orders, why the others were left
/// out, and each settlement or why it failed. Everything is rolled back.
fn settle_both_ways(orders: &[Order]) -> (Vec<usize>, Vec<String>, [Result<Settlement, String>; 2]) {
	use sp_runtime::DispatchError;

	let settle = |submitted: &[usize], split: bool| -> Result<Settlement, String> {
		with_transaction(|| {
			let before: Vec<Balance> = submitted
				.iter()
				.map(|&i| Currencies::free_balance(orders[i].asset_out, &orders[i].owner))
				.collect();
			let solution = if split {
				solve_as::<V4Solver>()
			} else {
				solve_as::<V4NoSplit>()
			};
			let result = match solution {
				None => Err("no solution".to_string()),
				Some(solution) => {
					let (score, split) = (solution.score, has_split(&solution));
					pallet_ice::Pallet::<hydradx_runtime::Runtime>::submit_solution(RuntimeOrigin::none(), solution)
						.map(|_| Settlement {
							received: submitted
								.iter()
								.zip(&before)
								.map(|(&i, b)| Currencies::free_balance(orders[i].asset_out, &orders[i].owner) - b)
								.collect(),
							score,
							split,
						})
						.map_err(|e| format!("{:?}", e.error))
				}
			};
			TransactionOutcome::Rollback(Ok::<_, DispatchError>(result))
		})
		.unwrap_or_else(|e| Err(format!("{e:?}")))
	};
	with_transaction(|| {
		let (mut submitted, mut skipped) = (Vec::new(), Vec::new());
		for (i, o) in orders.iter().enumerate() {
			if !fund(&o.owner, o.asset_in, o.amount_in) {
				skipped.push("owner not funded".to_string());
				continue;
			}
			match hydradx_runtime::Intent::submit_intent(
				RuntimeOrigin::signed(o.owner.clone()),
				pallet_intent::types::IntentInput {
					data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
						asset_in: o.asset_in,
						asset_out: o.asset_out,
						amount_in: o.amount_in,
						amount_out: o.min_out.max(1),
						partial: false,
					}),
					deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
					on_resolved: None,
				},
			) {
				Ok(_) => submitted.push(i),
				Err(e) => skipped.push(format!("intent refused: {e:?}")),
			}
		}
		let outcome = if submitted.is_empty() {
			[
				Err("nothing submitted".to_string()),
				Err("nothing submitted".to_string()),
			]
		} else {
			[settle(&submitted, false), settle(&submitted, true)]
		};
		TransactionOutcome::Rollback(Ok::<_, DispatchError>((submitted, skipped, outcome)))
	})
	.unwrap_or_else(|e| {
		(
			Vec::new(),
			vec![format!("{e:?}")],
			[Err(String::new()), Err(String::new())],
		)
	})
}

fn has_split(solution: &Solution) -> bool {
	let mut pairs = BTreeSet::new();
	solution
		.trades
		.iter()
		.filter_map(|t| Some((t.route.first()?.asset_in, t.route.last()?.asset_out)))
		.any(|pair| !pairs.insert(pair))
}

fn symbol(asset: AssetId) -> String {
	AssetRegistry::assets(asset)
		.and_then(|a| a.symbol)
		.map(|s| String::from_utf8_lossy(&s).into_owned())
		.unwrap_or_else(|| asset.to_string())
}

fn pools_of(route: &hydradx_traits::router::Route<AssetId>) -> Vec<PoolType<AssetId>> {
	route.iter().map(|t| t.pool).collect()
}
