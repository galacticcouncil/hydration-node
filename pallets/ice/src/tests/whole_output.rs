use crate::tests::mock::*;
use crate::Error;
use frame_support::assert_noop;
use frame_support::assert_ok;
use hydradx_traits::lazy_executor::ForwardAction;
use hydradx_traits::lazy_executor::Source;
use ice_support::DcaParams;
use ice_support::IntentData;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::Partial;
use ice_support::PoolTrade;
use ice_support::ResolvedIntent;
use ice_support::Score;
use ice_support::Solution;
use ice_support::SwapData;
use ice_support::SwapParams;
use ice_support::SwapType;
use orml_traits::MultiCurrency;
use pallet_intent::types::IntentInput;
use pallet_intent::types::OnResolved;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;
use primitives::EvmAddress;
use sp_runtime::BoundedVec;
use sp_runtime::Permill;

const USDT: AssetId = 10;
const ONE_USDT: Balance = 1_000_000;

const POT_HDX: Balance = 1_234 * ONE_HDX;
const POT_DOT: Balance = 1_234_567 * ONE_DOT;
const POT_ETH: Balance = 7 * ONE_QUINTIL;
const POT_USDT: Balance = 777 * ONE_USDT;

const SOLD_HDX: Balance = 5_000 * ONE_HDX;
const MIN_DOT: Balance = 4 * ONE_DOT;
const CLAIMED_DOT: Balance = 5 * ONE_DOT;
const TRADE_DOT: Balance = 52 * ONE_DOT / 10;

fn swap_intent(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
	partial: bool,
) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Swap(SwapParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			partial,
		}),
		deadline: Some(MAX_INTENT_DEADLINE - ONE_SECOND),
		on_resolved: None,
	}
}

fn dca_intent(
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	min_out: Balance,
	budget: Balance,
) -> IntentInput {
	IntentInput {
		data: IntentDataInput::Dca(DcaParams {
			asset_in,
			asset_out,
			amount_in,
			amount_out: min_out,
			slippage: Permill::zero(),
			budget: Some(budget),
			period: 5,
		}),
		deadline: None,
		on_resolved: None,
	}
}

fn forward() -> OnResolved {
	OnResolved::Forward {
		contract: EvmAddress::repeat_byte(1),
		data: BoundedVec::truncate_from(b"callback".to_vec()),
	}
}

fn sell_trade(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> PoolTrade {
	PoolTrade {
		direction: SwapType::ExactIn,
		amount_in,
		amount_out,
		route: vec![RTrade {
			pool: PoolType::XYK,
			asset_in,
			asset_out,
		}]
		.try_into()
		.expect("single hop route to fit"),
	}
}

fn resolved(
	id: IntentId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
	partial: Partial,
) -> ResolvedIntent {
	ResolvedIntent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial,
		}),
	}
}

fn solution(resolved_intents: Vec<ResolvedIntent>, trades: Vec<PoolTrade>, score: Score) -> Solution {
	Solution::new(
		resolved_intents.try_into().expect("resolved intents to fit"),
		trades.try_into().expect("trades to fit"),
		score,
	)
}

fn pot_seed(asset: AssetId) -> Balance {
	match asset {
		HDX => POT_HDX,
		DOT => POT_DOT,
		ETH => POT_ETH,
		USDT => POT_USDT,
		_ => panic!("no pot seed for asset {asset}"),
	}
}

fn with_seeded_pot(
	mut accounts: Vec<(AccountId, AssetId, Balance)>,
	assets: &[AssetId],
) -> Vec<(AccountId, AssetId, Balance)> {
	let pot = ICE::get_pallet_account();
	accounts.extend(assets.iter().map(|asset| (pot, *asset, pot_seed(*asset))));
	accounts
}

fn pot_balance(asset: AssetId) -> Balance {
	Currencies::free_balance(asset, &ICE::get_pallet_account())
}

fn intent_events() -> Vec<pallet_intent::Event<Test>> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.filter_map(|record| match record.event {
			RuntimeEvent::Intents(e) => Some(e),
			_ => None,
		})
		.collect()
}

fn single_swap_ext(intent: IntentInput, trade_out: Balance) -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(vec![(ALICE, HDX, SOLD_HDX)], &[HDX, DOT]))
		.with_intents(vec![(ALICE, intent)])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			SOLD_HDX,
			SOLD_HDX,
			trade_out,
		)
		.build()
}

fn single_swap_solution(intent_out: Balance) -> Solution {
	solution(
		vec![resolved(0, HDX, DOT, SOLD_HDX, intent_out, Partial::No)],
		vec![sell_trade(HDX, DOT, SOLD_HDX, CLAIMED_DOT)],
		intent_out - MIN_DOT,
	)
}

/// ALICE sells 2_000 HDX for DOT, BOB sells 10 DOT for HDX. 1_000 HDX and 10 DOT are matched, ALICE's other
/// 1_000 HDX are sold for a claimed 10 DOT, and the pool pays 10.5 DOT.
fn opposite_directions_ext() -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(
			vec![(ALICE, HDX, 2_000 * ONE_HDX), (BOB, DOT, 10 * ONE_DOT)],
			&[HDX, DOT],
		))
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 2_000 * ONE_HDX, 19 * ONE_DOT, false)),
			(BOB, swap_intent(DOT, HDX, 10 * ONE_DOT, 980 * ONE_HDX, false)),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			1_000 * ONE_HDX,
			1_000 * ONE_HDX,
			105 * ONE_DOT / 10,
		)
		.build()
}

/// Claimed: ALICE 10 matched + 10 pooled − 0.1 fee = 19.9 DOT, BOB 1_000 − 10 fee = 990 HDX.
fn opposite_directions_solution() -> Solution {
	solution(
		vec![
			resolved(0, HDX, DOT, 2_000 * ONE_HDX, 199 * ONE_DOT / 10, Partial::No),
			resolved(1, DOT, HDX, 10 * ONE_DOT, 990 * ONE_HDX, Partial::No),
		],
		vec![sell_trade(HDX, DOT, 1_000 * ONE_HDX, 10 * ONE_DOT)],
		9 * ONE_DOT / 10 + 10 * ONE_HDX,
	)
}

fn different_outputs_ext() -> sp_io::TestExternalities {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(
			vec![(ALICE, HDX, SOLD_HDX), (BOB, HDX, 3_000 * ONE_HDX)],
			&[HDX, DOT, ETH],
		))
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false)),
			(BOB, swap_intent(HDX, ETH, 3_000 * ONE_HDX, 2 * ONE_QUINTIL, false)),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			SOLD_HDX,
			SOLD_HDX,
			TRADE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			ETH,
			3_000 * ONE_HDX,
			3_000 * ONE_HDX,
			33 * ONE_QUINTIL / 10,
		)
		.build()
}

fn different_outputs_solution() -> Solution {
	solution(
		vec![
			resolved(0, HDX, DOT, SOLD_HDX, CLAIMED_DOT, Partial::No),
			resolved(1, HDX, ETH, 3_000 * ONE_HDX, 3 * ONE_QUINTIL, Partial::No),
		],
		vec![
			sell_trade(HDX, DOT, SOLD_HDX, CLAIMED_DOT),
			sell_trade(HDX, ETH, 3_000 * ONE_HDX, 3 * ONE_QUINTIL),
		],
		ONE_DOT + ONE_QUINTIL,
	)
}

#[test]
fn submit_solution_should_pay_whole_trade_output_when_solution_has_one_intent() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), TRADE_DOT).execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
	});
}

#[test]
fn submit_solution_should_leave_pot_balance_unchanged_when_solution_has_one_intent() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), TRADE_DOT).execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
	});
}

#[test]
fn submit_solution_should_leave_pot_balance_unchanged_when_intents_have_different_outputs() {
	different_outputs_ext().execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			different_outputs_solution()
		));

		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(ETH), POT_ETH);
		assert_eq!(pot_balance(HDX), POT_HDX);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
		assert_eq!(Currencies::free_balance(ETH, &BOB), 33 * ONE_QUINTIL / 10);
	});
}

#[test]
fn submit_solution_should_leave_pot_balance_unchanged_when_intents_trade_opposite_directions() {
	opposite_directions_ext().execute_with(|| {
		assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			opposite_directions_solution()
		));

		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), 204 * ONE_DOT / 10);
		assert_eq!(Currencies::free_balance(HDX, &BOB), 990 * ONE_HDX);
		assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), ONE_DOT / 10);
		assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 10 * ONE_HDX);
	});
}

/// Two trades pay 3.1 + 2.15 DOT for 3 + 2 claimed.
#[test]
fn submit_solution_should_pay_sum_of_outputs_when_one_intent_has_two_trades() {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(vec![(ALICE, HDX, SOLD_HDX)], &[HDX, DOT]))
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false))])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			3_000 * ONE_HDX,
			3_000 * ONE_HDX,
			31 * ONE_DOT / 10,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			2_000 * ONE_HDX,
			2_000 * ONE_HDX,
			215 * ONE_DOT / 100,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![resolved(0, HDX, DOT, SOLD_HDX, CLAIMED_DOT, Partial::No)],
					vec![
						sell_trade(HDX, DOT, 3_000 * ONE_HDX, 3 * ONE_DOT),
						sell_trade(HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT),
					],
					CLAIMED_DOT - MIN_DOT,
				)
			));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 525 * ONE_DOT / 100);
			assert_eq!(pot_balance(DOT), POT_DOT);
			assert_eq!(pot_balance(HDX), POT_HDX);
		});
}

#[test]
fn submit_solution_should_pay_each_owner_whole_output_when_intents_have_different_outputs() {
	different_outputs_ext().execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			different_outputs_solution()
		));

		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
		assert_eq!(Currencies::free_balance(ETH, &BOB), 33 * ONE_QUINTIL / 10);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(ETH), POT_ETH);
		assert_eq!(pot_balance(HDX), POT_HDX);
	});
}

/// DOT residual 10 + 10.5 − 19.9 = 0.6, fee 0.1, so ALICE gets 19.9 + 0.5 = 20.4 DOT; HDX residual equals its fee.
#[test]
fn submit_solution_should_pay_residual_after_fee_when_intents_trade_opposite_directions() {
	opposite_directions_ext().execute_with(|| {
		assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			opposite_directions_solution()
		));

		assert_eq!(Currencies::free_balance(DOT, &ALICE), 204 * ONE_DOT / 10);
		assert_eq!(Currencies::free_balance(HDX, &BOB), 990 * ONE_HDX);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
		assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), ONE_DOT / 10);
		assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 10 * ONE_HDX);
	});
}

/// DOT has two receivers, so its 0.2 + 0.1 DOT excess stays in the pot; DAVE gets all 51 USDT.
#[test]
fn submit_solution_should_pay_sole_receiver_whole_output_when_other_intents_share_an_output() {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(
			vec![
				(ALICE, HDX, SOLD_HDX),
				(BOB, ETH, 2 * ONE_QUINTIL),
				(DAVE, HDX, 1_000 * ONE_HDX),
			],
			&[HDX, DOT, ETH, USDT],
		))
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false)),
			(BOB, swap_intent(ETH, DOT, 2 * ONE_QUINTIL, ONE_DOT, false)),
			(DAVE, swap_intent(HDX, USDT, 1_000 * ONE_HDX, 40 * ONE_USDT, false)),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			SOLD_HDX,
			SOLD_HDX,
			TRADE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			ETH,
			DOT,
			2 * ONE_QUINTIL,
			2 * ONE_QUINTIL,
			21 * ONE_DOT / 10,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			USDT,
			1_000 * ONE_HDX,
			1_000 * ONE_HDX,
			51 * ONE_USDT,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![
						resolved(0, HDX, DOT, SOLD_HDX, CLAIMED_DOT, Partial::No),
						resolved(1, ETH, DOT, 2 * ONE_QUINTIL, 2 * ONE_DOT, Partial::No),
						resolved(2, HDX, USDT, 1_000 * ONE_HDX, 50 * ONE_USDT, Partial::No),
					],
					vec![
						sell_trade(HDX, DOT, SOLD_HDX, CLAIMED_DOT),
						sell_trade(ETH, DOT, 2 * ONE_QUINTIL, 2 * ONE_DOT),
						sell_trade(HDX, USDT, 1_000 * ONE_HDX, 50 * ONE_USDT),
					],
					2 * ONE_DOT + 10 * ONE_USDT,
				)
			));

			assert_eq!(Currencies::free_balance(USDT, &DAVE), 51 * ONE_USDT);
			assert_eq!(pot_balance(USDT), POT_USDT);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), CLAIMED_DOT);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 2 * ONE_DOT);
			assert_eq!(pot_balance(DOT), POT_DOT + 3 * ONE_DOT / 10);
			assert_eq!(pot_balance(HDX), POT_HDX);
			assert_eq!(pot_balance(ETH), POT_ETH);
		});
}

#[test]
fn submit_solution_should_emit_delivered_amount_when_asset_out_has_one_receiver() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), TRADE_DOT).execute_with(|| {
		frame_system::Pallet::<Test>::reset_events();

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(
			intent_events(),
			vec![pallet_intent::Event::IntentResolved {
				id: 0,
				amount_in: SOLD_HDX,
				amount_out: TRADE_DOT,
			}]
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
	});
}

/// Fills 2_000 of 5_000 HDX: claimed 2 DOT over a 1.6 DOT pro-rata floor, the pool pays 2.1 DOT.
#[test]
fn submit_solution_should_emit_delivered_amount_when_intent_is_partial() {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(vec![(ALICE, HDX, SOLD_HDX)], &[HDX, DOT]))
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, true))])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			2_000 * ONE_HDX,
			2_000 * ONE_HDX,
			21 * ONE_DOT / 10,
		)
		.build()
		.execute_with(|| {
			frame_system::Pallet::<Test>::reset_events();

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![resolved(0, HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT, Partial::Yes(0))],
					vec![sell_trade(HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT)],
					4 * ONE_DOT / 10,
				)
			));

			assert_eq!(
				intent_events(),
				vec![pallet_intent::Event::IntentResovedPartially {
					id: 0,
					amount_in: 2_000 * ONE_HDX,
					amount_out: 21 * ONE_DOT / 10,
				}]
			);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 21 * ONE_DOT / 10);
			assert_eq!(pot_balance(DOT), POT_DOT);
			assert_eq!(pot_balance(HDX), POT_HDX);
		});
}

#[test]
fn submit_solution_should_emit_delivered_amount_when_intent_is_dca() {
	let claimed: Balance = 2_000_000_000_000_000;
	let trade_out: Balance = 2_100_000_000_000_000;

	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(vec![(ALICE, HDX, 2_000 * ONE_HDX)], &[HDX, DOT]))
		.with_intents(vec![(
			ALICE,
			dca_intent(HDX, DOT, 1_000 * ONE_HDX, ONE_DOT, 2_000 * ONE_HDX),
		)])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			1_000 * ONE_HDX,
			1_000 * ONE_HDX,
			trade_out,
		)
		.build()
		.execute_with(|| {
			frame_system::Pallet::<Test>::set_block_number(10);
			frame_system::Pallet::<Test>::reset_events();

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![resolved(0, HDX, DOT, 1_000 * ONE_HDX, claimed, Partial::No)],
					vec![sell_trade(HDX, DOT, 1_000 * ONE_HDX, claimed)],
					claimed - ONE_DOT,
				)
			));

			assert_eq!(
				intent_events(),
				vec![pallet_intent::Event::DcaTradeExecuted {
					id: 0,
					amount_in: 1_000 * ONE_HDX,
					amount_out: trade_out,
					remaining_budget: 1_000 * ONE_HDX,
				}]
			);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), trade_out);
			assert_eq!(pot_balance(DOT), POT_DOT);
			assert_eq!(pot_balance(HDX), POT_HDX);
		});
}

#[test]
fn submit_solution_should_queue_forward_with_delivered_amount_when_intent_has_callback() {
	let intent = IntentInput {
		on_resolved: Some(forward()),
		..swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false)
	};

	single_swap_ext(intent, TRADE_DOT).execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(
			queued_forwards(),
			vec![(
				Source::ICE(0),
				ALICE,
				ForwardAction {
					contract: EvmAddress::repeat_byte(1),
					intent_id: 0,
					asset_in: HDX,
					amount_in: SOLD_HDX,
					asset_out: DOT,
					amount_out: TRADE_DOT,
					data: BoundedVec::truncate_from(b"callback".to_vec()),
				}
			)]
		);
		assert_eq!(Currencies::free_balance(DOT, &ALICE), TRADE_DOT);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
	});
}

#[test]
fn submit_solution_should_fail_when_claimed_amount_exceeds_trade_output() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), CLAIMED_DOT).execute_with(|| {
		assert_noop!(
			ICE::submit_solution(RuntimeOrigin::none(), single_swap_solution(55 * ONE_DOT / 10)),
			Error::<Test>::FeeMismatch
		);
	});
}

#[test]
fn submit_solution_should_fail_when_trade_sells_funds_the_pot_held_before() {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(vec![(ALICE, HDX, SOLD_HDX)], &[HDX, DOT, ETH]))
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false))])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			SOLD_HDX,
			SOLD_HDX,
			CLAIMED_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			ETH,
			DOT,
			ONE_QUINTIL,
			ONE_QUINTIL,
			ONE_DOT,
		)
		.build()
		.execute_with(|| {
			assert_noop!(
				ICE::submit_solution(
					RuntimeOrigin::none(),
					solution(
						vec![resolved(0, HDX, DOT, SOLD_HDX, CLAIMED_DOT + ONE_DOT, Partial::No)],
						vec![
							sell_trade(HDX, DOT, SOLD_HDX, CLAIMED_DOT),
							sell_trade(ETH, DOT, ONE_QUINTIL, ONE_DOT),
						],
						CLAIMED_DOT + ONE_DOT - MIN_DOT,
					)
				),
				Error::<Test>::FeeMismatch
			);
		});
}

#[test]
fn submit_solution_should_pay_claimed_amount_when_trade_returns_exactly_the_claim() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), CLAIMED_DOT).execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(Currencies::free_balance(DOT, &ALICE), CLAIMED_DOT);
		assert_eq!(pot_balance(DOT), POT_DOT);
		assert_eq!(pot_balance(HDX), POT_HDX);
	});
}

#[test]
fn submit_solution_should_accept_claimed_score_when_trade_returns_more_than_claimed() {
	single_swap_ext(swap_intent(HDX, DOT, SOLD_HDX, MIN_DOT, false), TRADE_DOT).execute_with(|| {
		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			single_swap_solution(CLAIMED_DOT)
		));

		assert_eq!(
			last_ice_event(),
			crate::Event::SolutionExecuted {
				intents_executed: 1,
				trades_executed: 1,
				score: CLAIMED_DOT - MIN_DOT,
				built_at: 0,
			}
		);
	});
}

#[test]
fn submit_solution_should_keep_excess_in_pot_when_intents_share_an_output() {
	ExtBuilder::default()
		.with_endowed_accounts(with_seeded_pot(
			vec![(ALICE, HDX, 3_000 * ONE_HDX), (BOB, HDX, 2_000 * ONE_HDX)],
			&[HDX, DOT],
		))
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 3_000 * ONE_HDX, 2 * ONE_DOT, false)),
			(BOB, swap_intent(HDX, DOT, 2_000 * ONE_HDX, ONE_DOT, false)),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			SOLD_HDX,
			SOLD_HDX,
			54 * ONE_DOT / 10,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![
						resolved(0, HDX, DOT, 3_000 * ONE_HDX, 3 * ONE_DOT, Partial::No),
						resolved(1, HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT, Partial::No),
					],
					vec![sell_trade(HDX, DOT, SOLD_HDX, CLAIMED_DOT)],
					2 * ONE_DOT,
				)
			));

			assert_eq!(pot_balance(DOT), POT_DOT + 4 * ONE_DOT / 10);
			assert_eq!(pot_balance(HDX), POT_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 3 * ONE_DOT);
			assert_eq!(Currencies::free_balance(DOT, &BOB), 2 * ONE_DOT);
		});
}

#[test]
fn submit_solution_should_sweep_same_matched_fee_when_trade_returns_more_than_claimed() {
	opposite_directions_ext().execute_with(|| {
		assert_ok!(ICE::set_protocol_fee(RuntimeOrigin::root(), Permill::from_percent(1)));

		assert_ok!(ICE::submit_solution(
			RuntimeOrigin::none(),
			opposite_directions_solution()
		));

		assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), ONE_DOT / 10);
		assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 10 * ONE_HDX);
	});
}
