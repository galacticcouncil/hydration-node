#![cfg(test)]
#![allow(clippy::identity_op)]
use super::assert_balance;
use crate::polkadot_test_net::*;
use hydradx_runtime::{
	AssetRegistry, BlockNumber, Currencies, Omnipool, Router, RouterWeightInfo, Runtime, RuntimeOrigin, Stableswap,
	LBP, XYK,
};
use pallet_broadcast::types::Destination;

use hydradx_traits::router::AssetPair as Pair;
use hydradx_traits::router::RouteSpotPriceProvider;
use hydradx_traits::{
	registry::Create,
	router::{PoolType, Trade},
	AssetKind, AMM,
};
use pallet_broadcast::types::Asset;
use pallet_broadcast::types::ExecutionType;
use pallet_broadcast::types::Fee;
use pallet_broadcast::types::Filler;
use pallet_broadcast::types::TradeOperation;
use pallet_lbp::weights::WeightInfo as LbpWeights;
use pallet_lbp::WeightCurveType;
use pallet_omnipool::traits::OmnipoolHooks;
use pallet_omnipool::types::Tradability;
use pallet_omnipool::weights::WeightInfo as OmnipoolWeights;
use pallet_route_executor::AmmTradeWeights;
use primitives::AssetId;
use sp_runtime::FixedPointNumber;
use std::convert::Into;

use frame_support::{assert_noop, assert_ok, BoundedVec};
use xcm_emulator::TestExt;

use frame_support::storage::with_transaction;
use hydradx_traits::stableswap::AssetAmount;
use pallet_stableswap::MAX_ASSETS_IN_POOL;
use sp_runtime::{traits::Zero, DispatchError, DispatchResult, FixedU128, Permill, TransactionOutcome};

use hydradx_runtime::{AccountIdFor, InsufficientEDinHDX};
use orml_traits::MultiCurrency;
pub const LBP_SALE_START: BlockNumber = 10;
pub const LBP_SALE_END: BlockNumber = 40;

#[test]
fn router_weights_should_be_non_zero() {
	assert!(!RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1).is_zero());
	assert!(!RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(1, 1).is_zero());

	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1).is_zero());
	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(1, 0).is_zero());
	assert!(!RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(2, 1).is_zero());
}

mod router_different_pools_tests {
	use super::*;
	use hydradx_traits::router::PoolType;
	use pallet_broadcast::types::ExecutionType;

	#[test]
	fn route_should_fail_when_route_is_not_consistent() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();
			create_xyk_pool_with_amounts(DAI, 1000000 * UNITS, DOT, 1000000 * UNITS);
			create_xyk_pool_with_amounts(ETH, 1000000 * UNITS, DOT, 1000000 * UNITS);

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				BOB.into(),
				ETH,
				300000000 * UNITS as i128,
			));

			let amount_to_sell = UNITS;
			let limit = 0;
			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DAI,
				},
				Trade {
					pool: PoolType::XYK,
					asset_in: ETH,
					asset_out: DOT,
				},
			];

			//Act
			assert_noop!(
				Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					HDX,
					DOT,
					amount_to_sell,
					limit,
					trades
				),
				pallet_route_executor::Error::<Runtime>::InvalidRoute
			);
		});
	}

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
			let amount_out = 2_230_008_413_831;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
			assert_balance!(BOB.into(), LRNA, BOB_INITIAL_LRNA_BALANCE);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_out);

			expect_hydra_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)),
					filler_type: pallet_broadcast::types::Filler::LBP,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(DAI, 9980000000)],
					outputs: vec![Asset::new(LRNA, 5640664064)],
					fees: vec![Fee::new(
						DAI,
						20000000,
						Destination::Account(
							LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)))
								.unwrap()
								.fee_collector,
						),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4682924837974)],
					fees: vec![Fee::new(
						HDX,
						11736653730,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 4682924837974)],
					outputs: vec![Asset::new(DOT, 2230008413831)],
					fees: vec![Fee::new(
						DOT,
						6710155707,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_route_executor::Event::Executed {
					asset_in: DAI,
					asset_out: DOT,
					amount_in: amount_to_sell,
					amount_out,
					event_id: 0,
				}
				.into(),
			]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (stable_pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
				create_lbp_pool(DAI, HDX);

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					3000 * UNITS as i128,
				));

				create_xyk_pool(HDX, stable_asset_1);

				let amount_to_sell = UNITS / 100;
				let limit = 0;
				let trades = vec![
					Trade {
						pool: PoolType::LBP,
						asset_in: DAI,
						asset_out: HDX,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::Stableswap(stable_pool_id),
						asset_in: stable_asset_1,
						asset_out: stable_asset_2,
					},
				];

				start_lbp_campaign();

				//Act
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					stable_asset_2,
					amount_to_sell,
					limit,
					trades
				));

				//Assert
				let amount_out = 2_783_595_233;

				assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);
				assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
				assert_balance!(BOB.into(), stable_asset_1, 0);
				assert_balance!(BOB.into(), stable_asset_2, amount_out);

				expect_hydra_events(vec![
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)),
						filler_type: Filler::LBP,
						operation: TradeOperation::ExactIn,
						inputs: vec![Asset::new(DAI, 9980000000)],
						outputs: vec![Asset::new(HDX, 5640664064)],
						fees: vec![Fee::new(
							DAI,
							20000000,
							Destination::Account(
								LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)))
									.unwrap()
									.fee_collector,
							),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: stable_asset_1,
						}),
						filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
							pallet_xyk::types::AssetPair {
								asset_in: HDX,
								asset_out: stable_asset_1,
							},
						))),
						operation: pallet_broadcast::types::TradeOperation::ExactIn,
						inputs: vec![Asset::new(HDX, 5640664064)],
						outputs: vec![Asset::new(stable_asset_1, 2811712439)],
						fees: vec![Fee::new(
							stable_asset_1,
							8460516,
							Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
								asset_in: HDX,
								asset_out: stable_asset_1,
							})),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: <Runtime as pallet_stableswap::Config>::ShareAccountId::from_assets(
							&stable_pool_id,
							Some(pallet_stableswap::POOL_IDENTIFIER),
						),
						filler_type: pallet_broadcast::types::Filler::Stableswap(stable_pool_id),
						operation: TradeOperation::ExactIn,
						inputs: vec![Asset::new(stable_asset_1, 2811712439)],
						outputs: vec![Asset::new(stable_asset_2, 2783595233)],
						fees: vec![Fee::new(
							stable_asset_2,
							28117123,
							Destination::Account(<Runtime as pallet_stableswap::Config>::ShareAccountId::from_assets(
								&stable_pool_id,
								Some(pallet_stableswap::POOL_IDENTIFIER),
							)),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_route_executor::Event::Executed {
						asset_in: DAI,
						asset_out: stable_asset_2,
						amount_in: amount_to_sell,
						amount_out,
						event_id: 0,
					}
					.into(),
				]);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
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
			let amount_in = 4_370_898_989;

			assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
			assert_balance!(BOB.into(), LRNA, 1_000 * UNITS);
			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
			assert_balance!(BOB.into(), DOT, amount_to_buy);

			expect_hydra_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)),
					filler_type: pallet_broadcast::types::Filler::LBP,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(DAI, 4362157193)],
					outputs: vec![Asset::new(LRNA, 2465566245)],
					fees: vec![Fee::new(
						DAI,
						8741796,
						Destination::Account(
							LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)))
								.unwrap()
								.fee_collector,
						),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(LRNA, 2465566245)],
					outputs: vec![Asset::new(HDX, 2046938775509)],
					fees: vec![Fee::new(
						HDX,
						5130172370,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(HDX, 1000000000000)],
					outputs: vec![Asset::new(DOT, 2040816326531)],
					fees: vec![Fee::new(
						HDX,
						6122448978,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_route_executor::Event::Executed {
					asset_in: DAI,
					asset_out: DOT,
					amount_in,
					amount_out: amount_to_buy,
					event_id: 0,
				}
				.into(),
			]);
		});

		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (stable_pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
				create_lbp_pool(DAI, HDX);

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					3000 * UNITS as i128,
				));

				create_xyk_pool(HDX, stable_asset_1);

				let amount_to_buy = UNITS;
				let limit = 100 * UNITS;
				let trades = vec![
					Trade {
						pool: PoolType::LBP,
						asset_in: DAI,
						asset_out: HDX,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::Stableswap(stable_pool_id),
						asset_in: stable_asset_1,
						asset_out: stable_asset_2,
					},
				];

				start_lbp_campaign();

				//Act
				assert_ok!(Router::buy(
					RuntimeOrigin::signed(BOB.into()),
					DAI,
					stable_asset_2,
					amount_to_buy,
					limit,
					trades
				));

				//Assert
				let amount_in = 3_753_549_142_038;

				assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_in);
				assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE);
				assert_balance!(BOB.into(), stable_asset_1, 0);
				assert_balance!(BOB.into(), stable_asset_2, amount_to_buy);

				expect_hydra_events(vec![
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)),
						filler_type: pallet_broadcast::types::Filler::LBP,
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(DAI, 3746042043754)],
						outputs: vec![Asset::new(HDX, 2067851065323)],
						fees: vec![Fee::new(
							DAI,
							7507098284,
							Destination::Account(
								LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)))
									.unwrap()
									.fee_collector,
							),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: stable_asset_1,
						}),
						filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
							pallet_xyk::types::AssetPair {
								asset_in: HDX,
								asset_out: stable_asset_1,
							},
						))),
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(HDX, 1010010000114)],
						outputs: vec![Asset::new(stable_asset_1, 2061666067122)],
						fees: vec![Fee::new(
							HDX,
							6184998201,
							Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
								asset_in: HDX,
								asset_out: stable_asset_1,
							})),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_broadcast::Event::Swapped {
						swapper: BOB.into(),
						filler: <Runtime as pallet_stableswap::Config>::ShareAccountId::from_assets(
							&stable_pool_id,
							Some(pallet_stableswap::POOL_IDENTIFIER),
						),
						filler_type: pallet_broadcast::types::Filler::Stableswap(stable_pool_id),
						operation: pallet_broadcast::types::TradeOperation::ExactOut,
						inputs: vec![Asset::new(stable_asset_1, 1010010000114)],
						outputs: vec![Asset::new(stable_asset_2, 1000000000000)],
						fees: vec![Fee::new(
							stable_asset_1,
							10000099012,
							Destination::Account(<Runtime as pallet_stableswap::Config>::ShareAccountId::from_assets(
								&stable_pool_id,
								Some(pallet_stableswap::POOL_IDENTIFIER),
							)),
						)],
						operation_stack: vec![ExecutionType::Router(0)],
					}
					.into(),
					pallet_route_executor::Event::Executed {
						asset_in: DAI,
						asset_out: stable_asset_2,
						amount_in,
						amount_out: amount_to_buy,
						event_id: 0,
					}
					.into(),
				]);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn multiple_trades_should_increase_event_id() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(HDX, DOT);

			let amount_to_sell = UNITS / 100;
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
				trades.clone()
			));

			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				10 * amount_to_sell,
				trades.clone()
			));

			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DOT,
				amount_to_sell,
				limit,
				trades
			));

			//Assert
			expect_hydra_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 10000000000)],
					outputs: vec![Asset::new(DOT, 4984501549)],
					fees: vec![Fee::new(
						DOT,
						14998500,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(HDX, 10000000000)],
					outputs: vec![Asset::new(DOT, 20007996198)],
					fees: vec![Fee::new(
						HDX,
						60023988,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Router(1)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_broadcast::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 10000000000)],
					outputs: vec![Asset::new(DOT, 4981510054)],
					fees: vec![Fee::new(
						DOT,
						14989497,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Router(2)],
				}
				.into(),
			]);
		});
	}

	#[test]
	fn router_should_work_for_hopping_from_omnipool_to_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
					hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
					ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
				);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_router_should_add_liquidity_to_stableswap_when_selling_for_shareasset_in_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
					hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
					ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
				);

				assert_balance!(ALICE.into(), pool_id, 4638992258357);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn router_should_remove_liquidity_from_stableswap_when_selling_shareasset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
				assert_balance!(ALICE.into(), stable_asset_1, 2899390145403);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_router_should_remove_liquidity_from_stableswap_when_asset_in_is_shareasset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
				let amount_to_buy = UNITS / 1000;

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
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_router_should_add_liquidity_from_stableswap_when_asset_out_is_share_asset_in_stable() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn trade_should_return_correct_weight() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange

			let trades = vec![
				Trade {
					pool: PoolType::Omnipool,
					asset_in: DAI,
					asset_out: ACA,
				},
				Trade {
					pool: PoolType::LBP,
					asset_in: ACA,
					asset_out: DOT,
				},
			];

			//Act & Assert
			assert_eq!(
				RouterWeightInfo::sell_weight(trades.as_slice()),
				hydradx_runtime::weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_sell(1, 1)
					.checked_add(&<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_trade_weight())
					.unwrap()
					.checked_add(&<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_liquidity_changed_weight())
					.unwrap()
					.checked_add(
						&hydradx_runtime::weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_sell(1, 1)
					)
					.unwrap()
					.checked_add(
						&RouterWeightInfo::sell_and_calculate_sell_trade_amounts_overhead_weight(0, 1)
							.checked_mul(2)
							.unwrap()
					)
					.unwrap()
			);
			assert_eq!(
				RouterWeightInfo::buy_weight(trades.as_slice()),
				hydradx_runtime::weights::pallet_omnipool::HydraWeight::<Runtime>::router_execution_buy(1, 1)
					.checked_add(&<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_trade_weight())
					.unwrap()
					.checked_add(&<Runtime as pallet_omnipool::Config>::OmnipoolHooks::on_liquidity_changed_weight())
					.unwrap()
					.checked_add(
						&hydradx_runtime::weights::pallet_lbp::HydraWeight::<Runtime>::router_execution_buy(1, 1)
					)
					.unwrap()
					.checked_add(
						&RouterWeightInfo::buy_and_calculate_buy_trade_amounts_overhead_weight(0, 1)
							.checked_mul(2)
							.unwrap()
					)
					.unwrap()
			);
		});
	}
}

mod omnipool_router_tests {
	use super::*;
	use frame_support::assert_noop;
	use hydradx_runtime::{Balances, Omnipool, Treasury, XYK};
	use hydradx_traits::router::PoolType;
	use hydradx_traits::AssetKind;
	use pallet_broadcast::types::{Destination, ExecutionType};

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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_should_work_when_user_has_left_less_than_existential_in_nonnative() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

				init_omnipool();

				let init_balance = 3000 * UNITS + 1;
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					init_balance as i128,
				));

				let trades = vec![Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				}];

				assert_balance!(ALICE.into(), pool_id, 0);

				//Act
				let amount_to_sell = 3000 * UNITS;
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
					0
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_fail_when_all_asset_in_spent_for_altcoin() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"SHITCO".to_vec();
				let altcoin = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				assert_ok!(Currencies::deposit(altcoin, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					100000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					HDX,
					100000 * UNITS,
					altcoin,
					100000 * UNITS,
				));

				let trades = vec![Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: altcoin,
				}];

				//Act
				let amount_to_sell = ALICE_INITIAL_NATIVE_BALANCE;
				assert_noop!(
					Router::sell(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						HDX,
						altcoin,
						amount_to_sell,
						0,
						trades
					),
					orml_tokens::Error::<Runtime>::ExistentialDeposit
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_pass_when_user_has_asset_in_covering_the_fee_for_altcoin() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"SHITCO".to_vec();
				let altcoin = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(altcoin, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					100000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					HDX,
					100000 * UNITS,
					altcoin,
					100000 * UNITS,
				));

				let trades = vec![Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: altcoin,
				}];

				//Act
				let amount_to_sell = ALICE_INITIAL_NATIVE_BALANCE - 20 * UNITS;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					altcoin,
					amount_to_sell,
					0,
					trades
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_not_charge_ed_when_insufficient_in_middle_of_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"INSUFF".to_vec();
				let insufficient_asset = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(ETH, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(DOT, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					10000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					HDX,
					10000 * UNITS,
					DOT,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					DOT,
					10000 * UNITS,
					insufficient_asset,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset,
					10000 * UNITS,
					ETH,
					10000 * UNITS,
				));

				let trades = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: insufficient_asset,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset,
						asset_out: ETH,
					},
				];

				//Act
				assert_balance!(ALICE.into(), HDX, 1000 * UNITS);

				let amount_to_sell = 20 * UNITS;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					ETH,
					amount_to_sell,
					0,
					trades
				));

				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - amount_to_sell);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_only_insufficient_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"INSUF1".to_vec();
				let insufficient_asset1 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF2".to_vec();
				let insufficient_asset2 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF3".to_vec();
				let insufficient_asset3 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF4".to_vec();
				let insufficient_asset4 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(insufficient_asset1, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset2, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset3, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset4, &DAVE.into(), 100000 * UNITS,));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset1,
					10000 * UNITS,
					insufficient_asset2,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset2,
					10000 * UNITS,
					insufficient_asset3,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset3,
					10000 * UNITS,
					insufficient_asset4,
					10000 * UNITS,
				));

				let trades = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset1,
						asset_out: insufficient_asset2,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset2,
						asset_out: insufficient_asset3,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset3,
						asset_out: insufficient_asset4,
					},
				];

				assert_ok!(Currencies::deposit(insufficient_asset1, &ALICE.into(), 1500 * UNITS,));

				let ed = InsufficientEDinHDX::get();
				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - ed);

				let amount_to_sell = 20 * UNITS;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					insufficient_asset1,
					insufficient_asset4,
					amount_to_sell,
					0,
					trades
				));

				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - 2 * ed);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn ed_should_be_refunded_when_all_insufficient_assets_sold() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"INSUF1".to_vec();
				let insufficient_asset1 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF2".to_vec();
				let insufficient_asset2 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(insufficient_asset1, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset2, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(ETH, &DAVE.into(), 100000 * UNITS,));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset1,
					10000 * UNITS,
					insufficient_asset2,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset2,
					10000 * UNITS,
					ETH,
					10000 * UNITS,
				));

				let trades = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset1,
						asset_out: insufficient_asset2,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset2,
						asset_out: ETH,
					},
				];

				let alice_balance_before_trade = Balances::free_balance(AccountId::from(ALICE));

				let insufficient_asset1_balance = 100 * UNITS;
				assert_ok!(Currencies::deposit(
					insufficient_asset1,
					&ALICE.into(),
					insufficient_asset1_balance,
				));

				let extra_ed_charge = UNITS / 10;

				let amount_to_sell = insufficient_asset1_balance;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					insufficient_asset1,
					ETH,
					amount_to_sell,
					0,
					trades
				));
				let alice_balance_after_trade = Balances::free_balance(AccountId::from(ALICE));

				//ED should be refunded to alice as she sold all her asset, minus the 10% extra
				assert_eq!(alice_balance_before_trade, alice_balance_after_trade + extra_ed_charge);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn ed_charging_should_not_be_disabled_when_only_one_trade_with_insufficient_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"INSUF1".to_vec();
				let insufficient_asset_1 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF12".to_vec();
				let insufficient_asset_2 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();
				assert_ok!(Currencies::deposit(insufficient_asset_1, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset_2, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					100000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset_1,
					100000 * UNITS,
					insufficient_asset_2,
					100000 * UNITS,
				));

				let trades = vec![Trade {
					pool: PoolType::XYK,
					asset_in: insufficient_asset_1,
					asset_out: insufficient_asset_2,
				}];

				//Act
				let amount_to_sell = 10 * UNITS;
				assert_ok!(Currencies::deposit(insufficient_asset_1, &ALICE.into(), amount_to_sell,));
				let ed = InsufficientEDinHDX::get();
				let extra_ed_charge = UNITS / 10;
				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - ed);

				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					insufficient_asset_1,
					insufficient_asset_2,
					amount_to_sell,
					0,
					trades
				),);

				//ED for insufficient_asset_1 is refunded, but ED for insufficient_asset_2 is charged plus extra 10%
				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - 1 * ed - extra_ed_charge);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn buy_should_work_with_only_insufficient_assets() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"INSUF1".to_vec();
				let insufficient_asset1 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF2".to_vec();
				let insufficient_asset2 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF3".to_vec();
				let insufficient_asset3 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				let name = b"INSUF4".to_vec();
				let insufficient_asset4 = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(insufficient_asset1, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset2, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset3, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::deposit(insufficient_asset4, &DAVE.into(), 100000 * UNITS,));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset1,
					10000 * UNITS,
					insufficient_asset2,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset2,
					10000 * UNITS,
					insufficient_asset3,
					10000 * UNITS,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					insufficient_asset3,
					10000 * UNITS,
					insufficient_asset4,
					10000 * UNITS,
				));

				let trades = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset1,
						asset_out: insufficient_asset2,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset2,
						asset_out: insufficient_asset3,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: insufficient_asset3,
						asset_out: insufficient_asset4,
					},
				];

				assert_ok!(Currencies::deposit(insufficient_asset1, &ALICE.into(), 1500 * UNITS,));

				let ed = InsufficientEDinHDX::get();
				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - ed);

				let amount_to_buy = 20 * UNITS;
				assert_ok!(Router::buy(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					insufficient_asset1,
					insufficient_asset4,
					amount_to_buy,
					u128::MAX,
					trades
				));

				assert_balance!(ALICE.into(), HDX, 1000 * UNITS - 2 * ed);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_pass_when_ed_refund_after_selling_all_shitcoin() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"SHITCO".to_vec();
				let shitcoin = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(shitcoin, &DAVE.into(), 110000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					DAI,
					100000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					DAI,
					100000 * UNITS,
					shitcoin,
					100000 * UNITS,
				));

				init_omnipool();

				let trades = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: shitcoin,
						asset_out: DAI,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DAI,
						asset_out: HDX,
					},
				];

				//Act
				assert_ok!(Currencies::deposit(shitcoin, &ALICE.into(), 127_733_235_715_547));
				let amount_to_sell = 127_733_235_715_547;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					shitcoin,
					HDX,
					amount_to_sell,
					0,
					trades
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_pass_when_ed_refund_happens_in_intermediare_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"SHITCO".to_vec();
				let shitcoin = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(shitcoin, &DAVE.into(), 11000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					10000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					shitcoin,
					10000 * UNITS,
					HDX,
					10000 * UNITS,
				));

				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					shitcoin,
					6000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					shitcoin,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					BTC,
					6000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					BTC,
					FixedU128::from_rational(1, 3),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					ETH,
					6000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					ETH,
					FixedU128::from_rational(1, 3),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: shitcoin,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: shitcoin,
						asset_out: HDX,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: BTC,
					},
				];

				//Act
				//let amount_to_buy = 127_733_235_715_547;
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					ETH,
					6000 * UNITS as i128,
				));
				//assert_ok!(Currencies::deposit(DAI, &ALICE.into(), 100000 * UNITS));
				assert_ok!(Router::buy(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					ETH,
					BTC,
					UNITS,
					u128::MAX,
					trades
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_when_receiving_shitcoin() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let name = b"SHITC1".to_vec();
				let shitcoin = AssetRegistry::register_insufficient_asset(
					None,
					Some(name.try_into().unwrap()),
					AssetKind::External,
					Some(1_000),
					None,
					None,
					None,
					None,
				)
				.unwrap();

				assert_ok!(Currencies::deposit(shitcoin, &DAVE.into(), 100000 * UNITS,));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					DAVE.into(),
					HDX,
					100000 * UNITS as i128,
				));

				assert_ok!(XYK::create_pool(
					RuntimeOrigin::signed(DAVE.into()),
					HDX,
					100000 * UNITS,
					shitcoin,
					100000 * UNITS,
				));

				let trades = vec![Trade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: shitcoin,
				}];

				//Act
				let amount_to_sell = ALICE_INITIAL_NATIVE_BALANCE - 20 * UNITS;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					shitcoin,
					amount_to_sell,
					0,
					trades
				));

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_when_user_has_left_less_than_existential_in_native() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			assert_ok!(Currencies::update_balance(
				hydradx_runtime::RuntimeOrigin::root(),
				ALICE.into(),
				HDX,
				2 * UNITS as i128,
			));

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act and assert
			let amount_to_sell = ALICE_INITIAL_NATIVE_BALANCE;
			assert_ok!(Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				amount_to_sell,
				0,
				trades
			));

			//Assert
			assert_eq!(
				hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(ALICE)),
				2 * UNITS
			);
		});
	}

	#[test]
	fn sell_should_work_when_account_providers_increases_during_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let _ = with_transaction(|| {
				let (pool_id, stable_asset_1, _stable_asset_2) = init_stableswap().unwrap();
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					60000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					HDX,
					2 * UNITS as i128,
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

				//We need to do this because this setup leads to different behaviour of reducable_balance in the post balance check in router
				hydradx_runtime::System::inc_consumers(&AccountId::from(ALICE)).unwrap();
				let acc = hydradx_runtime::System::account(AccountId::from(ALICE));
				assert_eq!(acc.consumers, 1);
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				let acc = hydradx_runtime::System::account(AccountId::from(ALICE));
				assert_eq!(acc.providers, 1);

				//Act and assert
				let amount_to_sell = ALICE_INITIAL_NATIVE_BALANCE;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					stable_asset_1,
					amount_to_sell,
					0,
					trades
				));

				//Assert
				assert_eq!(
					hydradx_runtime::Currencies::free_balance(HDX, &AccountId::from(ALICE)),
					2 * UNITS
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_selling_nonnaitve_when_account_providers_increases_during_trade() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			let _ = with_transaction(|| {
				let (pool_id, stable_asset_1, _stable_asset_2) = init_stableswap().unwrap();
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					pool_id,
					60000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					pool_id,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					DAI,
					2 * UNITS as i128,
				));

				let trades = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DAI,
						asset_out: pool_id,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: pool_id,
						asset_out: stable_asset_1,
					},
				];

				//We need to do this because this setup leads to different behaviour of reducable_balance in the post balance check in router
				hydradx_runtime::System::inc_consumers(&AccountId::from(ALICE)).unwrap();
				let acc = hydradx_runtime::System::account(AccountId::from(ALICE));
				assert_eq!(acc.consumers, 1);
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				hydradx_runtime::System::dec_providers(&AccountId::from(ALICE)).unwrap();
				let acc = hydradx_runtime::System::account(AccountId::from(ALICE));
				assert_eq!(acc.providers, 1);

				//Act and assert
				let amount_to_sell = ALICE_INITIAL_DAI_BALANCE;
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					DAI,
					stable_asset_1,
					amount_to_sell,
					0,
					trades
				));

				//Assert
				assert_eq!(
					hydradx_runtime::Currencies::free_balance(DAI, &AccountId::from(ALICE)),
					2 * UNITS
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: LRNA,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			expect_hydra_last_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, amount_to_sell)],
					outputs: vec![Asset::new(LRNA, 12014871681)],
					fees: vec![
						Fee::new(LRNA, 3003717, Destination::Burned),
						Fee::new(LRNA, 3003718, Destination::Account(Treasury::account_id())),
					],
					operation_stack: vec![ExecutionType::Router(0), ExecutionType::Omnipool(1)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 12_008_864_246)],
					outputs: vec![Asset::new(DAI, amount_out)],
					fees: vec![Fee::new(
						DAI,
						667155563986401,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Router(0), ExecutionType::Omnipool(1)],
				}
				.into(),
				pallet_route_executor::Event::Executed {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: amount_to_sell,
					amount_out,
					event_id: 0,
				}
				.into(),
			]);
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
			expect_hydra_last_events(vec![
				pallet_omnipool::Event::SellExecuted {
					who: BOB.into(),
					asset_in: HDX,
					asset_out: DAI,
					amount_in: amount_to_sell,
					amount_out,
					hub_amount_in: 12014871681,
					hub_amount_out: 12038886566,
					asset_fee_amount: 667_155_563_986_401,
					protocol_fee_amount: 6_007_435,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, amount_to_sell)],
					outputs: vec![Asset::new(LRNA, 12_014_871_681)],
					fees: vec![
						Fee::new(LRNA, 3003717, Destination::Burned),
						Fee::new(LRNA, 3003718, Destination::Account(Treasury::account_id())),
					],

					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 12_008_864_246)],
					outputs: vec![Asset::new(DAI, amount_out)],
					fees: vec![Fee::new(
						DAI,
						667155563986401,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
			]);

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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_when_after_trade_reamining_balance_is_less_than_existential_deposit() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = 26559360000000000000u128;

			let limit = ALICE_INITIAL_NATIVE_BALANCE;
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
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);
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
					LRNA,
					amount_to_buy,
					limit,
					trades
				),
				pallet_omnipool::Error::<Runtime>::NotAllowed
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

			expect_hydra_last_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(HDX, amount_in)],
					outputs: vec![Asset::new(LRNA, 45135)],
					fees: vec![
						Fee::new(LRNA, 11, Destination::Burned),
						Fee::new(LRNA, 11, Destination::Account(Treasury::account_id())),
					],
					operation_stack: vec![ExecutionType::Router(0), ExecutionType::Omnipool(1)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(LRNA, 45225)],
					outputs: vec![Asset::new(DAI, amount_to_buy)],
					fees: vec![Fee::new(
						DAI,
						2506265665,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Router(0), ExecutionType::Omnipool(1)],
				}
				.into(),
				pallet_route_executor::Event::Executed {
					asset_in: HDX,
					asset_out: DAI,
					amount_in,
					amount_out: amount_to_buy,
					event_id: 0,
				}
				.into(),
			]);
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
			expect_hydra_last_events(vec![
				pallet_omnipool::Event::BuyExecuted {
					who: BOB.into(),
					asset_in: HDX,
					asset_out: DAI,
					amount_in,
					amount_out: amount_to_buy,
					hub_amount_in: 45135,
					hub_amount_out: 45225,
					asset_fee_amount: 2_506_265_665,
					protocol_fee_amount: 22,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(HDX, amount_in)],
					outputs: vec![Asset::new(LRNA, 45135)],
					fees: vec![
						Fee::new(LRNA, 11, Destination::Burned),
						Fee::new(LRNA, 11, Destination::Account(Treasury::account_id())),
					],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactOut,
					inputs: vec![Asset::new(LRNA, 45113)],
					outputs: vec![Asset::new(DAI, amount_to_buy)],
					fees: vec![Fee::new(
						DAI,
						2506265665,
						Destination::Account(Omnipool::protocol_account()),
					)],
					operation_stack: vec![ExecutionType::Omnipool(0)],
				}
				.into(),
			]);

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
				pallet_omnipool::Error::<Runtime>::AssetNotFound
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
			let amount_out = 5_304_848_794_461;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: DAI,
				asset_out: HDX,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			let fee = 20000000000;

			expect_hydra_last_events(vec![
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)),
					filler_type: pallet_broadcast::types::Filler::LBP,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, amount_to_sell - fee)],
					outputs: vec![Asset::new(DAI, received_amount_out)],
					fees: vec![Fee::new(
						HDX,
						fee,
						Destination::Account(
							LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(HDX, DAI)))
								.unwrap()
								.fee_collector,
						),
					)],
					operation_stack: vec![ExecutionType::Router(0)],
				}
				.into(),
				pallet_route_executor::Event::Executed {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: amount_to_sell,
					amount_out: received_amount_out,
					event_id: 0,
				}
				.into(),
			]);
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
			expect_hydra_last_events(vec![
				pallet_lbp::Event::SellExecuted {
					who: BOB.into(),
					asset_in: HDX,
					asset_out: DAI,
					amount: 9_980_000_000_000,
					sale_price: received_amount_out,
					fee_asset: HDX,
					fee_amount: 20_000_000_000,
				}
				.into(),
				pallet_broadcast::Event::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, HDX)),
					filler_type: pallet_broadcast::types::Filler::LBP,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 9_980_000_000_000)],
					outputs: vec![Asset::new(DAI, received_amount_out)],
					fees: vec![Fee::new(
						HDX,
						20_000_000_000,
						Destination::Account(
							LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(HDX, DAI)))
								.unwrap()
								.fee_collector,
						),
					)],
					operation_stack: vec![],
				}
				.into(),
			]);

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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: DAI,
				asset_out: HDX,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: spent_amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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
				pallet_lbp::Error::<Runtime>::PoolNotFound
			);
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
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
				pallet_xyk::Error::<hydradx_runtime::Runtime>::InsufficientAssetBalance
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DOT,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
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
	fn buy_should_fail_when_balance_is_not_sufficient() {
		Hydra::execute_with(|| {
			//Arrange
			create_xyk_pool(DOT, HDX);

			assert_balance!(BOB.into(), DOT, 0);
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

mod omnipool_stableswap_router_tests {
	use super::*;

	#[test]
	fn sell_single_router_should_add_liquidity_to_stableswap_when_asset_out_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_out_is_share_asset() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn single_buy_router_should_work_one_stable_trade_when_asset_in_is_share() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

mod set_route {
	use super::*;
	use frame_support::assert_noop;
	use frame_support::storage::with_transaction;
	use hydradx_traits::router::inverse_route;
	use hydradx_traits::router::PoolType;
	use sp_runtime::TransactionOutcome;

	mod when_prestored_route_is_invalid {
		use super::*;
		use frame_support::assert_ok;
		use hydradx_runtime::EmaOracle;
		use hydradx_traits::AssetKind;
		use primitives::constants::chain::XYK_SOURCE;

		#[test]
		fn set_route_should_work_with_omnipool_xyk_and_stable_pools() {
			{
				TestNet::reset();

				Hydra::execute_with(|| {
					let _ = with_transaction(|| {
						//Arrange
						let (pool_id, stable_asset_1, _) =
							init_stableswap_with_details(1_000_000_000_000_000u128, 300_000_000_000_000u128, 18)
								.unwrap();

						init_omnipool();

						assert_ok!(Currencies::update_balance(
							hydradx_runtime::RuntimeOrigin::root(),
							Omnipool::protocol_account(),
							pool_id,
							1000 * UNITS as i128,
						));

						assert_ok!(hydradx_runtime::Omnipool::add_token(
							hydradx_runtime::RuntimeOrigin::root(),
							pool_id,
							FixedU128::from_rational(1, 2),
							Permill::from_percent(1),
							AccountId::from(BOB),
						));

						create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 2000 * UNITS);
						create_xyk_pool_with_amounts(HDX, 10000000 * UNITS, DOT, 10000 * UNITS);

						let route1 = vec![
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
							Trade {
								pool: PoolType::XYK,
								asset_in: stable_asset_1,
								asset_out: DOT,
							},
						];

						let route2_cheaper = vec![Trade {
							pool: PoolType::XYK,
							asset_in: HDX,
							asset_out: DOT,
						}];

						let asset_pair = Pair::new(HDX, DOT);

						//Verify if the cheaper route is indeed cheaper in both ways
						let amount_to_sell = 1 * UNITS;

						//Check for normal route
						let dot_amount_out = with_transaction::<_, _, _>(|| {
							assert_ok!(Router::sell(
								hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
								HDX,
								DOT,
								amount_to_sell,
								0,
								route1.clone()
							));
							let alice_received_dot =
								Currencies::free_balance(DOT, &AccountId::from(ALICE)) - ALICE_INITIAL_DOT_BALANCE;

							TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
						})
						.unwrap();

						//Check for normal route
						let dot_amout_out_for_cheaper_route = with_transaction::<_, _, _>(|| {
							assert_ok!(Router::sell(
								hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
								HDX,
								DOT,
								amount_to_sell,
								0,
								route2_cheaper.clone()
							));
							let alice_received_dot =
								Currencies::free_balance(DOT, &AccountId::from(ALICE)) - ALICE_INITIAL_DOT_BALANCE;

							TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_dot))
						})
						.unwrap();

						assert!(dot_amout_out_for_cheaper_route > dot_amount_out);

						// Check for inverse route
						let amount_out_for_inverse = with_transaction::<_, _, _>(|| {
							let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
							assert_ok!(Router::sell(
								hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
								DOT,
								HDX,
								amount_to_sell,
								0,
								inverse_route(route1.clone())
							));
							let alice_received_hdx =
								Currencies::free_balance(HDX, &AccountId::from(ALICE)) - alice_hdx_balance;

							TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_hdx))
						})
						.unwrap();

						let amount_out_for_inverse_with_chaper_route = with_transaction::<_, _, _>(|| {
							let alice_hdx_balance = Currencies::free_balance(HDX, &AccountId::from(ALICE));
							assert_ok!(Router::sell(
								hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
								DOT,
								HDX,
								amount_to_sell,
								0,
								inverse_route(route2_cheaper.clone())
							));
							let alice_received_hdx =
								Currencies::free_balance(HDX, &AccountId::from(ALICE)) - alice_hdx_balance;

							TransactionOutcome::Rollback(Ok::<u128, DispatchError>(alice_received_hdx))
						})
						.unwrap();

						assert!(amount_out_for_inverse_with_chaper_route > amount_out_for_inverse);

						//ACT AND ASSERT
						populate_oracle(HDX, DOT, route1.clone(), Some(10), None);

						//We set first the more expensive route
						assert_ok!(Router::set_route(
							hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
							asset_pair,
							route1.clone()
						));
						assert_eq!(Router::route(asset_pair).unwrap(), route1);

						//We set the cheaper one so it should replace existing one
						populate_oracle(HDX, DOT, route2_cheaper.clone(), Some(11), None);
						assert_ok!(Router::set_route(
							hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
							asset_pair,
							route2_cheaper.clone()
						));
						assert_eq!(Router::route(asset_pair).unwrap(), route2_cheaper);

						//We try to set back the more expensive but did not replace
						assert_noop!(
							Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
							pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
						);
						assert_eq!(Router::route(asset_pair).unwrap(), route2_cheaper);
						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});
				});
			}
		}

		#[test]
		fn set_route_should_fail_with_invalid_route() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					3000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(ETH, 1000 * UNITS, BTC, 1000 * UNITS);

				let route1 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: ETH,
						asset_out: BTC,
					},
				];

				let asset_pair = Pair::new(HDX, BTC);

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
				);
			});
		}

		#[test]
		fn set_route_should_work_when_stored_route_is_broken_due_to_not_existing_route() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000000000000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 50000 * UNITS, BTC, 4000000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route.clone(), None, None);

				//Act and assert
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route
				),);
			});
		}

		#[test]
		fn set_route_should_work_when_stored_route_is_broken_due_to_frozen_asset() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000000000000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
				create_xyk_pool_with_amounts(DOT, 50000 * UNITS, BTC, 4000000 * UNITS);

				let route1 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, BTC);

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1
				));

				assert_ok!(Omnipool::set_asset_tradable_state(
					RuntimeOrigin::root(),
					DOT,
					Tradability::FROZEN
				));

				let route = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route.clone(), Some(11), None);

				//Act and assert
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route
				),);
			});
		}

		#[test]
		fn set_route_should_fail_when_new_normal_route_is_invalid() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, BTC, 1000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route.clone(), None, None);

				assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					Tradability::SELL
				));

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route),
					pallet_omnipool::Error::<Runtime>::NotAllowed
				);
			});
		}

		#[test]
		fn set_route_should_fail_when_new_inverse_route_is_invalid() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000000000000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 10000000 * UNITS, BTC, 10000000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route.clone(), None, None);

				assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					Tradability::BUY
				));

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route),
					pallet_omnipool::Error::<Runtime>::NotAllowed
				);
			});
		}

		#[test]
		fn invalid_new_normal_route_should_be_revalidated_with_lowest_liquidity_of_assets() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					10000000000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 10 * UNITS, BTC, 1000000 * UNITS);

				let asset_pair = Pair::new(DAI, BTC);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DAI,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				//To prevent ED error
				assert_ok!(hydradx_runtime::Tokens::set_balance(
					RawOrigin::Root.into(),
					DAVE.into(),
					BTC,
					1 * UNITS,
					0,
				));

				populate_oracle(DAI, BTC, route.clone(), None, Some(90 * UNITS));

				//Act and assert
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route
				),);
			});
		}

		#[test]
		fn invalid_new_inverse_route_should_be_revalidated_with_lowest_liquidity_of_assets() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 100000000000 * UNITS, BTC, 100000000000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route.clone(), None, None);

				//Act and assert
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route
				),);
			});
		}

		#[test]
		fn set_should_work_when_omnipool_route_does_not_exist_for_pair() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
				create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, BTC, 1000000 * UNITS);

				let route1 = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, BTC);

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1
				));
			});
		}

		#[test]
		fn set_route_should_not_work_when_route_has_insufficient_asset_without_oracle() {
			{
				TestNet::reset();

				Hydra::execute_with(|| {
					let _ = with_transaction(|| {
						let name = b"INSUF1".to_vec();
						let insufficient_asset = AssetRegistry::register_insufficient_asset(
							None,
							Some(name.try_into().unwrap()),
							AssetKind::External,
							Some(1_000),
							None,
							None,
							None,
							None,
						)
						.unwrap();

						let route1 = vec![Trade {
							pool: PoolType::XYK,
							asset_in: DOT,
							asset_out: insufficient_asset,
						}];

						create_xyk_pool_with_amounts(DOT, 10000 * UNITS, insufficient_asset, 10000 * UNITS);

						//Act
						assert_noop!(
							Router::set_route(
								hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
								Pair::new(DOT, insufficient_asset),
								route1.clone()
							),
							pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteHasNoOracle
						);

						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});
				});
			}
		}

		#[test]
		fn set_route_should_work_when_route_has_insufficient_asset_with_oracle() {
			{
				TestNet::reset();

				Hydra::execute_with(|| {
					let _ = with_transaction(|| {
						let name = b"INSUF1".to_vec();
						let insufficient_asset = AssetRegistry::register_insufficient_asset(
							None,
							Some(name.try_into().unwrap()),
							AssetKind::External,
							Some(1_000),
							None,
							None,
							None,
							None,
						)
						.unwrap();

						let route1 = vec![Trade {
							pool: PoolType::XYK,
							asset_in: DOT,
							asset_out: insufficient_asset,
						}];

						create_xyk_pool_with_amounts(DOT, 10000 * UNITS, insufficient_asset, 10000 * UNITS);

						//Whitelist insufficient asset in oracle
						EmaOracle::add_oracle(
							hydradx_runtime::RuntimeOrigin::root(),
							XYK_SOURCE,
							(DOT, insufficient_asset),
						)
						.unwrap();

						populate_oracle(DOT, insufficient_asset, route1.clone(), None, Some(10 * UNITS));

						//Act
						assert_ok!(Router::set_route(
							hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
							Pair::new(DOT, insufficient_asset),
							route1.clone()
						),);

						TransactionOutcome::Commit(DispatchResult::Ok(()))
					});
				});
			}
		}
	}

	mod when_prestored_route_is_valid {
		use super::*;

		#[test]
		fn set_route_should_fail_with_trying_to_override_with_invalid_route() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					3000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, BTC, 1000 * UNITS);

				let route1 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, BTC);

				//Act and assert
				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1
				));

				create_xyk_pool_with_amounts(ETH, 1000 * UNITS, BTC, 1000 * UNITS);

				let invalid_route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: ETH,
						asset_out: BTC,
					},
				];
				assert_noop!(
					Router::set_route(
						hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
						asset_pair,
						invalid_route
					),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::InvalidRoute
				);
			});
		}

		#[test]
		fn set_route_should_not_work_when_new_route_is_invalid() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					3000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(HDX, 1000000 * UNITS, DOT, 1000000 * UNITS);
				create_xyk_pool_with_amounts(DOT, 1000000 * UNITS, BTC, 1000000 * UNITS);

				let route1 = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, BTC);

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1
				));

				assert_ok!(XYK::remove_liquidity(
					RuntimeOrigin::signed(DAVE.into()),
					HDX,
					DOT,
					1000000 * UNITS
				));

				let route2 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					Tradability::FROZEN
				));

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route2),
					pallet_omnipool::Error::<hydradx_runtime::Runtime>::NotAllowed
				);
			});
		}
		#[test]
		fn set_route_should_fail_when_new_normal_route_is_invalid() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(HDX, 1000 * UNITS, DOT, 1000 * UNITS);
				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, BTC, 1000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let prestored_route = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, prestored_route.clone(), None, None);

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					prestored_route
				),);

				let route2 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					Tradability::SELL
				));

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route2),
					pallet_omnipool::Error::<Runtime>::NotAllowed
				);
			});
		}

		#[test]
		fn set_route_should_fail_when_new_inverse_route_is_invalid() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(HDX, 1000 * UNITS, DOT, 1000 * UNITS);
				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, BTC, 1000 * UNITS);

				let asset_pair = Pair::new(HDX, BTC);

				let prestored_route = vec![
					Trade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				populate_oracle(HDX, BTC, prestored_route.clone(), None, None);

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					prestored_route
				),);

				let route2 = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DOT,
						asset_out: BTC,
					},
				];

				assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					Tradability::BUY
				));

				//Act and assert
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route2),
					pallet_omnipool::Error::<Runtime>::NotAllowed
				);
			});
		}

		#[test]
		fn invalid_new_normal_route_should_be_revalidated_with_other_asset_liquidity() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					BTC,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					BTC,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					ETH,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					ETH,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(BTC, 1000 * UNITS, DAI, 1000 * UNITS);
				create_xyk_pool_with_amounts(DAI, 1000 * UNITS, ETH, 1000 * UNITS);

				let asset_pair = Pair::new(BTC, ETH);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: BTC,
						asset_out: DAI,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DAI,
						asset_out: ETH,
					},
				];

				populate_oracle(BTC, ETH, route.clone(), None, Some(UNITS / 1000000));

				//Validation is fine so no AMM error, but since the route is not better, it results in unsuccessfull route setting
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
				);
			});
		}

		#[test]
		fn invalid_new_inversed_route_should_be_revalidated_with_other_asset_liquidity() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					BTC,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					BTC,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					ETH,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					ETH,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(BTC, 1000 * UNITS, DAI, 1000 * UNITS);
				create_xyk_pool_with_amounts(DAI, 10000000 * UNITS, ETH, 10000000 * UNITS);

				let asset_pair = Pair::new(BTC, ETH);

				let route = vec![
					Trade {
						pool: PoolType::Omnipool,
						asset_in: BTC,
						asset_out: DAI,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: DAI,
						asset_out: ETH,
					},
				];

				populate_oracle(BTC, ETH, route.clone(), None, Some(UNITS / 100));

				//Validation is fine, but since the route is not better, it results in unsuccessfull route setting
				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
				);
			});
		}

		#[test]
		fn set_route_should_not_work_when_setting_default_omni_route_again() {
			TestNet::reset();

			Hydra::execute_with(|| {
				//Arrange
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					DOT,
					1000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					DOT,
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				let route1 = vec![Trade {
					pool: PoolType::Omnipool,
					asset_in: HDX,
					asset_out: DOT,
				}];

				populate_oracle(HDX, DOT, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, DOT);

				assert_noop!(
					Router::set_route(hydradx_runtime::RuntimeOrigin::signed(ALICE.into()), asset_pair, route1),
					pallet_route_executor::Error::<hydradx_runtime::Runtime>::RouteUpdateIsNotSuccessful
				);
			});
		}
	}
}

mod with_on_chain_and_default_route {
	use super::*;
	use frame_support::assert_ok;

	#[test]
	fn buy_should_work_with_onchain_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

				let route1 = vec![
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
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_1,
						asset_out: DOT,
					},
				];

				populate_oracle(HDX, DOT, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, DOT);
				let amount_to_buy = 100 * UNITS;

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), route1);

				//Act
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					HDX,
					100000 * UNITS as i128,
				));

				assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE);
				assert_ok!(Router::buy(
					RuntimeOrigin::signed(ALICE.into()),
					HDX,
					DOT,
					amount_to_buy,
					u128::MAX,
					vec![],
				));

				assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE + amount_to_buy);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_onchain_route() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

				let route1 = vec![
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
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_1,
						asset_out: DOT,
					},
				];

				populate_oracle(HDX, DOT, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, DOT);
				let amount_to_sell = 100 * UNITS;

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), route1);

				//Act
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(ALICE.into()),
					HDX,
					DOT,
					amount_to_sell,
					0,
					vec![],
				));

				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_onchain_route_but_used_in_reversed_order() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
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
					FixedU128::from_rational(1, 2),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				create_xyk_pool_with_amounts(DOT, 1000 * UNITS, stable_asset_1, 1000 * UNITS);

				let route1 = vec![
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
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_1,
						asset_out: DOT,
					},
				];

				populate_oracle(HDX, DOT, route1.clone(), None, None);

				let asset_pair = Pair::new(HDX, DOT);
				let amount_to_sell = 100 * UNITS;

				assert_ok!(Router::set_route(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					asset_pair,
					route1.clone()
				));
				assert_eq!(Router::route(asset_pair).unwrap(), route1);

				//Act
				assert_balance!(ALICE.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE);
				assert_ok!(Router::sell(
					RuntimeOrigin::signed(ALICE.into()),
					DOT,
					HDX,
					amount_to_sell,
					0,
					vec![],
				));

				assert_balance!(ALICE.into(), DOT, ALICE_INITIAL_DOT_BALANCE - amount_to_sell);
				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn sell_should_work_with_default_omnipool_when_no_onchain_or_specified_route_present() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_sell = 10 * UNITS;
			let limit = 0;

			//Act
			assert_ok!(Router::sell(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_sell,
				limit,
				vec![]
			));

			//Assert
			let amount_out = 266_195_070_030_573_798;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_out);

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: amount_to_sell,
				amount_out,
				event_id: 0,
			}
			.into()]);
		});
	}

	#[test]
	fn buy_should_work_default_omni_route_when_no_onchain_or_specified_route_present() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			init_omnipool();

			let amount_to_buy = UNITS;
			let limit = 100 * UNITS;

			//Act
			assert_ok!(Router::buy(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				amount_to_buy,
				limit,
				vec![]
			));

			//Assert
			let amount_in = 37_565_544;

			assert_balance!(BOB.into(), HDX, BOB_INITIAL_NATIVE_BALANCE - amount_in);
			assert_balance!(BOB.into(), DAI, BOB_INITIAL_DAI_BALANCE + amount_to_buy);

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in,
				amount_out: amount_to_buy,
				event_id: 0,
			}
			.into()]);
		});
	}
}

mod route_spot_price {
	use super::*;
	use hydradx_traits::router::PoolType;
	use sp_runtime::FixedU128;

	#[test]
	fn spot_price_should_be_ok_for_lbp() {
		TestNet::reset();

		Hydra::execute_with(|| {
			//Arrange
			create_lbp_pool(HDX, DOT);

			set_relaychain_block_number(LBP_SALE_START + 7);

			let amount_to_sell = 1 * UNITS;
			let limit = 0;
			let trades = vec![Trade {
				pool: PoolType::LBP,
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
				trades.clone()
			));

			//Assert
			let amount_out = 1_022562572986; //+7 blocks

			assert_balance!(BOB.into(), HDX, ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell);
			assert_balance!(BOB.into(), DOT, amount_out);

			let spot_price_of_hdx_per_dot = Router::spot_price_with_fee(&trades).unwrap();
			let calculated_amount_out = spot_price_of_hdx_per_dot
				.reciprocal()
				.unwrap()
				.checked_mul_int(amount_to_sell)
				.unwrap();
			let difference = amount_out - calculated_amount_out;
			let relative_difference = FixedU128::from_rational(difference, amount_out);
			let tolerated_difference = FixedU128::from_rational(1, 100);
			// The difference of the amount out calculated with spot price should be less than 1%
			assert!(relative_difference < tolerated_difference);
			//assert_eq!(relative_difference, FixedU128::from_float(0.009468191066027364)); //TEMP assertion
		});
	}

	#[test]
	fn route_should_have_spot_price_for_all_pools() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				create_lbp_pool(HDX, DAI);
				assert_eq!(
					hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
					ALICE_INITIAL_NATIVE_BALANCE
				);
				let (pool_id, stable_asset_1, stable_asset_2) = init_stableswap().unwrap();
				init_omnipool();
				create_xyk_pool_with_amounts(stable_asset_2, 1000 * UNITS, DOT, 1000 * UNITS);

				set_relaychain_block_number(LBP_SALE_START + 7);

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					Omnipool::protocol_account(),
					stable_asset_1,
					3000 * UNITS as i128,
				));

				assert_ok!(hydradx_runtime::Omnipool::add_token(
					hydradx_runtime::RuntimeOrigin::root(),
					stable_asset_1,
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
					AccountId::from(BOB),
				));

				let trades = vec![
					Trade {
						pool: PoolType::LBP,
						asset_in: HDX,
						asset_out: DAI,
					},
					Trade {
						pool: PoolType::Omnipool,
						asset_in: DAI,
						asset_out: stable_asset_1,
					},
					Trade {
						pool: PoolType::Stableswap(pool_id),
						asset_in: stable_asset_1,
						asset_out: stable_asset_2,
					},
					Trade {
						pool: PoolType::XYK,
						asset_in: stable_asset_2,
						asset_out: DOT,
					},
				];
				let amount_to_sell = 1 * UNITS;

				//Act
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					DOT,
					amount_to_sell,
					0,
					trades.clone()
				));

				//Assert
				let expected_amount_out = 1765376;

				assert_eq!(
					hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
					ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
				);

				assert_eq!(
					hydradx_runtime::Currencies::free_balance(DOT, &AccountId::from(ALICE)),
					ALICE_INITIAL_DOT_BALANCE + expected_amount_out
				);

				let spot_price_of_hdx_per_dot = Router::spot_price_with_fee(&trades).unwrap();
				let calculated_amount_out = spot_price_of_hdx_per_dot
					.reciprocal()
					.unwrap()
					.checked_mul_int(amount_to_sell)
					.unwrap();
				let difference = if calculated_amount_out > expected_amount_out {
					calculated_amount_out - expected_amount_out
				} else {
					expected_amount_out - calculated_amount_out
				};
				let relative_difference = FixedU128::from_rational(difference, expected_amount_out);
				let tolerated_difference = FixedU128::from_rational(1, 100);
				// The difference of the amount out calculated with spot price should be less than 1%
				assert!(relative_difference < tolerated_difference);
				//assert_eq!(relative_difference, FixedU128::from_float(0.002541101725638051)); //TEMP assertion

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn route_should_have_spot_price_when_stable_share_asset_included() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, _) =
					init_stableswap_with_details(1_000_000_000_000_000_000u128, 300_000_000_000_000_000u128, 12)
						.unwrap();
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
					FixedU128::from_inner(25_650_000_000_000_000),
					Permill::from_percent(1),
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
				let amount_to_sell = 1 * UNITS;

				//Act
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					HDX,
					stable_asset_1,
					amount_to_sell,
					0,
					trades.clone()
				));

				//Assert
				let expected_amount_out = 46467;

				assert_eq!(
					hydradx_runtime::Balances::free_balance(AccountId::from(ALICE)),
					ALICE_INITIAL_NATIVE_BALANCE - amount_to_sell
				);

				assert_eq!(
					hydradx_runtime::Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)),
					expected_amount_out
				);

				let spot_price_of_hdx_per_dot = Router::spot_price_with_fee(&trades).unwrap();
				let calculated_amount_out = spot_price_of_hdx_per_dot
					.reciprocal()
					.unwrap()
					.checked_mul_int(amount_to_sell)
					.unwrap();
				let difference = if expected_amount_out > calculated_amount_out {
					expected_amount_out - calculated_amount_out
				} else {
					calculated_amount_out - expected_amount_out
				};
				let relative_difference = FixedU128::from_rational(difference, expected_amount_out);
				let tolerated_difference = FixedU128::from_rational(1, 100);
				// The difference of the amount out calculated with spot price should be less than 1%
				//assert_eq!(relative_difference, FixedU128::from_float(0.002991370219725827)); //TEMP assertion
				assert!(relative_difference < tolerated_difference);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}

	#[test]
	fn route_should_have_spot_price_when_only_stable_share_asset_included() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, _) =
					init_stableswap_with_details(1_000_000_000_000_000_000u128, 300_000_000_000_000_000u128, 12)
						.unwrap();
				init_omnipool();

				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					pool_id,
					3000 * UNITS as i128,
				));

				let trades = vec![Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: pool_id,
					asset_out: stable_asset_1,
				}];
				let amount_to_sell = 1 * UNITS;

				//Act
				assert_ok!(Router::sell(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					pool_id,
					stable_asset_1,
					amount_to_sell,
					0,
					trades.clone()
				));

				//Assert
				let expected_amount_out = 994999;

				assert_eq!(
					hydradx_runtime::Currencies::free_balance(pool_id, &AccountId::from(ALICE)),
					3000 * UNITS - amount_to_sell
				);

				assert_eq!(
					hydradx_runtime::Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)),
					expected_amount_out
				);

				let spot_price_of_hdx_per_dot = Router::spot_price_with_fee(&trades).unwrap();
				let calculated_amount_out = spot_price_of_hdx_per_dot
					.reciprocal()
					.unwrap()
					.checked_mul_int(amount_to_sell)
					.unwrap();
				let difference = if expected_amount_out > calculated_amount_out {
					expected_amount_out - calculated_amount_out
				} else {
					calculated_amount_out - expected_amount_out
				};
				let relative_difference = FixedU128::from_rational(difference, expected_amount_out);
				let tolerated_difference = FixedU128::from_rational(1, 100);
				// The difference of the amount out calculated with spot price should be less than 1%
				//assert_eq!(relative_difference, FixedU128::from_float(0.003019098511656796)); //TEMP assertion
				assert!(relative_difference < tolerated_difference);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

mod sell_all {
	use super::*;
	use hydradx_runtime::Currencies;
	use hydradx_traits::router::PoolType;

	#[test]
	fn sell_should_sell_all_user_native_balance() {
		TestNet::reset();

		let limit = 0;
		let amount_out = 26577363534770086553;

		Hydra::execute_with(|| {
			let bob_hdx_balance = Currencies::free_balance(HDX, &BOB.into());

			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: DAI,
			}];

			//Act
			assert_ok!(Router::sell_all(
				RuntimeOrigin::signed(BOB.into()),
				HDX,
				DAI,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), HDX, 0);

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: bob_hdx_balance,
				amount_out,
				event_id: 0,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_all_should_sell_all_user_nonnative_balance() {
		TestNet::reset();

		let limit = 0;
		let amount_out = 35227901268414708;

		Hydra::execute_with(|| {
			let bob_nonnative_balance = Currencies::free_balance(DAI, &BOB.into());

			//Arrange
			init_omnipool();

			let trades = vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: DAI,
				asset_out: HDX,
			}];

			//Act
			assert_ok!(Router::sell_all(
				RuntimeOrigin::signed(BOB.into()),
				DAI,
				HDX,
				limit,
				trades
			));

			//Assert
			assert_balance!(BOB.into(), DAI, 0);

			expect_hydra_last_events(vec![pallet_route_executor::Event::Executed {
				asset_in: DAI,
				asset_out: HDX,
				amount_in: bob_nonnative_balance,
				amount_out,
				event_id: 0,
			}
			.into()]);
		});
	}

	#[test]
	fn sell_all_should_work_when_selling_all_nonnative_in_stableswap() {
		TestNet::reset();

		Hydra::execute_with(|| {
			let _ = with_transaction(|| {
				//Arrange
				let (pool_id, stable_asset_1, _) = init_stableswap().unwrap();

				init_omnipool();

				let init_balance = 3000 * UNITS + 1;
				assert_ok!(Currencies::update_balance(
					hydradx_runtime::RuntimeOrigin::root(),
					ALICE.into(),
					stable_asset_1,
					init_balance as i128,
				));

				let trades = vec![Trade {
					pool: PoolType::Stableswap(pool_id),
					asset_in: stable_asset_1,
					asset_out: pool_id,
				}];

				assert_balance!(ALICE.into(), pool_id, 0);

				//Act
				assert_ok!(Router::sell_all(
					hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
					stable_asset_1,
					pool_id,
					0,
					trades
				));

				//Assert
				assert_eq!(
					hydradx_runtime::Currencies::free_balance(stable_asset_1, &AccountId::from(ALICE)),
					0
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});
	}
}

pub fn create_lbp_pool(accumulated_asset: u32, distributed_asset: u32) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		accumulated_asset,
		1000 * UNITS as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		distributed_asset,
		1000 * UNITS as i128,
	));
	assert_ok!(LBP::create_pool(
		RuntimeOrigin::root(),
		DAVE.into(),
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
		RuntimeOrigin::signed(DAVE.into()),
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
	let asset_pair = pallet_lbp::AssetPair {
		asset_in: asset_a,
		asset_out: asset_b,
	};
	LBP::get_pair_id(asset_pair)
}

fn start_lbp_campaign() {
	set_relaychain_block_number(LBP_SALE_START + 1);
}

pub fn create_xyk_pool(asset_a: u32, asset_b: u32) {
	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(ALICE.into()),
		asset_a,
		100 * UNITS,
		asset_b,
		50 * UNITS,
	));
}

fn create_xyk_pool_with_amounts(asset_a: u32, amount_a: u128, asset_b: u32, amount_b: u128) {
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_a,
		amount_a as i128,
	));
	assert_ok!(Currencies::update_balance(
		hydradx_runtime::RuntimeOrigin::root(),
		DAVE.into(),
		asset_b,
		amount_b as i128,
	));

	assert_ok!(XYK::create_pool(
		RuntimeOrigin::signed(DAVE.into()),
		asset_a,
		amount_a,
		asset_b,
		amount_b,
	));
}

pub fn init_stableswap() -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let initial_liquidity = 1_000_000_000_000_000u128;
	let liquidity_added = 300_000_000_000_000u128;

	init_stableswap_with_details(initial_liquidity, liquidity_added, 18)
}

pub fn init_stableswap_with_details(
	initial_liquidity: Balance,
	liquidity_added: Balance,
	decimals: u8,
) -> Result<(AssetId, AssetId, AssetId), DispatchError> {
	let mut initial: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> = vec![];
	let mut added_liquidity: Vec<AssetAmount<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId>> =
		vec![];

	let mut asset_ids: Vec<<hydradx_runtime::Runtime as pallet_stableswap::Config>::AssetId> = Vec::new();
	for idx in 0u32..MAX_ASSETS_IN_POOL {
		let name: Vec<u8> = idx.to_ne_bytes().to_vec();
		let asset_id = AssetRegistry::register_sufficient_asset(
			None,
			Some(name.try_into().unwrap()),
			AssetKind::Token,
			1000u128,
			Some(b"xDUM".to_vec().try_into().unwrap()),
			Some(decimals),
			None,
			None,
		)?;
		asset_ids.push(asset_id);

		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(BOB),
			asset_id,
			initial_liquidity as i128,
		)?;
		Currencies::update_balance(
			hydradx_runtime::RuntimeOrigin::root(),
			AccountId::from(CHARLIE),
			asset_id,
			initial_liquidity as i128,
		)?;

		initial.push(AssetAmount::new(asset_id, initial_liquidity));
		added_liquidity.push(AssetAmount::new(asset_id, liquidity_added));
	}
	let pool_id = AssetRegistry::register_sufficient_asset(
		None,
		Some(b"pool".to_vec().try_into().unwrap()),
		AssetKind::Token,
		1u128,
		None,
		None,
		None,
		None,
	)?;

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

	Stableswap::add_liquidity(
		hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
		pool_id,
		BoundedVec::truncate_from(initial),
	)?;

	Ok((pool_id, asset_in, asset_out))
}

fn populate_oracle(
	asset_in: AssetId,
	asset_out: AssetId,
	route: Vec<Trade<u32>>,
	block: Option<BlockNumber>,
	amount: Option<u128>,
) {
	assert_ok!(hydradx_runtime::Tokens::set_balance(
		RawOrigin::Root.into(),
		DAVE.into(),
		asset_in,
		amount.unwrap_or(100 * UNITS),
		0,
	));
	assert_ok!(Router::sell(
		hydradx_runtime::RuntimeOrigin::signed(DAVE.into()),
		asset_in,
		asset_out,
		amount.unwrap_or(1 * UNITS),
		0,
		route.clone()
	));
	set_relaychain_block_number(block.unwrap_or(10));
}
