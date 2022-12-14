#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::{CircuitBreaker, Omnipool, Tokens};
use orml_traits::MultiCurrency;
use primitives::constants::chain::CORE_ASSET_ID;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use xcm_emulator::TestExt;

#[test]
fn sell_in_omnipool_should_fail_when_max_trade_limit_per_block_exceeded() {
	Hydra::execute_with(|| {
		//Arrange
		assert_ok!(Omnipool::initialize_pool(
			RawOrigin::Root.into(),
			FixedU128::from_float(0.00001),
			FixedU128::from(1),
			Permill::from_percent(100),
			Permill::from_percent(100)
		));

		let lrna_balance_in_omnipool = Tokens::free_balance(LRNA, &Omnipool::protocol_account());
		let trade_volume_limit = CircuitBreaker::trade_volume_limit_per_asset(CORE_ASSET_ID);
		let sell_amount =
			CircuitBreaker::calculate_limit(lrna_balance_in_omnipool, trade_volume_limit).unwrap() + UNITS;
		assert_ok!(Tokens::set_balance(
			RawOrigin::Root.into(),
			ALICE.into(),
			LRNA,
			sell_amount,
			0,
		));
		let min_limit = 0;

		//Act and assert
		assert_noop!(
			Omnipool::sell(
				hydradx_runtime::Origin::signed(ALICE.into()),
				LRNA,
				CORE_ASSET_ID,
				sell_amount,
				min_limit
			),
			pallet_circuit_breaker::Error::<hydradx_runtime::Runtime>::MaxTradeVolumePerBlockReached
		);
	});
}
