#![cfg(test)]

use crate::polkadot_test_net::{hydra_live_ext, ALICE, BOB};
use frame_support::assert_ok;
use hydradx_runtime::{AssetId, Balance, Currencies, Omnipool, Router, RuntimeOrigin, Stableswap, System, XYK};
use hydradx_traits::router::{PoolType, Trade};
use orml_traits::MultiCurrency;

const PATH_TO_SNAPSHOT: &str = "snapshots/slim/SNAPSHOT";

const HDX: AssetId = 0;
const HDX_UNITS: Balance = 1_000_000_000_000;

/// Reset block number and clear stale DynamicFees entries to avoid debug_assert panics.
fn init_block() {
	System::set_block_number(1);
	pallet_timestamp::Pallet::<hydradx_runtime::Runtime>::set_timestamp(1);
	// Clear DynamicFees.AssetFee entries to avoid timestamp mismatch with reset oracle entries
	let prefix = {
		let mut p = Vec::new();
		p.extend_from_slice(&sp_io::hashing::twox_128(b"DynamicFees"));
		p.extend_from_slice(&sp_io::hashing::twox_128(b"AssetFee"));
		p
	};
	sp_io::storage::clear_prefix(&prefix, None);
}

/// Find the first asset in the Omnipool (besides `exclude`).
fn find_omnipool_asset(exclude: AssetId) -> Option<(AssetId, Balance)> {
	let omnipool_account = Omnipool::protocol_account();
	let prefix = {
		let mut p = Vec::new();
		p.extend_from_slice(&sp_io::hashing::twox_128(b"Omnipool"));
		p.extend_from_slice(&sp_io::hashing::twox_128(b"Assets"));
		p
	};
	let mut key = sp_io::storage::next_key(&prefix);
	while let Some(k) = key {
		if !k.starts_with(&prefix) {
			break;
		}
		if k.len() >= 52 {
			let asset_id = u32::from_le_bytes(k[48..52].try_into().unwrap());
			if asset_id != exclude {
				let balance = Currencies::free_balance(asset_id, &omnipool_account);
				if balance > 0 {
					return Some((asset_id, balance));
				}
			}
		}
		key = sp_io::storage::next_key(&k);
	}
	None
}

/// Find a Stableswap pool and its constituent assets.
/// Returns (pool_id, asset_a, asset_b) where asset_a and asset_b are the first two pool assets.
fn find_stableswap_pool() -> Option<(AssetId, AssetId, AssetId)> {
	let prefix = {
		let mut p = Vec::new();
		p.extend_from_slice(&sp_io::hashing::twox_128(b"Stableswap"));
		p.extend_from_slice(&sp_io::hashing::twox_128(b"Pools"));
		p
	};
	let mut key = sp_io::storage::next_key(&prefix);
	while let Some(ref k) = key {
		if !k.starts_with(&prefix) {
			break;
		}
		if k.len() >= 52 {
			let pool_id = u32::from_le_bytes(k[48..52].try_into().unwrap());
			if let Some(val) = sp_io::storage::get(k) {
				// Pool struct starts with: assets: BoundedVec<AssetId, MaxAssets>
				// SCALE-encoded BoundedVec starts with compact length prefix
				if val.len() >= 10 {
					let (asset_count, offset) = if val[0] < 0xFC {
						((val[0] >> 2) as usize, 1usize)
					} else {
						key = sp_io::storage::next_key(k);
						continue;
					};
					if asset_count >= 2 && val.len() >= offset + 8 {
						let asset_a = u32::from_le_bytes(val[offset..offset + 4].try_into().unwrap());
						let asset_b = u32::from_le_bytes(val[offset + 4..offset + 8].try_into().unwrap());

						let pool_account = {
							let mut hash_input = Vec::new();
							hash_input.extend_from_slice(b"sts");
							hash_input.extend_from_slice(&pool_id.to_le_bytes());
							sp_runtime::AccountId32::new(sp_io::hashing::blake2_256(&hash_input))
						};
						let bal_a = Currencies::free_balance(asset_a, &pool_account);
						let bal_b = Currencies::free_balance(asset_b, &pool_account);
						// Require meaningful but not huge liquidity to avoid overflow
						// Skip pools with very large balances (likely 18-decimal EVM assets)
						let max_bal = 1_000_000_000_000_000_000_000u128; // 10^21
						if bal_a > 1_000_000_000_000_000
							&& bal_b > 1_000_000_000_000_000
							&& bal_a < max_bal && bal_b < max_bal
						{
							return Some((pool_id, asset_a, asset_b));
						}
					}
				}
			}
		}
		key = sp_io::storage::next_key(k);
	}
	None
}

/// Find an XYK pool with liquidity.
/// Returns (asset_a, asset_b).
fn find_xyk_pool() -> Option<(AssetId, AssetId)> {
	let prefix = {
		let mut p = Vec::new();
		p.extend_from_slice(&sp_io::hashing::twox_128(b"XYK"));
		p.extend_from_slice(&sp_io::hashing::twox_128(b"PoolAssets"));
		p
	};
	let mut key = sp_io::storage::next_key(&prefix);
	while let Some(k) = key {
		if !k.starts_with(&prefix) {
			break;
		}
		if let Some(val) = sp_io::storage::get(&k) {
			if val.len() >= 8 {
				let asset_a = u32::from_le_bytes(val[0..4].try_into().unwrap());
				let asset_b = u32::from_le_bytes(val[4..8].try_into().unwrap());

				// Check the pool account has liquidity
				let pool_account_bytes = &k[k.len() - 32..];
				let pool_account = sp_runtime::AccountId32::new(pool_account_bytes.try_into().unwrap());
				let bal_a = Currencies::free_balance(asset_a, &pool_account);
				let bal_b = Currencies::free_balance(asset_b, &pool_account);
				// Prefer pools where both assets are well-known (id < 100)
				let has_known_asset = asset_a < 100 && asset_b < 100;
				let max_bal = 1_000_000_000_000_000_000_000u128;
				if has_known_asset
					&& bal_a > 1_000_000_000_000_000
					&& bal_b > 1_000_000_000_000_000
					&& bal_a < max_bal
					&& bal_b < max_bal
				{
					return Some((asset_a, asset_b));
				}
			}
		}
		key = sp_io::storage::next_key(&k);
	}
	None
}

// =============================================================================
// Snapshot loading
// =============================================================================

#[test]
fn slim_snapshot_should_load() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		let block_number = frame_system::Pallet::<hydradx_runtime::Runtime>::block_number();
		assert!(block_number > 0, "Block number should be > 0 from production snapshot");
	});
}

#[test]
fn protocol_accounts_have_balances() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		let omnipool_account = Omnipool::protocol_account();
		let omnipool_hdx = Currencies::free_balance(HDX, &omnipool_account);
		assert!(omnipool_hdx > 0, "Omnipool should hold HDX: {omnipool_hdx}");

		let treasury = hydradx_runtime::Treasury::account_id();
		let treasury_hdx = Currencies::free_balance(HDX, &treasury);
		assert!(treasury_hdx > 0, "Treasury should hold HDX: {treasury_hdx}");
	});
}

// =============================================================================
// Omnipool trades
// =============================================================================

#[test]
fn omnipool_sell_hdx_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (asset_out, _) = find_omnipool_asset(HDX).expect("No tradeable Omnipool asset found");

		assert_ok!(Currencies::deposit(HDX, &alice, 10_000 * HDX_UNITS));
		let balance_before = Currencies::free_balance(asset_out, &alice);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(alice.clone()),
			HDX,
			asset_out,
			1_000 * HDX_UNITS,
			0u128,
		));

		let balance_after = Currencies::free_balance(asset_out, &alice);
		assert!(
			balance_after > balance_before,
			"Alice should have received asset {asset_out}"
		);
	});
}

#[test]
fn omnipool_buy_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (asset_out, _) = find_omnipool_asset(HDX).expect("No tradeable Omnipool asset found");

		// Buy 0.1% of what the Omnipool holds
		let omnipool_account = Omnipool::protocol_account();
		let pool_bal = Currencies::free_balance(asset_out, &omnipool_account);
		let buy_amount = pool_bal / 1000;
		assert_ok!(Currencies::deposit(HDX, &alice, 1_000_000 * HDX_UNITS));
		let balance_before = Currencies::free_balance(asset_out, &alice);

		assert_ok!(Omnipool::buy(
			RuntimeOrigin::signed(alice.clone()),
			asset_out,
			HDX,
			buy_amount,
			u128::MAX,
		));

		let balance_after = Currencies::free_balance(asset_out, &alice);
		assert!(
			balance_after > balance_before,
			"Alice should have bought asset {asset_out}"
		);
	});
}

#[test]
fn omnipool_sell_between_two_non_hdx_assets_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let bob = sp_runtime::AccountId32::from(BOB);

		let (asset_a, _) = find_omnipool_asset(HDX).expect("No tradeable Omnipool asset found");
		let (asset_b, _) = match find_omnipool_asset(asset_a) {
			Some(ab) => ab,
			None => {
				println!("Only one non-HDX Omnipool asset found, skipping");
				return;
			}
		};

		// Use 0.1% of the Omnipool's balance of asset_a to avoid liquidity limits
		let omnipool_account = Omnipool::protocol_account();
		let pool_balance = Currencies::free_balance(asset_a, &omnipool_account);
		let sell_amount = pool_balance / 1000; // 0.1%
		assert_ok!(Currencies::deposit(asset_a, &bob, sell_amount * 2));
		let balance_before = Currencies::free_balance(asset_b, &bob);

		assert_ok!(Omnipool::sell(
			RuntimeOrigin::signed(bob.clone()),
			asset_a,
			asset_b,
			sell_amount,
			0u128,
		));

		let balance_after = Currencies::free_balance(asset_b, &bob);
		assert!(
			balance_after > balance_before,
			"Bob should have received asset {asset_b}"
		);
	});
}

// =============================================================================
// Router trades
// =============================================================================

#[test]
fn router_sell_via_omnipool_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (asset_out, _) = find_omnipool_asset(HDX).expect("No tradeable Omnipool asset found");

		assert_ok!(Currencies::deposit(HDX, &alice, 10_000 * HDX_UNITS));
		let balance_before = Currencies::free_balance(asset_out, &alice);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(alice.clone()),
			HDX,
			asset_out,
			500 * HDX_UNITS,
			0,
			vec![Trade {
				pool: PoolType::Omnipool,
				asset_in: HDX,
				asset_out,
			}]
			.try_into()
			.unwrap(),
		));

		let balance_after = Currencies::free_balance(asset_out, &alice);
		assert!(balance_after > balance_before, "Router Omnipool sell should work");
	});
}

// =============================================================================
// Stableswap trades
// =============================================================================

#[test]
fn stableswap_sell_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (pool_id, asset_in, asset_out) = match find_stableswap_pool() {
			Some(pool) => pool,
			None => {
				println!("No Stableswap pool with liquidity found, skipping");
				return;
			}
		};
		println!("Stableswap sell: pool={pool_id}, {asset_in} -> {asset_out}");

		// Use a small fraction of pool liquidity
		let pool_account = {
			let mut hash_input = Vec::new();
			hash_input.extend_from_slice(b"sts");
			hash_input.extend_from_slice(&pool_id.to_le_bytes());
			sp_runtime::AccountId32::new(sp_io::hashing::blake2_256(&hash_input))
		};
		let pool_bal = Currencies::free_balance(asset_in, &pool_account);
		let sell_amount = pool_bal / 100; // 1% of pool
		assert_ok!(Currencies::deposit(asset_in, &alice, sell_amount * 2));

		let balance_before = Currencies::free_balance(asset_out, &alice);

		assert_ok!(Stableswap::sell(
			RuntimeOrigin::signed(alice.clone()),
			pool_id,
			asset_in,
			asset_out,
			sell_amount,
			0u128,
		));

		let balance_after = Currencies::free_balance(asset_out, &alice);
		assert!(
			balance_after > balance_before,
			"Alice should have received asset {asset_out} from Stableswap"
		);
	});
}

#[test]
fn stableswap_buy_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let bob = sp_runtime::AccountId32::from(BOB);

		let (pool_id, asset_in, asset_out) = match find_stableswap_pool() {
			Some(pool) => pool,
			None => {
				println!("No Stableswap pool with liquidity found, skipping");
				return;
			}
		};
		println!("Stableswap buy: pool={pool_id}, pay {asset_in} -> buy {asset_out}");

		let pool_account = {
			let mut hash_input = Vec::new();
			hash_input.extend_from_slice(b"sts");
			hash_input.extend_from_slice(&pool_id.to_le_bytes());
			sp_runtime::AccountId32::new(sp_io::hashing::blake2_256(&hash_input))
		};
		let pool_bal_out = Currencies::free_balance(asset_out, &pool_account);
		let buy_amount = pool_bal_out / 100; // 1% of pool
		assert_ok!(Currencies::deposit(asset_in, &bob, pool_bal_out)); // enough to cover

		let balance_before = Currencies::free_balance(asset_out, &bob);

		assert_ok!(Stableswap::buy(
			RuntimeOrigin::signed(bob.clone()),
			pool_id,
			asset_out,
			asset_in,
			buy_amount,
			u128::MAX,
		));

		let balance_after = Currencies::free_balance(asset_out, &bob);
		assert!(
			balance_after > balance_before,
			"Bob should have received asset {asset_out} from Stableswap buy"
		);
	});
}

#[test]
fn router_sell_via_stableswap_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (pool_id, asset_in, asset_out) = match find_stableswap_pool() {
			Some(pool) => pool,
			None => {
				println!("No Stableswap pool with liquidity found, skipping");
				return;
			}
		};
		println!("Router Stableswap sell: pool={pool_id}, {asset_in} -> {asset_out}");

		let pool_account = {
			let mut hash_input = Vec::new();
			hash_input.extend_from_slice(b"sts");
			hash_input.extend_from_slice(&pool_id.to_le_bytes());
			sp_runtime::AccountId32::new(sp_io::hashing::blake2_256(&hash_input))
		};
		let pool_bal = Currencies::free_balance(asset_in, &pool_account);
		let sell_amount = pool_bal / 1000;
		assert_ok!(Currencies::deposit(asset_in, &alice, sell_amount * 2));
		let balance_before = Currencies::free_balance(asset_out, &alice);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(alice.clone()),
			asset_in,
			asset_out,
			sell_amount,
			0,
			vec![Trade {
				pool: PoolType::Stableswap(pool_id),
				asset_in,
				asset_out,
			}]
			.try_into()
			.unwrap(),
		));

		let balance_after = Currencies::free_balance(asset_out, &alice);
		assert!(balance_after > balance_before, "Router Stableswap sell should work");
	});
}

// =============================================================================
// XYK trades
// =============================================================================

#[test]
fn xyk_sell_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (asset_a, asset_b) = match find_xyk_pool() {
			Some(pool) => pool,
			None => {
				println!("No XYK pool with liquidity found, skipping");
				return;
			}
		};
		println!("XYK sell: {asset_a} -> {asset_b}");

		// Use a fraction of pool liquidity to avoid ED/slippage issues
		let pool_account_bytes = {
			// The XYK pool account is determined by the find_xyk_pool key — re-derive it
			let mut buf: Vec<u8> = b"xyk".to_vec();
			let (min, max) = if asset_a < asset_b {
				(asset_a, asset_b)
			} else {
				(asset_b, asset_a)
			};
			buf.extend_from_slice(&min.to_le_bytes());
			buf.extend_from_slice(&max.to_le_bytes());
			sp_io::hashing::blake2_256(&buf)
		};
		let pool_account = sp_runtime::AccountId32::new(pool_account_bytes);
		let pool_bal = Currencies::free_balance(asset_a, &pool_account);
		let sell_amount = pool_bal / 100; // 1% of pool
		assert_ok!(Currencies::deposit(asset_a, &alice, sell_amount * 2));

		let balance_before = Currencies::free_balance(asset_b, &alice);

		assert_ok!(XYK::sell(
			RuntimeOrigin::signed(alice.clone()),
			asset_a,
			asset_b,
			sell_amount,
			0u128,
			false,
		));

		let balance_after = Currencies::free_balance(asset_b, &alice);
		assert!(
			balance_after > balance_before,
			"Alice should have received asset {asset_b} from XYK sell"
		);
	});
}

#[test]
fn xyk_buy_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let bob = sp_runtime::AccountId32::from(BOB);

		let (asset_a, asset_b) = match find_xyk_pool() {
			Some(pool) => pool,
			None => {
				println!("No XYK pool with liquidity found, skipping");
				return;
			}
		};
		println!("XYK buy: buy {asset_b} with {asset_a}");

		let pool_account_bytes = {
			let mut buf: Vec<u8> = b"xyk".to_vec();
			let (min, max) = if asset_a < asset_b {
				(asset_a, asset_b)
			} else {
				(asset_b, asset_a)
			};
			buf.extend_from_slice(&min.to_le_bytes());
			buf.extend_from_slice(&max.to_le_bytes());
			sp_io::hashing::blake2_256(&buf)
		};
		let pool_account = sp_runtime::AccountId32::new(pool_account_bytes);
		let pool_bal_b = Currencies::free_balance(asset_b, &pool_account);
		let buy_amount = pool_bal_b / 10; // 10% of pool
		assert_ok!(Currencies::deposit(
			asset_a,
			&bob,
			Currencies::free_balance(asset_a, &pool_account)
		));

		let balance_before = Currencies::free_balance(asset_b, &bob);

		assert_ok!(XYK::buy(
			RuntimeOrigin::signed(bob.clone()),
			asset_b,
			asset_a,
			buy_amount,
			u128::MAX,
			false,
		));

		let balance_after = Currencies::free_balance(asset_b, &bob);
		assert!(
			balance_after > balance_before,
			"Bob should have received asset {asset_b} from XYK buy"
		);
	});
}

#[test]
fn router_sell_via_xyk_should_work() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		init_block();
		let alice = sp_runtime::AccountId32::from(ALICE);

		let (asset_a, asset_b) = match find_xyk_pool() {
			Some(pool) => pool,
			None => {
				println!("No XYK pool with liquidity found, skipping");
				return;
			}
		};
		println!("Router XYK sell: {asset_a} -> {asset_b}");

		let pool_account_bytes = {
			let mut buf: Vec<u8> = b"xyk".to_vec();
			let (min, max) = if asset_a < asset_b {
				(asset_a, asset_b)
			} else {
				(asset_b, asset_a)
			};
			buf.extend_from_slice(&min.to_le_bytes());
			buf.extend_from_slice(&max.to_le_bytes());
			sp_io::hashing::blake2_256(&buf)
		};
		let pool_account = sp_runtime::AccountId32::new(pool_account_bytes);
		let pool_bal = Currencies::free_balance(asset_a, &pool_account);
		let sell_amount = pool_bal / 100; // 1% of pool
		assert_ok!(Currencies::deposit(asset_a, &alice, sell_amount * 2));

		let balance_before = Currencies::free_balance(asset_b, &alice);

		assert_ok!(Router::sell(
			RuntimeOrigin::signed(alice.clone()),
			asset_a,
			asset_b,
			sell_amount,
			0,
			vec![Trade {
				pool: PoolType::XYK,
				asset_in: asset_a,
				asset_out: asset_b,
			}]
			.try_into()
			.unwrap(),
		));

		let balance_after = Currencies::free_balance(asset_b, &alice);
		assert!(balance_after > balance_before, "Router XYK sell should work");
	});
}

// =============================================================================
// Aave (supply via EVM)
// =============================================================================

#[test]
fn aave_supply_dot_should_work() {
	use fp_evm::ExitReason::Succeed;
	use fp_evm::ExitSucceed::Returned;
	use hydradx_runtime::{
		evm::{precompiles::erc20_mapping::HydraErc20Mapping, Executor},
		AccountId, EVMAccounts, Runtime,
	};
	use hydradx_traits::evm::{CallContext, Erc20Encoding, InspectEvmAccounts, EVM};
	use liquidation_worker_support::*;
	use pallet_liquidation::BorrowingContract;
	use sp_core::U256;

	const DOT: AssetId = 5;
	const DOT_UNIT: Balance = 10_000_000_000;

	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		// Don't call init_block() — Aave needs realistic timestamps for interest calculations

		// BorrowingContract and ApprovedContract are already set from production state
		let pool_contract = <BorrowingContract<Runtime>>::get();
		println!("Pool contract from snapshot: {pool_contract:?}");

		let alice = sp_runtime::AccountId32::from(ALICE);
		assert_ok!(Currencies::deposit(DOT, &alice, 1_000 * DOT_UNIT));

		assert_ok!(EVMAccounts::bind_evm_address(RuntimeOrigin::signed(alice.clone())));
		let alice_evm = EVMAccounts::evm_address(&AccountId::from(ALICE));

		// Supply DOT to Aave
		let evm_dot = HydraErc20Mapping::encode_evm_address(DOT);
		let context = CallContext::new_call(pool_contract, alice_evm);
		let data = hydradx_runtime::evm::precompiles::handle::EvmDataWriter::new_with_selector(Function::Supply)
			.write(evm_dot)
			.write(100 * DOT_UNIT)
			.write(alice_evm)
			.write(0u32)
			.build();

		let call_result = Executor::<Runtime>::call(context, data, U256::zero(), 5_000_000);
		assert_eq!(
			call_result.exit_reason,
			Succeed(Returned),
			"Aave supply failed: {:?}",
			hex::encode(call_result.value)
		);
		println!("Aave supply succeeded!");
	});
}

// =============================================================================
// Discovery (diagnostic, ignored by default)
// =============================================================================

#[test]
#[ignore]
fn discover_pools_and_assets() {
	let mut ext = hydra_live_ext(PATH_TO_SNAPSHOT);
	ext.execute_with(|| {
		let prefix = {
			let mut p = Vec::new();
			p.extend_from_slice(&sp_io::hashing::twox_128(b"Stableswap"));
			p.extend_from_slice(&sp_io::hashing::twox_128(b"Pools"));
			p
		};
		let mut key = sp_io::storage::next_key(&prefix);
		while let Some(k) = key {
			if !k.starts_with(&prefix) {
				break;
			}
			if k.len() >= 52 {
				let pool_id = u32::from_le_bytes(k[48..52].try_into().unwrap());
				println!("Stableswap pool_id: {pool_id}");
			}
			key = sp_io::storage::next_key(&k);
		}

		let prefix = {
			let mut p = Vec::new();
			p.extend_from_slice(&sp_io::hashing::twox_128(b"Omnipool"));
			p.extend_from_slice(&sp_io::hashing::twox_128(b"Assets"));
			p
		};
		let mut key = sp_io::storage::next_key(&prefix);
		while let Some(k) = key {
			if !k.starts_with(&prefix) {
				break;
			}
			if k.len() >= 52 {
				let asset_id = u32::from_le_bytes(k[48..52].try_into().unwrap());
				println!("Omnipool asset: {asset_id}");
			}
			key = sp_io::storage::next_key(&k);
		}
	});
}
