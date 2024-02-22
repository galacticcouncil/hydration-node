use crate::tests::mock::*;
use crate::types::PoolInfo;
use crate::Error;
use crate::Pools;
use frame_support::{assert_noop, assert_ok};
use sp_runtime::Permill;
use std::num::NonZeroU16;

#[test]
fn create_two_asset_pool_should_work_when_assets_are_registered() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(100).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(0),
				}
			);
		});
}

#[test]
fn create_multi_asset_pool_should_work_when_assets_are_registered() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c, asset_d],
				100,
				Permill::from_percent(0),
			));

			assert!(<Pools<Test>>::get(pool_id).is_some());
		});
}

#[test]
fn create_pool_should_store_assets_correctly_when_input_is_not_sorted() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("four".as_bytes().to_vec(), asset_d, 12)
		.build()
		.execute_with(|| {
			let amplification = 100u16;

			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_c, asset_d, asset_b, asset_a],
				amplification,
				Permill::from_percent(5),
			));

			assert_eq!(
				<Pools<Test>>::get(pool_id).unwrap(),
				PoolInfo {
					assets: vec![asset_a, asset_b, asset_c, asset_d].try_into().unwrap(),
					initial_amplification: NonZeroU16::new(amplification).unwrap(),
					final_amplification: NonZeroU16::new(100).unwrap(),
					initial_block: 0,
					final_block: 0,
					fee: Permill::from_percent(5),
				}
			);
		});
}

#[test]
fn create_pool_should_fail_when_same_assets_is_specified() {
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let asset_a: AssetId = 1;
			let amplification = 100u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, 3, 4, asset_a],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn create_pool_should_fail_when_same_assets_is_empty() {
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let amplification = 100u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn create_pool_should_fail_when_single_asset_is_provided() {
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let asset_a: AssetId = 1;
			let amplification = 100u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::IncorrectAssets
			);
		});
}

#[test]
fn create_pool_should_fail_when_share_asset_is_not_registered() {
	let pool_id: AssetId = 100;
	ExtBuilder::default().build().execute_with(|| {
		let asset_a: AssetId = 1;
		let amplification = 100u16;

		assert_noop!(
			Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, 3, 4],
				amplification,
				Permill::from_percent(0),
			),
			Error::<Test>::ShareAssetNotRegistered
		);
	});
}

#[test]
fn create_pool_should_fail_when_share_asset_is_among_assets() {
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let asset_a: AssetId = 1;
			let amplification = 100u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, pool_id],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::ShareAssetInPoolAssets
			);
		});
}

#[test]
fn create_pool_should_fail_when_asset_is_not_registered() {
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_registered_asset("one".as_bytes().to_vec(), 1000, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let registered: AssetId = 1000;
			let not_registered: AssetId = 2000;
			let amplification = 100u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![not_registered, registered],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::AssetNotRegistered
			);

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![registered, not_registered],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::AssetNotRegistered
			);
		});
}

#[test]
fn create_pool_should_fail_when_same_share_asset_pool_already_exists() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.build()
		.execute_with(|| {
			let amplification = 100u16;

			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				amplification,
				Permill::from_percent(0),
			));

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, asset_b],
					amplification,
					Permill::from_percent(0),
				),
				Error::<Test>::PoolExists
			);
		});
}

#[test]
fn create_pool_should_fail_when_amplification_is_incorrect() {
	let asset_a: AssetId = 1000;
	let asset_b: AssetId = 2000;
	let pool_id: AssetId = 100;
	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 200 * ONE), (ALICE, asset_b, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.build()
		.execute_with(|| {
			let amplification_min = 1u16;
			let amplification_max = 10_001u16;

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, asset_b],
					0,
					Permill::from_percent(0),
				),
				Error::<Test>::InvalidAmplification
			);

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, asset_b],
					amplification_min,
					Permill::from_percent(0),
				),
				Error::<Test>::InvalidAmplification
			);

			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, asset_b],
					amplification_max,
					Permill::from_percent(0),
				),
				Error::<Test>::InvalidAmplification
			);
		});
}

#[test]
fn create_pool_should_add_account_to_whitelist() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b],
				100,
				Permill::from_percent(0),
			));

			let pool_account = pool_account(pool_id);
			assert!(DUSTER_WHITELIST.with(|v| v.borrow().contains(&pool_account)));
		});
}

#[test]
fn create_pool_should_fail_when_number_of_assets_exceeds_maximum() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let asset_d: AssetId = 4;
	let asset_e: AssetId = 5;
	let asset_f: AssetId = 6;
	let pool_id: AssetId = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, 1, 200 * ONE), (ALICE, 2, 200 * ONE)])
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.build()
		.execute_with(|| {
			assert_noop!(
				Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					vec![asset_a, asset_b, asset_c, asset_d, asset_e, asset_f],
					100,
					Permill::from_percent(0),
				),
				Error::<Test>::MaxAssetsExceeded
			);
		});
}
