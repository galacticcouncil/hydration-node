#![cfg(test)]

use crate::polkadot_test_net::*;

use frame_support::assert_ok;
use frame_system::RawOrigin;
use hydradx_adapters::OraclePriceProviderAdapterForOmnipool;
use hydradx_runtime::{Omnipool, RuntimeOrigin, Tokens};
use hydradx_traits::{OraclePeriod, PriceOracle};
use primitives::{AssetId, Balance};
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

#[test]
fn omnipool_oracle_adapter_should_return_price_for_arbitraty_pairs() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipol();

		set_relaychain_block_number(100);

		let price =
			OraclePriceProviderAdapterForOmnipool::<AssetId, hydradx_runtime::EmaOracle, hydradx_runtime::LRNA>::price(
				HDX,
				DAI,
				OraclePeriod::Short,
			);

		assert!(price.is_some());
	});
}

#[test]
fn omnipool_oracle_adapter_should_return_price_for_when_lrna_is_asset_a() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipol();

		set_relaychain_block_number(100);

		let price =
			OraclePriceProviderAdapterForOmnipool::<AssetId, hydradx_runtime::EmaOracle, hydradx_runtime::LRNA>::price(
				LRNA,
				DAI,
				OraclePeriod::Short,
			);

		assert!(price.is_some());
	});
}

#[test]
fn omnipool_oracle_adapter_should_return_price_for_when_lrna_is_asset_b() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipol();

		set_relaychain_block_number(100);

		let price =
			OraclePriceProviderAdapterForOmnipool::<AssetId, hydradx_runtime::EmaOracle, hydradx_runtime::LRNA>::price(
				DAI,
				LRNA,
				OraclePeriod::Short,
			);

		assert!(price.is_some());
	});
}

pub fn init_omnipol() {
	let native_price = FixedU128::from_float(0.5);
	let stable_price = FixedU128::from_float(0.7);
	hydradx_runtime::Omnipool::protocol_account();

	assert_ok!(hydradx_runtime::Omnipool::set_tvl_cap(RuntimeOrigin::root(), u128::MAX));

	assert_ok!(hydradx_runtime::Omnipool::initialize_pool(
		hydradx_runtime::RuntimeOrigin::root(),
		stable_price,
		native_price,
		Permill::from_percent(60),
		Permill::from_percent(60)
	));

	do_trade_to_populate_oracle(HDX, DAI, UNITS);
}

fn do_trade_to_populate_oracle(asset_1: AssetId, asset_2: AssetId, amount: Balance) {
	assert_ok!(Tokens::set_balance(
		RawOrigin::Root.into(),
		CHARLIE.into(),
		LRNA,
		1000000000000 * UNITS,
		0,
	));

	assert_ok!(Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_1,
		amount,
		Balance::MIN
	));

	assert_ok!(Omnipool::sell(
		hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
		LRNA,
		asset_2,
		amount,
		Balance::MIN
	));
}
