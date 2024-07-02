use super::*;
use frame_support::assert_ok;
use hydradx_runtime::OmniX;
use pallet_omnix::types::{BoundedPrices, BoundedResolvedIntents, Solution};
use sp_core::Encode;
use sp_runtime::traits::Hash;

#[test]
fn submit_solution_should_work() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool();

		let deadline: Moment = NOW + 86_400_000;

		let intent_ids = submit_intents(vec![(
			BOB.into(),
			Swap {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 1_000_000_000_000,
				amount_out: 0,
				swap_type: pallet_omnix::types::SwapType::ExactIn,
			},
			deadline,
		)]);

		let resolved_intents = BoundedResolvedIntents::try_from(vec![pallet_omnix::types::ResolvedIntent {
			intent_id: intent_ids[0],
			amount: 1_000_000_000_000,
		}])
		.unwrap();

		let sell_prices = BoundedPrices::new();
		let buy_prices = BoundedPrices::new();

		let solution = Solution::<AccountId, AssetId> {
			proposer: BOB.into(),
			intents: resolved_intents.clone(),
			sell_prices: sell_prices.clone(),
			buy_prices: buy_prices.clone(),
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved_intents.into_inner(),
			sell_prices.into_inner(),
			buy_prices.into_inner(),
		));

		let hash = <hydradx_runtime::Runtime as frame_system::Config>::Hashing::hash(&solution.encode());
	});
}
