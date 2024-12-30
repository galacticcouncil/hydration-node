#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;

use crate::assert_balance;
use hydradx_runtime::LBP;
use hydradx_runtime::XYK;
use hydradx_runtime::{Currencies, Omnipool, Runtime, RuntimeEvent};
use hydradx_runtime::{RuntimeCall, Utility};
use hydradx_traits::router::PoolType;
use pallet_amm_support::types::Asset;
use xcm_emulator::TestExt;

use hydradx_traits::router::Trade;
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use pallet_amm_support::types::ExecutionType;
use pallet_amm_support::types::Fee;
#[test]
fn batch_execution_type_should_be_included_in_batch() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		crate::router::create_lbp_pool(DAI, LRNA);
		crate::router::create_xyk_pool(HDX, DOT);

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
		let router_call = RuntimeCall::Router(pallet_route_executor::Call::sell {
			asset_in: DAI,
			asset_out: DOT,
			amount_in: amount_to_sell,
			min_amount_out: limit,
			route: trades.clone(),
		});
		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![router_call.clone()]
		));

		//Assert
		assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);

		let swapped_events = get_last_swapped_events();

		pretty_assertions::assert_eq!(
			swapped_events,
			vec![
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)),
					filler_type: pallet_amm_support::types::Filler::LBP,
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(DAI, 9980000000)],
					outputs: vec![Asset::new(LRNA, 5640664064)],
					fees: vec![Fee::new(
						DAI,
						20000000,
						LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)))
							.unwrap()
							.fee_collector,
					)],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				}),
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_amm_support::types::Filler::Omnipool,
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4682924837974)],
					fees: vec![Fee::new(HDX, 11736653730, Omnipool::protocol_account())],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				}),
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_amm_support::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 4682924837974)],
					outputs: vec![Asset::new(DOT, 2230008413831)],
					fees: vec![Fee::new(
						DOT,
						6710155707,
						XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						}),
					)],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				})
			]
		);
	});
}

#[test]
fn batch_execution_type_should_be_popped_when_multiple_batch_calls_happen() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		crate::router::create_lbp_pool(DAI, LRNA);
		crate::router::create_xyk_pool(HDX, DOT);

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

		let router_call = RuntimeCall::Router(pallet_route_executor::Call::sell {
			asset_in: DAI,
			asset_out: DOT,
			amount_in: amount_to_sell,
			min_amount_out: limit,
			route: trades.clone(),
		});
		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![router_call.clone()]
		));

		//Act
		let trades = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: DOT,
		}];
		let router_call = RuntimeCall::Router(pallet_route_executor::Call::sell {
			asset_in: HDX,
			asset_out: DOT,
			amount_in: amount_to_sell,
			min_amount_out: limit,
			route: trades.clone(),
		});
		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![router_call.clone()]
		));

		//Assert
		pretty_assertions::assert_eq!(
			*get_last_swapped_events().last().unwrap(),
			RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
				swapper: BOB.into(),
				filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
					asset_in: HDX,
					asset_out: DOT,
				}),
				filler_type: pallet_amm_support::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
					pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					},
				))),
				operation: pallet_amm_support::types::TradeOperation::ExactIn,
				inputs: vec![Asset::new(HDX, amount_to_sell)],
				outputs: vec![Asset::new(DOT, 4549178628)],
				fees: vec![Fee::new(
					DOT,
					13688601,
					XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
				)],
				operation_stack: vec![ExecutionType::Batch(2), ExecutionType::Router(3)],
			})
		);
	});
}

#[test]
fn nested_batch_should_represent_embeddedness() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		crate::router::create_lbp_pool(DAI, LRNA);
		crate::router::create_xyk_pool(HDX, DOT);

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

		let sell_via_utility = RuntimeCall::Utility(pallet_utility::Call::batch {
			calls: vec![RuntimeCall::Router(pallet_route_executor::Call::sell {
				asset_in: DAI,
				asset_out: DOT,
				amount_in: amount_to_sell,
				min_amount_out: limit,
				route: trades.clone(),
			})],
		});

		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![sell_via_utility.clone()]
		));

		//Assert
		assert_balance!(BOB.into(), DAI, 1_000_000_000 * UNITS - amount_to_sell);

		let swapped_events = get_last_swapped_events();

		pretty_assertions::assert_eq!(
			swapped_events,
			vec![
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)),
					filler_type: pallet_amm_support::types::Filler::LBP,
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(DAI, 9980000000)],
					outputs: vec![Asset::new(LRNA, 5640664064)],
					fees: vec![Fee::new(
						DAI,
						20000000,
						LBP::pool_data(LBP::get_pair_id(pallet_lbp::types::AssetPair::new(DAI, LRNA)))
							.unwrap()
							.fee_collector,
					)],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				}),
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_amm_support::types::Filler::Omnipool,
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4682924837974)],
					fees: vec![Fee::new(HDX, 11736653730, Omnipool::protocol_account())],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				}),
				RuntimeEvent::AmmSupport(pallet_amm_support::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					}),
					filler_type: pallet_amm_support::types::Filler::XYK(XYK::share_token(XYK::get_pair_id(
						pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						},
					))),
					operation: pallet_amm_support::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(HDX, 4682924837974)],
					outputs: vec![Asset::new(DOT, 2230008413831)],
					fees: vec![Fee::new(
						DOT,
						6710155707,
						XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						}),
					)],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				})
			]
		);
	});
}

fn start_lbp_campaign() {
	set_relaychain_block_number(crate::router::LBP_SALE_START + 1);
}
