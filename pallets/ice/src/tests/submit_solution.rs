use crate::tests::mock::*;
use crate::types::Solution;
use crate::types::Trade;
use crate::types::TradeType;
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
				resolved: intents,
				trades: trades,
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
