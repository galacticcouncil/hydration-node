#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::{assert_ok, traits::Get};
use hydradx_runtime::RuntimeOrigin;
use hydradx_traits::OraclePeriod;
use orml_traits::MultiCurrency;
use pallet_omnipool::types::Tradability;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

const DOT_UNITS: u128 = 10_000_000_000;

fn init_omnipool_with_oracle() {
	let native_price = FixedU128::from_inner(1_201_500_000_000_000);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	// Add tokens to omnipool
	assert_ok!(hydradx_runtime::Omnipool::add_token(
		RuntimeOrigin::root(),
		HDX,
		native_price,
		Permill::from_percent(10),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		RuntimeOrigin::root(),
		DAI,
		stable_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));

	assert_ok!(hydradx_runtime::Omnipool::add_token(
		RuntimeOrigin::root(),
		DOT,
		dot_price,
		Permill::from_percent(100),
		hydradx_runtime::Omnipool::protocol_account(),
	));
}

fn populate_oracle_for_dot() {
	// Give DAVE some tokens for trading
	set_balance(DAVE.into(), DOT, 1_000 * DOT_UNITS as i128);
	set_balance(DAVE.into(), HDX, 1_000_000 * UNITS as i128);

	// Execute trades to populate oracle with DOT data
	assert_ok!(hydradx_runtime::Omnipool::sell(
		RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		2 * DOT_UNITS,
		0,
	));

	hydradx_run_to_next_block();

	assert_ok!(hydradx_runtime::Omnipool::buy(
		RuntimeOrigin::signed(DAVE.into()),
		DOT,
		HDX,
		DOT_UNITS,
		u128::MAX,
	));

	hydradx_run_to_next_block();
}

#[test]
fn remove_token_should_clear_both_fees_and_oracle_entries() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle();
		populate_oracle_for_dot();

		// Verify both dynamic fees and oracle entries exist
		assert!(
			hydradx_runtime::DynamicFees::current_fees(DOT).is_some(),
			"DOT should have dynamic fees before removal"
		);

		// Verify AssetFee storage exists
		let asset_fee = pallet_dynamic_fees::AssetFee::<hydradx_runtime::Runtime>::get(DOT);
		assert!(asset_fee.is_some(), "DOT AssetFee storage should exist before removal");

		// Verify AssetFeeConfiguration storage exists
		let asset_fee_config = pallet_dynamic_fees::AssetFeeConfiguration::<hydradx_runtime::Runtime>::get(DOT);
		assert!(
			asset_fee_config.is_some(),
			"DOT AssetFeeConfiguration storage should exist before removal"
		);

		let hub_asset = LRNA;
		let assets = if DOT < hub_asset {
			(DOT, hub_asset)
		} else {
			(hub_asset, DOT)
		};

		// Check oracle entries exist for all supported periods
		let supported_periods = hydradx_runtime::SupportedPeriods::get();
		for period in supported_periods.iter() {
			let oracle_entry =
				pallet_ema_oracle::Oracles::<hydradx_runtime::Runtime>::get((*b"omnipool", assets, period));
			assert!(
				oracle_entry.is_some(),
				"Oracle entry should exist for DOT before removal for period {:?}",
				period
			);
		}

		// Verify whitelist entry exists
		let whitelist = pallet_ema_oracle::WhitelistedAssets::<hydradx_runtime::Runtime>::get();
		assert!(
			whitelist.contains(&(*b"omnipool", assets)),
			"DOT should be in oracle whitelist before removal"
		);

		// Act
		// Prepare for token removal
		let position_id = 2;
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			RuntimeOrigin::root(),
			DOT,
			Tradability::FROZEN
		));

		assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
			RuntimeOrigin::signed(hydradx_runtime::Omnipool::protocol_account()),
			position_id
		));

		// Remove token
		assert_ok!(hydradx_runtime::Omnipool::remove_token(
			RuntimeOrigin::root(),
			DOT,
			AccountId::from(BOB),
		));

		// Assert
		// Verify dynamic fees are cleared
		assert!(
			hydradx_runtime::DynamicFees::current_fees(DOT).is_none(),
			"DOT dynamic fees should be cleared"
		);

		// Verify AssetFee storage is cleared
		let asset_fee_after = pallet_dynamic_fees::AssetFee::<hydradx_runtime::Runtime>::get(DOT);
		assert!(asset_fee_after.is_none(), "DOT AssetFee storage should be cleared");

		// Verify AssetFeeConfiguration storage is cleared
		let asset_fee_config_after = pallet_dynamic_fees::AssetFeeConfiguration::<hydradx_runtime::Runtime>::get(DOT);
		assert!(
			asset_fee_config_after.is_none(),
			"DOT AssetFeeConfiguration storage should be cleared"
		);

		// Verify oracle entries are cleared for all periods
		for period in supported_periods.iter() {
			let oracle_entry =
				pallet_ema_oracle::Oracles::<hydradx_runtime::Runtime>::get((*b"omnipool", assets, period));
			assert!(
				oracle_entry.is_none(),
				"Oracle entry should be cleared for DOT after removal for period {:?}",
				period
			);
		}

		// Verify whitelist entry is removed
		let whitelist_after = pallet_ema_oracle::WhitelistedAssets::<hydradx_runtime::Runtime>::get();
		assert!(
			!whitelist_after.contains(&(*b"omnipool", assets)),
			"DOT should be removed from oracle whitelist"
		);

		// Verify Accumulator storage is cleared
		let accumulator = pallet_ema_oracle::Accumulator::<hydradx_runtime::Runtime>::get();
		assert!(
			!accumulator.contains_key(&(*b"omnipool", assets)),
			"DOT should be removed from oracle accumulator"
		);

		// Verify the asset itself is removed
		let asset_state = hydradx_runtime::Omnipool::assets(DOT);
		assert!(asset_state.is_none(), "DOT asset should be removed from omnipool");
	});
}

#[test]
fn remove_token_should_not_affect_other_assets() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// Arrange
		init_omnipool_with_oracle();
		populate_oracle_for_dot();

		// Get HDX fees and oracle entries before DOT removal
		let hdx_fees_before = hydradx_runtime::DynamicFees::current_fees(HDX);
		let dai_fees_before = hydradx_runtime::DynamicFees::current_fees(DAI);

		let hub_asset = LRNA;
		let hdx_assets = if HDX < hub_asset {
			(HDX, hub_asset)
		} else {
			(hub_asset, HDX)
		};

		let period = OraclePeriod::LastBlock;
		let hdx_oracle_before =
			pallet_ema_oracle::Oracles::<hydradx_runtime::Runtime>::get((*b"omnipool", hdx_assets, period));

		// Act
		// Remove DOT
		let position_id = 2;
		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			RuntimeOrigin::root(),
			DOT,
			Tradability::FROZEN
		));

		assert_ok!(hydradx_runtime::Omnipool::sacrifice_position(
			RuntimeOrigin::signed(hydradx_runtime::Omnipool::protocol_account()),
			position_id
		));

		assert_ok!(hydradx_runtime::Omnipool::remove_token(
			RuntimeOrigin::root(),
			DOT,
			AccountId::from(BOB),
		));

		// Assert
		// Other assets' fees and oracle entries should remain
		let hdx_fees_after = hydradx_runtime::DynamicFees::current_fees(HDX);
		let dai_fees_after = hydradx_runtime::DynamicFees::current_fees(DAI);

		assert_eq!(
			hdx_fees_before, hdx_fees_after,
			"HDX fees should not be affected by DOT removal"
		);
		assert_eq!(
			dai_fees_before, dai_fees_after,
			"DAI fees should not be affected by DOT removal"
		);

		let hdx_oracle_after =
			pallet_ema_oracle::Oracles::<hydradx_runtime::Runtime>::get((*b"omnipool", hdx_assets, period));

		assert_eq!(
			hdx_oracle_before, hdx_oracle_after,
			"HDX oracle entries should not be affected by DOT removal"
		);

		// Verify HDX and DAI are still in omnipool
		assert!(
			hydradx_runtime::Omnipool::assets(HDX).is_some(),
			"HDX should still be in omnipool"
		);
		assert!(
			hydradx_runtime::Omnipool::assets(DAI).is_some(),
			"DAI should still be in omnipool"
		);
	});
}
