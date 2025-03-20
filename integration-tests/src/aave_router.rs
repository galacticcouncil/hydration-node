#![cfg(test)]

use crate::dca::create_schedule;
use crate::dca::schedule_fake_with_sell_order;
use crate::liquidation::supply;
use crate::liquidation::PATH_TO_SNAPSHOT;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use frame_support::pallet_prelude::DispatchError::Other;
use frame_support::storage::with_transaction;
use frame_support::traits::OnInitialize;
use frame_support::{assert_noop, BoundedVec};
use hex_literal::hex;
use hydradx_runtime::evm::aave_trade_executor::AaveTradeExecutor;
use hydradx_runtime::evm::precompiles::erc20_mapping::HydraErc20Mapping;
use hydradx_runtime::{AssetId, Currencies, EVMAccounts, Liquidation, Router, Runtime, RuntimeOrigin};
use hydradx_runtime::{AssetRegistry, Stableswap};
use hydradx_traits::evm::Erc20Encoding;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::router::ExecutorError;
use hydradx_traits::router::PoolType::Aave;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::router::Trade;
use hydradx_traits::router::{AssetPair, PoolType};
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::AssetKind;
use hydradx_traits::Create;
use orml_traits::MultiCurrency;
use pallet_asset_registry::Assets;
use pallet_liquidation::BorrowingContract;
use pallet_route_executor::TradeExecution;
use primitives::Balance;
use sp_runtime::traits::Zero;
use sp_runtime::DispatchResult;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::TransactionOutcome;

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

		assert_ok!(Currencies::deposit(DOT, &ALICE.into(), 3 * BAG));

		let _ = with_transaction(|| {
			execution();
			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

fn with_atoken(execution: impl FnOnce()) {
	with_aave(|| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			BAG,
			BAG,
			vec![Trade {
				pool: Aave,
				asset_in: DOT,
				asset_out: ADOT,
			}]
		));
		execution();
	})
}

fn with_stablepool(execution: impl FnOnce(AssetId)) {
	with_atoken(|| {
		let pool = AssetRegistry::register_sufficient_asset(
			None,
			Some(b"pool".to_vec().try_into().unwrap()),
			AssetKind::StableSwap,
			Zero::zero(),
			None,
			None,
			None,
			None,
		)
		.unwrap();

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		assert_ok!(Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool,
			BoundedVec::truncate_from([DOT, ADOT].to_vec()),
			amplification,
			fee,
		));

		assert_ok!(Stableswap::add_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			pool,
			BoundedVec::truncate_from(vec![
				AssetAmount {
					asset_id: DOT,
					amount: BAG,
				},
				AssetAmount {
					asset_id: ADOT,
					amount: BAG,
				},
			]),
		));

		execution(pool);
	});
}

const HDX: AssetId = 0;
const DOT: AssetId = 5;
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
		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(ALICE.into())));
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
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), BAG - ONE);
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
		assert_eq!(Currencies::free_balance(ADOT, &ALICE.into()), BAG - ONE);
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

#[test]
fn buy_adot_from_stablepool() {
	with_stablepool(|pool| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			Balance::MAX,
			vec![Trade {
				pool: PoolType::Stableswap(pool),
				asset_in: DOT,
				asset_out: ADOT,
			},]
		));
	});
}

#[test]
fn sell_in_stable_after_rebase() {
	with_stablepool(|pool| {
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			0,
			vec![
				Trade {
					pool: Aave,
					asset_in: DOT,
					asset_out: ADOT,
				},
				Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(pool),
					asset_in: DOT,
					asset_out: ADOT,
				},
			]
		));
	});
}

#[test]
fn buy_in_stable_after_rebase() {
	with_stablepool(|pool| {
		assert_ok!(Router::buy(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			DOT,
			ADOT,
			ONE,
			Balance::MAX,
			vec![
				Trade {
					pool: Aave,
					asset_in: DOT,
					asset_out: ADOT,
				},
				Trade {
					pool: Aave,
					asset_in: ADOT,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(pool),
					asset_in: DOT,
					asset_out: ADOT,
				},
			]
		));
	});
}
