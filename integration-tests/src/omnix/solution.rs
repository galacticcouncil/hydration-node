use super::*;
use frame_support::assert_ok;
use frame_support::weights::Weight;
use hydradx_runtime::{Currencies, OmniX, Omnipool, Router};
use hydradx_traits::router::AssetPair as Pair;
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;
use pallet_omnix::types::{BoundedInstructions, BoundedResolvedIntents, ProposedSolution, Solution};
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

		let solution = solve_intents(vec![(
			intent_ids[0],
			pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
		)])
		.unwrap();

		let hash = <hydradx_runtime::Runtime as frame_system::Config>::Hashing::hash(&solution.encode());

		assert_ok!(OmniX::submit_solution(RuntimeOrigin::signed(BOB.into()), solution,));

		assert!(pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_solution(hash).is_some());
	});
}
/*

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
		let solution = ProposedSolution::<AccountId, AssetId> {
			intents: resolved_intents.clone(),
			instructions: solved.instructions,
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			solution,
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
		assert_eq!(received, 8978102355397551u128);
	});
}

#[test]
fn execute_one_intent_solution_should_work_when_swapping_stable_asset_with_omnipool_asset() {
	Hydra::execute_with(|| {
		let (pool_id, assets) = crate::utils::pools::setup_omnipool_with_stable_subpool();
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			LRNA,
			pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
		));

		let route1 = vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: LRNA,
				asset_out: pool_id,
			},
			Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: pool_id,
				asset_out: assets[0],
			},
		];

		let asset_pair = Pair::new(LRNA, assets[0]);

		assert_ok!(Router::set_route(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			asset_pair,
			route1.clone()
		));

		let initial_hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		let initial_asset_balance = Currencies::free_balance(assets[0], &AccountId32::from(BOB));

		let deadline: Moment = NOW + 86_400_000;

		let intent_ids = submit_intents(vec![(
			BOB.into(),
			Swap {
				asset_in: HDX,
				asset_out: assets[0],
				amount_in: 1_000_000_000_000,
				amount_out: 1,
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
		let solution = ProposedSolution::<AccountId, AssetId> {
			intents: resolved_intents.clone(),
			instructions: solved.instructions,
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved_intents.into_inner(),
		));

		let hash = <hydradx_runtime::Runtime as frame_system::Config>::Hashing::hash(&solution.encode());
		assert_ok!(OmniX::execute_solution(RuntimeOrigin::signed(BOB.into()), hash));

		let hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		assert_eq!(hdx_balance, initial_hdx_balance - 1_000_000_000_000u128);
		let asset_balance = Currencies::free_balance(assets[0], &AccountId32::from(BOB));

		let lrna_balance = Currencies::free_balance(
			LRNA,
			&pallet_omnix::Pallet::<hydradx_runtime::Runtime>::holding_account(),
		);
		let received = asset_balance - initial_asset_balance;
		assert_eq!(received, 26117);
		assert_eq!(lrna_balance, 0u128);
	});
}

#[test]
fn execute_two_intents_solution_should_work() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool();
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			LRNA,
			pallet_omnipool::types::Tradability::SELL | pallet_omnipool::types::Tradability::BUY
		));

		let initial_hdx_balance = Currencies::free_balance(HDX, &AccountId32::from(BOB));
		let initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));
		let alice_initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(ALICE));

		let deadline: Moment = NOW + 86_400_000;

		let intent_ids = submit_intents(vec![
			(
				BOB.into(),
				Swap {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: 1_000_000_000_000,
					amount_out: 8_973_613_312_776_918,
					swap_type: pallet_omnix::types::SwapType::ExactIn,
				},
				deadline,
			),
			(
				ALICE.into(),
				Swap {
					asset_in: HDX,
					asset_out: DAI,
					amount_in: 1_000_000_000_000,
					amount_out: 8_973_613_312_776_918,
					swap_type: pallet_omnix::types::SwapType::ExactIn,
				},
				deadline,
			),
		]);

		let solved = solve_intents(vec![
			(
				intent_ids[0],
				pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[0]).unwrap(),
			),
			(
				intent_ids[1],
				pallet_omnix::Pallet::<hydradx_runtime::Runtime>::get_intent(intent_ids[1]).unwrap(),
			),
		])
		.unwrap();
		let resolved_intents = BoundedResolvedIntents::try_from(solved.intents).unwrap();
		let solution = Solution::<AccountId> {
			proposer: BOB.into(),
			intents: resolved_intents.clone(),
		};

		assert_ok!(OmniX::submit_solution(
			RuntimeOrigin::signed(BOB.into()),
			resolved_intents.into_inner(),
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

		let alice_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(ALICE));
		let received = alice_dai_balance - alice_initial_dai_balance;
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

#[test]
fn test_omnipool_swap_via_router() {
	Hydra::execute_with(|| {
		crate::utils::pools::setup_omnipool_with_stable_subpool();
		let initial_dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			DAI,
			1_000_000_000_000,
			0,
			vec![]
		));
		let dai_balance = Currencies::free_balance(DAI, &AccountId32::from(BOB));

		assert_eq!(dai_balance - initial_dai_balance, 8973613312776918);
	});
}

#[test]
fn test_omnipool_stable_swap() {
	Hydra::execute_with(|| {
		let (pool_id, assets) = crate::utils::pools::setup_omnipool_with_stable_subpool();

		let route1 = vec![
			Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out: pool_id,
			},
			Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in: pool_id,
				asset_out: assets[0],
			},
		];

		let asset_pair = Pair::new(HDX, assets[0]);

		assert_ok!(Router::set_route(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			asset_pair,
			route1.clone()
		));

		let initial_stable_balance = Currencies::free_balance(assets[0], &AccountId32::from(BOB));
		assert_ok!(Router::sell(
			RuntimeOrigin::signed(BOB.into()),
			HDX,
			assets[0],
			1_000_000_000_000,
			0,
			vec![]
		));
		let stable_balance = Currencies::free_balance(assets[0], &AccountId32::from(BOB));

		assert_eq!(stable_balance - initial_stable_balance, 26105);
	});
}


 */
