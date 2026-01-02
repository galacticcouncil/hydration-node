use crate::tests::mock::*;
use crate::types::Solution;
use crate::types::Trade;
use crate::types::TradeType;
use crate::*;
use frame_support::assert_noop;
use hydra_dx_math::types::Ratio;
use pallet_intent::types::Intent;
use pallet_intent::types::IntentKind;
use pallet_intent::types::SwapData;
use pallet_intent::types::SwapType;
use pallet_route_executor::PoolType;
use pallet_route_executor::Trade as RTrade;
use pretty_assertions::assert_eq;

#[test]
fn validate_unsingned_should_work_when_submitted_solution_is_valid() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s,
				score,
				valid_for_block: 2,
			};

			assert_eq!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					requires: vec![],
					provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
					longevity: 1,
					propagate: false
				})
			);
		});
}

#[test]
fn validate_unsingned_should_not_work_when_submitted_solution_is_not_for_next_block() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			//NOTE: just to make sure everything except `valid_for_block` is ok
			assert_eq!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					requires: vec![],
					provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
					longevity: 1,
					propagate: false
				})
			);

			//solution for current block
			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);

			//solution for future block
			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 2,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);

			//solution for past block
			let call = Call::submit_solution {
				solution: s,
				score,
				valid_for_block: current_block - 1,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsingned_should_not_work_when_submitted_solution_score_is_not_correct() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			//NOTE: just to make sure everything except `valid_for_block` is ok
			assert_eq!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				Ok(ValidTransaction {
					priority: UNSIGNED_TXS_PRIORITY,
					requires: vec![],
					provides: vec![(OCW_TAG_PREFIX, OCW_PROVIDES.to_vec()).encode()],
					longevity: 1,
					propagate: false
				})
			);

			//Act 1
			let call = Call::submit_solution {
				solution: s.clone(),
				score: score - 1,
				valid_for_block: current_block,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);

			//Act 2
			let call = Call::submit_solution {
				solution: s.clone(),
				score: score + 1,
				valid_for_block: current_block,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);

			//Act 3
			let call = Call::submit_solution {
				solution: s.clone(),
				score: 0,
				valid_for_block: current_block,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);

			//Act 4
			let call = Call::submit_solution {
				solution: s.clone(),
				score: Score::max_value(),
				valid_for_block: current_block,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}

#[test]
fn validate_unsingned_should_not_work_when_submitted_solution_has_invalid_clearing_price() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
				clearing_prices: vec![
					(
						HDX,
						Ratio {
							n: 177,
							d: 0, //INVALID PRICE
						},
					),
					(
						DOT,
						Ratio {
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}

#[test]
fn validate_unsingned_should_not_work_when_submitted_solution_has_duplicate_clearing_price() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}

#[test]
fn validate_unsingned_should_not_work_when_intentent_not_found() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128 - 10, //intent that doesn't exist
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}

#[test]
fn validate_unsingned_should_not_work_when_solution_has_duplicate_intents() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					//Duplicate intent - copy of 1th
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let current_block = 1;
			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s.clone(),
				score,
				valid_for_block: current_block + 1,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}

#[test]
fn validate_unsingned_should_work_when_submitted_solution_is_missing_clearing_price() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
				clearing_prices: vec![
					//DOT's price is missing and GETH price is not used
					(
						HDX,
						Ratio {
							n: 177,
							d: 100_000_000_000_000,
						},
					),
					(
						GETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s,
				score,
				valid_for_block: 2,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		});
}

#[test]
fn validate_unsingned_should_work_when_submitted_solution_has_inconsistent_clearing_prices() {
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
			15_000 * ONE_HDX,
			15_000 * ONE_HDX,
			15 * ONE_DOT,
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
			let resolved = vec![
				(
					73786976294838206464002_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 4,
							asset_out: 0,
							amount_in: 500000000000000000,
							amount_out: 16000000000000000000,
							swap_type: SwapType::ExactOut,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464001_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 10000000000000000,
							amount_out: 100000000000 + 1, //breaks price consistency, should receive 10.0[DOT]
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
				(
					73786976294838206464000_u128,
					Intent {
						kind: IntentKind::Swap(SwapData {
							asset_in: 0,
							asset_out: 2,
							amount_in: 5000000000000000,
							amount_out: 50000000000,
							swap_type: SwapType::ExactIn,
							partial: false,
						}),
						deadline: 4000,
						on_success: None,
						on_failure: None,
					},
				),
			];

			let trades = vec![
				Trade {
					amount_in: 15_000 * ONE_HDX,
					amount_out: 12 * ONE_DOT,
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
					amount_in: ONE_QUINTIL / 2,
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
				resolved,
				trades,
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
							n: 177,
							d: 1_000_000_000,
						},
					),
					(
						ETH,
						Ratio {
							n: 177,
							d: 3_125_000_000_000,
						},
					),
				],
			};

			let score = 500_000_030_000_000_000_u128;

			let call = Call::submit_solution {
				solution: s,
				score,
				valid_for_block: 2,
			};

			assert_noop!(
				ICE::validate_unsigned(TransactionSource::Local, &call),
				TransactionValidityError::Invalid(InvalidTransaction::Call)
			);
		})
}
