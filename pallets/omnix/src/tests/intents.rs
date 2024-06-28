use super::*;
use crate::tests::{ExtBuilder, OmniX};
use crate::types::{Intent, Swap, SwapType};
use frame_support::assert_ok;

#[test]
fn submit_intent_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		let swap = Swap {
			asset_in: 100,
			asset_out: 200,
			amount_in: 100_000_000_000_000,
			amount_out: 200_000_000_000_000,
			swap_type: SwapType::ExactIn,
		};
		assert_ok!(OmniX::submit_intent(
			RuntimeOrigin::signed(ALICE),
			swap.clone(),
			NOW,
			false,
			None,
			None,
		));

		let intent_id = get_intent_id(NOW, 0);
		let intent = crate::Pallet::<Test>::get_intent(intent_id);
		assert!(intent.is_some());
		let intent = intent.unwrap();
		let expected_intent = Intent {
			who: ALICE,
			swap,
			deadline: NOW,
			partial: false,
			on_success: None,
			on_failure: None,
		};
		assert_eq!(intent, expected_intent);
	});
}
