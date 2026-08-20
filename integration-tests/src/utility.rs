#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use pallet_broadcast::types::Destination;

use crate::assert_balance;
use hydradx_runtime::LBP;
use hydradx_runtime::XYK;
use hydradx_runtime::{Broadcast, Currencies, FeeProcessor, Omnipool, Router, Runtime};
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
				pallet_broadcast::Event::<Runtime>::Swapped3 {
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
				pallet_broadcast::Event::<Runtime>::Swapped3 {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4682924837974)],
					fees: vec![
						Fee::new(HDX, 6455159552, Destination::Account(Omnipool::protocol_account())),
						Fee::new(HDX, 5281494178, Destination::Account(FeeProcessor::pot_account_id())),
					],
					operation_stack: vec![ExecutionType::Batch(0), ExecutionType::Router(1)],
				},
				pallet_broadcast::Event::<Runtime>::Swapped3 {
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
			pallet_broadcast::Event::<Runtime>::Swapped3 {
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
				pallet_broadcast::Event::<Runtime>::Swapped3 {
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
				pallet_broadcast::Event::<Runtime>::Swapped3 {
					swapper: BOB.into(),
					filler: Omnipool::protocol_account(),
					filler_type: pallet_broadcast::types::Filler::Omnipool,
					operation: pallet_broadcast::types::TradeOperation::ExactIn,
					inputs: vec![Asset::new(LRNA, 5640664064)],
					outputs: vec![Asset::new(HDX, 4682924837974)],
					fees: vec![
						Fee::new(HDX, 6455159552, Destination::Account(Omnipool::protocol_account())),
						Fee::new(HDX, 5281494178, Destination::Account(FeeProcessor::pot_account_id())),
					],
					operation_stack: vec![
						ExecutionType::Batch(0),
						ExecutionType::Batch(1),
						ExecutionType::Router(2)
					],
				},
				pallet_broadcast::Event::<Runtime>::Swapped3 {
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
	go_to_block(crate::router::LBP_SALE_START + 1);
}

// An inner call that always fails and never touches the broadcast execution context, so a batch
// built from it exercises nothing but the batch's own context handling.
fn failing_batch_call() -> RuntimeCall {
	RuntimeCall::Currencies(pallet_currencies::Call::transfer {
		dest: ALICE.into(),
		currency_id: DOT,
		amount: u128::MAX, // more than BOB can ever hold -> inner dispatch returns Err
	})
}

fn xyk_hdx_dot_route() -> hydradx_traits::router::Route<hydradx_runtime::AssetId> {
	BoundedVec::truncate_from(vec![Trade {
		pool: PoolType::XYK,
		asset_in: HDX,
		asset_out: DOT,
	}])
}

/// A batch releases its execution context at the batch boundary, so a later unrelated trade in
/// the same block records only its own provenance.
#[test]
fn batch_should_release_execution_context_when_an_item_fails() {
	TestNet::reset();

	Hydra::execute_with(|| {
		//Arrange
		init_omnipool();
		crate::router::create_xyk_pool(HDX, DOT);
		assert_eq!(Broadcast::execution_context().to_vec(), vec![]);

		//Act 1 — a batch whose only inner call fails. `batch` still returns Ok.
		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![failing_batch_call()]
		));

		//Assert 1 — the batch left the stack exactly as it found it.
		assert_eq!(Broadcast::execution_context().to_vec(), vec![]);

		//Act 2 — an unrelated, direct (non-batched) trade later in the same block.
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			HDX,
			DOT,
			10 * UNITS,
			0,
			xyk_hdx_dot_route()
		));

		//Assert 2 — the trade records only its own frame. The id is 1 because the batch consumed
		// id 0 while it held its context.
		let operation_stack = match get_last_swapped_events().last().unwrap().clone() {
			pallet_broadcast::Event::<Runtime>::Swapped3 { operation_stack, .. } => operation_stack,
		};
		assert_eq!(operation_stack, vec![ExecutionType::Router(1)]);
	});
}

/// Baseline for the test above: with no batch in the block at all, the same trade records the
/// same single frame at id 0.
#[test]
fn direct_trade_should_carry_only_its_own_execution_context() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		crate::router::create_xyk_pool(HDX, DOT);

		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			HDX,
			DOT,
			10 * UNITS,
			0,
			xyk_hdx_dot_route()
		));

		let operation_stack = match get_last_swapped_events().last().unwrap().clone() {
			pallet_broadcast::Event::<Runtime>::Swapped3 { operation_stack, .. } => operation_stack,
		};
		assert_eq!(operation_stack, vec![ExecutionType::Router(0)]);
		// A well-behaved trade leaves the stack exactly as it found it.
		assert_eq!(Broadcast::execution_context().to_vec(), vec![]);
	});
}

/// The execution context is a shared per-block stack bounded by `MAX_STACK_SIZE`, so batches
/// must not accumulate frames across a block: repeated failing batches leave it empty and a
/// later trade still acquires its own context.
#[test]
fn repeated_failing_batches_should_not_accumulate_execution_context_frames() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		crate::router::create_xyk_pool(HDX, DOT);

		// One more than the stack can hold, to show depth is independent of batch count.
		for _ in 0..=pallet_broadcast::MAX_STACK_SIZE {
			assert_ok!(Utility::batch(
				hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
				vec![failing_batch_call()]
			));
			assert_eq!(Broadcast::execution_context().to_vec(), vec![]);
		}

		// A trade from an unrelated account still acquires its context and settles normally.
		assert_ok!(Router::sell(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			HDX,
			DOT,
			UNITS,
			0,
			xyk_hdx_dot_route(),
		));
	});
}

/// `Broadcast::on_finalize` clears the shared stack unconditionally, so no execution context
/// carries from one block into the next.
#[test]
fn execution_context_should_be_cleared_at_block_finalization() {
	use frame_support::traits::OnFinalize;

	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();
		crate::router::create_xyk_pool(HDX, DOT);

		assert_ok!(Utility::batch(
			hydradx_runtime::RuntimeOrigin::signed(BOB.into()),
			vec![failing_batch_call()]
		));

		// The executive runs every pallet's `on_finalize` at the block boundary. The test helper
		// `go_to_block` happens not to list Broadcast, so invoke exactly the hook it would.
		let b = hydradx_runtime::System::block_number();
		<hydradx_runtime::Broadcast as OnFinalize<_>>::on_finalize(b);

		assert_eq!(Broadcast::execution_context().to_vec(), vec![]);
	});
}
