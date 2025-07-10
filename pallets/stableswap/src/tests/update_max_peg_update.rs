use crate::tests::mock::*;
use crate::types::{BoundedPegSources, PegSource};
use crate::{Error, Event, PoolPegs};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use sp_runtime::Permill;

#[test]
fn update_pool_max_peg_update_should_work() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]).unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Create pool with pegs
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Get initial peg info
			let initial_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(initial_peg_info.max_peg_update, Permill::from_percent(10));

			// Update max peg update
			let new_max_peg_update = Permill::from_percent(25);
			assert_ok!(Stableswap::update_pool_max_peg_update(
				RuntimeOrigin::root(),
				pool_id,
				new_max_peg_update,
			));

			// Check that max peg update was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.max_peg_update, new_max_peg_update);

			// Check event was emitted
			System::assert_last_event(
				Event::PoolMaxPegUpdateUpdated {
					pool_id,
					max_peg_update: new_max_peg_update,
				}
				.into(),
			);
		});
}

#[test]
fn update_pool_max_peg_update_should_fail_when_pool_not_found() {
	let pool_id = 100;

	ExtBuilder::default().build().execute_with(|| {
		let new_max_peg_update = Permill::from_percent(25);

		assert_noop!(
			Stableswap::update_pool_max_peg_update(
				RuntimeOrigin::root(),
				pool_id, // Pool doesn't exist
				new_max_peg_update,
			),
			Error::<Test>::PoolNotFound
		);
	});
}

#[test]
fn update_pool_max_peg_update_should_fail_when_pool_has_no_pegs() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Create pool without pegs
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
			));

			let new_max_peg_update = Permill::from_percent(25);

			assert_noop!(
				Stableswap::update_pool_max_peg_update(RuntimeOrigin::root(), pool_id, new_max_peg_update,),
				Error::<Test>::NoPegSource
			);
		});
}

#[test]
fn update_pool_max_peg_update_should_fail_when_invalid_origin() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]).unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
			(BOB, asset_a, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Create pool with pegs
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			let new_max_peg_update = Permill::from_percent(25);

			// BOB doesn't have UpdateTradabilityOrigin permission
			assert_noop!(
				Stableswap::update_pool_max_peg_update(RuntimeOrigin::signed(BOB), pool_id, new_max_peg_update,),
				sp_runtime::DispatchError::BadOrigin
			);
		});
}

#[test]
fn update_pool_max_peg_update_should_allow_zero_percent() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]).unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Create pool with pegs
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Update max peg update to 0%
			let new_max_peg_update = Permill::zero();
			assert_ok!(Stableswap::update_pool_max_peg_update(
				RuntimeOrigin::root(),
				pool_id,
				new_max_peg_update,
			));

			// Check that max peg update was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.max_peg_update, new_max_peg_update);

			// Check event was emitted
			System::assert_last_event(
				Event::PoolMaxPegUpdateUpdated {
					pool_id,
					max_peg_update: new_max_peg_update,
				}
				.into(),
			);
		});
}

#[test]
fn update_pool_max_peg_update_should_allow_hundred_percent() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]).unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Create pool with pegs
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Update max peg update to 100%
			let new_max_peg_update = Permill::from_percent(100);
			assert_ok!(Stableswap::update_pool_max_peg_update(
				RuntimeOrigin::root(),
				pool_id,
				new_max_peg_update,
			));

			// Check that max peg update was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.max_peg_update, new_max_peg_update);

			// Check event was emitted
			System::assert_last_event(
				Event::PoolMaxPegUpdateUpdated {
					pool_id,
					max_peg_update: new_max_peg_update,
				}
				.into(),
			);
		});
}
