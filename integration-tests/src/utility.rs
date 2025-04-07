#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use pallet_broadcast::types::Destination;

use crate::assert_balance;
use hydradx_runtime::LBP;
use hydradx_runtime::XYK;
use hydradx_runtime::{Currencies, Omnipool, Runtime};
use hydradx_runtime::{RuntimeCall, Utility};
use hydradx_traits::router::PoolType;
use pallet_broadcast::types::Asset;
use xcm_emulator::TestExt;

use hydradx_traits::router::Trade;
use hydradx_traits::AMM;
use orml_traits::MultiCurrency;
use pallet_broadcast::types::ExecutionType;
use pallet_broadcast::types::Fee;
use sp_core::bounded_vec::BoundedVec;
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
			route: BoundedVec::truncate_from(trades.clone()),
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
				pallet_broadcast::Event::<Runtime>::Swapped {
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
								.fee_collector
						),
					)],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				},
				pallet_broadcast::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4687619499466)],
					fees: vec![Fee::new(
						HDX,
						7041992238,
						Destination::Account(Omnipool::protocol_account())
					)],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				},
				pallet_broadcast::Event::<Runtime>::Swapped {
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
					inputs: vec![Asset::new(HDX, 4687619499466)],
					outputs: vec![Asset::new(DOT, 2232143907425)],
					fees: vec![Fee::new(
						DOT,
						6716581464,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				}
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
		crate::router::create_xyk_pool(HDX, DOT);

		let amount_to_sell = UNITS * 10;
		let trades = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: DOT,
		}];
		let router_call = RuntimeCall::Router(pallet_route_executor::Call::sell {
			asset_in: HDX,
			asset_out: DOT,
			amount_in: amount_to_sell,
			min_amount_out: 0,
			route: trades.clone().try_into().unwrap(),
		});

		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![router_call.clone()]
		));

		//Act
		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![router_call.clone()]
		));

		//Assert
		pretty_assertions::assert_eq!(
			*get_last_swapped_events().last().unwrap(),
			pallet_broadcast::Event::<Runtime>::Swapped {
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
				inputs: vec![Asset::new(HDX, amount_to_sell)],
				outputs: vec![Asset::new(DOT, 3777648106062)],
				fees: vec![Fee::new(
					DOT,
					11367045453,
					Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
						asset_in: HDX,
						asset_out: DOT,
					})),
				)],
				operation_stack: vec![ExecutionType::Batch(2), ExecutionType::Router(3)],
			}
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
				route: BoundedVec::truncate_from(trades.clone()),
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
				pallet_broadcast::Event::<Runtime>::Swapped {
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
								.fee_collector
						),
					)],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				},
				pallet_broadcast::Event::<Runtime>::Swapped {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4687619499466)],
					fees: vec![Fee::new(
						HDX,
						7041992238,
						Destination::Account(Omnipool::protocol_account())
					)],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				},
				pallet_broadcast::Event::<Runtime>::Swapped {
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
					inputs: vec![Asset::new(HDX, 4687619499466)],
					outputs: vec![Asset::new(DOT, 2232143907425)],
					fees: vec![Fee::new(
						DOT,
						6716581464,
						Destination::Account(XYK::get_pair_id(pallet_xyk::types::AssetPair {
							asset_in: HDX,
							asset_out: DOT,
						})),
					)],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				}
			]
		);
	});
}

fn start_lbp_campaign() {
	set_relaychain_block_number(crate::router::LBP_SALE_START + 1);
}
