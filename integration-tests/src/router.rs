#![cfg(test)]

use super::assert_balance;
use crate::polkadot_test_net::*;
use std::convert::Into;

use hydradx_runtime::{BlockNumber, Omnipool, Router, RuntimeOrigin, LBP};
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

pub const LBP_SALE_START: BlockNumber = 10;
pub const LBP_SALE_END: BlockNumber = 40;

mod router_different_pools_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);

			let amount_to_sell = 10 * UNITS;
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
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 4_383_480_416_162_085;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_out);

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
	fn buy_should_work_when_route_contains_trades_with_different_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_lbp_pool(DAI, LRNA);

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
			];

			start_lbp_campaign();

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 2_135_301_508;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_to_buy);

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
					pool: PoolType::XYK,
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
					pool: PoolType::XYK,
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

mod omnipool_router_tests {
	use super::*;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 266_195_070_030_573_798;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

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
	fn sell_hub_asset_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				LRNA,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 220_685_840_707_964_601_769;

			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: LRNA,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});
	}

	#[test]
	fn direct_sell_should_yield_the_same_result_as_router() {
		TestNet::reset();

		let amount_to_sell = 10 * UNITS;
		let limit = 0;
		let amount_out = 266_195_070_030_573_798;

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			//Act
			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
			));

			//Assert
			expect_hydra_events(vec![pallet_omnipool::Event::SellExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				asset_fee_amount: 667_155_563_986_401,
				protocol_fee_amount: 6_007_435,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);
		});
	}

	#[test]
	fn buy_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

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
			let amount_in = 37_565_544;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
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
	fn buy_hub_asset_should_not_work() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: LRNA,
			}];

			//Act & Assert
			assert_noop!(
				Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DAI,
					amount_to_buy,
					limit,
					trades
				),
				pallet_omnipool::Error::<hydradx_runtime::Runtime>::NotAllowed
			);
		});
	}

	#[test]
	fn direct_buy_should_yield_the_same_result_as_router() {
		TestNet::reset();

		let amount_to_buy = UNITS;
		let limit = 100 * UNITS;
		let amount_in = 37_565_544;

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

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
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_events(vec![pallet_route_executor::Event::RouteExecuted {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
			}
			.into()]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			//Act
			assert_ok!(Omnipool::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
			));

			//Assert
			expect_hydra_events(vec![pallet_omnipool::Event::BuyExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				asset_fee_amount: 111_528,
				protocol_fee_amount: 22,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn trade_should_fail_when_asset_is_not_in_omnipool() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: DAI,
				asset_out: ACA,
			}];

			//Act & Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					ACA,
					amount_to_sell,
					limit,
					trades
				),
				pallet_omnipool::Error::<hydradx_runtime::Runtime>::AssetNotFound
			);
		});
	}
}

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
	use crate::assert_balance;

	#[test]
	fn sell_should_work_when_route_contains_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 5_304_848_794_461;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

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
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			let amount_out = 15_853_064_919_440;

			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_out);

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
			start_lbp_campaign();

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
			let amount_out = 2_894_653_623_153;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
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
	fn sell_should_work_when_route_contains_double_trades_with_selling_distributed_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);
			start_lbp_campaign();

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
			let amount_out = 23_648_944_192_390;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
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
	fn lbp_direct_sell_should_yield_the_same_result_as_router_sell() {
		TestNet::reset();

		let amount_to_sell = 10 * UNITS;
		let limit = 0;
		let received_amount_out = 5_304_848_794_461;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + received_amount_out);

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
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit
			));

			//Assert
			expect_hydra_events(vec![pallet_lbp::Event::SellExecuted {
				who: BOB.into(),
				asset_in: HDX,
				asset_out: DAI,
				amount: 9_980_000_000_000,
				sale_price: received_amount_out,
				fee_asset: HDX,
				fee_amount: 20_000_000_000,
			}
			.into()]);

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + received_amount_out);
		});
	}

	#[test]
	fn buy_should_work_when_when_buying_distributed_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

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
			let amount_in = 19_944_391_321_918;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
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
	fn buy_should_work_when_buying_accumulated_asset_in_a_single_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_buy = 10 * UNITS;
			let limit = 100 * UNITS;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: HDX,
			}];

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit,
				trades
			));

			//Assert
			let amount_in = 6_045_520_997_664;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE + amount_to_buy);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE - amount_in);

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
			start_lbp_campaign();

			let amount_to_buy = UNITS;
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
			let amount_in = 3_244_461_218_396;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
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
	fn buy_should_work_when_having_double_trades_with_buying_accumulated_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(DAI, HDX);
			create_lbp_pool(DOT, DAI);
			start_lbp_campaign();

			let amount_to_buy = UNITS;
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
			let amount_in = 322_733_757_240;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE);
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
	fn lbp_direct_buy_should_yield_the_same_result_as_router_buy() {
		TestNet::reset();

		let amount_to_buy = 10 * UNITS;
		let limit = 100 * UNITS;
		let spent_amount_in = 19_944_391_321_918;

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: HDX,
				asset_out: DAI,
			}];

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
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

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
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				amount_to_buy,
				limit
			));

			//Assert
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - spent_amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);
		});
	}

	#[test]
	fn trade_should_fail_when_asset_is_not_in_lbp() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DAI);
			start_lbp_campaign();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
				asset_in: DAI,
				asset_out: ACA,
			}];

			//Act & Assert
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					ACA,
					amount_to_sell,
					limit,
					trades
				),
				pallet_lbp::Error::<hydradx_runtime::Runtime>::PoolNotFound
			);
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
		RuntimeOrigin::signed(ALICE.into()),
		account_id,
		None,
		Some(LBP_SALE_START),
		Some(LBP_SALE_END),
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
	set_relaychain_block_number(LBP_SALE_START + 1);
}
