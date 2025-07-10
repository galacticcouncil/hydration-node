use crate::tests::mock::*;
use crate::types::{BoundedPegSources, PegSource};
use crate::{Error, Event, PoolPegs};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_traits::OraclePeriod;
use sp_runtime::Permill;

#[test]
fn update_asset_peg_source_should_work() {
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
			let initial_price = initial_peg_info.current[0];

			// Update peg source for asset_a (price always preserved)
			let new_peg_source = PegSource::Value((2, 3));
			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_a,
				new_peg_source.clone(),
			));

			// Check that peg source was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.source[0], new_peg_source);

			// Check that price was preserved
			assert_eq!(updated_peg_info.current[0], initial_price);

			// Check event was emitted
			System::assert_last_event(
				Event::PoolPegSourceUpdated {
					pool_id,
					asset_id: asset_a,
					peg_source: new_peg_source,
				}
				.into(),
			);
		});
}

#[test]
fn update_asset_peg_source_should_preserve_price() {
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
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Get initial price to verify it's preserved
			let initial_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			let initial_price = initial_peg_info.current[0];

			// Update peg source for asset_a (price always preserved)
			let new_peg_source = PegSource::Value((2, 3));
			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_a,
				new_peg_source.clone(),
			));

			// Check that peg source was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.source[0], new_peg_source);

			// Check that price was preserved (not updated)
			assert_eq!(updated_peg_info.current[0], initial_price);

			// Check event was emitted
			System::assert_last_event(
				Event::PoolPegSourceUpdated {
					pool_id,
					asset_id: asset_a,
					peg_source: new_peg_source,
				}
				.into(),
			);
		});
}

#[test]
fn update_asset_peg_source_should_fail_when_pool_not_found() {
	let asset_a: AssetId = 1;
	let pool_id = 100;

	ExtBuilder::default()
		.with_endowed_accounts(vec![(ALICE, asset_a, 1_000_000 * ONE)])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.build()
		.execute_with(|| {
			let new_peg_source = PegSource::Value((2, 3));

			assert_noop!(
				Stableswap::update_asset_peg_source(
					RuntimeOrigin::root(),
					pool_id, // Pool doesn't exist
					asset_a,
					new_peg_source,
				),
				Error::<Test>::PoolNotFound
			);
		});
}

#[test]
fn update_asset_peg_source_should_fail_when_pool_has_no_pegs() {
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
			assert_ok!(Stableswap::create_pool(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
			));

			let new_peg_source = PegSource::Value((2, 3));

			assert_noop!(
				Stableswap::update_asset_peg_source(RuntimeOrigin::root(), pool_id, asset_a, new_peg_source,),
				Error::<Test>::NoPegSource
			);
		});
}

#[test]
fn update_asset_peg_source_should_fail_when_asset_not_in_pool() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3; // Not in pool
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((1, 1))]).unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
			(ALICE, asset_c, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			let new_peg_source = PegSource::Value((2, 3));

			assert_noop!(
				Stableswap::update_asset_peg_source(
					RuntimeOrigin::root(),
					pool_id,
					asset_c, // Not in pool
					new_peg_source,
				),
				Error::<Test>::AssetNotInPool
			);
		});
}

#[test]
fn update_asset_peg_source_should_fail_when_invalid_origin() {
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
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			let new_peg_source = PegSource::Value((2, 3));

			// BOB doesn't have UpdateTradabilityOrigin permission
			assert_noop!(
				Stableswap::update_asset_peg_source(RuntimeOrigin::signed(BOB), pool_id, asset_a, new_peg_source,),
				sp_runtime::DispatchError::BadOrigin
			);
		});
}

#[test]
fn update_asset_peg_source_should_work_with_oracle_source() {
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
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Update with oracle source (should work since price is always preserved)
			let oracle_source = PegSource::Oracle((*b"nonexist", OraclePeriod::LastBlock, asset_a));

			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_a,
				oracle_source.clone(),
			));

			// Check that peg source was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.source[0], oracle_source);
		});
}

#[test]
fn update_asset_peg_source_should_update_second_asset_correctly() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> =
		BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((2, 2))]).unwrap();

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
			let initial_price_a = initial_peg_info.current[0];
			let _initial_price_b = initial_peg_info.current[1];

			// Update peg source for asset_b (index 1) - price will be preserved
			let new_peg_source = PegSource::Value((3, 4));
			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_b, // Second asset
				new_peg_source.clone(),
			));

			// Check that only asset_b's peg source was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.source[0], PegSource::Value((1, 1))); // asset_a unchanged
			assert_eq!(updated_peg_info.source[1], new_peg_source); // asset_b updated

			// Check that both prices were preserved (not updated)
			assert_eq!(updated_peg_info.current[0], initial_price_a); // asset_a price unchanged
			assert_eq!(updated_peg_info.current[1], (2, 2)); // asset_b price preserved

			// Check event was emitted
			System::assert_last_event(
				Event::PoolPegSourceUpdated {
					pool_id,
					asset_id: asset_b,
					peg_source: new_peg_source,
				}
				.into(),
			);
		});
}

#[test]
fn update_asset_peg_source_should_work_with_three_assets() {
	let asset_a: AssetId = 1;
	let asset_b: AssetId = 2;
	let asset_c: AssetId = 3;
	let pool_id = 100;

	let amp = 1000;
	let fee = Permill::from_percent(1);

	let peg_sources: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
		PegSource::Value((1, 1)),
		PegSource::Value((2, 2)),
		PegSource::Value((3, 3)),
	])
	.unwrap();

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_a, 1_000_000 * ONE),
			(ALICE, asset_b, 1_000_000 * ONE),
			(ALICE, asset_c, 1_000_000 * ONE),
		])
		.with_registered_asset("one".as_bytes().to_vec(), asset_a, 12)
		.with_registered_asset("two".as_bytes().to_vec(), asset_b, 12)
		.with_registered_asset("three".as_bytes().to_vec(), asset_c, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				vec![asset_a, asset_b, asset_c].try_into().unwrap(),
				amp,
				fee,
				peg_sources,
				Permill::from_percent(10),
			));

			// Update peg source for middle asset (asset_b)
			let new_peg_source = PegSource::Value((5, 6));
			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_b,
				new_peg_source.clone(),
			));

			// Check that only asset_b's peg source was updated
			let updated_peg_info = PoolPegs::<Test>::get(pool_id).unwrap();
			assert_eq!(updated_peg_info.source[0], PegSource::Value((1, 1)));
			assert_eq!(updated_peg_info.source[1], new_peg_source);
			assert_eq!(updated_peg_info.source[2], PegSource::Value((3, 3)));

			// Check that all prices were preserved (not updated)
			assert_eq!(updated_peg_info.current[0], (1, 1)); // asset_a price preserved
			assert_eq!(updated_peg_info.current[1], (2, 2)); // asset_b price preserved
			assert_eq!(updated_peg_info.current[2], (3, 3)); // asset_c price preserved
		});
}
