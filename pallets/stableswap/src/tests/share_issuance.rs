use crate::tests::mock::*;
use crate::tests::to_bounded_asset_vec;
use crate::{Error, Pallet, Pools, ShareIssuance};
use frame_support::{assert_noop, assert_ok, BoundedVec};
use hydradx_traits::router::{PoolType, TradeExecution};
use hydradx_traits::stableswap::AssetAmount;
use sp_runtime::Permill;

const POOL_ID: AssetId = 100;
const ASSET_A: AssetId = 1;
const ASSET_B: AssetId = 2;
const INITIAL_SHARES: Balance = 200 * ONE * 1_000_000;

fn pool_with_initial_liquidity() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default()
		.with_endowed_accounts(vec![
			(BOB, ASSET_A, 200 * ONE),
			(BOB, ASSET_B, 200 * ONE),
			(ALICE, ASSET_A, 200 * ONE),
			(ALICE, ASSET_B, 200 * ONE),
		])
		.with_registered_asset("pool".as_bytes().to_vec(), POOL_ID, 12)
		.with_registered_asset("one".as_bytes().to_vec(), ASSET_A, 12)
		.with_registered_asset("two".as_bytes().to_vec(), ASSET_B, 12)
		.build();
	ext.execute_with(|| {
		assert_ok!(Stableswap::create_pool(
			RuntimeOrigin::root(),
			POOL_ID,
			to_bounded_asset_vec(vec![ASSET_A, ASSET_B]),
			100u16,
			Permill::from_percent(0),
		));
		assert_ok!(Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(BOB),
			POOL_ID,
			BoundedVec::truncate_from(vec![
				AssetAmount::new(ASSET_A, 100 * ONE),
				AssetAmount::new(ASSET_B, 100 * ONE),
			]),
			Balance::zero(),
		));
	});
	ext
}

#[test]
fn add_liquidity_should_increase_share_issuance_when_shares_are_minted() {
	pool_with_initial_liquidity().execute_with(|| {
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), INITIAL_SHARES);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), Tokens::total_issuance(POOL_ID));

		assert_ok!(Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(ALICE),
			POOL_ID,
			BoundedVec::truncate_from(vec![
				AssetAmount::new(ASSET_A, 100 * ONE),
				AssetAmount::new(ASSET_B, 100 * ONE),
			]),
			Balance::zero(),
		));

		let alice_shares = Tokens::free_balance(POOL_ID, &ALICE);
		assert_eq!(alice_shares, 199_999_999_999_999_999_998);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), INITIAL_SHARES + alice_shares);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), Tokens::total_issuance(POOL_ID));
	});
}

#[test]
fn add_liquidity_shares_should_increase_share_issuance_when_shares_are_minted() {
	pool_with_initial_liquidity().execute_with(|| {
		let shares = 10 * ONE * 1_000_000;
		assert_ok!(Stableswap::add_liquidity_shares(
			RuntimeOrigin::signed(ALICE),
			POOL_ID,
			shares,
			ASSET_A,
			200 * ONE,
		));

		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), INITIAL_SHARES + shares);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), Tokens::total_issuance(POOL_ID));
	});
}

#[test]
fn remove_liquidity_one_asset_should_decrease_share_issuance_when_shares_are_burned() {
	pool_with_initial_liquidity().execute_with(|| {
		let share_amount = 50 * ONE * 1_000_000;
		assert_ok!(Stableswap::remove_liquidity_one_asset(
			RuntimeOrigin::signed(BOB),
			POOL_ID,
			ASSET_A,
			share_amount,
			Balance::zero(),
		));

		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), INITIAL_SHARES - share_amount);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), Tokens::total_issuance(POOL_ID));
	});
}

#[test]
fn withdraw_asset_amount_should_decrease_share_issuance_when_shares_are_burned() {
	pool_with_initial_liquidity().execute_with(|| {
		let bob_shares_before = Tokens::free_balance(POOL_ID, &BOB);
		assert_ok!(Stableswap::withdraw_asset_amount(
			RuntimeOrigin::signed(BOB),
			POOL_ID,
			ASSET_A,
			10 * ONE,
			Balance::MAX,
		));

		let burned = bob_shares_before - Tokens::free_balance(POOL_ID, &BOB);
		assert_eq!(burned, 10_002_612_654_028_406_416);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), INITIAL_SHARES - burned);
		assert_eq!(ShareIssuance::<Test>::get(POOL_ID), Tokens::total_issuance(POOL_ID));
	});
}

#[test]
fn remove_liquidity_should_remove_issuance_entry_when_pool_is_destroyed() {
	pool_with_initial_liquidity().execute_with(|| {
		assert_ok!(Stableswap::remove_liquidity(
			RuntimeOrigin::signed(BOB),
			POOL_ID,
			INITIAL_SHARES,
			BoundedVec::truncate_from(vec![AssetAmount::new(ASSET_A, 1), AssetAmount::new(ASSET_B, 1),]),
		));

		assert!(!Pools::<Test>::contains_key(POOL_ID));
		assert!(!ShareIssuance::<Test>::contains_key(POOL_ID));
	});
}

#[test]
fn burn_shares_should_fail_when_amount_exceeds_tracked_issuance() {
	pool_with_initial_liquidity().execute_with(|| {
		assert_noop!(
			Pallet::<Test>::burn_shares(POOL_ID, &BOB, INITIAL_SHARES + 1),
			Error::<Test>::InsufficientShareIssuance
		);
	});
}

#[test]
fn calculate_out_given_in_should_ignore_external_mint_when_buying_shares() {
	pool_with_initial_liquidity().execute_with(|| {
		let before =
			<Pallet<Test> as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
				PoolType::Stableswap(POOL_ID),
				ASSET_A,
				POOL_ID,
				10 * ONE,
			)
			.unwrap();

		// External mint inflates total issuance but must not affect pool math.
		assert_ok!(Tokens::set_balance(
			RuntimeOrigin::root(),
			ALICE,
			POOL_ID,
			1_000_000 * ONE * 1_000_000,
			0,
		));
		assert!(Tokens::total_issuance(POOL_ID) > ShareIssuance::<Test>::get(POOL_ID));

		let after =
			<Pallet<Test> as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
				PoolType::Stableswap(POOL_ID),
				ASSET_A,
				POOL_ID,
				10 * ONE,
			)
			.unwrap();

		assert_eq!(before, after);
	});
}

#[test]
fn calculate_out_given_in_should_ignore_external_mint_when_selling_shares() {
	pool_with_initial_liquidity().execute_with(|| {
		let before =
			<Pallet<Test> as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
				PoolType::Stableswap(POOL_ID),
				POOL_ID,
				ASSET_A,
				10 * ONE * 1_000_000,
			)
			.unwrap();

		assert_ok!(Tokens::set_balance(
			RuntimeOrigin::root(),
			ALICE,
			POOL_ID,
			1_000_000 * ONE * 1_000_000,
			0,
		));

		let after =
			<Pallet<Test> as TradeExecution<RuntimeOrigin, AccountId, AssetId, Balance>>::calculate_out_given_in(
				PoolType::Stableswap(POOL_ID),
				POOL_ID,
				ASSET_A,
				10 * ONE * 1_000_000,
			)
			.unwrap();

		assert_eq!(before, after);
	});
}

#[test]
fn create_snapshot_should_use_tracked_issuance_when_external_mint_exists() {
	pool_with_initial_liquidity().execute_with(|| {
		assert_ok!(Tokens::set_balance(
			RuntimeOrigin::root(),
			ALICE,
			POOL_ID,
			1_000_000 * ONE * 1_000_000,
			0,
		));

		let snapshot = Pallet::<Test>::create_snapshot(POOL_ID).unwrap();
		assert_eq!(snapshot.share_issuance, ShareIssuance::<Test>::get(POOL_ID));
		assert_eq!(snapshot.share_issuance, INITIAL_SHARES);
	});
}

// debug_assert compiles out in release builds, so the panic only exists under debug_assertions
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "virtual share issuance")]
fn add_liquidity_should_panic_when_issuance_desynced_in_debug_build() {
	pool_with_initial_liquidity().execute_with(|| {
		assert_ok!(Tokens::set_balance(
			RuntimeOrigin::root(),
			ALICE,
			POOL_ID,
			1_000_000 * ONE * 1_000_000,
			0,
		));

		let _ = Stableswap::add_assets_liquidity(
			RuntimeOrigin::signed(ALICE),
			POOL_ID,
			BoundedVec::truncate_from(vec![
				AssetAmount::new(ASSET_A, 100 * ONE),
				AssetAmount::new(ASSET_B, 100 * ONE),
			]),
			Balance::zero(),
		);
	});
}
