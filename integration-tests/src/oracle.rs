#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::traits::tokens::fungibles::Mutate;
use frame_support::{
	assert_ok,
	traits::{OnFinalize, OnInitialize},
};
use hydradx_runtime::{EmaOracle, Origin};
use hydradx_traits::{AggregatedPriceOracle, OraclePeriod::*};
use pallet_ema_oracle::OracleError;
use polkadot_primitives::v2::BlockNumber;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

pub fn hydradx_run_to_block(to: BlockNumber) {
	while hydradx_runtime::System::block_number() < to {
		let b = hydradx_runtime::System::block_number();

		hydradx_runtime::System::on_finalize(b);
		hydradx_runtime::EmaOracle::on_finalize(b);

		hydradx_runtime::System::on_initialize(b + 1);
		hydradx_runtime::EmaOracle::on_initialize(b + 1);

		hydradx_runtime::System::set_block_number(b + 1);
	}
}

use common_runtime::{adapters::OMNIPOOL_SOURCE, AssetId, CORE_ASSET_ID};

const HDX: AssetId = CORE_ASSET_ID;

#[test]
fn omnipool_trades_are_ingested_into_oracle() {
	TestNet::reset();

	let asset_a = HDX;
	let asset_b = DOT;

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_block(2);

		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
			hydradx_runtime::Origin::root(),
			522_222_000_000_000_000_000_000,
		));

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::Origin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));

		let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::Origin::root(),
			DOT,
			token_price,
			Permill::from_percent(100),
			AccountId::from(BOB),
		));

		assert_ok!(hydradx_runtime::Omnipool::sell(
			Origin::signed(ALICE.into()),
			asset_a,
			asset_b,
			5 * UNITS,
			0,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_block(3);

		// assert
		let expected_a = ((5000000000000, 6007467920).into(), 0);
		let expected_b = ((233506317, 6004464187).into(), 0);
		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, LastBlock, OMNIPOOL_SOURCE),
			Ok(expected_a)
		);
		assert_eq!(
			EmaOracle::get_price(asset_b, LRNA, LastBlock, OMNIPOOL_SOURCE),
			Ok(expected_b)
		);
		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, Hour, OMNIPOOL_SOURCE),
			Err(OracleError::NotPresent)
		);
		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, Hour, OMNIPOOL_SOURCE),
			Err(OracleError::NotPresent)
		);

		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, TenMinutes, OMNIPOOL_SOURCE),
			Ok(expected_a)
		);
		assert_eq!(
			EmaOracle::get_price(asset_b, LRNA, TenMinutes, OMNIPOOL_SOURCE),
			Ok(expected_b)
		);

		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, Day, OMNIPOOL_SOURCE),
			Ok(expected_a)
		);
		assert_eq!(
			EmaOracle::get_price(asset_a, LRNA, Week, OMNIPOOL_SOURCE),
			Ok(expected_a)
		);

		assert_eq!(
			EmaOracle::get_price(asset_b, LRNA, Day, OMNIPOOL_SOURCE),
			Ok(expected_b)
		);
		assert_eq!(
			EmaOracle::get_price(asset_b, LRNA, Week, OMNIPOOL_SOURCE),
			Ok(expected_b)
		);
	});
}

#[test]
fn omnipool_hub_asset_trades_are_ingested_into_oracle() {
	TestNet::reset();

	Hydra::execute_with(|| {
		// arrange
		hydradx_run_to_block(2);

		let native_price = FixedU128::from_inner(1201500000000000);
		let stable_price = FixedU128::from_inner(45_000_000_000);

		assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(
			hydradx_runtime::Origin::root(),
			522_222_000_000_000_000_000_000,
		));

		assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
			hydradx_runtime::Origin::root(),
			stable_price,
			native_price,
			Permill::from_percent(100),
			Permill::from_percent(10)
		));

		assert_ok!(hydradx_runtime::Tokens::mint_into(LRNA, &ALICE.into(), 5 * UNITS,));

		assert_ok!(hydradx_runtime::Omnipool::buy(
			Origin::signed(ALICE.into()),
			HDX,
			LRNA,
			5 * UNITS,
			5 * UNITS,
		));

		// act
		// will store the data received in the sell as oracle values
		hydradx_run_to_block(3);

		// assert
		let expected = ((5000000000000, 6022588633).into(), 0);
		assert_eq!(
			EmaOracle::get_price(HDX, LRNA, LastBlock, OMNIPOOL_SOURCE),
			Ok(expected)
		);
		assert_eq!(
			EmaOracle::get_price(HDX, LRNA, TenMinutes, OMNIPOOL_SOURCE),
			Ok(expected)
		);
	});
}
