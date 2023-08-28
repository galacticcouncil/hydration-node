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
			let amount_out = 4_383_480_141_260_650;

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
			let amount_in = 2_135_300_210;

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
			let amount_out = 5_304_848_460_209;

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
			let amount_out = 15_853_065_839_194;

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
			let amount_out = 2_894_653_262_401;

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
			let amount_out = 23_648_946_648_916;

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
		let received_amount_out = 5_304_848_460_209;

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
			let amount_in = 19_944_392_706_756;

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
			let amount_in = 6_045_520_606_503;

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
			let amount_in = 3_244_461_635_777;

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
			let amount_in = 322_733_714_720;

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
		let spent_amount_in = 19_944_392_706_756;

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
