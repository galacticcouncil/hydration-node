#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_runtime::Omnipool;
use orml_traits::MultiCurrency;
use primitives::constants::chain::CORE_ASSET_ID;
use sp_runtime::Permill;
use sp_runtime::{FixedU128, Percent};
use xcm_emulator::TestExt;

pub const MAX_TRADE_LIMIT_PER_BLOCK_IN_HYDRA: Percent = Percent::from_percent(20);

#[test]
fn sell_in_omnipool_should_fail_when_max_trade_limit_per_block_exceeded() {
	Hydra::execute_with(|| {
		//Arrange
		const LRNA: u32 = 1;
		assert_ok!(Omnipool::initialize_pool(
			RawOrigin::Root.into(),
			FixedU128::from_float(0.2),
			FixedU128::from(1),
			Permill::from_percent(100),
			Permill::from_percent(100)
		));

		let lrna_balance_in_omnipool = hydradx_runtime::Tokens::free_balance(LRNA, &omnipool_protocol_account());
		let sell_amount = MAX_TRADE_LIMIT_PER_BLOCK_IN_HYDRA.mul_floor(lrna_balance_in_omnipool) + UNITS;
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
