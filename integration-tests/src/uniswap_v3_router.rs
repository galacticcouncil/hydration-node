#![cfg(test)]

use crate::dca::schedule_fake_with_sell_order;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::uniswap_v3_trade_executor::UniswapV3;
use hydradx_runtime::{AssetId, Currencies, EmaOracle, Parameters, Router, Runtime, RuntimeEvent, RuntimeOrigin, DCA};
use hydradx_traits::router::{PoolType, Trade};
use hydradx_traits::{AggregatedPriceOracle, OraclePeriod};
use orml_traits::MultiCurrency;
use pallet_broadcast::types::Filler;
use pallet_route_executor::TradeExecution;
use primitives::constants::chain::UNISWAPV3_SOURCE;
use primitives::{AccountId, Balance, EvmAddress};
use sp_core::H160;

pub const PATH_TO_SNAPSHOT: &str = "uniswap-snapshot/SNAPSHOT";

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

/// KSM / KUSD, the pair the zombienet chainspec registers and `uniswap-v3-lark seed`
/// creates a full-range 0.3% pool for. Both are 12-decimal `Token`-kind assets.
const ASSET_IN: AssetId = 1;
const ASSET_OUT: AssetId = 2;
/// The gas token on Hydration's EVM — `eth_getBalance` is this asset's balance.
const GAS_ASSET: AssetId = 20;
const FEE_TIER: u32 = 3000;
/// 1 token against a pool holding 1e18 of each side: small enough that price
/// impact stays negligible and the exact-output test can round-trip.
const SELL_AMOUNT: Balance = 1_000_000_000_000;
const FUND_AMOUNT: Balance = 1_000_000_000_000_000;

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
fn reset_consensus_slots() {
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

		// The snapshot's balances belong to the accounts that existed on the source
		// chain; the harness's ALICE ([4u8; 32]) is not one of them. Fund her here —
		// Root is free inside test externalities — including the gas asset, since the
		// executor's approve/swap run as her bound EVM address.
		for (asset, amount) in [
			(ASSET_IN, FUND_AMOUNT),
			(ASSET_OUT, FUND_AMOUNT),
			(GAS_ASSET, FUND_AMOUNT),
		] {
			assert_ok!(Currencies::update_balance(
				RuntimeOrigin::root(),
				AccountId::from(ALICE),
				asset,
				amount as i128,
			));
		}

		execution();
	});
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
fn calculate_in_given_out_should_exceed_output_when_pool_charges_fee() {
	with_uniswap_v3(|| {
		let amount_out = SELL_AMOUNT / 2;
		let amount_in =
			UniswapV3::calculate_in_given_out(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, amount_out)
				.expect("quote should succeed");
		assert!(amount_in > amount_out);
	});
}

#[test]
fn router_sell_should_increase_output_balance_when_routed_through_uniswap_v3() {
	with_uniswap_v3(|| {
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

		// Cold pair: nothing recorded yet.
		assert!(
			EmaOracle::get_price(pair.0, pair.1, OraclePeriod::LastBlock, UNISWAPV3_SOURCE).is_err(),
			"pair should have no uniswap v3 oracle entry before the first swap"
		);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
			0,
			uniswap_route().try_into().unwrap(),
		));
		hydradx_run_to_next_block();

		let (price, _) = EmaOracle::get_price(pair.0, pair.1, OraclePeriod::LastBlock, UNISWAPV3_SOURCE)
			.expect("swap should have written a uniswap v3 oracle entry");
		assert!(price.n != 0 && price.d != 0, "oracle price should be non-zero");
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

		let budget = SELL_AMOUNT * 10;
		let schedule = schedule_fake_with_sell_order(
			ALICE,
			PoolType::UniswapV3(FEE_TIER),
			budget,
			ASSET_IN,
			ASSET_OUT,
			SELL_AMOUNT,
		);
		assert_ok!(DCA::schedule(RuntimeOrigin::signed(ALICE.into()), schedule, None));

		let before = Currencies::free_balance(ASSET_OUT, &ALICE.into());
		for _ in 0..10 {
			hydradx_run_to_next_block();
		}
		let after = Currencies::free_balance(ASSET_OUT, &ALICE.into());

		// Executed at least once, and the schedule was not terminated.
		assert!(after > before, "DCA should have executed through the v3 leg");
		assert!(
			DCA::schedules(0).is_some(),
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
