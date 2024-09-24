use super::*;

#[test]
fn submit_intent_should_work() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool();

		let deadline: Moment = NOW + 43_200_000;

		let swap = Swap {
			asset_in: HDX,
			asset_out: DAI,
			amount_in: 1_000_000_000_000,
			amount_out: 2_000_000_000_000,
			swap_type: pallet_ice::types::SwapType::ExactIn,
		};
		assert_ok!(ICE::submit_intent(
			RuntimeOrigin::signed(BOB.into()),
			Intent {
				who: BOB.into(),
				swap: swap.clone(),
				deadline,
				partial: false,
				on_success: None,
				on_failure: None,
			}
		));

		let intent_id = pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent_id(deadline, 0);

		let expected_entry = pallet_ice::types::Intent {
			who: BOB.into(),
			swap,
			deadline,
			partial: false,
			on_success: None,
			on_failure: None,
		};
		assert_eq!(
			pallet_ice::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_id),
			Some(expected_entry)
		);
	});
}
