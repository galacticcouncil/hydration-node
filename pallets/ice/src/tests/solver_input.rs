use crate::tests::mock::*;
use crate::*;
use ice_support::IntentDataInput;
use ice_support::SwapParams;
use pallet_intent::types::IntentInput;
use pretty_assertions::assert_eq;
use sp_runtime::Permill;

#[test]
fn solver_input_should_return_none_when_no_valid_intents() {
	ExtBuilder::default().build().execute_with(|| {
		assert_eq!(Pallet::<Test>::solver_input(), None);
	});
}

#[test]
fn solver_input_should_collect_intents_eds_and_fee_when_intents_exist() {
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, HDX, 10_000 * ONE_HDX), (BOB, ETH, 10_000 * ONE_QUINTIL)])
		.with_intents(vec![
			(
				ALICE,
				IntentInput {
					data: IntentDataInput::Swap(SwapParams {
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
				BOB,
				IntentInput {
					data: IntentDataInput::Swap(SwapParams {
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
		.build()
		.execute_with(|| {
			let (intents, state, eds, fee) =
				Pallet::<Test>::solver_input().expect("solver_input should be Some when valid intents exist");

			// Exactly the two submitted intents.
			assert_eq!(intents.len(), 2);
			let mut pairs: Vec<_> = intents
				.iter()
				.map(|i| (i.data.asset_in(), i.data.asset_out()))
				.collect();
			pairs.sort();
			assert_eq!(pairs, vec![(HDX, DOT), (ETH, HDX)]);

			// Trivial mock snapshot: `()` encodes to empty bytes.
			assert_eq!(state, Vec::<u8>::new());

			// ED universe = snapshot pool assets (none in the mock) ∪ intent assets,
			// sorted; the mock SimulatorConfig uses the default ED of 0.
			assert_eq!(eds, vec![(HDX, 0), (DOT, 0), (ETH, 0)]);

			// Fee = mock MatchedFee = 0%.
			assert_eq!(fee, Permill::from_percent(0));
		});
}
