use crate::tests::mock::*;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use ice_support::PoolTrade;
use ice_support::Solution;
use ice_support::SwapData;
use ice_support::SwapType;
use pallet_intent::types::Intent;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;

#[test]
fn solution_execution_should_work_when_solution_is_valid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: None,
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL / 2,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			17_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			17_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500_000_000_000_000_000,
						amount_out: 17_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 5 * ONE_DOT,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 17_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 1_000_000_030_000_000_000_u128,
			};

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s, 1));
		});
}

#[test]
fn solution_execution_should_not_work_when_score_is_not_valid() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: None,
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL / 2,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			17_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			17_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500_000_000_000_000_000,
						amount_out: 17_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 5 * ONE_DOT,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 17_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 500_000_000_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::ScoreMismatch
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_solution_is_not_valid_for_current_block() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 16_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 2),
				Error::<Test>::InvalidTargetBlock
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_contains_duplicate_intents() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 0_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::DuplicateIntent
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_intent_owner_is_not_found() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 999999999_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 16_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::IntentOwnerNotFound
			);
		});
}

#[test]
fn solution_execution_should_work_when_solution_has_single_intent() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![ResolvedIntent {
				id: 0_u128,
				data: IntentData::Swap(SwapData {
					asset_in: 0,
					asset_out: 2,
					amount_in: 5000000000000000,
					amount_out: 50000000000,
					partial: false,
				}),
			}];

			let trades = vec![PoolTrade {
				amount_in: 5_000 * ONE_HDX,
				amount_out: 5 * ONE_DOT,
				direction: SwapType::ExactIn,
				route: vec![RTrade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				}]
				.try_into()
				.unwrap(),
			}];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 10_000_000_000_u128,
			};

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s, 1));
		});
}

#[test]
fn solution_execution_should_work_when_solution_has_zero_score() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 5 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![ResolvedIntent {
				id: 0_u128,
				data: IntentData::Swap(SwapData {
					asset_in: 0,
					asset_out: 2,
					amount_in: 5000000000000000,
					amount_out: 50000000000,
					partial: false,
				}),
			}];

			let trades = vec![PoolTrade {
				amount_in: 5_000 * ONE_HDX,
				amount_out: 5 * ONE_DOT,
				direction: SwapType::ExactIn,
				route: vec![RTrade {
					pool: PoolType::XYK,
					asset_in: HDX,
					asset_out: DOT,
				}]
				.try_into()
				.unwrap(),
			}];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 0_u128,
			};

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s, 1));
		});
}

#[test]
fn solution_execution_should_not_work_when_solution_have_intent_with_amount_in_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500_000_000_000_000_000,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: DummyRegistry::existential_deposit(HDX).expect("dummy registry to work") - 1,
						amount_out: 5 * ONE_DOT,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 16_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::InvalidAmount
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_solution_have_intent_with_amount_out_less_than_ed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500000000000000000,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: DummyRegistry::existential_deposit(DOT).expect("dummy registry to work") - 1,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 16_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::InvalidAmount
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_intent_is_not_resolved_at_execution_price() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, HDX, 10_000 * ONE_HDX),
			(ALICE, DOT, 10_000 * ONE_DOT),
			(BOB, HDX, 10_000 * ONE_HDX),
			(BOB, ETH, 10_000 * ONE_QUINTIL),
			(DAVE, HDX, 20_000 * ONE_HDX),
			(DAVE, DOT, 20_000 * ONE_DOT),
		])
		.with_intents(vec![
			(
				ALICE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
			(
				DAVE,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						partial: false,
					}),
					deadline: None,
					on_resolved: None,
				},
			),
			(
				BOB,
				Intent {
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: ONE_QUINTIL / 2,
						amount_out: 16_000_000 * ONE_HDX,
						partial: false,
					}),
					deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
					on_resolved: None,
				},
			),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactOut,
			PoolType::Omnipool,
			ETH,
			HDX,
			17_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			17_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let resolved = vec![
				ResolvedIntent {
					id: 2_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500_000_000_000_000_000,
						amount_out: 17_000_000 * ONE_HDX,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 4 * ONE_DOT,
						partial: false,
					}),
				},
			];

			let trades = vec![
				PoolTrade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 15 * ONE_DOT,
					direction: SwapType::ExactIn,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				PoolTrade {
					amount_in: ONE_QUINTIL / 2,
					amount_out: 17_000_000 * ONE_HDX,
					direction: SwapType::ExactOut,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				score: 1_000_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::PriceInconsistency
			);
		});
}
