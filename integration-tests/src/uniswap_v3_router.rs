#![cfg(test)]

use crate::dca::schedule_fake_with_sell_order;
use crate::polkadot_test_net::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::uniswap_v3_trade_executor::UniswapV3;
use hydradx_runtime::{
	AssetId, AssetRegistry, Currencies, Dispatcher, Parameters, Router, Runtime, RuntimeCall, RuntimeEvent,
	RuntimeOrigin, System, Treasury, DCA,
};
use hydradx_traits::router::{PoolType, Trade};
use hydradx_traits::OraclePeriod;
use orml_traits::MultiCurrency;
use pallet_broadcast::types::Filler;
use pallet_route_executor::TradeExecution;
use primitives::constants::chain::UNISWAPV3_SOURCE;
use primitives::{AccountId, Balance, EvmAddress};
use sp_core::H160;

pub const PATH_TO_SNAPSHOT: &str = "snapshots/ice/SNAPSHOT_uni";

/// Fallback deployment addresses, used only when the snapshot does not already
/// carry them in `Parameters` storage.
///
/// Scraping the `Parameters` pallet after calling `parameters.setUniswapV3Addresses`
/// on the source chain bakes the real addresses into the snapshot, which is the
/// preferred path — it keeps the addresses and the EVM state that backs them in
/// lockstep. These constants exist for a snapshot taken before that call.
const UNISWAP_V3_FACTORY: EvmAddress = H160(hex!("A7E6615794613Eb652d3E6e5D93ad4582eE88c07"));
const UNISWAP_V3_SWAP_ROUTER: EvmAddress = H160(hex!("424eD53e987cbaB5BfdA0dbefa7c937482AaE184"));
const UNISWAP_V3_QUOTER: EvmAddress = H160(hex!("e26B29a77E0d73c2E9eFC247a3DF201A88B6D5eA"));

/// aDOT / HOLLAR, the only Uniswap v3 pool deployed on mainnet.
/// Both are `Erc20`-kind assets, so they cannot be minted — see `fund_alice`.
const ASSET_IN: AssetId = 1001;
const ASSET_OUT: AssetId = 222;
/// The gas token on Hydration's EVM — `eth_getBalance` is this asset's balance.
/// `Token`-kind, so it can still be minted.
const GAS_ASSET: AssetId = 20;
const FEE_TIER: u32 = 3000;
/// 0.1 aDOT (10 decimals). Small enough against the pool that price impact stays
/// negligible and the exact-output test can round-trip.
const SELL_AMOUNT: Balance = 1_000_000_000;
/// 100 aDOT — well inside what the treasury holds in the snapshot (~361).
const FUND_ASSET_IN: Balance = 1_000_000_000_000;
/// DCA enforces `amount_in >= transaction_fee * 20` and a total budget of at
/// least `MinBudgetInNativeCurrency` (1000 HDX), both converted into the sold
/// asset — so a DCA leg needs far more than `SELL_AMOUNT`. 5 aDOT per trade.
const DCA_SELL_AMOUNT: Balance = 50_000_000_000;
/// 100 HOLLAR (18 decimals).
const FUND_ASSET_OUT: Balance = 100_000_000_000_000_000_000;
const FUND_GAS: Balance = 1_000_000_000_000_000_000;
/// Matches `pallet_ice`'s own settlement allowance.
const EXTRA_GAS: u64 = 1_000_000;

/// Drop the consensus slot counters the snapshot carries, keeping `Aura::Authorities`.
///
/// The snapshot's `Aura::CurrentSlot` is real wall-clock derived (a slot in the
/// hundreds of millions), while the test harness synthesises a relay slot counted
/// from the block number. `AuraExt`'s consensus hook compares the two and panics
/// with "Parachain slot is too far in the future" the moment a test advances a
/// block. `go_to_block` already clears `AuraExt::RelaySlotInfo` for the same
/// reason; a live snapshot needs `Aura::CurrentSlot` cleared too.
///
/// `Aura::Authorities` must survive: `pallet_aura::find_author` takes
/// `slot % authorities_len()`, so an empty authority set divides by zero on every
/// EVM call.
pub(crate) fn reset_consensus_slots() {
	use frame_support::storage::{storage_prefix, unhashed};

	unhashed::kill(&storage_prefix(b"Aura", b"CurrentSlot"));
	unhashed::kill(&storage_prefix(b"AuraExt", b"RelaySlotInfo"));
}

/// Load the snapshot and make sure the runtime knows where Uniswap v3 lives.
///
/// Prefers whatever the snapshot already has: overwriting it with the fallback
/// constants would point the runtime at addresses that hold no code in this
/// snapshot's EVM state, and every test would then fail with "pool not found"
/// rather than saying what was actually missing.
fn with_uniswap_v3(execution: impl FnOnce()) {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_consensus_slots();

		match Parameters::uniswap_v3_factory() {
			Some(factory) => {
				println!("uniswap v3 addresses came from the snapshot (factory {factory:?})");
			}
			None => {
				assert!(
					UNISWAP_V3_FACTORY != EvmAddress::zero(),
					"snapshot has no Parameters::UniswapV3Factory and the fallback constants are \
					 still zero. Either scrape the Parameters pallet AFTER calling \
					 parameters.setUniswapV3Addresses on the source chain (preferred), or fill in \
					 UNISWAP_V3_FACTORY / UNISWAP_V3_SWAP_ROUTER / UNISWAP_V3_QUOTER from the \
					 deployment. See uniswap-snapshot/README.md."
				);
				assert_ok!(Parameters::set_uniswap_v3_addresses(
					RuntimeOrigin::root(),
					UNISWAP_V3_FACTORY,
					UNISWAP_V3_SWAP_ROUTER,
					UNISWAP_V3_QUOTER,
				));
			}
		}

		assert!(
			UniswapV3::find_pool(ASSET_IN, ASSET_OUT, FEE_TIER)
				.expect("factory should be readable")
				.is_some(),
			"no {ASSET_IN}/{ASSET_OUT} pool at fee {FEE_TIER} in this snapshot — check ASSET_IN, \
			 ASSET_OUT and FEE_TIER against the pool that was actually created before scraping"
		);

		fund_alice();

		execution();
	});
}

/// The snapshot's balances belong to the accounts that existed on mainnet; the
/// harness's ALICE ([4u8; 32]) is not one of them.
///
/// `Token`-kind assets are minted under Root, which is free inside test
/// externalities. The pool's two assets are `Erc20`-kind — their ledgers live in
/// contract storage and there is no mint the runtime can perform — so they are
/// transferred out of the treasury, which holds both in this snapshot.
///
/// The transfers go through `dispatch_with_extra_gas` because aDOT is an Aave
/// aToken: its `transfer` calls back into the pool (`finalizeTransfer`), which
/// does not fit `erc20_currency`'s 400k ceiling and reverts as `EvmOutOfGas`.
/// HOLLAR is a plain token and would transfer without it.
fn fund_alice() {
	assert_ok!(Currencies::update_balance(
		RuntimeOrigin::root(),
		AccountId::from(ALICE),
		GAS_ASSET,
		FUND_GAS as i128,
	));

	let treasury = Treasury::account_id();
	for (asset, amount) in [(ASSET_IN, FUND_ASSET_IN), (ASSET_OUT, FUND_ASSET_OUT)] {
		let held = Currencies::free_balance(asset, &treasury);
		assert!(
			held >= amount,
			"treasury holds {held} of asset {asset}, need {amount} — lower FUND_ASSET_* or pick \
			 another source"
		);
		assert_ok!(Dispatcher::dispatch_with_extra_gas(
			RuntimeOrigin::signed(treasury.clone()),
			Box::new(RuntimeCall::Currencies(pallet_currencies::Call::transfer {
				dest: AccountId::from(ALICE).into(),
				currency_id: asset,
				amount,
			})),
			EXTRA_GAS,
		));
	}
}

fn uniswap_route() -> Vec<Trade<AssetId>> {
	vec![Trade {
		pool: PoolType::UniswapV3(FEE_TIER),
		asset_in: ASSET_IN,
		asset_out: ASSET_OUT,
	}]
}

#[test]
fn calculate_out_given_in_should_return_positive_quote_when_pool_has_liquidity() {
	with_uniswap_v3(|| {
		let amount_out =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, SELL_AMOUNT)
				.expect("quote should succeed");
		assert!(amount_out > 0);
	});
}

#[test]
fn round_trip_quote_should_cost_more_than_the_original_input_when_pool_charges_fee() {
	with_uniswap_v3(|| {
		// Comparing amount_in against amount_out directly is meaningless when the
		// pair's decimals differ (aDOT has 10, HOLLAR 18). Round-tripping is not:
		// buying back exactly what a sell produced must cost more than was sold,
		// because the fee is charged on both legs.
		let amount_out =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, SELL_AMOUNT)
				.expect("sell quote should succeed");
		let amount_in =
			UniswapV3::calculate_in_given_out(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, amount_out)
				.expect("buy quote should succeed");

		assert!(
			amount_in > SELL_AMOUNT,
			"round trip should cost more than it returned: {amount_in} <= {SELL_AMOUNT}"
		);
	});
}

/// Within the pool's capacity the whole input reaches the pool, so the router
/// account is left as it was found.
#[test]
fn router_sell_should_increase_output_balance_when_routed_through_uniswap_v3() {
	with_uniswap_v3(|| {
		let router_before = router_balances();
		let before = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		let after = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		assert!(after > before);
		assert_eq!(
			router_balances(),
			router_before,
			"the router holds nothing after the trade"
		);
	});
}

#[test]
fn router_buy_should_deliver_exact_output_when_routed_through_uniswap_v3() {
	with_uniswap_v3(|| {
		let buy_amount = SELL_AMOUNT / 2;
		let before = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		assert_ok!(Router::buy(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			buy_amount,
			u128::MAX,
			uniswap_route().try_into().unwrap(),
		));
		let after = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		assert_eq!(after - before, buy_amount);
	});
}

#[test]
fn router_sell_should_emit_uniswap_v3_filler_event() {
	with_uniswap_v3(|| {
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		let emitted = frame_system::Pallet::<Runtime>::events().into_iter().any(|record| {
			matches!(
				record.event,
				RuntimeEvent::Broadcast(pallet_broadcast::Event::Swapped3 {
					filler_type: Filler::UniswapV3,
					..
				})
			)
		});
		assert!(emitted);
	});
}

/// A swap through a v3 pool must leave an EMA-oracle entry behind it.
///
/// Without one, `OraclePriceProvider` has nothing to return for a `UniswapV3` leg and
/// every consumer reads that as failure: `pallet-dca` treats it as "price unstable"
/// and terminates the schedule, and `route-executor::set_route` rejects the route with
/// `RouteHasNoOracle`. Reading a live `slot0` price instead would satisfy both callers
/// while quietly removing the manipulation resistance they exist to provide, so the
/// executor reports to the oracle and this is the test that it does.
#[test]
fn swap_should_feed_the_ema_oracle_under_the_uniswap_v3_source() {
	with_uniswap_v3(|| {
		let pair = if ASSET_IN < ASSET_OUT {
			(ASSET_IN, ASSET_OUT)
		} else {
			(ASSET_OUT, ASSET_IN)
		};

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		let swapped_at = System::block_number();
		hydradx_run_to_next_block();

		// A mainnet snapshot's pool has already traded, so "an entry exists" proves
		// nothing. What the swap must do is refresh it in the block it ran in.
		// The tuple's second element is the block the oracle was FIRST initialized,
		// so freshness has to come off the entry itself.
		let (entry, _initialized_at) =
			pallet_ema_oracle::Oracles::<Runtime>::get((UNISWAPV3_SOURCE, (pair.0, pair.1), OraclePeriod::LastBlock))
				.expect("swap should have written a uniswap v3 oracle entry");

		assert_eq!(entry.updated_at, swapped_at);
		assert!(
			entry.price.n != 0 && entry.price.d != 0,
			"oracle price should be non-zero"
		);
	});
}

/// The whole point of the oracle wiring: a DCA schedule routed through a v3 pool has to
/// survive execution. Before the `UniswapV3` arm existed it was terminated after
/// `MaxNumberOfRetriesOnError` retries, reporting `PriceUnstable` — which was never true,
/// the price simply could not be looked up.
#[test]
fn dca_should_execute_through_a_uniswap_v3_leg_once_the_pool_has_oracle_history() {
	with_uniswap_v3(|| {
		// Prime the oracle: a DCA leg cannot be priced until the pair has traded.
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		hydradx_run_to_next_block();

		let budget = DCA_SELL_AMOUNT * 10;
		let schedule = schedule_fake_with_sell_order(
			ALICE,
			PoolType::UniswapV3(FEE_TIER),
			budget,
			ASSET_IN,
			ASSET_OUT,
			DCA_SELL_AMOUNT,
		);
		// A mainnet snapshot already carries schedules, so id 0 is not ours.
		let schedule_id = DCA::next_schedule_id();
		assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE.into()), schedule, None));

		// A mainnet snapshot's near-term blocks are already full of real schedules,
		// so the first execution can be pushed well past the next block. Take the
		// planned block from the event rather than guessing how long to run.
		let planned_at = System::events()
			.into_iter()
			.find_map(|record| match record.event {
				RuntimeEvent::DCA(pallet_dca::Event::ExecutionPlanned { id, block, .. }) if id == schedule_id => {
					Some(block)
				}
				_ => None,
			})
			.expect("schedule should have been planned");

		let before = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		// Step block by block: `hydradx_run_to_block` jumps straight to the target,
		// which skips `on_initialize` for every block in between — including the one
		// the execution was planned for.
		while System::block_number() <= planned_at {
			hydradx_run_to_next_block();
		}
		let after = Currencies::free_balance(ASSET_OUT, &ALICE.into());

		// Executed at least once, and the schedule was not terminated.
		assert!(after > before, "DCA should have executed through the v3 leg");
		assert!(
			DCA::schedules(schedule_id).is_some(),
			"schedule should still exist — it used to be terminated with PriceUnstable"
		);
	});
}

/// `get_liquidity_depth` must report the IN-RANGE liquidity, not the pool's balance.
///
/// `route-executor::set_route` sizes its reference trade at 1% of this figure, so a
/// concentrated pool that returns its whole `balanceOf` — including bands the price
/// has left and uncollected fees — makes every route comparison use a trade the pool
/// cannot actually serve.
#[test]
fn liquidity_depth_should_report_in_range_liquidity_not_the_pool_balance() {
	with_uniswap_v3(|| {
		let depth = UniswapV3::get_liquidity_depth(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT)
			.expect("depth should be readable");
		assert!(depth > 0, "a pool with liquidity should report non-zero depth");

		// A trade of 1% of the reported depth — what set_route uses as its reference —
		// must be quotable against the real pool.
		let reference = depth / 100;
		assert!(reference > 0, "reference amount should be non-zero");
		let out = UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, reference)
			.expect("the reference trade must be quotable");
		assert!(out > 0, "reference trade should quote a positive output");
	});
}

/// The declared weight must cover the gas the buy path can actually reserve.
#[test]
fn trade_weight_should_cover_the_whole_buy_path() {
	use hydradx_runtime::Runtime as R;
	use pallet_evm::GasWeightMapping;

	// getPool + quote + approve + swap + approve-reset + 2x balanceOf + slot0.
	let path_gas = 250_000 + 1_000_000 + 100_000 + 1_000_000 + 100_000 + 2 * 100_000 + 250_000;
	let declared = UniswapV3::trade_weight();
	let needed = <R as pallet_evm::Config>::GasWeightMapping::gas_to_weight(path_gas, true);

	assert!(
		declared.ref_time() >= needed.ref_time(),
		"trade_weight() declares {} but the buy path can reserve {} ({} gas)",
		declared.ref_time(),
		needed.ref_time(),
		path_gas
	);
}

// ---------------------------------------------------------------------------
// ICE simulator
// ---------------------------------------------------------------------------

use hydradx_runtime::evm::uniswap_v3_trade_executor::evm_token_address;
use hydradx_runtime::HydrationSimulators;
use hydradx_traits::amm::SimulatorSet;
use ice_support::RoutingState;
use ice_support::RoutingTarget;

/// The pool the router already resolved, registered with the solver.
///
/// `SolverRouting` is the only way the Uniswap simulator learns about a pool —
/// it has no on-chain registry to enumerate — so a snapshot test has to write
/// the entry before taking the simulator snapshot.
fn register_pool() -> EvmAddress {
	let pool = UniswapV3::find_pool(ASSET_IN, ASSET_OUT, FEE_TIER)
		.expect("factory should be readable")
		.expect("pool should exist in this snapshot");

	assert_ok!(hydradx_runtime::ICE::update_routing(
		RuntimeOrigin::root(),
		RoutingTarget::UniswapV3Pool(pool),
		Some(RoutingState::Included),
	));

	pool
}

fn uniswap_snapshot() -> amm_simulator::uniswap_v3::Snapshot {
	<HydrationSimulators as SimulatorSet>::initial_state().3
}

#[test]
fn solver_snapshot_should_sample_the_pool_when_it_is_registered() {
	with_uniswap_v3(|| {
		let pool = register_pool();

		let snapshot = uniswap_snapshot();
		let curve = snapshot.pools.get(&pool).expect("registered pool to be sampled");

		assert_eq!(curve.fee, FEE_TIER);
		// asset_a is token0, which sorts first by EVM address.
		let (token0, token1) = if evm_token_address(ASSET_IN) < evm_token_address(ASSET_OUT) {
			(ASSET_IN, ASSET_OUT)
		} else {
			(ASSET_OUT, ASSET_IN)
		};
		assert_eq!((curve.asset_a, curve.asset_b), (token0, token1));
		assert!(!curve.traded);
		assert_ne!(curve.sqrt_price_x96, sp_core::U256::zero());

		// Both curves must be strictly increasing, or interpolation cannot bracket.
		for samples in [&curve.a_to_b, &curve.b_to_a] {
			assert!(!samples.is_empty(), "no samples taken");
			for window in samples.windows(2) {
				assert!(
					window[1].0 > window[0].0 && window[1].1 > window[0].1,
					"samples not strictly increasing: {:?} then {:?}",
					window[0],
					window[1]
				);
			}
		}
	});
}

#[test]
fn solver_snapshot_should_be_empty_when_no_pool_is_registered() {
	with_uniswap_v3(|| {
		assert!(uniswap_snapshot().pools.is_empty());
	});
}

#[test]
fn pool_edges_should_expose_the_registered_pair_to_route_discovery() {
	with_uniswap_v3(|| {
		register_pool();

		let edges =
			<HydrationSimulators as SimulatorSet>::pool_edges(&<HydrationSimulators as SimulatorSet>::initial_state());

		assert!(
			edges.iter().any(|edge| edge.pool_type == PoolType::UniswapV3(FEE_TIER)
				&& edge.assets.contains(&ASSET_IN)
				&& edge.assets.contains(&ASSET_OUT)),
			"uniswap pair missing from pool edges: {edges:?}"
		);
	});
}

/// The assertion the whole sampled-curve design rests on: the solver must never
/// claim more output than the pool actually delivers, or the batch fails
/// conservation and every intent in it dies.
#[test]
fn simulated_sell_should_not_exceed_executed_output_when_routed_through_uniswap_v3() {
	with_uniswap_v3(|| {
		register_pool();

		let (_, simulated) = <HydrationSimulators as SimulatorSet>::simulate_sell(
			PoolType::UniswapV3(FEE_TIER),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("simulation to succeed");

		let before = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE));
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		let executed = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE)) - before;

		assert!(
			simulated.amount_out <= executed,
			"simulation over-quoted: simulated {} > executed {}",
			simulated.amount_out,
			executed
		);
	});
}

/// Buys invert the same curve, so the input must be over-, never under-quoted.
#[test]
fn simulated_buy_should_not_understate_input_when_routed_through_uniswap_v3() {
	with_uniswap_v3(|| {
		register_pool();

		let target_out =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, SELL_AMOUNT)
				.expect("quote to succeed");

		let (_, simulated) = <HydrationSimulators as SimulatorSet>::simulate_buy(
			PoolType::UniswapV3(FEE_TIER),
			ASSET_IN,
			ASSET_OUT,
			target_out,
			Balance::MAX,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("simulation to succeed");

		let before = Currencies::free_balance(ASSET_IN, &AccountId::from(ALICE));
		assert_ok!(Router::buy(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			target_out,
			Balance::MAX,
			uniswap_route().try_into().unwrap(),
		));
		let executed = before - Currencies::free_balance(ASSET_IN, &AccountId::from(ALICE));

		assert!(
			simulated.amount_in >= executed,
			"simulation under-quoted the input: simulated {} < executed {}",
			simulated.amount_in,
			executed
		);
	});
}

/// A pool prices one trade per solution. A second leg would have to be read from a
/// mid-curve offset, where differencing two interpolated points over-quotes.
#[test]
fn simulated_sell_should_fail_when_the_pool_was_already_traded_the_same_way() {
	with_uniswap_v3(|| {
		register_pool();

		let half = SELL_AMOUNT / 2;
		let (state, _) = <HydrationSimulators as SimulatorSet>::simulate_sell(
			PoolType::UniswapV3(FEE_TIER),
			ASSET_IN,
			ASSET_OUT,
			half,
			0,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("first leg to succeed");

		assert_eq!(
			<HydrationSimulators as SimulatorSet>::simulate_sell(
				PoolType::UniswapV3(FEE_TIER),
				ASSET_IN,
				ASSET_OUT,
				half,
				0,
				&state,
			)
			.map(|(_, result)| result),
			Err(hydradx_traits::amm::SimulatorError::NotSupported)
		);
	});
}

/// The same refusal applies across directions: the curve is sampled one way and
/// knows nothing about a pool the batch has already pushed back.
#[test]
fn simulated_sell_should_fail_when_the_pool_was_already_traded_the_other_way() {
	with_uniswap_v3(|| {
		register_pool();

		let (state, _) = <HydrationSimulators as SimulatorSet>::simulate_sell(
			PoolType::UniswapV3(FEE_TIER),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			&<HydrationSimulators as SimulatorSet>::initial_state(),
		)
		.expect("first leg to succeed");

		assert_eq!(
			<HydrationSimulators as SimulatorSet>::simulate_sell(
				PoolType::UniswapV3(FEE_TIER),
				ASSET_OUT,
				ASSET_IN,
				SELL_AMOUNT,
				0,
				&state,
			)
			.map(|(_, result)| result),
			Err(hydradx_traits::amm::SimulatorError::NotSupported)
		);
	});
}

#[test]
#[ignore]
fn probe_uniswap_snapshot() {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		reset_consensus_slots();

		println!("factory      {:?}", Parameters::uniswap_v3_factory());
		println!("swap router  {:?}", Parameters::uniswap_v3_swap_router());
		println!("quoter       {:?}", Parameters::uniswap_v3_quoter());
		println!("pool(1001,222,3000) {:?}", UniswapV3::find_pool(1001, 222, 3000));

		let omnipool = hydradx_runtime::Omnipool::protocol_account();
		println!("omnipool acct aDOT {}", Currencies::free_balance(1001, &omnipool));
		println!("omnipool acct HOLLAR {}", Currencies::free_balance(222, &omnipool));

		// What the sampled curve can actually price.
		assert_ok!(hydradx_runtime::ICE::update_routing(
			RuntimeOrigin::root(),
			RoutingTarget::UniswapV3Pool(UniswapV3::find_pool(1001, 222, 3000).unwrap().unwrap()),
			Some(RoutingState::Included),
		));
		let snap = <HydrationSimulators as SimulatorSet>::initial_state().3;
		for (addr, curve) in snap.pools.iter() {
			println!(
				"curve {addr:?}: a_to_b {} samples, max_in {:?}; b_to_a {} samples, max_in {:?}",
				curve.a_to_b.len(),
				curve.a_to_b.last(),
				curve.b_to_a.len(),
				curve.b_to_a.last(),
			);
		}

		let treasury = Treasury::account_id();
		for asset in [0u32, 20, 222, 1001, 5, 10] {
			let meta = AssetRegistry::assets(asset);
			println!(
				"asset {asset:>5}: treasury={:<30} alice={:<20} meta={:?}",
				Currencies::free_balance(asset, &treasury),
				Currencies::free_balance(asset, &AccountId::from(ALICE)),
				meta.map(|m| (m.decimals, m.asset_type)),
			);
		}
	});
}

/// End to end: an intent on the pool's pair is solved and settled on chain.
///
/// This is the only test that exercises the whole path — registry, simulator,
/// route discovery, solver, and `submit_solution` settling a real EVM swap —
/// rather than the simulator in isolation.
///
/// Both assets are also in the Omnipool, and the solver picks that route when it
/// is available, so aDOT is excluded from the solver's Omnipool routing first.
/// Without that the solution settles fine but never touches Uniswap, which is
/// what this test is for.
#[test]
fn intent_should_resolve_and_settle_when_solution_routes_through_uniswap_v3() {
	with_uniswap_v3(|| {
		register_pool();

		exclude_from_omnipool(ASSET_IN);

		// The minimum output has to clear the asset's existential deposit, so take
		// it from the live quote rather than a token value.
		let quoted = UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, SELL_AMOUNT)
			.expect("quote should succeed");
		let min_out = quoted * 9 / 10;

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_IN,
					asset_out: ASSET_OUT,
					amount_in: SELL_AMOUNT,
					amount_out: min_out,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let before = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE));
		let router_before = router_balances();
		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"uniswap_v3_intent",
		);
		let after = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE));

		assert_eq!(
			router_balances(),
			router_before,
			"the router holds nothing after settlement"
		);

		assert_eq!(solution.resolved_intents.len(), 1);
		assert!(
			solution
				.trades
				.iter()
				.any(|trade| trade.route.iter().any(|hop| matches!(hop.pool, PoolType::UniswapV3(_)))),
			"solution should route through the registered v3 pool"
		);
		assert!(
			after - before >= min_out,
			"intent owner should have been paid at least the limit: {} < {min_out}",
			after - before
		);
	});
}

/// Extra aDOT for tests that need more than the treasury holds, taken from the
/// Omnipool's own reserve. Sound only because its callers either freeze aDOT in
/// the Omnipool or never trade the Omnipool at all, so the reserve they borrow
/// from is never traded against while short.
fn fund_alice_from_omnipool(amount: Balance) {
	let omnipool = hydradx_runtime::Omnipool::protocol_account();
	assert!(Currencies::free_balance(ASSET_IN, &omnipool) >= amount);

	assert_ok!(Dispatcher::dispatch_with_extra_gas(
		RuntimeOrigin::signed(omnipool),
		Box::new(RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: AccountId::from(ALICE).into(),
			currency_id: ASSET_IN,
			amount,
		})),
		EXTRA_GAS,
	));
}

/// Largest `ASSET_IN` the sampled curve can price, i.e. the last ladder step the
/// pool absorbed in full.
fn curve_limit() -> Balance {
	let snapshot = uniswap_snapshot();
	let curve = snapshot.pools.values().next().expect("pool to be sampled");
	let (max_in, _) = *curve.a_to_b.last().expect("curve to have samples");
	max_in
}

/// Set the pool up as the only route and hand ALICE more than the curve can price.
fn only_uniswap_with_oversized_balance() -> Balance {
	register_pool();
	exclude_from_omnipool(ASSET_IN);

	let limit = curve_limit();
	let oversized = limit * 2;
	fund_alice_from_omnipool(oversized * 2);
	oversized
}

/// A rate the pool can beat comfortably at any fill inside the curve.
fn slack_min_out(amount_in: Balance) -> Balance {
	let probe = SELL_AMOUNT;
	let out = UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, probe)
		.expect("quote should succeed");
	// 70% of the small-trade rate, so price impact across a large fill cannot
	// make the limit itself the reason a fill is refused.
	(out / 10 * 7) * (amount_in / probe)
}

/// The curve ends where the pool stops absorbing input in full, so an
/// all-or-nothing intent above that size has no v3 fill and stays unresolved.
#[test]
fn oversized_non_partial_intent_should_not_resolve_through_uniswap_v3() {
	with_uniswap_v3(|| {
		let oversized = only_uniswap_with_oversized_balance();

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_IN,
					asset_out: ASSET_OUT,
					amount_in: oversized,
					amount_out: slack_min_out(oversized),
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		crate::ice::harness::set_solver_mode(ice_support::SolverMode::V4);
		assert_eq!(
			crate::ice::harness::solve_as::<crate::ice::harness::V4Solver>(),
			None,
			"an all-or-nothing intent above the curve must not be solvable"
		);
	});
}

/// A partial intent above the curve is trimmed: the solver bisects down until the
/// fill is one the simulator will price, rather than dropping the intent.
#[test]
fn oversized_partial_intent_should_be_trimmed_to_what_uniswap_v3_can_price() {
	with_uniswap_v3(|| {
		let oversized = only_uniswap_with_oversized_balance();
		let limit = curve_limit();

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_IN,
					asset_out: ASSET_OUT,
					amount_in: oversized,
					amount_out: slack_min_out(oversized),
					partial: true,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let received_before = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE));
		let router_before = router_balances();
		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"uniswap_v3_partial",
		);
		assert_eq!(
			router_balances(),
			router_before,
			"the router holds nothing after settlement"
		);
		let received = Currencies::free_balance(ASSET_OUT, &AccountId::from(ALICE)) - received_before;

		assert_eq!(solution.resolved_intents.len(), 1);

		// The input is reserved at submission, so the fill has to come off the
		// resolved intent rather than a free-balance delta.
		let ice_support::IntentData::Swap(filled) = &solution.resolved_intents[0].data else {
			panic!("expected a swap intent");
		};
		assert!(
			filled.amount_in > 0 && filled.amount_in <= limit,
			"fill {} should be trimmed into the curve's range (0, {limit}]",
			filled.amount_in
		);
		assert!(
			filled.amount_in < oversized,
			"an oversized intent should not fill in full"
		);
		assert!(received > 0, "the trimmed fill should still pay out");
		assert!(
			solution
				.trades
				.iter()
				.any(|trade| trade.route.iter().any(|hop| matches!(hop.pool, PoolType::UniswapV3(_)))),
			"the trimmed fill should still route through the v3 pool"
		);
	});
}

/// Smallest amount at which this pool stops absorbing the whole input, found by
/// walking the quoter until the output stops growing.
///
/// Deliberately uses only the executor, so the number is independent of the ICE
/// simulator's own ladder.
fn oversized_for_pool(asset_in: AssetId, asset_out: AssetId, start: Balance) -> Balance {
	let mut amount = start;
	let mut previous = 0;
	for _ in 0..24 {
		let out =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), asset_in, asset_out, amount).unwrap_or(0);
		if out <= previous {
			return amount;
		}
		previous = out;
		amount = amount.saturating_mul(2);
	}
	panic!("pool absorbed every probe — it is deeper than this walk expects");
}

/// Move `amount` of `asset` from the treasury to ALICE, through the extra-gas
/// path so aToken transfers fit.
fn fund_alice_from_treasury(asset: AssetId, amount: Balance) {
	let treasury = Treasury::account_id();
	assert!(Currencies::free_balance(asset, &treasury) >= amount);
	assert_ok!(Dispatcher::dispatch_with_extra_gas(
		RuntimeOrigin::signed(treasury),
		Box::new(RuntimeCall::Currencies(pallet_currencies::Call::transfer {
			dest: AccountId::from(ALICE).into(),
			currency_id: asset,
			amount,
		})),
		EXTRA_GAS,
	));
}

/// Take an asset out of the solver's Omnipool routing.
///
/// Unlike freezing its tradability this touches nothing outside ICE: the asset
/// stays tradable for everyone else, it simply stops appearing in the solver's
/// Omnipool snapshot, so route discovery has to find another venue for it.
fn exclude_from_omnipool(asset: AssetId) {
	assert_ok!(hydradx_runtime::ICE::update_routing(
		RuntimeOrigin::root(),
		RoutingTarget::OmnipoolAsset(asset),
		Some(RoutingState::Excluded),
	));
}

/// Router account holdings of both pool assets.
///
/// Settling an intent must leave these untouched — the router is a pass-through
/// for the trade, never a resting place for any part of it.
fn router_balances() -> (Balance, Balance) {
	let router = Router::router_account();
	(
		Currencies::free_balance(ASSET_IN, &router),
		Currencies::free_balance(ASSET_OUT, &router),
	)
}

/// A sell past the pool's capacity is refused outright.
///
/// Selling `ASSET_IN` that far drives the pool to `MIN_SQRT_RATIO`, where the
/// executor's post-trade oracle read (`spot_price_raw`) has no representable
/// price, so `execute_sell` fails and the router call rolls back. Worth pinning:
/// an ICE solution carrying such a leg would abort the whole batch at settlement,
/// which is why the simulator must never size one.
#[test]
fn oversized_router_sell_should_revert_when_the_post_trade_price_underflows() {
	with_uniswap_v3(|| {
		let oversized = oversized_for_pool(ASSET_IN, ASSET_OUT, SELL_AMOUNT);
		fund_alice_from_omnipool(oversized * 2);

		assert_noop!(
			Router::sell(
				RuntimeOrigin::signed(ALICE.into()),
				ASSET_IN,
				ASSET_OUT,
				oversized,
				0,
				uniswap_route().try_into().unwrap(),
			),
			sp_runtime::DispatchError::Other("uniswapv3: zero price")
		);
	});
}

/// A sell larger than the pool can absorb must never reach a solution.
///
/// Sized by walking the quoter until output stops growing, so the bound tracks
/// the pool's real depth rather than a pinned constant.
#[test]
fn intent_should_be_excluded_when_sell_exceeds_pool_capacity() {
	with_uniswap_v3(|| {
		register_pool();
		exclude_from_omnipool(ASSET_IN);

		// Past what the pool can absorb in one swap.
		let beyond_capacity = oversized_for_pool(ASSET_OUT, ASSET_IN, 1_000_000_000_000_000_000);
		fund_alice_from_treasury(ASSET_OUT, beyond_capacity * 2);

		// The simulator must already refuse to price it.
		assert_eq!(
			<HydrationSimulators as SimulatorSet>::simulate_sell(
				PoolType::UniswapV3(FEE_TIER),
				ASSET_OUT,
				ASSET_IN,
				beyond_capacity,
				0,
				&<HydrationSimulators as SimulatorSet>::initial_state(),
			)
			.map(|(_, result)| result),
			Err(hydradx_traits::amm::SimulatorError::TradeTooLarge)
		);

		let quoted =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_OUT, ASSET_IN, beyond_capacity)
				.expect("pre-trade quote");

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_OUT,
					asset_out: ASSET_IN,
					amount_in: beyond_capacity,
					// Even at a limit the truncated trade would satisfy, it must not run.
					amount_out: quoted / 2,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		crate::ice::harness::set_solver_mode(ice_support::SolverMode::V4);
		assert_eq!(
			crate::ice::harness::solve_as::<crate::ice::harness::V4Solver>(),
			None,
			"a sell beyond pool capacity must not reach a solution"
		);
	});
}

/// The same sell as a partial intent: it resolves at a trimmed size, and because
/// the trim lands inside the curve the pool consumes the fill in full — so the
/// router account is left exactly as it was found.
///
/// That last assertion is the invariant every intent test shares.
#[test]
fn partial_intent_should_be_trimmed_and_fully_consumed_when_sell_exceeds_capacity() {
	with_uniswap_v3(|| {
		register_pool();
		exclude_from_omnipool(ASSET_IN);

		let beyond_capacity = oversized_for_pool(ASSET_OUT, ASSET_IN, 1_000_000_000_000_000_000);
		fund_alice_from_treasury(ASSET_OUT, beyond_capacity * 2);

		let quoted =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_OUT, ASSET_IN, beyond_capacity)
				.expect("pre-trade quote");

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_OUT,
					asset_out: ASSET_IN,
					amount_in: beyond_capacity,
					amount_out: quoted / 2,
					partial: true,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let router_before = router_balances();

		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"uniswap_v3_partial_within_capacity",
		);

		assert_eq!(solution.resolved_intents.len(), 1);
		let ice_support::IntentData::Swap(filled) = &solution.resolved_intents[0].data else {
			panic!("expected a swap intent");
		};
		assert!(
			filled.amount_in > 0 && filled.amount_in < beyond_capacity,
			"fill {} should be trimmed below the requested {beyond_capacity}",
			filled.amount_in
		);

		// The whole point: the trimmed fill is fully consumed by the pool.
		assert_eq!(
			router_balances(),
			router_before,
			"a trimmed fill must leave nothing behind in the router account"
		);
	});
}

/// The ordinary case in the reverse direction: a within-capacity, all-or-nothing
/// intent resolves in full through the v3 leg and leaves the router account
/// untouched.
#[test]
fn intent_should_resolve_in_full_when_sell_is_within_capacity_in_the_reverse_direction() {
	with_uniswap_v3(|| {
		register_pool();
		exclude_from_omnipool(ASSET_IN);

		// A tenth of the largest fill the curve prices — comfortably inside capacity.
		let snapshot = uniswap_snapshot();
		let curve = snapshot.pools.values().next().expect("pool to be sampled");
		let amount_in = curve.b_to_a.last().expect("curve samples").0 / 10;
		fund_alice_from_treasury(ASSET_OUT, amount_in * 2);

		let quoted = UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_OUT, ASSET_IN, amount_in)
			.expect("pre-trade quote");

		assert_ok!(hydradx_runtime::Intent::submit_intent(
			RuntimeOrigin::signed(AccountId::from(ALICE)),
			pallet_intent::types::IntentInput {
				data: ice_support::IntentDataInput::Swap(ice_support::SwapParams {
					asset_in: ASSET_OUT,
					asset_out: ASSET_IN,
					amount_in,
					amount_out: quoted / 2,
					partial: false,
				}),
				deadline: Some(<hydradx_runtime::Timestamp as frame_support::traits::Time>::now() + 600_000),
				on_resolved: None,
			}
		));

		let router_before = router_balances();
		let received_before = Currencies::free_balance(ASSET_IN, &AccountId::from(ALICE));

		let solution = crate::ice::harness::run_and_submit_as::<crate::ice::harness::V4Solver>(
			ice_support::SolverMode::V4,
			"uniswap_v3_within_capacity",
		);

		assert_eq!(solution.resolved_intents.len(), 1);
		let ice_support::IntentData::Swap(filled) = &solution.resolved_intents[0].data else {
			panic!("expected a swap intent");
		};
		assert_eq!(filled.amount_in, amount_in, "a within-capacity intent fills in full");
		assert!(Currencies::free_balance(ASSET_IN, &AccountId::from(ALICE)) > received_before);
		assert_eq!(
			router_balances(),
			router_before,
			"the router holds nothing after settlement"
		);
	});
}

// ---------------------------------------------------------------------------
// Routing exclusion, per venue
// ---------------------------------------------------------------------------

fn pool_edges() -> Vec<hydradx_traits::router::PoolEdge<AssetId>> {
	<HydrationSimulators as SimulatorSet>::pool_edges(&<HydrationSimulators as SimulatorSet>::initial_state())
}

fn exclude(target: RoutingTarget) {
	assert_ok!(hydradx_runtime::ICE::update_routing(
		RuntimeOrigin::root(),
		target,
		Some(RoutingState::Excluded),
	));
}

/// Excluding an Omnipool asset removes it from the Omnipool's tradeable set, so
/// route discovery cannot build a hop through it.
#[test]
fn excluding_an_omnipool_asset_should_remove_it_from_the_pool_edges() {
	with_uniswap_v3(|| {
		let before = pool_edges();
		let omnipool_assets = before
			.iter()
			.find(|edge| edge.pool_type == PoolType::Omnipool)
			.expect("an omnipool edge")
			.assets
			.clone();
		let victim = *omnipool_assets.first().expect("omnipool to hold assets");

		exclude(RoutingTarget::OmnipoolAsset(victim));

		let after = pool_edges();
		let remaining = after
			.iter()
			.find(|edge| edge.pool_type == PoolType::Omnipool)
			.expect("an omnipool edge")
			.assets
			.clone();

		assert!(!remaining.contains(&victim), "asset {victim} should be gone");
		assert_eq!(remaining.len(), omnipool_assets.len() - 1);
	});
}

/// Excluding a stableswap pool removes its edge entirely.
#[test]
fn excluding_a_stableswap_pool_should_remove_its_pool_edge() {
	with_uniswap_v3(|| {
		let before = pool_edges();
		let victim = before
			.iter()
			.find_map(|edge| match edge.pool_type {
				PoolType::Stableswap(pool_id) => Some(pool_id),
				_ => None,
			})
			.expect("a stableswap pool in the snapshot");

		exclude(RoutingTarget::StableswapPool(victim));

		assert!(
			!pool_edges()
				.iter()
				.any(|edge| edge.pool_type == PoolType::Stableswap(victim)),
			"stableswap pool {victim} should be gone"
		);
	});
}

/// Excluding an Aave wrap removes that reserve/aToken edge.
#[test]
fn excluding_an_aave_wrap_should_remove_its_pool_edge() {
	with_uniswap_v3(|| {
		let before = pool_edges();
		let victim = before
			.iter()
			.find(|edge| edge.pool_type == PoolType::Aave)
			.expect("an aave edge in the snapshot")
			.assets
			.clone();

		exclude(RoutingTarget::AaveWrap(victim[0], victim[1]));

		assert!(
			!pool_edges()
				.iter()
				.any(|edge| edge.pool_type == PoolType::Aave && edge.assets == victim),
			"aave wrap {victim:?} should be gone"
		);
	});
}

/// Excluding a registered Uniswap pool takes it back out of the snapshot.
#[test]
fn excluding_a_uniswap_pool_should_remove_it_from_the_snapshot() {
	with_uniswap_v3(|| {
		let pool = register_pool();
		assert!(uniswap_snapshot().pools.contains_key(&pool));

		exclude(RoutingTarget::UniswapV3Pool(pool));

		assert!(
			uniswap_snapshot().pools.is_empty(),
			"the pool should no longer be sampled"
		);
	});
}
