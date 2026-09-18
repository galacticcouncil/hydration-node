//! How settlement moves an intent's input into the holding pot.
//!
//! The input goes reserve → pot in one repatriate. It must never land on the owner on the way,
//! because for a bound erc20 the owner would become the token sender and pay for aave's solvency
//! walk over their whole position.

use crate::tests::mock::*;
use frame_support::assert_noop;
use frame_support::assert_ok;
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
use orml_traits::BalanceStatus;
use orml_traits::MultiCurrency;
use orml_traits::NamedMultiReservableCurrency;
use pallet_intent::types::IntentInput;
use pallet_intent::NAMED_RESERVE_ID;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

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

fn token_events() -> Vec<orml_tokens::Event<Test>> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.filter_map(|record| match record.event {
			RuntimeEvent::Currencies(e) => Some(e),
			_ => None,
		})
		.collect()
}

/// The owner receiving their own input back is exactly what the old unreserve+transfer pair did.
fn owner_was_credited_with(asset: AssetId, who: AccountId) -> bool {
	token_events().iter().any(
		|e| matches!(e, orml_tokens::Event::Unreserved { currency_id, who: w, .. } if *currency_id == asset && *w == who),
	)
}

fn reserved(asset: AssetId, who: AccountId) -> Balance {
	Currencies::reserved_balance_named(&NAMED_RESERVE_ID, asset, &who)
}

#[test]
fn submit_solution_should_move_the_input_from_the_reserve_to_the_pot_when_settling() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(
			ALICE,
			swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT, false),
		)])
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
			let pot = ICE::get_pallet_account();
			assert_eq!(reserved(HDX, ALICE), 5_000 * ONE_HDX);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT, Partial::No)],
					vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
					ONE_DOT,
				)
			));

			assert!(token_events().contains(&orml_tokens::Event::ReserveRepatriated {
				currency_id: HDX,
				from: ALICE,
				to: pot,
				amount: 5_000 * ONE_HDX,
				status: BalanceStatus::Free,
			}));
			assert!(!owner_was_credited_with(HDX, ALICE));

			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(reserved(HDX, ALICE), 0);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
			assert_eq!(Currencies::free_balance(HDX, &pot), 0);
			assert_eq!(Currencies::free_balance(DOT, &pot), 0);
		});
}

#[test]
fn submit_solution_should_keep_the_unfilled_remainder_reserved_when_intent_is_partial() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT, true))])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			2_000 * ONE_HDX,
			2_000 * ONE_HDX,
			2 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			let pot = ICE::get_pallet_account();

			// Pro-rata floor for 2_000 HDX is 1.6 DOT, so 2 DOT scores 0.4 DOT.
			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![resolved(0, HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT, Partial::Yes(0))],
					vec![sell_trade(HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT)],
					4_000_000_000,
				)
			));

			assert!(token_events().contains(&orml_tokens::Event::ReserveRepatriated {
				currency_id: HDX,
				from: ALICE,
				to: pot,
				amount: 2_000 * ONE_HDX,
				status: BalanceStatus::Free,
			}));
			assert!(!owner_was_credited_with(HDX, ALICE));

			// Only the filled part left the reserve; the rest is still locked for the intent.
			assert_eq!(reserved(HDX, ALICE), 3_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 2 * ONE_DOT);

			let intent = pallet_intent::Intents::<Test>::get(0).expect("partial intent to remain");
			let IntentData::Swap(swap) = intent.data else {
				panic!("expected a swap intent");
			};
			assert_eq!(swap.partial, Partial::Yes(2_000 * ONE_HDX));
		});
}

#[test]
fn submit_solution_should_keep_the_remaining_budget_reserved_when_intent_is_dca() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 2_000 * ONE_HDX)])
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
			2_000_000_000_000_000,
		)
		.build()
		.execute_with(|| {
			let pot = ICE::get_pallet_account();
			frame_system::Pallet::<Test>::set_block_number(10);
			assert_eq!(reserved(HDX, ALICE), 2_000 * ONE_HDX);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![ResolvedIntent {
						id: 0,
						data: IntentData::Swap(SwapData {
							asset_in: HDX,
							asset_out: DOT,
							amount_in: 1_000 * ONE_HDX,
							amount_out: 2_000_000_000_000_000,
							partial: Partial::No,
						}),
					}],
					vec![sell_trade(HDX, DOT, 1_000 * ONE_HDX, 2_000_000_000_000_000)],
					2_000_000_000_000_000 - ONE_DOT,
				)
			));

			assert!(token_events().contains(&orml_tokens::Event::ReserveRepatriated {
				currency_id: HDX,
				from: ALICE,
				to: pot,
				amount: 1_000 * ONE_HDX,
				status: BalanceStatus::Free,
			}));
			assert!(!owner_was_credited_with(HDX, ALICE));

			// One tranche spent, the rest of the budget stays locked.
			assert_eq!(reserved(HDX, ALICE), 1_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 0);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 2_000_000_000_000_000);

			let intent = pallet_intent::Intents::<Test>::get(0).expect("DCA intent to remain");
			let IntentData::Dca(dca) = intent.data else {
				panic!("expected a DCA intent");
			};
			assert_eq!(dca.remaining_budget, 1_000 * ONE_HDX);
		});
}

#[test]
fn submit_solution_should_fail_when_the_reserve_is_short_of_the_resolved_amount() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(
			ALICE,
			swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT, false),
		)])
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
			// Something released part of the lock behind the intent's back.
			assert_ok!(pallet_intent::Pallet::<Test>::unlock_funds(&ALICE, HDX, ONE_HDX));

			assert_noop!(
				ICE::submit_solution(
					RuntimeOrigin::none(),
					solution(
						vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT, Partial::No)],
						vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
						ONE_DOT,
					)
				),
				pallet_intent::Error::<Test>::InsufficientReservedBalance
			);

			assert_eq!(reserved(HDX, ALICE), 4_999 * ONE_HDX);
		});
}

#[test]
fn submit_solution_should_pay_every_owner_from_the_pot_when_batch_shares_an_owner() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 3_000 * ONE_HDX, 2 * ONE_DOT, false)),
			(ALICE, swap_intent(HDX, DOT, 2_000 * ONE_HDX, ONE_DOT, false)),
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
			assert_eq!(reserved(HDX, ALICE), 5_000 * ONE_HDX);

			assert_ok!(ICE::submit_solution(
				RuntimeOrigin::none(),
				solution(
					vec![
						resolved(0, HDX, DOT, 3_000 * ONE_HDX, 3 * ONE_DOT, Partial::No),
						resolved(1, HDX, DOT, 2_000 * ONE_HDX, 2 * ONE_DOT, Partial::No),
					],
					vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
					2 * ONE_DOT,
				)
			));

			assert!(!owner_was_credited_with(HDX, ALICE));
			assert_eq!(reserved(HDX, ALICE), 0);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
		});
}
