use super::*;
use frame_support::assert_ok;
use hydradx_runtime::{Currencies, OmniX, Omnipool};
use orml_traits::MultiCurrency;
use pallet_omnix::types::{BoundedPrices, BoundedResolvedIntents, Solution};
use sp_core::crypto::AccountId32;
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

		let solved = solve_intents(vec![(
			intent_ids[0],
			pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)])
		.unwrap();
		let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
		let sell_prices = solved.sell_prices;
		let buy_prices = solved.buy_prices;

		let b_sell_prices = BoundedPrices::try_from(sell_prices.clone()).unwrap();
		let b_buy_prices = BoundedPrices::try_from(buy_prices.clone()).unwrap();

		let solution = Solution::<AccountId, AssetId> {
			proposer: BOB.into(),
			intents: resolved_intents.clone(),
			sell_prices: b_sell_prices,
			buy_prices: b_buy_prices,
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved_intents.into_inner(),
			sell_prices,
			buy_prices,
		));

		let hash = <hydradx_runtime::Runtime as frame_system::Config>::Hashing::hash(&solution.encode());
		assert!(pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_solution(hash).is_some());
	});
}

#[test]
fn execute_one_intent_solution_should_work() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool();
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			LRNA,
			pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
		));

		let initial_hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		let initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));

		let deadline: Moment = NOW + 86_400_000;

		let intent_ids = submit_intents(vec![(
			BOB.into(),
			Swap {
				asset_in: HDX,
				asset_out: DAI,
				amount_in: 1_000_000_000_000,
				amount_out: 8_973_613_312_776_918,
				swap_type: pallet_omnix::types::SwapType::ExactIn,
			},
			deadline,
		)]);

		let solved = solve_intents(vec![(
			intent_ids[0],
			pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)])
		.unwrap();
		let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
		let sell_prices = solved.sell_prices;
		let buy_prices = solved.buy_prices;

		let b_sell_prices = BoundedPrices::try_from(sell_prices.clone()).unwrap();
		let b_buy_prices = BoundedPrices::try_from(buy_prices.clone()).unwrap();

		let solution = Solution::<AccountId, AssetId> {
			proposer: BOB.into(),
			intents: resolved_intents.clone(),
			sell_prices: b_sell_prices,
			buy_prices: b_buy_prices,
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved_intents.into_inner(),
			sell_prices,
			buy_prices,
		));

		let hash = <hydradx_runtime::Runtime as frame_system::Config>::Hashing::hash(&solution.encode());
		assert_ok!(OmniX::execute_solution(RuntimeOrigin::signed(BOB.into()), hash));

		let hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		assert_eq!(hdx_balance, initial_hdx_balance - 1_000_000_000_000u128);
		let dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));

		let lrna_balance = Currencies::free_balance(
			LRNA,
			&pallet_omnix::Pallet::<hydradx_runtime::Runtime>::holding_account(),
		);
		assert_eq!(lrna_balance, 0u128);
		let received = dai_balance - initial_dai_balance;
		assert_eq!(received, 8978102355397552u128);
	});
}

#[test]
fn test_omnipool_swap() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool();
		let initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));
		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0,
		));
		let dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));

		assert_eq!(dai_balance - initial_dai_balance, 8973613312776918);
	});
}
