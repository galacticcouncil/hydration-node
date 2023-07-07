use crate::tests::mock::*;
use crate::types::{PoolInfo, Tradability};
use crate::{AssetTradability, Error, Pools};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn update_pool_should_work_when_all_parames_are_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool_fees(
				RuntimeOrigin::root(),
				pool_id,
				Some(Permill::from_percent(10)),
				Some(Permill::from_percent(20)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
					trade_fee: Permill::from_percent(10),
					withdraw_fee: Permill::from_percent(20)
				}
			);
		});
}

#[test]
fn update_pool_should_work_when_only_trade_fee_is_updated() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool_fees(
				RuntimeOrigin::root(),
				pool_id,
				Some(Permill::from_percent(20)),
				None,
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
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
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool_fees(
				RuntimeOrigin::root(),
				pool_id,
				None,
				Some(Permill::from_percent(21)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
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
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::update_pool_fees(
				RuntimeOrigin::root(),
				pool_id,
				Some(Permill::from_percent(11)),
				Some(Permill::from_percent(21)),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
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
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_noop!(
				Stableswap::update_pool_fees(RuntimeOrigin::root(), pool_id, None, None),
				Error::<Test>::NothingToUpdate
			);

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
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
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			let pool_id = retrieve_current_asset_id();

			assert_noop!(
				Stableswap::update_pool_fees(RuntimeOrigin::root(), pool_id, Some(Permill::from_percent(1)), None),
				Error::<Test>::PoolNotFound
			);
		});
}

#[test]
fn set_tradable_state_should_work_when_asset_in_pool() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_ok!(Stableswap::set_asset_tradable_state(
				RuntimeOrigin::root(),
				pool_id,
				asset_a,
				Tradability::FROZEN,
			));

			assert_eq!(
				<AssetTradability<Test>>::get(pool_id, asset_a),
				Tradability::FROZEN,
			);
		});
}
#[test]
fn set_tradable_state_should_fail_when_asset_not_in_pool() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
				Permill::from_percent(0),
			));

			assert_noop!(Stableswap::set_asset_tradable_state(
				RuntimeOrigin::root(),
				pool_id,
				3,
				Tradability::FROZEN,
			),
			Error::<Test>::AssetNotInPool);
		});
}
#[test]
fn set_tradable_state_should_fail_when_pool_does_not_exist() {
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id)
		.build()
		.execute_with(|| {
			assert_noop!(Stableswap::set_asset_tradable_state(
				RuntimeOrigin::root(),
				pool_id,
				1,
				Tradability::FROZEN,
			),
			Error::<Test>::PoolNotFound);
		});
}