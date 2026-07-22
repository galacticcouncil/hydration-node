// Integration coverage for the slippage-guarded XYK LP paths that the suite
// otherwise exercises only at the pallet level: `add_liquidity_with_limits` and
// `remove_liquidity_with_limits`. Also includes a regression test asserting
// `create_pool` (which is not `#[transactional]`) leaves no orphan pool state
// when an inner asset transfer fails.

use crate::driver::HydrationTestDriver;
use crate::polkadot_test_net::*;
use frame_support::{assert_noop, assert_ok};
use hydradx_runtime::*;
use hydradx_traits::AMM;
use orml_traits::{MultiCurrency, MultiLockableCurrency};
use pallet_xyk::types::AssetPair;
use pretty_assertions::assert_eq;

const ASSET_A: AssetId = 4400;
const ASSET_B: AssetId = 4401;
const ORPHAN_A: AssetId = 4402;
const ORPHAN_B: AssetId = 4403;

const DEC: u8 = 12;
const ENDOWMENT: Balance = 1_000_000 * UNITS;
const INITIAL_A: Balance = 100 * UNITS;
const INITIAL_B: Balance = 200 * UNITS;

fn balance(who: [u8; 32], asset: AssetId) -> Balance {
	Tokens::free_balance(asset, &who.into())
}

fn pair() -> AssetPair {
	AssetPair {
		asset_in: ASSET_A,
		asset_out: ASSET_B,
	}
}

/// XYK pool `ASSET_A`/`ASSET_B` created by ALICE with a 100:200 reserve ratio.
/// ALICE keeps a large balance of both assets for follow-up liquidity ops.
fn xyk_pool() -> HydrationTestDriver {
	let driver = HydrationTestDriver::default()
		.register_asset(ASSET_A, b"exXKA", DEC, None)
		.register_asset(ASSET_B, b"exXKB", DEC, None);
	driver
		.new_block()
		.endow_account(ALICE.into(), ASSET_A, ENDOWMENT)
		.endow_account(ALICE.into(), ASSET_B, ENDOWMENT)
		.execute(|| {
			assert_ok!(XYK::create_pool(
				RuntimeOrigin::signed(ALICE.into()),
				ASSET_A,
				INITIAL_A,
				ASSET_B,
				INITIAL_B,
			));
		});
	driver
}

#[test]
fn add_liquidity_with_limits_should_mint_shares_when_within_limits() {
	xyk_pool().execute(|| {
		let share_token = XYK::get_share_token(pair());

		let amount_a = 10 * UNITS;
		let amount_b_max = 1_000 * UNITS;
		let min_shares = 1;

		let a_before = balance(ALICE, ASSET_A);
		let b_before = balance(ALICE, ASSET_B);
		let shares_before = balance(ALICE, share_token);

		assert_ok!(XYK::add_liquidity_with_limits(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_A,
			ASSET_B,
			amount_a,
			amount_b_max,
			min_shares,
		));

		// asset_a is debited by exactly the requested amount
		assert_eq!(a_before - balance(ALICE, ASSET_A), amount_a);

		// asset_b is debited by a positive amount within the supplied max limit
		let spent_b = b_before - balance(ALICE, ASSET_B);
		assert!(spent_b > 0, "add_liquidity must consume the paired asset");
		assert!(spent_b <= amount_b_max, "spent {spent_b} exceeded max {amount_b_max}");

		// shares were minted to ALICE
		let minted = balance(ALICE, share_token) - shares_before;
		assert!(minted > 0, "shares must be minted");
		assert!(minted >= min_shares, "minted {minted} below min_shares {min_shares}");
	});
}

#[test]
fn add_liquidity_with_limits_should_fail_when_min_shares_not_met() {
	xyk_pool().execute(|| {
		assert_noop!(
			XYK::add_liquidity_with_limits(
				RuntimeOrigin::signed(ALICE.into()),
				ASSET_A,
				ASSET_B,
				10 * UNITS,
				1_000 * UNITS,
				1_000_000 * UNITS, // far above the shares this deposit can mint
			),
			pallet_xyk::Error::<hydradx_runtime::Runtime>::SlippageLimit
		);
	});
}

#[test]
fn remove_liquidity_with_limits_should_return_assets_when_within_limits() {
	xyk_pool().execute(|| {
		let share_token = XYK::get_share_token(pair());

		let shares_before = balance(ALICE, share_token);
		assert!(shares_before > 0, "setup should have issued shares to ALICE");

		let burn = shares_before / 2;
		let a_before = balance(ALICE, ASSET_A);
		let b_before = balance(ALICE, ASSET_B);

		assert_ok!(XYK::remove_liquidity_with_limits(
			RuntimeOrigin::signed(ALICE.into()),
			ASSET_A,
			ASSET_B,
			burn,
			1, // min_amount_a
			1, // min_amount_b
		));

		// exactly the requested shares are burned
		assert_eq!(shares_before - balance(ALICE, share_token), burn);

		// both underlying assets are returned
		assert!(balance(ALICE, ASSET_A) > a_before, "asset_a must be returned");
		assert!(balance(ALICE, ASSET_B) > b_before, "asset_b must be returned");
	});
}

#[test]
fn remove_liquidity_with_limits_should_fail_when_min_amount_not_met() {
	xyk_pool().execute(|| {
		let share_token = XYK::get_share_token(pair());
		let burn = balance(ALICE, share_token) / 2;

		assert_noop!(
			XYK::remove_liquidity_with_limits(
				RuntimeOrigin::signed(ALICE.into()),
				ASSET_A,
				ASSET_B,
				burn,
				1_000_000 * UNITS, // min_amount_a far above what the burn returns
				1,
			),
			pallet_xyk::Error::<hydradx_runtime::Runtime>::SlippageLimit
		);
	});
}

// Regression: `create_pool` is not `#[transactional]`. It writes `ShareToken`
// and `PoolAssets`, transfers `asset_a` into the pool, and only then transfers
// `asset_b`. Real extrinsic dispatch runs inside the executive's storage layer,
// which rolls the whole call back on error; this test reproduces that layer with
// `with_storage_layer` and asserts no orphan pool state survives a failed create.
//
// The failure is constructed cleanly with an ORML lock on `asset_b`: `free_balance`
// ignores locks, so the pre-transfer balance check passes, but `transfer`'s
// `ensure_can_withdraw` requires `free - amount >= frozen`, so the second transfer
// (asset_b) fails after the first (asset_a) has already succeeded.
#[test]
fn create_pool_should_not_leave_orphan_state_when_asset_transfer_fails() {
	let driver = HydrationTestDriver::default()
		.register_asset(ORPHAN_A, b"exXKC", DEC, None)
		.register_asset(ORPHAN_B, b"exXKD", DEC, None);
	driver
		.new_block()
		.endow_account(ALICE.into(), ORPHAN_A, 1_000 * UNITS)
		.endow_account(ALICE.into(), ORPHAN_B, 1_000 * UNITS)
		.execute(|| {
			// Lock most of asset_b: free stays at 1_000 units (passes the balance
			// check) but only 100 units are transferable.
			assert_ok!(<Tokens as MultiLockableCurrency<AccountId>>::set_lock(
				*b"xykorph1",
				ORPHAN_B,
				&ALICE.into(),
				900 * UNITS,
			));

			let orphan_pair = AssetPair {
				asset_in: ORPHAN_A,
				asset_out: ORPHAN_B,
			};
			let pool_account = XYK::get_pair_id(orphan_pair);
			let a_before = balance(ALICE, ORPHAN_A);

			let result = frame_support::storage::with_storage_layer(|| -> sp_runtime::DispatchResult {
				XYK::create_pool(
					RuntimeOrigin::signed(ALICE.into()),
					ORPHAN_A,
					100 * UNITS,
					ORPHAN_B,
					200 * UNITS,
				)
			});
			assert!(
				result.is_err(),
				"create_pool should fail when the asset_b transfer is blocked"
			);

			// No orphan pool state may survive the failed call.
			assert!(
				!XYK::exists(orphan_pair),
				"pool must not be registered after a failed create_pool"
			);
			assert_eq!(
				balance(ALICE, ORPHAN_A),
				a_before,
				"asset_a must not be moved when create_pool fails"
			);
			assert_eq!(
				Tokens::free_balance(ORPHAN_A, &pool_account),
				0,
				"pool account must hold no asset_a after a failed create_pool"
			);
		});
}
