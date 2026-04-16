#![allow(deprecated)]

use crate::tests::mock::*;
use crate::types::{BoundedPegSources, PegSource, PoolPegInfo};
use crate::{Event, PoolPegs, Pools};
use frame_support::{assert_ok, BoundedVec};
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use sp_runtime::{Perbill, Permill};

// Re-use ONE from mock; TVL sized for balanced initial liquidity in peg pools.
const TVL: u128 = 2_000_000 * ONE;

type PoolId = AssetId;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Create a peg pool with `Value`-typed pegs. `assets` and `pegs` are
/// positionally paired — any ordering of assets is accepted.
fn make_peg_pool(pool_id: PoolId, assets: Vec<AssetId>, pegs: Vec<(u128, u128)>) {
	let assets_bv: BoundedVec<AssetId, _> = assets.try_into().unwrap();
	let pegs_bv: BoundedPegSources<AssetId> =
		BoundedVec::try_from(pegs.into_iter().map(PegSource::Value).collect::<Vec<_>>()).unwrap();

	assert_ok!(Stableswap::create_pool_with_pegs(
		RuntimeOrigin::root(),
		pool_id,
		assets_bv,
		1000,
		Permill::zero(),
		pegs_bv,
		Perbill::from_percent(100),
	));
}

/// Add liquidity to a pool. Zero-amount entries are excluded so callers can
/// omit assets by passing 0 (respects MinTradingLimit).
fn add_liquidity(pool_id: PoolId, who: AccountId, amounts: Vec<(AssetId, Balance)>) {
	let entries: Vec<AssetAmount<AssetId>> = amounts
		.into_iter()
		.filter(|(_, a)| *a > 0)
		.map(|(asset, amount)| AssetAmount::new(asset, amount))
		.collect();
	assert_ok!(Stableswap::add_assets_liquidity(
		RuntimeOrigin::signed(who),
		pool_id,
		BoundedVec::try_from(entries).unwrap(),
		Balance::zero(),
	));
}

// ── Phase 1 & 2: storage correctness ─────────────────────────────────────────

/// RED/GREEN – Task 1.3 / 2.1
///
/// After creating a pool with assets provided in unsorted order, both
/// `PoolPegs.source` and `PoolPegs.current` must be co-sorted with the sorted
/// `Pools.assets`.  Read storage immediately after pool creation, before any
/// operation that could mutate PoolPegs.
///
/// Uses three assets [10, 5, 20] (partially unsorted) as specified in the plan.
#[test]
fn pool_pegs_should_be_cosorted_with_assets() {
	// Input [10, 5, 20] is partially unsorted; sorted order is [5, 10, 20].
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Caller intent: asset_10 → (2,1), asset_5 → (1,1), asset_20 → (3,1).
			// Input order: [10, 5, 20] with pegs [(2,1), (1,1), (3,1)].
			let unsorted_assets: BoundedVec<AssetId, _> = vec![asset_10, asset_5, asset_20].try_into().unwrap();
			let unsorted_pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Value((2, 1)),
				PegSource::Value((1, 1)),
				PegSource::Value((3, 1)),
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				unsorted_assets,
				1000,
				Permill::zero(),
				unsorted_pegs,
				Perbill::from_percent(100),
			));

			// Read immediately — no other operation touches PoolPegs yet.
			let pool = Pools::<Test>::get(pool_id).expect("pool must exist");
			let peg_info = PoolPegs::<Test>::get(pool_id).expect("peg info must exist");

			// Sorted pool assets: [5, 10, 20].
			assert_eq!(
				pool.assets.to_vec(),
				vec![asset_5, asset_10, asset_20],
				"pool assets must be sorted",
			);

			// Peg sources must be co-sorted to match sorted assets [5, 10, 20]:
			//   asset_5 → (1,1), asset_10 → (2,1), asset_20 → (3,1).
			// BUG: currently stored as input order [(2,1), (1,1), (3,1)].
			assert_eq!(
				peg_info.source.to_vec(),
				vec![
					PegSource::Value((1, 1)),
					PegSource::Value((2, 1)),
					PegSource::Value((3, 1)),
				],
				"peg sources must be co-sorted with pool assets; got {:?}",
				peg_info.source.to_vec(),
			);

			// PoolPegs.current holds resolved peg values; must also be co-sorted.
			// PegSource::Value is resolved verbatim, so current[i] == source[i] value.
			assert_eq!(
				peg_info.current.to_vec(),
				vec![(1u128, 1u128), (2u128, 1u128), (3u128, 1u128)],
				"current pegs must be co-sorted with pool assets; got {:?}",
				peg_info.current.to_vec(),
			);
		});
}

/// RED/GREEN – Task 1.3 variant: three assets with maximally wrong order.
///
/// Input [20, 10, 5] (reverse-sorted) with pegs [(3,1), (2,1), (1,1)].
/// After sorting, pool stores [5, 10, 20]; pegs must follow as [(1,1), (2,1), (3,1)].
#[test]
fn pool_pegs_should_be_cosorted_with_three_assets_reverse_order() {
	let asset_20: AssetId = 20;
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Reverse-sorted input.
			let assets: BoundedVec<AssetId, _> = vec![asset_20, asset_10, asset_5].try_into().unwrap();
			let pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Value((3, 1)), // for asset_20
				PegSource::Value((2, 1)), // for asset_10
				PegSource::Value((1, 1)), // for asset_5
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				assets,
				1000,
				Permill::zero(),
				pegs,
				Perbill::from_percent(100),
			));

			let pool = Pools::<Test>::get(pool_id).unwrap();
			let peg_info = PoolPegs::<Test>::get(pool_id).unwrap();

			assert_eq!(pool.assets.to_vec(), vec![asset_5, asset_10, asset_20]);

			assert_eq!(
				peg_info.source.to_vec(),
				vec![
					PegSource::Value((1, 1)), // asset_5
					PegSource::Value((2, 1)), // asset_10
					PegSource::Value((3, 1)), // asset_20
				],
				"sources must be co-sorted; got {:?}",
				peg_info.source.to_vec(),
			);

			assert_eq!(
				peg_info.current.to_vec(),
				vec![(1u128, 1u128), (2u128, 1u128), (3u128, 1u128)],
				"current pegs must be co-sorted; got {:?}",
				peg_info.current.to_vec(),
			);
		});
}

// ── Phase 1 & 2: trade correctness ───────────────────────────────────────────

/// RED/GREEN – Task 1.1 / 2.1
///
/// Sell through an unsorted pool must yield the same output as an identical
/// pool created with assets already in sorted order.
///
/// Three assets [10, 5, 20] as specified in the plan to exercise the
/// partial-permutation mismatch (positions 0 and 1 swapped, position 2 correct).
#[test]
fn sell_should_use_correct_pegs_when_assets_are_unsorted() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let pool_id: PoolId = 100;
	let pool_id_ref: PoolId = 101;
	let liquid = TVL / 2;
	let trade = 100 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_10, liquid * 2),
			(ALICE, asset_5, liquid * 2),
			(ALICE, asset_20, liquid * 2),
			(BOB, asset_10, trade * 2),
		])
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool1".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("pool2".as_bytes().to_vec(), pool_id_ref, 12)
		.build()
		.execute_with(|| {
			// Unsorted pool: input [10, 5, 20] with pegs [(2,1), (1,1), (3,1)].
			// Intent: asset_10 → (2,1), asset_5 → (1,1), asset_20 → (3,1).
			make_peg_pool(pool_id, vec![asset_10, asset_5, asset_20], vec![(2, 1), (1, 1), (3, 1)]);
			add_liquidity(
				pool_id,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_10,
				asset_5,
				trade,
				0
			));
			let received_unsorted = Tokens::free_balance(asset_5, &BOB) - bal_before;

			// Reference pool: sorted [5, 10, 20] with co-sorted pegs [(1,1), (2,1), (3,1)].
			make_peg_pool(
				pool_id_ref,
				vec![asset_5, asset_10, asset_20],
				vec![(1, 1), (2, 1), (3, 1)],
			);
			add_liquidity(
				pool_id_ref,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before2 = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id_ref,
				asset_10,
				asset_5,
				trade,
				0,
			));
			let received_sorted = Tokens::free_balance(asset_5, &BOB) - bal_before2;

			assert_eq!(
				received_unsorted, received_sorted,
				"unsorted pool sell output ({}) must equal sorted pool output ({})",
				received_unsorted, received_sorted,
			);
		});
}

/// RED/GREEN – Task 1.2 / 2.1  (buy variant)
///
/// Buy through an unsorted pool must yield the same cost as the reference pool.
///
/// Three assets [10, 5, 20] as per the plan.
#[test]
fn buy_should_use_correct_pegs_when_assets_are_unsorted() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let pool_id: PoolId = 100;
	let pool_id_ref: PoolId = 101;
	let liquid = TVL / 2;
	let buy_amount = 50 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_10, liquid * 2),
			(ALICE, asset_5, liquid * 2),
			(ALICE, asset_20, liquid * 2),
			(BOB, asset_10, TVL),
		])
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool1".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("pool2".as_bytes().to_vec(), pool_id_ref, 12)
		.build()
		.execute_with(|| {
			// Unsorted pool: input [10, 5, 20] with pegs [(2,1), (1,1), (3,1)].
			make_peg_pool(pool_id, vec![asset_10, asset_5, asset_20], vec![(2, 1), (1, 1), (3, 1)]);
			add_liquidity(
				pool_id,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before = Tokens::free_balance(asset_10, &BOB);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_5,
				asset_10,
				buy_amount,
				TVL,
			));
			let spent_unsorted = bal_before - Tokens::free_balance(asset_10, &BOB);

			// Reference pool: sorted [5, 10, 20] with pegs [(1,1), (2,1), (3,1)].
			make_peg_pool(
				pool_id_ref,
				vec![asset_5, asset_10, asset_20],
				vec![(1, 1), (2, 1), (3, 1)],
			);
			add_liquidity(
				pool_id_ref,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before2 = Tokens::free_balance(asset_10, &BOB);
			assert_ok!(Stableswap::buy(
				RuntimeOrigin::signed(BOB),
				pool_id_ref,
				asset_5,
				asset_10,
				buy_amount,
				TVL,
			));
			let spent_sorted = bal_before2 - Tokens::free_balance(asset_10, &BOB);

			assert_eq!(
				spent_unsorted, spent_sorted,
				"unsorted pool buy cost ({}) must equal sorted pool buy cost ({})",
				spent_unsorted, spent_sorted,
			);
		});
}

/// RED/GREEN – Task 1.4 / 2.1  (Oracle peg variant)
///
/// Most real peg pools use oracle sources, where each asset has a distinct oracle
/// pair.  When assets are unsorted, the oracle lookups are misrouted to the wrong
/// asset, silently applying the wrong price feed.
///
/// Three assets [10, 5, 20]: asset_10 and asset_20 use Oracle sources, asset_5
/// uses Value.  The unsorted pool mis-assigns Oracle to asset_5 (wrong) and
/// Value to asset_10 (wrong), while asset_20 coincidentally stays correct.
#[test]
fn sell_should_use_correct_oracle_pegs_when_assets_are_unsorted() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	// oracle_asset is the "denominator" asset used in the oracle pair lookup.
	// set_peg_oracle_value(oracle_asset, peg_asset, price, ts) registers
	// the price for (oracle_asset, peg_asset) in the mock oracle.
	let oracle_source = *b"testtest";
	let pool_id: PoolId = 100;
	let pool_id_ref: PoolId = 101;
	let liquid = TVL / 2;
	let trade = 100 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_10, liquid * 2),
			(ALICE, asset_5, liquid * 2),
			(ALICE, asset_20, liquid * 2),
			(BOB, asset_10, trade * 2),
		])
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool1".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("pool2".as_bytes().to_vec(), pool_id_ref, 12)
		.build()
		.execute_with(|| {
			// Register oracle entries: asset_10 priced in asset_5 → (2,1).
			set_peg_oracle_value(asset_5, asset_10, (2, 1), 1);
			// asset_20 priced in asset_5 → (3,1).
			set_peg_oracle_value(asset_5, asset_20, (3, 1), 1);
			// Fallback for the misaligned lookup (asset_5, asset_5) that the buggy
			// pool will request when asset_5 incorrectly inherits the Oracle source.
			// Returns (1,1) — same as the intended Value peg for asset_5, making
			// add_liquidity succeed so the sell can expose the misalignment.
			set_peg_oracle_value(asset_5, asset_5, (1, 1), 1);

			// ── Unsorted pool ────────────────────────────────────────────────────
			// Intent: asset_10 → Oracle(2,1), asset_5 → Value(1,1), asset_20 → Oracle(3,1).
			// Input order: [10, 5, 20].  Bug stores source as-is, so sorted pool
			// maps: asset_5 → Oracle (wrong), asset_10 → Value (wrong), asset_20 → Oracle (correct by coincidence).
			let unsorted_assets: BoundedVec<AssetId, _> = vec![asset_10, asset_5, asset_20].try_into().unwrap();
			let unsorted_pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)), // intended for asset_10
				PegSource::Value((1, 1)),                                         // intended for asset_5
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)), // intended for asset_20
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				unsorted_assets,
				1000,
				Permill::zero(),
				unsorted_pegs,
				Perbill::from_percent(100),
			));
			add_liquidity(
				pool_id,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_10,
				asset_5,
				trade,
				0
			));
			let received_unsorted = Tokens::free_balance(asset_5, &BOB) - bal_before;

			// ── Reference pool (sorted) ──────────────────────────────────────────
			// Sorted [5, 10, 20], pegs: [Value(1,1), Oracle(..), Oracle(..)].
			let sorted_assets: BoundedVec<AssetId, _> = vec![asset_5, asset_10, asset_20].try_into().unwrap();
			let sorted_pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Value((1, 1)),
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)),
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)),
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id_ref,
				sorted_assets,
				1000,
				Permill::zero(),
				sorted_pegs,
				Perbill::from_percent(100),
			));
			add_liquidity(
				pool_id_ref,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);

			let bal_before2 = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::sell(
				RuntimeOrigin::signed(BOB),
				pool_id_ref,
				asset_10,
				asset_5,
				trade,
				0,
			));
			let received_sorted = Tokens::free_balance(asset_5, &BOB) - bal_before2;

			assert_eq!(
				received_unsorted, received_sorted,
				"unsorted oracle-peg pool sell output ({}) must equal sorted pool output ({})",
				received_unsorted, received_sorted,
			);
		});
}

// ── Phase 1 & 2: update_asset_peg_source ─────────────────────────────────────

/// RED/GREEN – Task 1.5 / 2.1
///
/// `update_asset_peg_source` derives the storage index from the *sorted* pool
/// assets via `pool.find_asset(asset_id)`.  If `PoolPegs.source` is stored in
/// unsorted (input) order, the update writes to the wrong position.
///
/// Three assets [10, 5, 20]: sorted order is [5, 10, 20], so asset_10 is at
/// index 1 and asset_20 at index 2.  After updating asset_10, both other
/// slots must remain unchanged.
#[test]
fn update_asset_peg_source_should_target_correct_asset_after_unsorted_creation() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Unsorted input [10, 5, 20] with pegs [(2,1), (1,1), (3,1)].
			make_peg_pool(pool_id, vec![asset_10, asset_5, asset_20], vec![(2, 1), (1, 1), (3, 1)]);

			let new_peg = PegSource::Value((5, 1));
			assert_ok!(Stableswap::update_asset_peg_source(
				RuntimeOrigin::root(),
				pool_id,
				asset_10,
				new_peg.clone(),
			));

			let pool = Pools::<Test>::get(pool_id).unwrap();
			let peg_info = PoolPegs::<Test>::get(pool_id).unwrap();

			// Derive the expected index the same way the extrinsic does.
			let expected_idx = pool.find_asset(asset_10).expect("asset_10 must be in pool");
			// After sorting [10, 5, 20] → [5, 10, 20], asset_10 is at index 1.
			assert_eq!(expected_idx, 1, "asset_10 must be at sorted index 1");

			assert_eq!(
				peg_info.source[expected_idx], new_peg,
				"peg source at sorted index {} must be updated; got {:?}",
				expected_idx, peg_info.source[expected_idx],
			);

			// asset_5 at index 0 must remain unchanged.
			assert_eq!(
				peg_info.source[0],
				PegSource::Value((1, 1)),
				"asset_5 peg source must be unchanged; got {:?}",
				peg_info.source[0],
			);

			// asset_20 at index 2 must remain unchanged.
			assert_eq!(
				peg_info.source[2],
				PegSource::Value((3, 1)),
				"asset_20 peg source must be unchanged; got {:?}",
				peg_info.source[2],
			);
		});
}

// ── Phase 1 & 2: remove_liquidity ────────────────────────────────────────────

/// RED/GREEN – Task 1.6 / 2.1
///
/// Removing liquidity uses peg-adjusted reserves to price the withdrawal.
/// With misaligned pegs the withdrawal amount differs from the reference pool.
///
/// Three assets [10, 5, 20] as per the plan.
#[test]
fn remove_liquidity_should_use_correct_pegs_when_assets_unsorted() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let pool_id: PoolId = 100;
	let pool_id_ref: PoolId = 101;
	let liquid = TVL / 2;
	let add_liq = 10_000 * ONE;

	ExtBuilder::default()
		.with_endowed_accounts(vec![
			(ALICE, asset_10, liquid * 2),
			(ALICE, asset_5, liquid * 2),
			(ALICE, asset_20, liquid * 2),
			(BOB, asset_10, add_liq * 2),
		])
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool1".as_bytes().to_vec(), pool_id, 12)
		.with_registered_asset("pool2".as_bytes().to_vec(), pool_id_ref, 12)
		.build()
		.execute_with(|| {
			// Unsorted pool: input [10, 5, 20] with pegs [(2,1), (1,1), (3,1)].
			make_peg_pool(pool_id, vec![asset_10, asset_5, asset_20], vec![(2, 1), (1, 1), (3, 1)]);
			add_liquidity(
				pool_id,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);
			add_liquidity(pool_id, BOB, vec![(asset_10, add_liq), (asset_5, 0), (asset_20, 0)]);

			let shares = Tokens::free_balance(pool_id, &BOB);
			let bal_before = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id,
				asset_5,
				shares / 2,
				0,
			));
			let received_unsorted = Tokens::free_balance(asset_5, &BOB) - bal_before;

			// Reference pool: sorted [5, 10, 20] with pegs [(1,1), (2,1), (3,1)].
			make_peg_pool(
				pool_id_ref,
				vec![asset_5, asset_10, asset_20],
				vec![(1, 1), (2, 1), (3, 1)],
			);
			add_liquidity(
				pool_id_ref,
				ALICE,
				vec![(asset_10, liquid), (asset_5, liquid), (asset_20, liquid)],
			);
			add_liquidity(pool_id_ref, BOB, vec![(asset_10, add_liq), (asset_5, 0), (asset_20, 0)]);

			let shares2 = Tokens::free_balance(pool_id_ref, &BOB);
			let bal_before2 = Tokens::free_balance(asset_5, &BOB);
			assert_ok!(Stableswap::remove_liquidity_one_asset(
				RuntimeOrigin::signed(BOB),
				pool_id_ref,
				asset_5,
				shares2 / 2,
				0,
			));
			let received_sorted = Tokens::free_balance(asset_5, &BOB) - bal_before2;

			assert_eq!(
				received_unsorted, received_sorted,
				"unsorted pool remove_liquidity_one_asset ({}) must equal sorted pool ({})",
				received_unsorted, received_sorted,
			);
		});
}

// ── Phase 2: edge cases ───────────────────────────────────────────────────────

/// GREEN – already-sorted assets are unaffected (regression guard).
///
/// When assets are provided in sorted order the co-sort is a no-op; behaviour
/// must be identical to the pre-fix baseline.
#[test]
fn pool_pegs_are_correct_when_assets_provided_already_sorted() {
	let asset_5: AssetId = 5;
	let asset_10: AssetId = 10;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Already sorted: [5, 10].
			let assets: BoundedVec<AssetId, _> = vec![asset_5, asset_10].try_into().unwrap();
			let pegs: BoundedPegSources<AssetId> =
				BoundedVec::try_from(vec![PegSource::Value((1, 1)), PegSource::Value((2, 1))]).unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				assets,
				1000,
				Permill::zero(),
				pegs,
				Perbill::from_percent(100),
			));

			let pool = Pools::<Test>::get(pool_id).unwrap();
			let peg_info = PoolPegs::<Test>::get(pool_id).unwrap();

			assert_eq!(pool.assets.to_vec(), vec![asset_5, asset_10]);
			assert_eq!(
				peg_info.source.to_vec(),
				vec![PegSource::Value((1, 1)), PegSource::Value((2, 1))],
			);
			assert_eq!(peg_info.current.to_vec(), vec![(1u128, 1u128), (2u128, 1u128)]);
		});
}

/// GREEN – five assets in random order (MAX_ASSETS_IN_POOL).
///
/// Verifies the co-sort works at the maximum pool size with a non-trivial
/// permutation (not already sorted, not reverse-sorted).
#[test]
fn pool_pegs_should_be_cosorted_with_five_assets_random_order() {
	// Sorted order would be [1, 3, 5, 7, 9]; provide as [5, 1, 9, 3, 7].
	let asset_1: AssetId = 1;
	let asset_3: AssetId = 3;
	let asset_5: AssetId = 5;
	let asset_7: AssetId = 7;
	let asset_9: AssetId = 9;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a1".as_bytes().to_vec(), asset_1, 12)
		.with_registered_asset("a3".as_bytes().to_vec(), asset_3, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a7".as_bytes().to_vec(), asset_7, 12)
		.with_registered_asset("a9".as_bytes().to_vec(), asset_9, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Input order [5, 1, 9, 3, 7]; pegs paired to this order.
			let assets: BoundedVec<AssetId, _> = vec![asset_5, asset_1, asset_9, asset_3, asset_7].try_into().unwrap();
			let pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Value((5, 1)), // for asset_5
				PegSource::Value((1, 1)), // for asset_1
				PegSource::Value((9, 1)), // for asset_9
				PegSource::Value((3, 1)), // for asset_3
				PegSource::Value((7, 1)), // for asset_7
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				assets,
				1000,
				Permill::zero(),
				pegs,
				Perbill::from_percent(100),
			));

			let pool = Pools::<Test>::get(pool_id).unwrap();
			let peg_info = PoolPegs::<Test>::get(pool_id).unwrap();

			// Sorted order: [1, 3, 5, 7, 9].
			assert_eq!(pool.assets.to_vec(), vec![asset_1, asset_3, asset_5, asset_7, asset_9],);

			// Peg values must match sorted asset order.
			assert_eq!(
				peg_info.source.to_vec(),
				vec![
					PegSource::Value((1, 1)), // asset_1
					PegSource::Value((3, 1)), // asset_3
					PegSource::Value((5, 1)), // asset_5
					PegSource::Value((7, 1)), // asset_7
					PegSource::Value((9, 1)), // asset_9
				],
				"sources must be co-sorted; got {:?}",
				peg_info.source.to_vec(),
			);

			assert_eq!(
				peg_info.current.to_vec(),
				vec![
					(1u128, 1u128),
					(3u128, 1u128),
					(5u128, 1u128),
					(7u128, 1u128),
					(9u128, 1u128),
				],
			);
		});
}

/// GREEN – mixed PegSource types with unsorted assets.
///
/// Ensures co-sorting works when the peg source slice contains a mix of
/// Value and Oracle variants (not all the same type).
#[test]
fn pool_pegs_should_be_cosorted_with_mixed_peg_source_types() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let asset_20: AssetId = 20;
	let oracle_source = *b"testtest";
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("a20".as_bytes().to_vec(), asset_20, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			// Oracle entry for asset_10 priced via asset_5.
			set_peg_oracle_value(asset_5, asset_10, (2, 1), 1);
			// Oracle entry for asset_20 priced via asset_5.
			set_peg_oracle_value(asset_5, asset_20, (3, 1), 1);

			// Unsorted input [10, 5, 20]; mixed peg source types.
			let assets: BoundedVec<AssetId, _> = vec![asset_10, asset_5, asset_20].try_into().unwrap();
			let pegs: BoundedPegSources<AssetId> = BoundedVec::try_from(vec![
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)), // for asset_10
				PegSource::Value((1, 1)),                                         // for asset_5
				PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)), // for asset_20
			])
			.unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				assets,
				1000,
				Permill::zero(),
				pegs,
				Perbill::from_percent(100),
			));

			let pool = Pools::<Test>::get(pool_id).unwrap();
			let peg_info = PoolPegs::<Test>::get(pool_id).unwrap();

			// Sorted assets: [5, 10, 20].
			assert_eq!(pool.assets.to_vec(), vec![asset_5, asset_10, asset_20]);

			// Sources co-sorted: Value for asset_5, Oracle for asset_10, Oracle for asset_20.
			assert_eq!(
				peg_info.source.to_vec(),
				vec![
					PegSource::Value((1, 1)),
					PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)),
					PegSource::Oracle((oracle_source, OraclePeriod::Short, asset_5)),
				],
				"sources must be co-sorted; got {:?}",
				peg_info.source.to_vec(),
			);
		});
}

// ── Phase 2: Task 2.3 — PoolCreated event correctness ────────────────────────

/// GREEN – `PoolCreated` event must carry sorted assets and co-sorted peg info.
///
/// After the fix the event should reflect the canonical sorted state stored
/// on-chain, not the raw (potentially unsorted) caller input.
#[test]
fn pool_created_event_should_contain_sorted_assets_and_cosorted_pegs() {
	let asset_10: AssetId = 10;
	let asset_5: AssetId = 5;
	let pool_id: PoolId = 100;

	ExtBuilder::default()
		.with_registered_asset("a10".as_bytes().to_vec(), asset_10, 12)
		.with_registered_asset("a5".as_bytes().to_vec(), asset_5, 12)
		.with_registered_asset("pool".as_bytes().to_vec(), pool_id, 12)
		.build()
		.execute_with(|| {
			let unsorted_assets: BoundedVec<AssetId, _> = vec![asset_10, asset_5].try_into().unwrap();
			let unsorted_pegs: BoundedPegSources<AssetId> =
				BoundedVec::try_from(vec![PegSource::Value((2, 1)), PegSource::Value((1, 1))]).unwrap();

			assert_ok!(Stableswap::create_pool_with_pegs(
				RuntimeOrigin::root(),
				pool_id,
				unsorted_assets,
				1000,
				Permill::zero(),
				unsorted_pegs,
				Perbill::from_percent(100),
			));

			// The PoolCreated event must carry the sorted representation.
			let expected_peg_info = PoolPegInfo {
				source: BoundedVec::try_from(vec![
					PegSource::Value((1, 1)), // asset_5
					PegSource::Value((2, 1)), // asset_10
				])
				.unwrap(),
				updated_at: 1,
				max_peg_update: Perbill::from_percent(100),
				current: BoundedVec::try_from(vec![(1u128, 1u128), (2u128, 1u128)]).unwrap(),
			};

			expect_events(vec![Event::PoolCreated {
				pool_id,
				assets: vec![asset_5, asset_10], // sorted
				amplification: 1000u16.try_into().unwrap(),
				fee: Permill::zero(),
				peg: Some(expected_peg_info),
			}
			.into()]);
		});
}
