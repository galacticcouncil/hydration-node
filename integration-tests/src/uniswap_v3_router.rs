#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hex_literal::hex;
use hydradx_runtime::evm::uniswap_v3_trade_executor::UniswapV3;
use hydradx_runtime::{AssetId, Currencies, Parameters, Router, Runtime, RuntimeEvent, RuntimeOrigin};
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;
use pallet_broadcast::types::Filler;
use pallet_route_executor::TradeExecution;
use primitives::{Balance, EvmAddress};
use sp_core::H160;

pub const PATH_TO_SNAPSHOT: &str = "uniswap-snapshot/SNAPSHOT";

const UNISWAP_V3_FACTORY: EvmAddress = H160(hex!("0000000000000000000000000000000000000000"));
const UNISWAP_V3_SWAP_ROUTER: EvmAddress = H160(hex!("0000000000000000000000000000000000000000"));
const UNISWAP_V3_QUOTER: EvmAddress = H160(hex!("0000000000000000000000000000000000000000"));

const ASSET_IN: AssetId = 0;
const ASSET_OUT: AssetId = 20;
const FEE_TIER: u32 = 3000;
const SELL_AMOUNT: Balance = 1_000_000_000_000;

fn with_uniswap_v3(execution: impl FnOnce()) {
	TestNet::reset();
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		assert_ok!(Parameters::set_uniswap_v3_addresses(
			RuntimeOrigin::root(),
			UNISWAP_V3_FACTORY,
			UNISWAP_V3_SWAP_ROUTER,
			UNISWAP_V3_QUOTER,
		));
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
#[ignore = "requires uniswap-snapshot/SNAPSHOT and deployment constants; see uniswap-snapshot/README.md"]
fn calculate_out_given_in_should_return_positive_quote_when_pool_has_liquidity() {
	with_uniswap_v3(|| {
		let amount_out =
			UniswapV3::calculate_out_given_in(PoolType::UniswapV3(FEE_TIER), ASSET_IN, ASSET_OUT, SELL_AMOUNT)
				.expect("quote should succeed");
		assert!(amount_out > 0);
	});
}

#[test]
#[ignore = "requires uniswap-snapshot/SNAPSHOT and deployment constants; see uniswap-snapshot/README.md"]
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
#[ignore = "requires uniswap-snapshot/SNAPSHOT and deployment constants; see uniswap-snapshot/README.md"]
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
#[ignore = "requires uniswap-snapshot/SNAPSHOT and deployment constants; see uniswap-snapshot/README.md"]
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
#[ignore = "requires uniswap-snapshot/SNAPSHOT and deployment constants; see uniswap-snapshot/README.md"]
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
