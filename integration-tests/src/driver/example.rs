use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::assert_ok;
use hydradx_runtime::*;

#[test]
fn driver_test_example() {
	HydrationTestDriver::default()
		.setup_hydration()
		.execute(|| {
			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				CHARLIE.into(),
				DOT,
				20_000_000_000_000_000_000_000_000,
				0,
			));

			assert_ok!(Omnipool::sell(
				hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
				DOT,
				HDX,
				1_000_000_000_000,
				0u128,
			));
		})
		.new_block()
		.execute_with_driver(|driver| {
			// This is useful, so we can have access to some info stored in the driver itself
			// such as list of omnipool assets or stablepools
			let stable_pool_id = driver.stablepools[0].0;
			let stable_asset_a = driver.stablepools[0].1[0].0;
			let stable_asset_b = driver.stablepools[0].1[1].0;
			let stable_asset_a_decimals = driver.stablepools[0].1[0].1 as u32;

			assert_ok!(Tokens::set_balance(
				RawOrigin::Root.into(),
				CHARLIE.into(),
				stable_asset_a,
				10 * 10u128.pow(stable_asset_a_decimals),
				0,
			));

			assert_ok!(Stableswap::sell(
				hydradx_runtime::RuntimeOrigin::signed(CHARLIE.into()),
				stable_pool_id,
				stable_asset_a,
				stable_asset_b,
				10u128.pow(stable_asset_a_decimals),
				0u128,
			));
		});
}
