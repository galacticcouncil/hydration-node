use crate::tests::mock::*;
use crate::tests::prices_to_map;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use ice_support::AssetId;
use ice_support::PoolTrade;
use ice_support::Price;
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_000_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::ScoreMismatch
			);
		});
}

#[ignore = "This is temporary, unignore when allowing clearing price validation again"]
#[test]
fn solution_execution_should_not_work_when_clearing_price_is_missing() {
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::MissingClearingPrice
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::DuplicateIntent
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_clearing_price_numerator_is_zero() {
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 0,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::InvalidPriceRatio
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_clearing_price_denominator_is_zero() {
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(ETH, Price { n: 177, d: 0 }),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::InvalidPriceRatio
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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
			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
				id: 73786976294838206464000_u128,
				data: IntentData::Swap(SwapData {
					asset_in: 0,
					asset_out: 2,
					amount_in: 5000000000000000,
					amount_out: 50000000000,
					swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
				id: 73786976294838206464000_u128,
				data: IntentData::Swap(SwapData {
					asset_in: 0,
					asset_out: 2,
					amount_in: 5000000000000000,
					amount_out: 50000000000,
					swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 0_u128,
			};

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s, 1));
		});
}

#[test]
fn solution_execution_should_not_work_when_solution_has_to_many_clearing_prices() {
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 4,
						asset_out: 0,
						amount_in: 500000000000000000,
						amount_out: 16000000000000000000,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 10000000000000000,
						amount_out: 100000000000,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: 0,
						asset_out: 2,
						amount_in: 5000000000000000,
						amount_out: 50000000000,
						swap_type: SwapType::ExactIn,
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

			let mut cp: Vec<(AssetId, Price)> = Vec::new();
			for i in 1..=(MAX_NUMBER_OF_RESOLVED_INTENTS * 2) + 1 {
				cp.push((
					i,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				));
			}

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: prices_to_map(cp),
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::ClearingPricesInvalidLength
			);
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500_000_000_000_000_000,
						amount_out: 16_000_000 * ONE_HDX,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: DummyRegistry::existential_deposit(HDX).expect("dummy registry to work") - 1,
						amount_out: 5 * ONE_DOT,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500000000000000000,
						amount_out: 16_000_000 * ONE_HDX,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: DummyRegistry::existential_deposit(DOT).expect("dummy registry to work") - 1,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
			16 * ONE_DOT,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500000000000000000,
						amount_out: 16_000_000 * ONE_HDX,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 10 * ONE_DOT,
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 6 * ONE_DOT,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::PriceInconsistency
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_execution_prices_are_not_consistent() {
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
						swap_type: SwapType::ExactIn,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
					deadline: MAX_INTENT_DEADLINE - ONE_SECOND,
					on_success: None,
					on_failure: None,
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
			16 * ONE_DOT,
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
					id: 73786976294838206464002_u128,
					data: IntentData::Swap(SwapData {
						asset_in: ETH,
						asset_out: HDX,
						amount_in: 500000000000000000,
						amount_out: 16_000_000 * ONE_HDX,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464001_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 10_000 * ONE_HDX,
						amount_out: 8 * ONE_DOT,
						swap_type: SwapType::ExactOut,
						partial: false,
					}),
				},
				ResolvedIntent {
					id: 73786976294838206464000_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 5_000 * ONE_HDX,
						amount_out: 6 * ONE_DOT,
						swap_type: SwapType::ExactIn,
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

			let cp = prices_to_map(vec![
				(
					HDX,
					Price {
						n: 177,
						d: 100_000_000_000_000,
					},
				),
				(
					DOT,
					Price {
						n: 177,
						d: 1_000_000_000,
					},
				),
				(
					ETH,
					Price {
						n: 177,
						d: 3_125_000_000_000,
					},
				),
			]);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: trades.try_into().unwrap(),
				clearing_prices: cp,
				score: 500_000_030_000_000_000_u128,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, 1),
				Error::<Test>::PriceToleranceInconsistency
			);
		});
}
