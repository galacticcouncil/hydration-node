#![cfg(test)]
#![allow(clippy::identity_op)]
use crate::assert_trader_hdx_balance;
use crate::assert_trader_non_native_balance;
use crate::polkadot_test_net::*;

use hydradx_runtime::{BlockNumber, Router, RuntimeOrigin, LBP};
use hydradx_traits::{router::PoolType, AMM};
use pallet_lbp::WeightCurveType;
use pallet_route_executor::Trade;
use primitives::asset::AssetPair;
use primitives::AssetId;

use frame_support::{assert_noop, assert_ok};
use xcm_emulator::TestExt;

use crate::assert_balance;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::Currencies;
use hydradx_runtime::Omnipool;
use hydradx_runtime::Stableswap;
use hydradx_traits::Registry;
use pallet_stableswap::types::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use sp_runtime::Permill;
use sp_runtime::{DispatchError, FixedU128};

use orml_traits::MultiCurrency;

const TRADER: [u8; 32] = BOB;

pub const LBP_SALE_START: Option<BlockNumber> = Some(10);
pub const LBP_SALE_END: Option<BlockNumber> = Some(40);

//NOTE: XYK pool is not supported in HydraDX. If you want to support it, also adjust router and dca benchmarking
#[test]
fn router_should_not_support_xyk() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let trades = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				trades.clone()
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);

		assert_noop!(
			Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				u128::MAX,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

mod lbp_router_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 5304848460209;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + amount_out, DAI);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_selling_distributed_asset_in_a_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(TRADER.into()),
				DAI,
				HDX,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 15853065839194;

			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE - amount_to_sell, DAI);
			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: HDX,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_route_contains_double_trades_with_selling_accumulated_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			create_lbp_pool(DAI, DOT);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 2894653262401;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE, DAI);
			assert_trader_non_native_balance!(amount_out, DOT);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_route_contains_double_trades_with_selling_distributed_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 23648946648916;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE, DAI);
			assert_trader_non_native_balance!(amount_out, DOT);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn lbp_direct_sell_should_yield_the_same_result_as_router_sell() {
		TestNet::reset();

		let amount_to_sell = 10 * UNITS;
		let limit = 0;
		let received_amount_out = 5304848460209;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + received_amount_out, DAI);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out: received_amount_out,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			start_lbp_campaign();

			//Act
			assert_ok!(LBP::sell(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit
			));

			//Assert
			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + received_amount_out, DAI);
		});
	}

	#[test]
	fn buy_should_work_when_when_buying_distributed_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 19944392706756;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + amount_to_buy, DAI);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_buying_accumulated_asset_in_a_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(TRADER.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 6045520606503;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE + amount_to_buy);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE - amount_in, DAI);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: HDX,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_having_double_trades_with_buying_distributed_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			create_lbp_pool(DAI, DOT);

			let amount_to_buy = 1 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 3244461635777;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE, DAI);
			assert_trader_non_native_balance!(amount_to_buy, DOT);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_having_double_trades_with_buying_accumulated_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);

			let amount_to_buy = 1 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 322733714720;

			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE, DAI);
			assert_trader_non_native_balance!(amount_to_buy, DOT);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn lbp_direct_buy_should_yield_the_same_result_as_router_buy() {
		TestNet::reset();

		let amount_to_buy = 10 * UNITS;
		let limit = 100 * UNITS;
		let spent_amount_in = 19944392706756;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(TRADER.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + amount_to_buy, DAI);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: spent_amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);

			start_lbp_campaign();

			//Act
			assert_ok!(LBP::buy(
				RuntimeOrigin::signed(TRADER.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit
			));

			//Assert
			assert_trader_hdx_balance!(BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_trader_non_native_balance!(BOB_INITIAL_DAI_BALANCE + amount_to_buy, DAI);
		});
	}
}

mod omnipool_stableswap_router_tests {
	use super::*;

	#[test]
	fn router_should_work_with_only_omnipool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//ACt
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				amount_to_sell,
				0,
				trades
			),);

			//Assert
			assert_eq!(
				hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
				ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
			);
		});
	}

	#[test]
	fn router_should_work_for_hopping_from_omniool_to_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				stable_asset_1,
				FixedU128::from_inner(25_650_000_000_000_000_000),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: stable_asset_2,
				},
			];

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_2,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
				ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
			);
		});
	}

	#[test]
	fn sell_single_router_should_add_liquidity_to_stableswap_when_asset_out_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: stable_asset_1,
				asset_out: pool_id,
			}];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				pool_id,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)),
				3000 * UNITS - amount_to_sell
			);
		});
	}

	#[test]
	fn sell_router_should_add_liquidity_to_stableswap_when_selling_for_shareasset_in_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				stable_asset_1,
				FixedU128::from_inner(25_650_000_000_000_000_000),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: stable_asset_1,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				pool_id,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Balances::free_balance(&AccountId::from(ALICE)),
				ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
			);

			assert_balance!(ALICE.into(), pool_id, 4646309366);
		});
	}

	#[test]
	fn router_should_remove_liquidity_from_stableswap_when_selling_shareasset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000_000),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_sell = 100 * UNITS;

			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_1,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_balance!(ALICE.into(), pool_id, 0);
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(ALICE.into(), stable_asset_1, 2903943370);
		});
	}

	#[test]
	fn buy_router_should_remove_liquidity_from_stableswap_when_asset_in_is_shareasset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000_000),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				},
			];

			assert_balance!(ALICE.into(), pool_id, 0);

			//Act
			let amount_to_buy = 1 * UNITS / 1000;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				stable_asset_1,
				amount_to_buy,
				u128::MAX,
				trades
			));

			//Assert
			//assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
			assert_balance!(ALICE.into(), stable_asset_1, amount_to_buy);
		});
	}

	#[test]
	fn buy_router_should_add_liquidity_from_stableswap_when_asset_out_is_share_asset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				Omnipool::protocol_account(),
				pool_id,
				3000 * UNITS as i128,
			));

			assert_ok!(hydradx_runtime::Omnipool::add_token(
				hydradx_runtime::RuntimeOrigin::root(),
				pool_id,
				FixedU128::from_inner(25_650_000_000_000_000_000),
				Permill::from_percent(100),
				AccountId::from(BOB),
			));

			let trades = vec![
				Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: pool_id,
					asset_out: HDX,
				},
			];

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				HDX,
				amount_to_buy,
				u128::MAX,
				trades
			));

			//Assert
			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_out_is_share_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: stable_asset_1,
				asset_out: pool_id,
			}];

			assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				stable_asset_1,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				stable_asset_1,
				pool_id,
				amount_to_buy,
				u128::MAX,
				trades
			));
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_in_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

			let trades = vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: pool_id,
				asset_out: stable_asset_1,
			}];

			//Act
			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				pool_id,
				3000 * UNITS as i128,
			));

			let amount_to_buy = 100 * UNITS;

			assert_ok!(Router::buy(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				pool_id,
				stable_asset_1,
				amount_to_buy,
				u128::MAX,
				trades
			));
		});
	}

	pub fn init_omnipool() {
		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
			hydradx_runtime::RuntimeOrigin::root(),
			u128::MAX,
		));

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));
	}

	pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
		let initial_liquidity = 1_000_000_000_000_000u128;
		let liquidity_added = 300_000_000_000_000u128;

		let mut initial: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
		let mut added_liquidity: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> =
			vec![];

		let mut asset_ids: Vec<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
		for idx in 0u32..MAX_ASSETS_IN_POOL {
			let name: Vec<u8> = idx.to_ne_bytes().to_vec();
			//let asset_id = regi_asset(name.clone(), 1_000_000, 10000 + idx as u32)?;
			let asset_id = AssetRegistry::create_asset(&name, 1u128)?;
			AssetRegistry::set_metadata(hydradx_runtime::RuntimeOrigin::root(), asset_id, b"xDUM".to_vec(), 18u8)?;
			asset_ids.push(asset_id);
			Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				AccountId::from(BOB),
				asset_id,
				1_000_000_000_000_000i128,
			)?;
			Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				AccountId::from(CHARLIE),
				asset_id,
				1_000_000_000_000_000_000_000i128,
			)?;
			initial.push(AssetAmount::new(asset_id, initial_liquidity));
			added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
		}
		let pool_id = AssetRegistry::create_asset(&b"pool".to_vec(), 1u128)?;

		let amplification = 100u16;
		let fee = Permill::from_percent(1);

		let asset_in: AssetId = *asset_ids.last().unwrap();
		let asset_out: AssetId = *asset_ids.first().unwrap();

		Stableswap::create_pool(
			hydradx_runtime::RuntimeOrigin::root(),
			pool_id,
			asset_ids,
			amplification,
			fee,
		)?;

		Stableswap::add_liquidity(hydradx_runtime::RuntimeOrigin::signed(BOB.into()), pool_id, initial)?;

		Ok((pool_id, asset_in, asset_out))
	}
}

fn create_lbp_pool(accumulated_asset: u32, distributed_asset: u32) {
	assert_ok!(LBP::create_pool(
		RuntimeOrigin::root(),
		ALICE.into(),
		accumulated_asset,
		100 * UNITS,
		distributed_asset,
		200 * UNITS,
		20_000_000,
		80_000_000,
		WeightCurveType::Linear,
		(2, 1_000),
		CHARLIE.into(),
		0,
	));

	let account_id = get_lbp_pair_account_id(accumulated_asset, distributed_asset);

	assert_ok!(LBP::update_pool_data(
		RuntimeOrigin::signed(AccountId::from(ALICE)),
		account_id,
		None,
		LBP_SALE_START,
		LBP_SALE_END,
		None,
		None,
		None,
		None,
		None,
	));
}

fn get_lbp_pair_account_id(asset_a: AssetId, asset_b: AssetId) -> AccountId {
	let asset_pair = AssetPair {
		asset_in: asset_a,
		asset_out: asset_b,
	};
	LBP::get_pair_id(asset_pair)
}

fn start_lbp_campaign() {
	set_relaychain_block_number(LBP_SALE_START.unwrap() + 1);
}

#[macro_export]
macro_rules! assert_trader_non_native_balance {
	($balance:expr,$asset_id:expr) => {{
		let trader_balance = hydradx_runtime::Tokens::free_balance($asset_id, &AccountId::from(TRADER));
		assert_eq!(
			trader_balance, $balance,
			"\r\nNon native asset({}) balance '{}' is not as expected '{}'",
			$asset_id, trader_balance, $balance
		);
	}};
}

#[macro_export]
macro_rules! assert_trader_hdx_balance {
	($balance:expr) => {{
		let trader_balance = hydradx_runtime::Balances::free_balance(&AccountId::from(TRADER));
		assert_eq!(
			trader_balance, $balance,
			"\r\nHDX asset balance '{}' is not as expected '{}'",
			trader_balance, $balance
		);
	}};
}
