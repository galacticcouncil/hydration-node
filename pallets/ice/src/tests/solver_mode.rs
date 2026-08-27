use crate::tests::mock::*;
use crate::Call;
use crate::CurrentSolverMode;
use crate::Error;
use crate::Event;
use crate::OCW_PROVIDES;
use crate::OCW_TAG_PREFIX;
use crate::UNSIGNED_TXS_PRIORITY;
use frame_support::assert_noop;
use frame_support::assert_ok;
use frame_support::pallet_prelude::*;
use frame_support::traits::ExistenceRequirement::AllowDeath;
use ice_support::DcaParams;
use ice_support::IntentData;
use ice_support::IntentDataInput;
use ice_support::IntentId;
use ice_support::Partial;
use ice_support::PoolTrade;
use ice_support::ResolvedIntent;
use ice_support::Score;
use ice_support::Solution;
use ice_support::SolverMode;
use ice_support::SwapData;
use ice_support::SwapParams;
use ice_support::SwapType;
use orml_traits::MultiCurrency;
use orml_traits::MultiReservableCurrency;
use pallet_intent::types::IntentInput;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;
use sp_runtime::DispatchError::BadOrigin;
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

fn route(asset_in: AssetId, asset_out: AssetId) -> hydradx_traits::router::Route<AssetId> {
	vec![RTrade {
		pool: PoolType::XYK,
		asset_in,
		asset_out,
	}]
	.try_into()
	.expect("single hop route to fit")
}

fn sell_trade(asset_in: AssetId, asset_out: AssetId, amount_in: Balance, amount_out: Balance) -> PoolTrade {
	PoolTrade {
		direction: SwapType::ExactIn,
		amount_in,
		amount_out,
		route: route(asset_in, asset_out),
	}
}

fn resolved(
	id: IntentId,
	asset_in: AssetId,
	asset_out: AssetId,
	amount_in: Balance,
	amount_out: Balance,
) -> ResolvedIntent {
	ResolvedIntent {
		id,
		data: IntentData::Swap(SwapData {
			asset_in,
			asset_out,
			amount_in,
			amount_out,
			partial: Partial::No,
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

fn ice_events() -> Vec<Event<Test>> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.filter_map(|record| match record.event {
			RuntimeEvent::ICE(e) => Some(e),
			_ => None,
		})
		.collect()
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

fn last_ice_event() -> Event<Test> {
	ice_events().pop().expect("ICE event to be emitted")
}

/// Recreates the state a v4-era partial fill leaves behind: the intent tracks
/// `filled`, and that part of the reserve is already gone from the account.
/// `submit_intent` refuses to create partial intents, so it is injected directly.
fn make_partially_filled(id: IntentId, owner: AccountId, filled: Balance) {
	pallet_intent::Intents::<Test>::mutate(id, |maybe_intent| {
		let intent = maybe_intent.as_mut().expect("intent to exist");
		let IntentData::Swap(ref mut swap) = intent.data else {
			panic!("expected a swap intent");
		};
		swap.partial = Partial::Yes(filled);
	});

	let asset_in = pallet_intent::Intents::<Test>::get(id)
		.expect("intent to exist")
		.data
		.asset_in();
	assert_ok!(pallet_intent::Pallet::<Test>::unlock_funds(&owner, asset_in, filled));
	assert_ok!(Currencies::withdraw(asset_in, &owner, filled, AllowDeath));
}

fn partial_state(id: IntentId) -> Partial {
	let intent = pallet_intent::Intents::<Test>::get(id).expect("intent to exist");
	let IntentData::Swap(swap) = intent.data else {
		panic!("expected a swap intent");
	};
	swap.partial
}

#[test]
fn set_solver_mode_should_fail_when_origin_is_not_authority() {
	ExtBuilder::default().build().execute_with(|| {
		assert_noop!(
			ICE::set_solver_mode(RuntimeOrigin::signed(ALICE), SolverMode::Passthrough),
			BadOrigin
		);

		assert_eq!(CurrentSolverMode::<Test>::get(), SolverMode::V4);
	});
}

#[test]
fn set_solver_mode_should_store_mode_when_mode_is_not_default() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

		assert!(CurrentSolverMode::<Test>::exists());
		assert_eq!(CurrentSolverMode::<Test>::get(), SolverMode::Passthrough);
		assert_eq!(
			last_ice_event(),
			Event::SolverModeSet {
				mode: SolverMode::Passthrough
			}
		);
	});
}

#[test]
fn set_solver_mode_should_kill_storage_when_set_back_to_default() {
	ExtBuilder::default().build().execute_with(|| {
		assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Disabled));
		assert!(CurrentSolverMode::<Test>::exists());

		assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::V4));

		assert!(!CurrentSolverMode::<Test>::exists());
		assert_eq!(CurrentSolverMode::<Test>::get(), SolverMode::V4);
		assert_eq!(last_ice_event(), Event::SolverModeSet { mode: SolverMode::V4 });
	});
}

#[test]
fn validate_unsigned_should_reject_solution_when_mode_is_disabled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				ONE_DOT,
			);
			let call = Call::submit_solution { solution: s };

			// The very same solution is accepted under the default mode.
			assert_eq!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					requires: vec![],
					provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
					longevity: 1,
					propagate: false,
				})
			);

			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Disabled));

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn submit_solution_should_fail_when_mode_is_disabled() {
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
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Disabled));

			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				ONE_DOT,
			);

			assert_noop!(
				ICE::submit_solution(RuntimeOrigin::none(), s),
				Error::<Test>::InvalidSolution
			);
		});
}

#[test]
fn validate_unsigned_should_accept_shaped_solution_when_mode_is_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			// Claimed amounts and score are advisory — nonsense here on purpose.
			let s = solution(vec![resolved(0, HDX, DOT, 1, 1)], vec![sell_trade(HDX, DOT, 1, 1)], 0);
			let call = Call::submit_solution { solution: s };

			assert_eq!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					requires: vec![],
					provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
					longevity: 1,
					propagate: false,
				})
			);
		});
}

#[test]
fn validate_unsigned_should_reject_matching_solution_when_mode_is_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (BOB, DOT, 10_000 * ONE_DOT)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 1_000 * ONE_HDX, 900 * ONE_DOT)),
			(BOB, swap_intent(DOT, HDX, 1_000 * ONE_DOT, 900 * ONE_HDX)),
		])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			// Perfectly matched pair — two intents, zero trades.
			let s = solution(
				vec![
					resolved(0, HDX, DOT, 1_000 * ONE_HDX, 1_000 * ONE_DOT),
					resolved(1, DOT, HDX, 1_000 * ONE_DOT, 1_000 * ONE_HDX),
				],
				vec![],
				200 * ONE_DOT,
			);
			let call = Call::submit_solution { solution: s };

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsigned_should_reject_solution_when_route_endpoints_do_not_match_intent_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				vec![sell_trade(HDX, ETH, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				ONE_DOT,
			);
			let call = Call::submit_solution { solution: s };

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsigned_should_reject_solution_when_trade_direction_is_exact_out_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let mut trade = sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT);
			trade.direction = SwapType::ExactOut;

			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT)],
				vec![trade],
				ONE_DOT,
			);
			let call = Call::submit_solution { solution: s };

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsigned_should_reject_solution_when_intent_is_duplicated_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 2_500 * ONE_HDX, 3 * ONE_DOT),
					resolved(0, HDX, DOT, 2_500 * ONE_HDX, 3 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 2_500 * ONE_HDX, 3 * ONE_DOT),
					sell_trade(HDX, DOT, 2_500 * ONE_HDX, 3 * ONE_DOT),
				],
				2 * ONE_DOT,
			);
			let call = Call::submit_solution { solution: s };

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsigned_should_reject_passthrough_solution_when_mode_is_v4() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT))])
		.build()
		.execute_with(|| {
			assert_eq!(CurrentSolverMode::<Test>::get(), SolverMode::V4);

			// Advisory amounts are meaningless under Strict — claimed amounts bind there.
			let s = solution(vec![resolved(0, HDX, DOT, 1, 1)], vec![sell_trade(HDX, DOT, 1, 1)], 0);
			let call = Call::submit_solution { solution: s };

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn submit_solution_should_pay_actual_output_when_mode_is_passthrough() {
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
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			// The builder only promises the intent's own limit; the pool pays more.
			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)],
				vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 0);
			assert_eq!(pallet_intent::Intents::<Test>::get(0), None);

			let pot = ICE::get_pallet_account();
			assert_eq!(Currencies::free_balance(HDX, &pot), 0);
			assert_eq!(Currencies::free_balance(DOT, &pot), 0);
			assert_eq!(Currencies::free_balance(HDX, &ICE_FEE_RECEIVER), 0);
			assert_eq!(Currencies::free_balance(DOT, &ICE_FEE_RECEIVER), 0);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: ONE_DOT,
					built_at: 0,
				}
			);
			assert!(intent_events().contains(&pallet_intent::Event::IntentResolved {
				id: 0,
				amount_in: 5_000 * ONE_HDX,
				amount_out: 5 * ONE_DOT,
			}));
		});
}

#[test]
fn submit_solution_should_ignore_claimed_amounts_when_mode_is_passthrough() {
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
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			// Overpromised amounts and a fabricated score change nothing.
			let s = solution(
				vec![resolved(0, HDX, DOT, 1, 1_000 * ONE_DOT)],
				vec![sell_trade(HDX, DOT, 1, 1_000 * ONE_DOT)],
				999_999_999_999,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: ONE_DOT,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_execute_remaining_intents_when_one_intent_fails_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (DAVE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)),
			(DAVE, swap_intent(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT)),
		])
		// ALICE's route pays 3 DOT against a 4 DOT limit — the sell fails.
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			3 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			10 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT),
					resolved(1, HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT),
					sell_trade(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT),
				],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			// ALICE untouched — funds stay reserved, intent stays put.
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 0);
			assert!(pallet_intent::Intents::<Test>::get(0).is_some());

			// DAVE settled at the pool's actual output.
			assert_eq!(Currencies::reserved_balance(HDX, &DAVE), 0);
			assert_eq!(Currencies::free_balance(HDX, &DAVE), 0);
			assert_eq!(Currencies::free_balance(DOT, &DAVE), 10 * ONE_DOT);
			assert_eq!(pallet_intent::Intents::<Test>::get(1), None);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: 2 * ONE_DOT,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_succeed_when_all_intents_skip_in_passthrough() {
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
			3 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![resolved(0, HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)],
				vec![sell_trade(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)],
				0,
			);

			let post = ICE::submit_solution(RuntimeOrigin::none(), s).expect("all-skip solution to succeed");
			assert_eq!(post.actual_weight, Some(Weight::zero()));

			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 5_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 0);
			assert!(pallet_intent::Intents::<Test>::get(0).is_some());

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 0,
					trades_executed: 0,
					score: 0,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_execute_same_pair_intents_at_different_prices_when_mode_is_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (DAVE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 5_000 * ONE_HDX, 4 * ONE_DOT)),
			(DAVE, swap_intent(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT)),
		])
		// 1 HDX buys 0.001 DOT for ALICE, 0.0012 DOT for DAVE — two prices on one pair.
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			5_000 * ONE_HDX,
			5_000 * ONE_HDX,
			5 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			10_000 * ONE_HDX,
			10_000 * ONE_HDX,
			12 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT),
					resolved(1, HDX, DOT, 10_000 * ONE_HDX, 12 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 5_000 * ONE_HDX, 5 * ONE_DOT),
					sell_trade(HDX, DOT, 10_000 * ONE_HDX, 12 * ONE_DOT),
				],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 5 * ONE_DOT);
			assert_eq!(Currencies::free_balance(DOT, &DAVE), 12 * ONE_DOT);
			assert_eq!(pallet_intent::Intents::<Test>::get(0), None);
			assert_eq!(pallet_intent::Intents::<Test>::get(1), None);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 2,
					trades_executed: 2,
					score: 5 * ONE_DOT,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_fill_full_remaining_when_intent_is_partial_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![(ALICE, swap_intent(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT))])
		// Remaining 4_000 HDX at the pro-rata limit of 3.2 DOT; the pool pays 4 DOT.
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			4_000 * ONE_HDX,
			4_000 * ONE_HDX,
			4 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			make_partially_filled(0, ALICE, 6_000 * ONE_HDX);
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 4_000 * ONE_HDX);

			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			// The solution proposes a 1_000 HDX sliver — execution fills the remainder.
			let s = solution(
				vec![resolved(0, HDX, DOT, 1_000 * ONE_HDX, ONE_DOT)],
				vec![sell_trade(HDX, DOT, 1_000 * ONE_HDX, ONE_DOT)],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 4 * ONE_DOT);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 0);
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 0);
			assert_eq!(pallet_intent::Intents::<Test>::get(0), None);

			// 4 DOT received against a 3.2 DOT pro-rata limit.
			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: 8_000_000_000,
					built_at: 0,
				}
			);
			assert!(intent_events().contains(&pallet_intent::Event::IntentResolved {
				id: 0,
				amount_in: 4_000 * ONE_HDX,
				amount_out: 4 * ONE_DOT,
			}));
			assert!(!intent_events()
				.iter()
				.any(|e| matches!(e, pallet_intent::Event::IntentResovedPartially { .. })));
		});
}

#[test]
fn submit_solution_should_skip_partial_intent_when_remaining_cannot_be_fully_filled() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (DAVE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT)),
			(DAVE, swap_intent(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT)),
		])
		// 3 DOT for the 4_000 HDX remainder is below the 3.2 DOT pro-rata limit.
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			4_000 * ONE_HDX,
			4_000 * ONE_HDX,
			3 * ONE_DOT,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			6_000 * ONE_HDX,
			6_000 * ONE_HDX,
			6 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			make_partially_filled(0, ALICE, 6_000 * ONE_HDX);

			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 4_000 * ONE_HDX, 3 * ONE_DOT),
					resolved(1, HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 4_000 * ONE_HDX, 3 * ONE_DOT),
					sell_trade(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			// Never a further partial fill: state and reserve untouched.
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 4_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(HDX, &ALICE), 0);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 0);
			assert_eq!(partial_state(0), Partial::Yes(6_000 * ONE_HDX));

			assert_eq!(Currencies::free_balance(DOT, &DAVE), 6 * ONE_DOT);
			assert_eq!(pallet_intent::Intents::<Test>::get(1), None);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: ONE_DOT,
					built_at: 0,
				}
			);
			assert!(!intent_events()
				.iter()
				.any(|e| matches!(e, pallet_intent::Event::IntentResovedPartially { .. })));
		});
}

#[test]
fn submit_solution_should_skip_intent_when_remaining_is_dust_in_passthrough() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (DAVE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, swap_intent(HDX, DOT, 10_000 * ONE_HDX, 8 * ONE_DOT)),
			(DAVE, swap_intent(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT)),
		])
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			6_000 * ONE_HDX,
			6_000 * ONE_HDX,
			6 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			// Remaining = 999, one unit below the 1_000 existential deposit.
			make_partially_filled(0, ALICE, 10_000 * ONE_HDX - 999);
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 999);

			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 999, 1),
					resolved(1, HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 999, 1),
					sell_trade(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 999);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 0);
			assert_eq!(partial_state(0), Partial::Yes(10_000 * ONE_HDX - 999));

			assert_eq!(Currencies::free_balance(DOT, &DAVE), 6 * ONE_DOT);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: ONE_DOT,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_execute_dca_intent_when_router_output_clears_oracle_floor() {
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
			frame_system::Pallet::<Test>::set_block_number(10);
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![resolved(0, HDX, DOT, 1_000 * ONE_HDX, ONE_DOT)],
				vec![sell_trade(HDX, DOT, 1_000 * ONE_HDX, ONE_DOT)],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			assert_eq!(Currencies::free_balance(DOT, &ALICE), 2_000_000_000_000_000);
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 1_000 * ONE_HDX);
			assert!(intent_events().contains(&pallet_intent::Event::DcaTradeExecuted {
				id: 0,
				amount_in: 1_000 * ONE_HDX,
				amount_out: 2_000_000_000_000_000,
				remaining_budget: 1_000 * ONE_HDX,
			}));

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: 2_000_000_000_000_000 - ONE_DOT,
					built_at: 0,
				}
			);
		});
}

#[test]
fn submit_solution_should_skip_dca_intent_when_oracle_floor_rises_above_quote() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 2_000 * ONE_HDX), (DAVE, HDX, 10_000 * ONE_HDX)])
		.with_intents(vec![
			(ALICE, dca_intent(HDX, DOT, 1_000 * ONE_HDX, ONE_DOT, 2_000 * ONE_HDX)),
			(DAVE, swap_intent(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT)),
		])
		// Clears the DCA's own hard limit of 1 DOT but sits far below the oracle floor.
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			1_000 * ONE_HDX,
			1_000 * ONE_HDX,
			1_000_000_000_000,
		)
		.with_router_settlement(
			SwapType::ExactIn,
			PoolType::XYK,
			HDX,
			DOT,
			6_000 * ONE_HDX,
			6_000 * ONE_HDX,
			6 * ONE_DOT,
		)
		.build()
		.execute_with(|| {
			frame_system::Pallet::<Test>::set_block_number(10);
			assert_ok!(ICE::set_solver_mode(RuntimeOrigin::root(), SolverMode::Passthrough));

			let s = solution(
				vec![
					resolved(0, HDX, DOT, 1_000 * ONE_HDX, ONE_DOT),
					resolved(1, HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				vec![
					sell_trade(HDX, DOT, 1_000 * ONE_HDX, ONE_DOT),
					sell_trade(HDX, DOT, 6_000 * ONE_HDX, 5 * ONE_DOT),
				],
				0,
			);

			assert_ok!(ICE::submit_solution(RuntimeOrigin::none(), s));

			// The DCA tranche is skipped: budget and reserve untouched.
			assert_eq!(Currencies::reserved_balance(HDX, &ALICE), 2_000 * ONE_HDX);
			assert_eq!(Currencies::free_balance(DOT, &ALICE), 0);
			let intent = pallet_intent::Intents::<Test>::get(0).expect("DCA intent to remain");
			let IntentData::Dca(dca) = intent.data else {
				panic!("expected a DCA intent");
			};
			assert_eq!(dca.remaining_budget, 2_000 * ONE_HDX);
			assert_eq!(dca.last_execution_block, 1);

			assert_eq!(Currencies::free_balance(DOT, &DAVE), 6 * ONE_DOT);

			assert_eq!(
				last_ice_event(),
				Event::SolutionExecuted {
					intents_executed: 1,
					trades_executed: 1,
					score: ONE_DOT,
					built_at: 0,
				}
			);
		});
}
