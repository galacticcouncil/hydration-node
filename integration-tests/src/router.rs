#![cfg(test)]

use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::Router;
use hydradx_traits::router::PoolType;
use pallet_route_executor::Trade;
use xcm_emulator::TestExt;

//NOTE: XYK pool is not supported in HydraDX. If you want to support it, also adjust router and dca benchmarking
#[test]
fn router_should_not_work_for_xyk() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let trades = vec![Trade {
			pool: PoolType::XYK,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}

//NOTE: LBP pool is not supported in HydraDX. If you want to support it, also adjust router and dca benchmarking
#[test]
fn router_should_not_work_for_lbp() {
	TestNet::reset();

	Hydra::execute_with(|| {
		let trades = vec![Trade {
			pool: PoolType::LBP,
			asset_in: HDX,
			asset_out: DAI,
		}];

		assert_noop!(
			Router::sell(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				HDX,
				DAI,
				100 * UNITS,
				0,
				trades
			),
			pallet_route_executor::Error::<hydradx_runtime::Runtime>::PoolNotSupported
		);
	});
}
