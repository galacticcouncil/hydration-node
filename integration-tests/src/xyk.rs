#![cfg(test)]

use crate::polkadot_test_net::*;

use hydradx_runtime::{DustRemovalWhitelist, RuntimeOrigin, LBP, XYK};
use hydradx_traits::AMM;
use pallet_xyk::types::AssetPair;
use xcm_emulator::TestExt;

use frame_support::{assert_noop, assert_ok, traits::Contains};

fn pair_account(asset_a: AssetId, asset_b: AssetId) -> AccountId {
	let asset_pair = AssetPair {
		asset_in: asset_a,
		asset_out: asset_b,
	};
	XYK::get_pair_id(asset_pair)
}

#[test]
fn pair_account_should_be_added_into_whitelist_when_pool_is_created() {
	TestNet::reset();

	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		//arrange & act
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));

		//assert
		assert!(DustRemovalWhitelist::contains(&pair_account(asset_a, asset_b)));
	});
}

#[test]
fn pair_account_should_be_removed_from_whitelist_when_pool_was_destroyed() {
	TestNet::reset();

	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		//arrange
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));
		assert!(DustRemovalWhitelist::contains(&pair_account(asset_a, asset_b)));

		//act
		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			asset_b,
			100 * UNITS,
			0,
			0
		));

		//assert
		assert!(!DustRemovalWhitelist::contains(&pair_account(asset_a, asset_b)));
	});
}

#[test]
fn pool_should_be_created_when_it_was_destroyed_previously() {
	TestNet::reset();

	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		//arrange
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));
		assert_ok!(XYK::remove_liquidity(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			asset_b,
			100 * UNITS,
			0,
			0
		));

		//act & assert
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));
	});
}

#[test]
fn share_asset_id_should_be_offset() {
	TestNet::reset();

	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		//arrange
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));

		let share_token = XYK::get_share_token(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});
		let offset = <hydradx_runtime::Runtime as pallet_asset_registry::Config>::SequentialIdStartAt::get();
		//assert
		assert!(share_token >= offset);
	});
}

#[test]
fn creating_xyk_pool_should_fail_when_asset_is_pool_share_asset() {
	TestNet::reset();
	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		//arrange
		assert_ok!(XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			100 * UNITS,
			asset_b,
			200 * UNITS,
		));

		let share_token = XYK::get_share_token(AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_noop!(
			XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				share_token,
				100 * UNITS,
				asset_b,
				200 * UNITS,
			),
			pallet_xyk::Error::<hydradx_runtime::Runtime>::CannotCreatePool
		);
		assert_noop!(
			XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				asset_a,
				100 * UNITS,
				share_token,
				200 * UNITS,
			),
			pallet_xyk::Error::<hydradx_runtime::Runtime>::CannotCreatePool
		);
	});
}

#[test]
fn creating_xyk_pool_should_fail_when_lbp_pool_is_running() {
	TestNet::reset();
	let asset_a = 1;
	let asset_b = 2;

	Hydra::execute_with(|| {
		// Arrange
		assert_ok!(LBP::create_pool(
			RuntimeOrigin::root(),
			ALICE.into(),
			asset_a,
			1_000_000_000,
			asset_b,
			2_000_000_000,
			20_000_000u32,
			90_000_000u32,
			pallet_lbp::WeightCurveType::Linear,
			(2, 1_000),
			ALICE.into(),
			0,
		));

		let pool_id = LBP::get_pair_id(pallet_lbp::AssetPair {
			asset_in: asset_a,
			asset_out: asset_b,
		});

		assert_ok!(LBP::update_pool_data(
			RuntimeOrigin::signed(ALICE.into()),
			pool_id,
			None,
			Some(10),
			Some(100),
			None,
			None,
			None,
			None,
			None,
		));

		// running LBP
		hydradx_run_to_block(20);

		// Act & Assert
		assert_noop!(
			XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				asset_a,
				100 * UNITS,
				asset_b,
				200 * UNITS,
			),
			pallet_xyk::Error::<hydradx_runtime::Runtime>::CannotCreatePool
		);
	});
}
