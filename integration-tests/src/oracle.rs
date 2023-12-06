#![cfg(test)]

use crate::polkadot_test_net::*;

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
use primitives::constants::chain::OMNIPOOL_SOURCE;
use xcm_emulator::TestExt;

const HDX: AssetId = CORE_ASSET_ID;

const SUPPORTED_PERIODS: &[OraclePeriod] = &[LastBlock, Short, TenMinutes];
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
		let expected_a = ((936334588000000000, 1124993995517813).into(), 0);
		let expected_b = ((87719064743683, 2250006004576687).into(), 0);
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
