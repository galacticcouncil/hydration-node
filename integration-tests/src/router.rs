#![cfg(test)]
#![allow(clippy::identity_op)]
use super::assert_balance;
use crate::assert_trader_hdx_balance;
use crate::assert_trader_non_native_balance;
use crate::polkadot_test_net::*;

use hydradx_runtime::{BlockNumber, Router, RuntimeOrigin, LBP, XYK};
use hydradx_traits::{router::PoolType, AMM};
use pallet_lbp::WeightCurveType;
use pallet_route_executor::Trade;
use primitives::asset::AssetPair;
use primitives::AssetId;

use frame_support::{assert_noop, assert_ok};
use xcm_emulator::TestExt;

use orml_traits::MultiCurrency;

const TRADER: [u8; 32] = BOB;

pub const LBP_SALE_START: Option<BlockNumber> = Some(10);
pub const LBP_SALE_END: Option<BlockNumber> = Some(40);

mod router_different_pools_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = UNITS / 100;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: LRNA,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 2_230_007_954_600;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::LBP,
					asset_in: DAI,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::Omnipool,
					asset_in: LRNA,
					asset_out: HDX,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 4_370_898_031;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: DAI,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_fail_when_first_trade_is_successful_but_second_trade_has_no_supported_pool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::Stableswap(100),
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act & Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
			);
		});
	}

	#[test]
	fn buy_should_fail_when_first_trade_is_successful_but_second_trade_has_no_supported_pool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 1_000 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::Stableswap(100),
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act & Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_buy,
					limit,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
			);
		});
	}
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

mod xyk_router_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 4_531_818_181_819_u128;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DOT, amount_out);

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
	fn sell_should_work_when_route_contains_multiple_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, LRNA);
			create_xyk_pool(LRNA, DAI);
			create_xyk_pool(DAI, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: LRNA,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DAI,
					asset_out: DOT,
				},
			];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 1_054_553_059_484_u128;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

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
	fn sell_should_fail_when_there_is_no_pool_for_specific_asset_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::TokenPoolNotFound
			);
		});
	}

	#[test]
	fn sell_should_fail_when_first_trade_is_successful_but_second_trade_has_no_supported_pool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(100),
					asset_in: DOT,
					asset_out: DAI,
				},
			];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
			);
		});
	}

	#[test]
	fn sell_should_fail_when_balance_is_not_sufficient() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = 9999 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell * UNITS,
					0,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::InsufficientBalance
			);
		});
	}

	#[test]
	fn sell_should_fail_when_trading_limit_is_below_minimum() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = hydradx_runtime::MinTradingLimit::get() - 1;
			let limit = 0;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientTradingAmount
			);
		});
	}

	#[test]
	fn sell_should_fail_when_buying_more_than_max_in_ratio_out() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = 1000 * UNITS;
			let limit = 0;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::MaxInRatioExceeded
			);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_buy = 10 * UNITS;
			let limit = 30 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 25075000000001;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

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
	fn buy_should_work_when_route_contains_two_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);
			create_xyk_pool(DOT, DAI);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);

			let amount_to_buy = UNITS;
			let limit = 10 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: DAI,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 4_281_435_927_986;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

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
	fn buy_should_work_when_route_contains_multiple_trades() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);
			create_xyk_pool(DOT, LRNA);
			create_xyk_pool(LRNA, DAI);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);

			let amount_to_buy = UNITS;
			let limit = 10 * UNITS;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: DOT,
					asset_out: LRNA,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: LRNA,
					asset_out: DAI,
				},
			];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 9_392_858_946_762;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DOT, 0);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

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
	fn buy_should_fail_when_there_is_no_pool_for_specific_asset_pair() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::TokenPoolNotFound
			);
		});
	}

	#[test]
	fn buy_should_fail_when_first_trade_is_successful_but_second_trade_has_no_supported_pool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, 0);

			let amount_to_sell = 10;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				},
				Trade {
					pool: PoolType::Stableswap(100),
					asset_in: DOT,
					asset_out: ETH,
				},
			];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					ETH,
					amount_to_sell * UNITS,
					limit,
					trades
				),
				pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
			);
		});
	}

	#[test]
	fn buy_should_fail_when_balance_is_not_sufficient() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(DOT, HDX);

			assert_trader_non_native_balance!(0, DOT);
			let amount_to_buy = 10 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: DOT,
				asset_out: HDX,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					DOT,
					HDX,
					amount_to_buy,
					150 * UNITS,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientAssetBalance
			);
		});
	}

	#[test]
	fn buy_should_fail_when_trading_limit_is_below_minimum() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = hydradx_runtime::MinTradingLimit::get() - 1;
			let limit = 100 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_buy,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientTradingAmount
			);
		});
	}

	#[test]
	fn buy_should_fail_when_buying_more_than_max_ratio_out() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_buy = 20 * UNITS;
			let limit = 100 * UNITS;

			let trades = vec![Trade {
				pool: PoolType::XYK,
				asset_in: HDX,
				asset_out: DOT,
			}];

			//Act and Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_buy,
					limit,
					trades
				),
				pallet_xyk::Error::<hydradx_runtime::Runtime>::MaxOutRatioExceeded
			);
		});
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

fn create_xyk_pool(asset_a: u32, asset_b: u32) {
	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(ALICE.into()),
		asset_a,
		100 * UNITS,
		asset_b,
		50 * UNITS,
	));
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
			"\r\nBSX asset balance '{}' is not as expected '{}'",
			trader_balance, $balance
		);
	}};
}
