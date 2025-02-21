#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::storage::with_transaction;
use frame_support::traits::OnFinalize;
use frame_support::traits::OnInitialize;
use frame_support::{
	assert_ok,
	sp_runtime::{FixedU128, Permill},
	traits::tokens::fungibles::Mutate,
};
use hydradx_traits::Create;
use sp_runtime::DispatchResult;
use sp_runtime::TransactionOutcome;
use sp_std::sync::Arc;

use hydradx_runtime::bifrost_account;
use hydradx_runtime::AssetLocation;
use hydradx_runtime::AssetRegistry;
use hydradx_runtime::{EmaOracle, RuntimeOrigin};
use hydradx_traits::AssetKind;
use hydradx_traits::{
	AggregatedPriceOracle,
	OraclePeriod::{self, *},
};
use pallet_ema_oracle::BIFROST_SOURCE;

use hydra_dx_math::ema::smoothing_from_period;

use pallet_ema_oracle::into_smoothing;
use pallet_ema_oracle::OracleError;
use primitives::constants::chain::{OMNIPOOL_SOURCE, XYK_SOURCE};
use xcm_emulator::TestExt;

pub fn hydradx_run_to_block(to: BlockNumber) {
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::EmaOracle::on_finalize(b);
		hydradx_runtime::TransactionPayment::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::EmaOracle::on_initialize(b + 1);
		hydradx_runtime::DynamicEvmFee::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

const HDX: AssetId = CORE_ASSET_ID;

pub(crate) const SUPPORTED_PERIODS: &[OraclePeriod] = &[LastBlock, Short, TenMinutes];
const UNSUPPORTED_PERIODS: &[OraclePeriod] = &[Hour, Day, Week];

#[ignore]
#[test]
fn oracle_smoothing_period_matches_configuration() {
	for supported_period in SUPPORTED_PERIODS {
		let configured_length = supported_period.as_period();
		let configured_smoothing = into_smoothing(*supported_period);
		let smoothing_from_period = smoothing_from_period(configured_length);
		assert_eq!(
			configured_smoothing, smoothing_from_period,
			"Smoothing period for {:?} does not match configured length of {:?}",
			supported_period, configured_length,
		);
	}
}

#[test]
fn omnipool_trades_are_ingested_into_oracle() {
	TestNet::reset();

	let asset_a = HDX;
	let asset_b = DOT;

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_next_block();

		init_omnipool();

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::sell(
			RuntimeOrigin::signed(ALICE.into()),
			asset_a,
			asset_b,
			5 * UNITS,
			0,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_next_block();

		// assert
		let expected_a = ((936334588000000000, 1124993992514080).into(), 0);
		let expected_b = ((87719064509592, 2250006013583407).into(), 0);
		for supported_period in SUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(asset_a, LRNA, *supported_period, OMNIPOOL_SOURCE),
				Ok(expected_a)
			);
			assert_eq!(
				EmaOracle::get_price(asset_b, LRNA, *supported_period, OMNIPOOL_SOURCE),
				Ok(expected_b)
			);
		}
		for unsupported_period in UNSUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(asset_a, LRNA, *unsupported_period, OMNIPOOL_SOURCE),
				Err(OracleError::NotPresent)
			);
			assert_eq!(
				EmaOracle::get_price(asset_b, LRNA, *unsupported_period, OMNIPOOL_SOURCE),
				Err(OracleError::NotPresent)
			);
		}
	});
}

#[test]
fn omnipool_hub_asset_trades_are_ingested_into_oracle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_next_block();

		init_omnipool();

		assert_ok!(hydradx_runtime::Tokens::mint_into(LRNA, &ALICE.into(), 5 * UNITS,));

		assert_ok!(hydradx_runtime::Omnipool::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			LRNA,
			5 * UNITS,
			5 * UNITS,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_next_block();

		// assert
		let expected = ((936324588000000000, 1125006025563847).into(), 0);
		for supported_period in SUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(HDX, LRNA, *supported_period, OMNIPOOL_SOURCE),
				Ok(expected)
			);
		}
		for unsupported_period in UNSUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(HDX, LRNA, *unsupported_period, OMNIPOOL_SOURCE),
				Err(OracleError::NotPresent)
			);
		}
	});
}

#[test]
fn xyk_trades_with_insufficient_asset_are_not_tracked_by_oracle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_next_block();

		assert_ok!(hydradx_runtime::Tokens::mint_into(
			INSUFFICIENT_ASSET,
			&ALICE.into(),
			200 * UNITS,
		));

		assert_ok!(hydradx_runtime::XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			100 * UNITS,
			INSUFFICIENT_ASSET,
			100 * UNITS,
		));

		assert_ok!(hydradx_runtime::XYK::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			INSUFFICIENT_ASSET,
			2 * UNITS,
			200 * UNITS,
			false,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_next_block();

		// assert
		for supported_period in SUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(HDX, INSUFFICIENT_ASSET, *supported_period, XYK_SOURCE),
				Err(OracleError::NotPresent)
			);
		}
		for unsupported_period in UNSUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(HDX, INSUFFICIENT_ASSET, *unsupported_period, XYK_SOURCE),
				Err(OracleError::NotPresent)
			);
		}
	});
}

#[test]
fn xyk_trades_with_insufficient_asset_are_tracked_by_oracle_when_asset_is_whitelisted() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_next_block();

		assert_ok!(hydradx_runtime::Tokens::mint_into(
			INSUFFICIENT_ASSET,
			&ALICE.into(),
			200 * UNITS,
		));

		assert_ok!(hydradx_runtime::XYK::create_pool(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			100 * UNITS,
			INSUFFICIENT_ASSET,
			100 * UNITS,
		));

		assert_ok!(EmaOracle::add_oracle(
			RuntimeOrigin::root(),
			XYK_SOURCE,
			(HDX, INSUFFICIENT_ASSET)
		));

		assert_ok!(hydradx_runtime::XYK::buy(
			RuntimeOrigin::signed(ALICE.into()),
			HDX,
			INSUFFICIENT_ASSET,
			2 * UNITS,
			200 * UNITS,
			false,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_next_block();

		// assert
		for supported_period in SUPPORTED_PERIODS {
			assert!(EmaOracle::get_price(HDX, INSUFFICIENT_ASSET, *supported_period, XYK_SOURCE).is_ok(),);
		}
		for unsupported_period in UNSUPPORTED_PERIODS {
			assert_eq!(
				EmaOracle::get_price(HDX, INSUFFICIENT_ASSET, *unsupported_period, XYK_SOURCE),
				Err(OracleError::NotPresent)
			);
		}
	});
}

#[test]
fn bifrost_oracle_should_be_udpdated() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// arrange
			hydradx_run_to_next_block();

			let asset_a_id = 50;
			let asset_b_id = 51;

			let asset_a_loc = polkadot_xcm::v4::Location::new(
				1,
				polkadot_xcm::v4::Junctions::X2(Arc::new([
					polkadot_xcm::v4::Junction::Parachain(1500),
					polkadot_xcm::v4::Junction::GeneralIndex(0),
				])),
			);
			let asset_b_loc = polkadot_xcm::v4::Location::new(
				1,
				polkadot_xcm::v4::Junctions::X2(Arc::new([
					polkadot_xcm::v4::Junction::Parachain(2000),
					polkadot_xcm::v4::Junction::GeneralIndex(0),
				])),
			);

			assert_ok!(AssetRegistry::register_sufficient_asset(
				Some(asset_a_id),
				Some(b"ASS1".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1_000_000,
				None,
				None,
				Some(AssetLocation::try_from(asset_a_loc.clone()).unwrap()),
				None,
			));

			assert_ok!(AssetRegistry::register_sufficient_asset(
				Some(asset_b_id),
				Some(b"ASS2".to_vec().try_into().unwrap()),
				AssetKind::Token,
				1_000_000,
				None,
				None,
				Some(AssetLocation::try_from(asset_b_loc.clone()).unwrap()),
				None,
			));

			assert_ok!(EmaOracle::add_oracle(
				RuntimeOrigin::root(),
				BIFROST_SOURCE,
				(asset_a_id, asset_b_id)
			));

			// act
			let asset_a = Box::new(asset_a_loc.into_versioned());
			let asset_b = Box::new(asset_b_loc.into_versioned());
			assert_ok!(EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(bifrost_account()),
				asset_a,
				asset_b,
				(50, 100)
			));
			// will store the data received in the sell as oracle values
			hydradx_run_to_next_block();

			// assert
			for supported_period in SUPPORTED_PERIODS {
				assert!(EmaOracle::get_price(asset_a_id, asset_b_id, *supported_period, BIFROST_SOURCE).is_ok(),);
			}
			for unsupported_period in UNSUPPORTED_PERIODS {
				assert_eq!(
					EmaOracle::get_price(asset_a_id, asset_b_id, *unsupported_period, BIFROST_SOURCE),
					Err(OracleError::NotPresent)
				);
			}

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}

#[test]
fn bifrost_oracle_should_be_added_and_updated_when_not_exist() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let _ = with_transaction(|| {
			// arrange
			hydradx_run_to_next_block();

			let asset_a_id = 50;
			let asset_b_id = 51;

			let asset_a_loc = polkadot_xcm::v4::Location::new(
				1,
				polkadot_xcm::v4::Junctions::X2(Arc::new([
					polkadot_xcm::v4::Junction::Parachain(1500),
					polkadot_xcm::v4::Junction::GeneralIndex(0),
				])),
			);
			let asset_b_loc = polkadot_xcm::v4::Location::new(
				1,
				polkadot_xcm::v4::Junctions::X2(Arc::new([
					polkadot_xcm::v4::Junction::Parachain(2000),
					polkadot_xcm::v4::Junction::GeneralIndex(0),
				])),
			);

			assert_ok!(AssetRegistry::register_insufficient_asset(
				Some(asset_a_id),
				Some(b"ASS1".to_vec().try_into().unwrap()),
				AssetKind::Token,
				Some(1_000_000),
				None,
				None,
				Some(AssetLocation::try_from(asset_a_loc.clone()).unwrap()),
				None,
			));

			assert_ok!(AssetRegistry::register_insufficient_asset(
				Some(asset_b_id),
				Some(b"ASS2".to_vec().try_into().unwrap()),
				AssetKind::Token,
				Some(1_000_000),
				None,
				None,
				Some(AssetLocation::try_from(asset_b_loc.clone()).unwrap()),
				None,
			));

			// act
			let asset_a = Box::new(asset_a_loc.into_versioned());
			let asset_b = Box::new(asset_b_loc.into_versioned());
			assert_ok!(EmaOracle::update_bifrost_oracle(
				RuntimeOrigin::signed(bifrost_account()),
				asset_a,
				asset_b,
				(50, 100)
			));
			// will store the data received in the sell as oracle values
			hydradx_run_to_next_block();

			// assert
			for supported_period in SUPPORTED_PERIODS {
				assert!(EmaOracle::get_price(asset_a_id, asset_b_id, *supported_period, BIFROST_SOURCE).is_ok(),);
			}
			for unsupported_period in UNSUPPORTED_PERIODS {
				assert_eq!(
					EmaOracle::get_price(asset_a_id, asset_b_id, *unsupported_period, BIFROST_SOURCE),
					Err(OracleError::NotPresent)
				);
			}

			TransactionOutcome::Commit(DispatchResult::Ok(()))
		});
	});
}
