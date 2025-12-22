use crate::tests::mock::*;
use crate::types::Solution;
use crate::types::Trade;
use crate::types::TradeType;
use crate::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
use hydra_dx_math::types::Ratio;
use pallet_intent::types::Intent;
use pallet_intent::types::IntentKind;
use pallet_intent::types::SwapData;
use pallet_intent::types::SwapType;
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s, score, 1));
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_010_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
				Error::<Test>::ScoreMismatch
			);
		});
}

#[test]
fn solution_execution_should_not_work_when_duplicate_clearing_price_exists() {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
				Error::<Test>::DuplicateClearingPrice
			);
		});
}

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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 2),
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			intents.push(intents[0].clone());

			assert_eq!(intents.len(), 4);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: ETH,
						asset_out: HDX,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
			];

			let s = Solution {
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[0].0, intents[0].1.clone(), trades[0].clone()), //duplicate intent
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 0,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(intents[2].0, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(HDX, Ratio { n: 177, d: 0 }),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
					kind: IntentKind::Swap(SwapData {
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
			TradeType::Sell,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Sell,
			PoolType::Omnipool,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			9 * ONE_DOT,
		)
		.with_router_settlement(
			TradeType::Buy,
			PoolType::Omnipool,
			ETH,
			HDX,
			16_000_000 * ONE_HDX,
			ONE_QUINTIL / 2,
			16_000_000 * ONE_HDX,
		)
		.build()
		.execute_with(|| {
			let mut intents = Intents::get_valid_intents();
			intents.reverse();

			assert_eq!(intents.len(), 3);
			let trades = vec![
				Trade {
					amount_in: 5_000 * ONE_HDX,
					amount_out: 4 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::XYK,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: 10_000 * ONE_HDX,
					amount_out: 8 * ONE_DOT,
					trade_type: TradeType::Sell,
					route: vec![RTrade {
						pool: PoolType::Omnipool,
						asset_in: HDX,
						asset_out: DOT,
					}]
					.try_into()
					.unwrap(),
				},
				Trade {
					amount_in: ONE_QUINTIL,
					amount_out: 16_000_000 * ONE_HDX,
					trade_type: TradeType::Buy,
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
				resolved: vec![
					(intents[0].0, intents[0].1.clone(), trades[0].clone()),
					(intents[1].0, intents[1].1.clone(), trades[1].clone()),
					(9999999, intents[2].1.clone(), trades[2].clone()),
				],
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						DOT,
						Ratio {
							n: 22_125,
							d: 100_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 28_320_000,
							d: 1_000_000_000_000_000_000,
						},
					),
				],
			};

			let score = 500_000_020_000_000_000_u128;

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s, score, 1),
				Error::<Test>::IntentOwnerNotFound
			);
		});
}
