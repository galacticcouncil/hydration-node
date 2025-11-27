#![cfg(test)]

use crate::omnipool_init::hydra_run_to_block;
use crate::polkadot_test_net::*;

use frame_support::{assert_noop, assert_ok};
use pallet_omnipool::types::Tradability;
use sp_runtime::FixedPointNumber;
use sp_runtime::{FixedU128, Permill};
use xcm_emulator::TestExt;

#[test]
fn remove_all_liquidity_should_work() {
	TestNet::reset();

	Hydra::execute_with(|| {
		init_omnipool();

		let lp = AccountId::from(BOB);
		let dot_price = FixedU128::from_inner(25_650_000_000_000_000_000);
		let position_id = hydradx_runtime::Omnipool::next_position_id();

		assert_ok!(hydradx_runtime::Omnipool::add_token(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			dot_price,
			Permill::from_percent(100),
			lp.clone(),
		));

		hydra_run_to_block(10);

		assert_ok!(hydradx_runtime::Omnipool::set_asset_tradable_state(
			hydradx_runtime::RuntimeOrigin::root(),
			DOT,
			Tradability::ADD_LIQUIDITY | Tradability::REMOVE_LIQUIDITY
		));

		let position =
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, lp.clone()).unwrap();

		assert_ok!(hydradx_runtime::Omnipool::remove_all_liquidity(
			hydradx_runtime::RuntimeOrigin::signed(lp.clone().into()),
			position_id,
			Balance::MIN,
		));

		assert_noop!(
			pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_position(position_id, lp.clone()),
			pallet_omnipool::Error::<hydradx_runtime::Runtime>::Forbidden,
		);

		let dot_state = pallet_omnipool::Pallet::<hydradx_runtime::Runtime>::load_asset_state(DOT).unwrap();
		assert_eq!(dot_state.shares, 0);
	});
}
