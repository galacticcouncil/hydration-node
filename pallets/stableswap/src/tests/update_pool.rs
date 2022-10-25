use crate::tests::mock::*;
use crate::types::PoolInfo;
use crate::{Error, Pools};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;

#[test]
fn update_pool_should_work_when_all_parames_are_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool(
				Origin::signed(ALICE),
				pool_id,
				Some(55u16),
				Some(Permill::from_percent(10)),
				Some(Permill::from_percent(20)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 55u16,
					trade_fee: Permill::from_percent(10),
					withdraw_fee: Permill::from_percent(20)
				}
			);
		});
}

#[test]
fn update_pool_should_work_when_only_amplification_is_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool(
				Origin::signed(ALICE),
				pool_id,
				Some(55u16),
				None,
				None,
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 55u16,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0)
				}
			);
		});
}

#[test]
fn update_pool_should_work_when_only_trade_fee_is_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool(
				Origin::signed(ALICE),
				pool_id,
				None,
				Some(Permill::from_percent(20)),
				None,
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 100u16,
					trade_fee: Permill::from_percent(20),
					withdraw_fee: Permill::from_percent(0)
				}
			);
		});
}

#[test]
fn update_pool_should_work_when_only_withdraw_fee_is_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool(
				Origin::signed(ALICE),
				pool_id,
				None,
				None,
				Some(Permill::from_percent(21)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 100u16,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(21)
				}
			);
		});
}

#[test]
fn update_pool_should_work_when_only_fees_is_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool(
				Origin::signed(ALICE),
				pool_id,
				None,
				Some(Permill::from_percent(11)),
				Some(Permill::from_percent(21)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 100u16,
					trade_fee: Permill::from_percent(11),
					withdraw_fee: Permill::from_percent(21)
				}
			);
		});
}

#[test]
fn update_pool_should_fail_when_nothing_is_to_update() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_noop!(
				Stableswap::update_pool(Origin::signed(ALICE), pool_id, None, None, None),
				Error::<Test>::NothingToUpdate
			);

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					amplification: 100u16,
					trade_fee: Permill::from_percent(0),
					withdraw_fee: Permill::from_percent(0)
				}
			);
		});
}

#[test]
fn update_pool_should_fail_when_pool_does_not_exists() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_noop!(
				Stableswap::update_pool(Origin::signed(ALICE), pool_id, Some(100u16), None, None),
				Error::<Test>::PoolNotFound
			);
		});
}

#[test]
fn update_pool_should_fail_when_amplification_is_outside_allowed_range() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_ok!(Stableswap::create_pool(
				Origin::signed(ALICE),
				vec![asset_a, asset_b],
				100u16,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_noop!(
				Stableswap::update_pool(Origin::signed(ALICE), pool_id, Some(20_000u16), None, None),
				Error::<Test>::InvalidAmplification
			);
		});
}
