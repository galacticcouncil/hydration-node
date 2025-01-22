#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::traits::OnFinalize;
use frame_support::traits::OnInitialize;
use frame_support::{
	assert_ok,
	sp_runtime::{FixedU128, Permill},
	traits::tokens::fungibles::Mutate,
};
use hydradx_runtime::{EmaOracle, RuntimeOrigin};
use hydradx_traits::{
	AggregatedPriceOracle,
	OraclePeriod::{self, *},
};

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
		let expected_b = ((87719064743683, 2250006019587887).into(), 0);
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
		let expected = ((936324588000000000, 1125006022570633).into(), 0);
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
