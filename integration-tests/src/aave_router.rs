#![cfg(test)]
use crate::dca::create_schedule;
use crate::dca::init_omnipool_with_oracle_for_block_10;
use crate::dca::run_to_block;
use crate::dca::schedule_fake_with_sell_order;
use crate::liquidation::supply;
use crate::liquidation::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::traits::OnInitialize;
use hex_literal::hex;
use hydradx_runtime::evm::aave_trade_executor::AaveTradeExecutor;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::Omnipool;
use hydradx_runtime::{AssetId, Currencies, EVMAccounts, Liquidation, Router, Runtime, RuntimeOrigin, Treasury};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::router::ExecutorError;
use hydradx_traits::router::PoolType::Aave;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::router::Trade;
use hydradx_traits::router::{AssetPair, PoolType};
use orml_traits::MultiCurrency;
use pallet_asset_registry::Assets;
use pallet_broadcast::types::Destination;
use pallet_liquidation::BorrowingContract;
use pallet_route_executor::TradeExecution;
use primitives::Balance;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

fn with_aave(execution: impl FnOnce()) {
	TestNet::reset();
	// Snapshot contains the storage of EVM, AssetRegistry, Timestamp, Omnipool and Tokens pallets
	hydra_live_ext(PATH_TO_SNAPSHOT).execute_with(|| {
		let pap_contract = EvmAddress::from_slice(hex!("82db570265c37bE24caf5bc943428a6848c3e9a6").as_slice());
		let pool_contract = crate::liquidation::get_pool(pap_contract);
		assert_ok!(EVMAccounts::approve_contract(RuntimeOrigin::root(), pool_contract));
		assert_ok!(Liquidation::set_borrowing_contract(
			RuntimeOrigin::root(),
			pool_contract
		));

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), BAG));
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));

		execution();
	});
}

fn with_atoken(execution: impl FnOnce()) {
	with_aave(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			ONE,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
		));
		execution();
	})
}

const HDX: AssetId = 0;
const DOT: AssetId = 5;
const DAI: AssetId = 2;
const ADOT: AssetId = 1_000_037;
const ONE: u128 = 1 * 10_u128.pow(10);
const BAG: u128 = 100000 * ONE;

#[test]
fn nice_borrowing_contract_is_used() {
	with_aave(|| {
		let pool_address = EvmAddress::from_slice(hex!("f550bcd9b766843d72fc4c809a839633fd09b643").as_slice());
		assert_eq!(<BorrowingContract<Runtime>>::get(), pool_address)
	})
}

#[test]
fn adot_is_registered() {
	with_aave(|| assert!(<Assets<Runtime>>::get(ADOT).is_some()))
}

#[test]
fn alice_can_supply() {
	with_aave(|| {
		supply(
			EvmAddress::from_slice(hex!("f550bcd9b766843d72fc4c809a839633fd09b643").as_slice()),
			EVMAccounts::evm_address(&AccountId::from(ALICE)),
			HydraErc20Mapping::encode_evm_address(DOT),
			100 * 10_u128.pow(10),
		);
	})
}

#[test]
fn sell_dot() {
	with_aave(|| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), ONE);
	})
}

#[test]
fn buy_adot() {
	with_aave(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			ONE,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), ONE);
	})
}

#[test]
fn sell_adot() {
	with_atoken(|| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ADOT,
			DOT,
			ONE,
			0,
			vec![Trade {
				pool: Aave,
				asset_in: ADOT,
				asset_out: DOT,
			}]
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 0);
	})
}

#[test]
fn buy_dot() {
	with_atoken(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			ADOT,
			DOT,
			ONE,
			ONE,
			vec![Trade {
				pool: Aave,
				asset_in: ADOT,
				asset_out: DOT,
			}]
		));
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), 0);
	})
}

#[test]
fn executor_ensures_that_out_asset_is_underlying() {
	with_atoken(|| {
		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				ADOT,
				HDX,
				ONE,
				0,
				vec![Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: HDX,
				}]
			),
			Other("Asset mismatch: output asset must match aToken's underlying".into())
		);
	})
}

#[test]
fn executor_ensures_valid_asset_pair() {
	with_atoken(|| {
		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DOT,
				ONE,
				0,
				vec![Trade {
					pool: Aave,
					asset_in: HDX,
					asset_out: DOT,
				}]
			),
			Other("Invalid asset pair".into())
		);
	})
}

#[test]
fn liquidity_depth_of_dot_is_higher_after_buying_atoken() {
	let mut original = 0;
	let mut after = 0;
	with_aave(|| {
		original = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, DOT, ADOT).unwrap();
	});
	with_atoken(|| {
		after = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, DOT, ADOT).unwrap();
	});
	assert!(original < after);
}

#[test]
fn liquidity_depth_of_adot_is_lower_after_buying_atoken() {
	let mut original = 0;
	let mut after = 0;
	with_aave(|| {
		original = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, ADOT, DOT).unwrap();
	});
	with_atoken(|| {
		after = AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, ADOT, DOT).unwrap();
	});
	assert!(original > after);
}

#[test]
fn liquidity_depth_validates_tokens() {
	with_aave(|| {
		assert_eq!(
			AaveTradeExecutor::<Runtime>::get_liquidity_depth(Aave, HDX, DOT),
			Err(ExecutorError::Error(
				"Asset mismatch: first asset atoken has to match second asset reserve".into()
			))
		);
	});
}

#[test]
fn router_should_set_on_chain_route() {
	with_aave(|| {
		let pair = AssetPair {
			asset_in: ADOT,
			asset_out: DOT,
		};
		let route = vec![Trade {
			pool: Aave,
			asset_in: ADOT,
			asset_out: DOT,
		}];
		assert_ok!(Router::set_route(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pair,
			route.clone()
		));
		assert_eq!(Router::get_route(pair), route);
	})
}

#[test]
fn dca_schedule_buying_atokens_should_be_created() {
	with_aave(|| {
		create_schedule(
			ALICE,
			schedule_fake_with_sell_order(ALICE, Aave, 10 * ONE, DOT, ADOT, ONE),
		);
	})
}

#[test]
fn dca_schedule_selling_atokens_should_be_created() {
	with_aave(|| {
		assert_ok!(hydradx_runtime::MultiTransactionPayment::add_currency(
			hydradx_runtime::RuntimeOrigin::root(),
			ADOT,
			FixedU128::from_rational(1, 100000),
		));
		hydradx_runtime::MultiTransactionPayment::on_initialize(0);
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			1000 * ONE,
			1000 * ONE,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
		));
		create_schedule(
			ALICE,
			schedule_fake_with_sell_order(ALICE, Aave, 10 * ONE, ADOT, DOT, ONE),
		);
	})
}
