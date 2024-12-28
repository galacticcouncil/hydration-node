use super::*;
use crate::tests::{ExtBuilder, ICE};
use crate::types::{
	BoundedInstructions, BoundedResolvedIntents, BoundedRoute, BoundedTrades, Instruction, Intent, ResolvedIntent,
	Swap, SwapType, TradeInstruction,
};
use frame_support::assert_ok;

#[test]
fn validate_solution_should_fail_when_resolved_intent_does_exist() {}

#[test]
fn validate_solution_should_fail_when_resolved_intent_is_already_past_deadline() {}

#[test]
fn validate_solution_should_fail_when_limit_price_is_not_respected_in_partial_intent() {}
