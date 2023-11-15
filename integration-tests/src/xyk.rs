#![cfg(test)]

use crate::polkadot_test_net::*;

use hydradx_runtime::{DustRemovalWhitelist, RuntimeOrigin, XYK};
use hydradx_traits::AMM;
use pallet_xyk::types::AssetPair;
use xcm_emulator::TestExt;

use frame_support::{assert_ok, traits::Contains};

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
			100 * UNITS
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
			100 * UNITS
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
