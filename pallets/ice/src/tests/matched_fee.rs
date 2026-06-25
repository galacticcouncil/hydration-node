use crate::tests::mock::*;
use crate::{Error, IntentData, ResolvedIntent};
use frame_support::assert_noop;
use frame_support::assert_ok;
use ice_support::IntentDataInput;
use ice_support::Partial;
use ice_support::PoolTrade;
use ice_support::Solution;
use ice_support::SwapData;
use ice_support::SwapParams;
use ice_support::SwapType;
use orml_traits::MultiCurrency;
use pallet_intent::types::IntentInput;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

fn swap_intent(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, min_out: Balance) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial: false,
		}),
		deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
		on_resolved: None,
	}
}

/// Two intents that perfectly offset each other (no AMM hop). The matched
/// volume on each asset bears the protocol fee, which is swept to the
/// dedicated `FeeReceiver` account. The user receives the solver's net
/// `amount_out` in full.
#[test]
fn submit_solution_should_sweep_matched_fee_to_fee_receiver_when_intents_offset_exactly() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (BOB, DOT, 10_000 * ONE_DOT)])
		.with_intents(vec![
			// id 0: ALICE sells HDX for DOT
			(ALICE, swap_intent(HDX, DOT, 1_000 * ONE_HDX, 900 * ONE_DOT)),
			// id 1: BOB sells DOT for HDX
			(BOB, swap_intent(DOT, HDX, 1_000 * ONE_DOT, 900 * ONE_HDX)),
		])
		.build()
		.execute_with(|| {
			// 1% matched-volume fee.
			assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

			// Net (post-fee) amounts: gross 1000 each, 1% fee → 990 delivered.
			let net_dot = 990 * ONE_DOT;
			let net_hdx = 990 * ONE_HDX;

			let resolved = vec![
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 1_000 * ONE_HDX,
						amount_out: net_dot,
						partial: Partial::No,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: DOT,
						asset_out: HDX,
						amount_in: 1_000 * ONE_DOT,
						amount_out: net_hdx,
						partial: Partial::No,
					}),
				},
			];

			// surplus = net_out - min_out, summed across intents (raw u128 sum).
			let score = (net_dot - 900 * ONE_DOT) + (net_hdx - 900 * ONE_HDX);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: Default::default(),
				score,
			};

			let pot = ICE::get_pallet_account();
			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			// Users receive the full net amount.
			assert_eq!(Currencies::free_balance(DOT, &ALICE), net_dot);
			assert_eq!(Currencies::free_balance(HDX, &BOB), net_hdx);

			// Fee = 1% of matched volume per asset, swept to the receiver.
			assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 10 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), 10 * ONE_DOT);

			// Holding pot fully drained — nothing stuck.
			assert_eq!(Currencies::free_balance(HDX, &pot), 0);
			assert_eq!(Currencies::free_balance(DOT, &pot), 0);
		});
}

/// A single intent fully routed through the AMM has zero matched volume, so
/// no protocol fee is collected even when the fee rate is non-zero.
#[test]
fn submit_solution_should_collect_no_fee_when_all_volume_amm_routed() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
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
			assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

			let resolved = vec![ResolvedIntent {
				id: 0_u128,
				data: IntentData::Swap(SwapData {
					asset_in: HDX,
					asset_out: DOT,
					amount_in: 5_000 * ONE_HDX,
					amount_out: 5 * ONE_DOT,
					partial: Partial::No,
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
				score: ONE_DOT,
			};

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			// matched = intent_in − pool_in = 0 → no fee collected.
			assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 0);
			assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), 0);
			// User receives the full pool output.
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
		});
}

/// If the solver does not skim the matched-volume fee (delivers the full
/// gross amount), the holding-pot residual cannot cover the expected fee and
/// the solution is rejected with `FeeMismatch`.
#[test]
fn submit_solution_should_fail_when_holding_pot_residual_below_expected_fee() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (BOB, DOT, 10_000 * ONE_DOT)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 1_000 * ONE_HDX, 900 * ONE_DOT)),
			(BOB, swap_intent(DOT, HDX, 1_000 * ONE_DOT, 900 * ONE_HDX)),
		])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

			// Solver delivers the FULL gross amount — no fee skimmed. The pot
			// has no residual to cover the 1% matched fee.
			let resolved = vec![
				ResolvedIntent {
					id: 0_u128,
					data: IntentData::Swap(SwapData {
						asset_in: HDX,
						asset_out: DOT,
						amount_in: 1_000 * ONE_HDX,
						amount_out: 1_000 * ONE_DOT,
						partial: Partial::No,
					}),
				},
				ResolvedIntent {
					id: 1_u128,
					data: IntentData::Swap(SwapData {
						asset_in: DOT,
						asset_out: HDX,
						amount_in: 1_000 * ONE_DOT,
						amount_out: 1_000 * ONE_HDX,
						partial: Partial::No,
					}),
				},
			];

			let score = (1_000 * ONE_DOT - 900 * ONE_DOT) + (1_000 * ONE_HDX - 900 * ONE_HDX);

			let s = Solution {
				resolved_intents: resolved.try_into().unwrap(),
				trades: Default::default(),
				score,
			};

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s),
				Error::<Test>::FeeMismatch
			);
		});
}
