use super::*;
use frame_support::assert_noop;

#[test]
fn add_liquidity_should_fail_when_tvl_is_reached() {
	let stable_amount = 50_000 * ONE * 1_000_000;
	let native_amount = 936_329_588_000_000_000u128;
	let dot_amount = 87_719_298_250_000_u128;

	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	let dot_id = 1_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_amount),
			(Omnipool::protocol_account(), HDX, native_amount),
			(LP1, DAI, 200_000 * ONE * 1_000_000),
			(LP2, dot_id, dot_amount * 1000),
		])
		.with_registered_asset(dot_id)
		.with_initial_pool(stable_price, native_price)
		.with_token(1_000, token_price, LP2, dot_amount)
		.with_tvl_cap(222_222 * ONE * 1_000_000)
		.build()
		.execute_with(|| {
			assert_pool_state!(
				5625000000094500,
				125000000002100000000000,
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			assert_noop!(
				Omnipool::add_liquidity(RuntimeOrigin::signed(LP1), DAI, 100_000 * ONE * 1_000_000),
				Error::<Test>::TVLCapExceeded
			);
		});
}

#[test]
fn remove_liquidity_should_work_when_tvl_is_reached() {
	let stable_amount = 50_000 * ONE * 1_000_000;
	let native_amount = 936_329_588_000_000_000u128;
	let dot_amount = 87_719_298_250_000_u128;

	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	let dot_id = 1_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_amount),
			(Omnipool::protocol_account(), HDX, native_amount),
			(LP1, DAI, 200_000 * ONE * 1_000_000),
			(LP2, dot_id, dot_amount),
			(LP2, DAI, 20_000 * ONE * 1_000_000),
		])
		.with_registered_asset(dot_id)
		.with_initial_pool(stable_price, native_price)
		.with_token(1_000, token_price, LP2, dot_amount)
		.with_tvl_cap(222_222 * ONE * 1_000_000)
		.build()
		.execute_with(|| {
			let position_id = <NextPositionId<Test>>::get();
			// Ensure that tvl cap has been reached
			assert_ok!(Omnipool::add_liquidity(
				RuntimeOrigin::signed(LP1),
				DAI,
				97_000 * ONE * 1_000_000
			),);

			assert_ok!(Omnipool::sell(
				RuntimeOrigin::signed(LP2),
				DAI,
				dot_id,
				10_000 * ONE * 1_000_000,
				0u128
			));

			assert_pool_state!(
				9990000000094500,
				253231431350444840163976, // current tvl
				SimpleImbalance {
					value: 0u128,
					negative: true
				}
			);

			let position = Positions::<Test>::get(position_id).unwrap();

			assert_ok!(Omnipool::remove_liquidity(
				RuntimeOrigin::signed(LP1),
				position_id,
				position.shares / 100
			),);
		});
}

#[test]
fn set_tvl_cap_should_work() {
	let stable_amount = 50_000 * ONE * 1_000_000;
	let native_amount = 936_329_588_000_000_000u128;
	let dot_amount = 87_719_298_250_000_u128;

	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	let dot_id = 1_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_amount),
			(Omnipool::protocol_account(), HDX, native_amount),
			(LP1, DAI, 200_000 * ONE * 1_000_000),
			(LP2, dot_id, dot_amount),
			(LP2, DAI, 20_000 * ONE * 1_000_000),
		])
		.with_registered_asset(dot_id)
		.with_initial_pool(stable_price, native_price)
		.with_token(1_000, token_price, LP2, dot_amount)
		.with_tvl_cap(222_222 * ONE * 1_000_000)
		.build()
		.execute_with(|| {
			assert_eq!(TvlCap::<Test>::get(), 222_222 * ONE * 1_000_000);

			assert_ok!(Omnipool::set_tvl_cap(RuntimeOrigin::root(), u128::MAX));

			assert_eq!(TvlCap::<Test>::get(), u128::MAX);
		});
}

#[test]
fn set_tvl_cap_should_fail_when_not_root_origin() {
	let stable_amount = 50_000 * ONE * 1_000_000;
	let native_amount = 936_329_588_000_000_000u128;
	let dot_amount = 87_719_298_250_000_u128;

	let native_price = FixedU128::from_inner(1201500000000000);
	let stable_price = FixedU128::from_inner(45_000_000_000);
	let token_price = FixedU128::from_inner(25_650_000_000_000_000_000);

	let dot_id = 1_000;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(Omnipool::protocol_account(), DAI, stable_amount),
			(Omnipool::protocol_account(), HDX, native_amount),
			(LP1, DAI, 200_000 * ONE * 1_000_000),
			(LP2, dot_id, dot_amount),
			(LP2, DAI, 20_000 * ONE * 1_000_000),
		])
		.with_registered_asset(dot_id)
		.with_initial_pool(stable_price, native_price)
		.with_token(1_000, token_price, LP2, dot_amount)
		.with_tvl_cap(222_222 * ONE * 1_000_000)
		.build()
		.execute_with(|| {
			assert_eq!(TvlCap::<Test>::get(), 222_222 * ONE * 1_000_000);

			assert_noop!(
				Omnipool::set_tvl_cap(RuntimeOrigin::signed(LP1), u128::MAX),
				sp_runtime::traits::BadOrigin,
			);
		});
}
