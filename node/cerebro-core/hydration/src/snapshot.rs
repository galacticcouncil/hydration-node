//! Pool-state reads → core's generic snapshot types (`SnapshotProvider`). Pool reserves use FREE
//! balance (a pool holds no reserved funds); this is distinct from the confirmation layer's
//! free+reserved account reads. When the `AmmSnapshotApi` runtime call lands on mainnet this whole
//! module is replaced by one `state_call`, with core untouched (see MIGRATION_PLAN §1.3b).

use crate::balance;
use crate::transport::Transport;
use cerebro_core::reader::{AssetReserve, OmnipoolAssetSnapshot, StableswapSnapshot, XykSnapshot};
use cerebro_core::{AssetId, Balance};
use codec::{Decode, Encode};
use hydra_dx_math::stableswap::calculate_amplification;
use hydradx_runtime::Runtime;
use orml_tokens::AccountData;
use pallet_stableswap::types::{PoolInfo, PoolPegInfo};
use sp_core::hashing::{blake2_128, blake2_256, twox_128};
use sp_core::H256;

type PoolInfoOf = PoolInfo<AssetId, u32>;
type PoolPegInfoOf = PoolPegInfo<u32, AssetId>;

const NATIVE: AssetId = 0;

/// Build a stableswap pool snapshot with amplification interpolated to `at`'s block and pegs live.
pub fn stableswap<T: Transport>(api: &T, pool_id: AssetId, at: H256) -> Option<StableswapSnapshot> {
	let pool = pool_info(api, pool_id, at)?;
	let block = api.block_number(at.0).ok()?;
	let amplification = calculate_amplification(
		pool.initial_amplification.get() as u128,
		pool.final_amplification.get() as u128,
		pool.initial_block as u128,
		pool.final_block as u128,
		block as u128,
	);
	// Every read is issued before any `?`, so a batching transport sees them all at once.
	let reserves: Vec<Option<AssetReserve>> = pool
		.assets
		.iter()
		.map(|a| {
			let amount = reserve_of(api, pool_account(pool_id), *a, at);
			let decimals = crate::pricing::decimals(api, at, *a);
			Some(AssetReserve {
				amount: amount?,
				decimals: decimals? as u8,
			})
		})
		.collect();
	let pegs = pool_pegs(api, pool_id, pool.assets.len(), at);
	let share_issuance = total_issuance(api, pool_id, at);
	let reserves = reserves.into_iter().collect::<Option<Vec<_>>>()?;
	let share_issuance = share_issuance?;
	Some(StableswapSnapshot {
		assets: pool.assets.to_vec(),
		reserves,
		amplification,
		pegs,
		share_issuance,
	})
}

/// The asset ids composing a stableswap pool (for par-anchoring its members). Public so the
/// monitor's anchor-seed builder can reach it without a full snapshot.
pub fn stable_pool_assets<T: Transport>(api: &T, pool_id: AssetId, at: H256) -> Option<Vec<AssetId>> {
	pool_info(api, pool_id, at).map(|p| p.assets.to_vec())
}

pub fn xyk<T: Transport>(api: &T, pair_account: &[u8; 32], at: H256) -> Option<XykSnapshot> {
	let share_token = xyk_share_token(api, pair_account, at);
	let share_issuance = xyk_total_liquidity(api, pair_account, at);
	let assets = xyk_pair(api, pair_account, at)?;
	let ra = reserve_of(api, *pair_account, assets.0, at);
	let rb = reserve_of(api, *pair_account, assets.1, at);
	let (share_token, share_issuance) = (share_token?, share_issuance?);
	Some(XykSnapshot {
		assets,
		reserves: (ra?, rb?),
		share_token,
		share_issuance,
	})
}

pub fn omnipool_asset<T: Transport>(api: &T, asset: AssetId, at: H256) -> Option<OmnipoolAssetSnapshot> {
	let reserve = reserve_of(api, omnipool_account(), asset, at);
	let head = omni_asset_state(api, asset, at)?;
	let reserve = reserve?;
	Some(OmnipoolAssetSnapshot {
		reserve,
		hub_reserve: head.hub_reserve,
		shares: head.shares,
	})
}

// ---- reserves (FREE balance): orml Tokens / native System.Account / Erc20 balanceOf ----

fn reserve_of<T: Transport>(api: &T, acc: [u8; 32], asset: AssetId, at: H256) -> Option<u128> {
	if asset == NATIVE {
		native_free(api, &acc, at)
	} else if let Some(contract) = balance::atoken_contract(api, asset, at) {
		balance::erc20_balance_of(api, contract, balance::evm_h160(&acc), at)
	} else {
		token_free(api, &acc, asset, at)
	}
}

fn token_free<T: Transport>(api: &T, who: &[u8; 32], asset: AssetId, at: H256) -> Option<Balance> {
	let key = orml_tokens::Accounts::<Runtime>::hashed_key_for(runtime_account(who), asset);
	match api.storage(&key, at.0).ok()? {
		Some(raw) => AccountData::<Balance>::decode(&mut &raw[..]).ok().map(|d| d.free),
		None => Some(0),
	}
}

#[derive(Decode)]
struct SysAccountHead {
	_nonce: u32,
	_consumers: u32,
	_providers: u32,
	_sufficients: u32,
	free: u128,
}

fn native_free<T: Transport>(api: &T, who: &[u8; 32], at: H256) -> Option<Balance> {
	match api.storage(&map_key("System", "Account", who), at.0).ok()? {
		Some(raw) => SysAccountHead::decode(&mut &raw[..]).ok().map(|d| d.free),
		None => Some(0),
	}
}

// ---- stableswap storage ----

fn pool_info<T: Transport>(api: &T, pool_id: AssetId, at: H256) -> Option<PoolInfoOf> {
	let key = pallet_stableswap::Pools::<Runtime>::hashed_key_for(pool_id);
	let raw = api.storage(&key, at.0).ok()??;
	PoolInfoOf::decode(&mut &raw[..]).ok()
}

fn pool_pegs<T: Transport>(api: &T, pool_id: AssetId, n_assets: usize, at: H256) -> Vec<(Balance, Balance)> {
	let default = vec![(1u128, 1u128); n_assets];
	let key = pallet_stableswap::PoolPegs::<Runtime>::hashed_key_for(pool_id);
	let Ok(Some(raw)) = api.storage(&key, at.0) else {
		return default;
	};
	match PoolPegInfoOf::decode(&mut &raw[..]) {
		Ok(info) if info.current.len() == n_assets => info.current.into_inner(),
		_ => default,
	}
}

fn total_issuance<T: Transport>(api: &T, asset: AssetId, at: H256) -> Option<Balance> {
	let key = orml_tokens::TotalIssuance::<Runtime>::hashed_key_for(asset);
	match api.storage(&key, at.0).ok()? {
		Some(raw) => Balance::decode(&mut &raw[..]).ok(),
		None => Some(0),
	}
}

// ---- omnipool storage ----

#[derive(Decode)]
struct OmniAssetHead {
	hub_reserve: u128,
	shares: u128,
}

fn omni_asset_state<T: Transport>(api: &T, asset: AssetId, at: H256) -> Option<OmniAssetHead> {
	let raw = api
		.storage(&map_key("Omnipool", "Assets", &asset.encode()), at.0)
		.ok()??;
	OmniAssetHead::decode(&mut &raw[..]).ok()
}

// ---- xyk storage ----

fn xyk_pair<T: Transport>(api: &T, acc: &[u8; 32], at: H256) -> Option<(AssetId, AssetId)> {
	let raw = api.storage(&map_key("XYK", "PoolAssets", acc), at.0).ok()??;
	<(AssetId, AssetId)>::decode(&mut &raw[..]).ok()
}

fn xyk_share_token<T: Transport>(api: &T, acc: &[u8; 32], at: H256) -> Option<AssetId> {
	let raw = api.storage(&map_key("XYK", "ShareToken", acc), at.0).ok()??;
	AssetId::decode(&mut &raw[..]).ok()
}

fn xyk_total_liquidity<T: Transport>(api: &T, acc: &[u8; 32], at: H256) -> Option<u128> {
	match api.storage(&map_key("XYK", "TotalLiquidity", acc), at.0).ok()? {
		Some(raw) => Balance::decode(&mut &raw[..]).ok(),
		None => Some(0),
	}
}

// ---- account derivation ----

fn runtime_account(who: &[u8; 32]) -> primitives::AccountId {
	primitives::AccountId::from(*who)
}

/// The stableswap pool account: `blake2_256("sts" ++ pool_id_le)`.
fn pool_account(pool_id: AssetId) -> [u8; 32] {
	let mut name = b"sts".to_vec();
	name.extend_from_slice(&pool_id.to_le_bytes());
	blake2_256(&name)
}

/// The single omnipool account (`modl` ++ "omnipool" pallet id, zero-padded).
pub fn omnipool_account() -> [u8; 32] {
	let mut a = [0u8; 32];
	a[..4].copy_from_slice(b"modl");
	a[4..12].copy_from_slice(b"omnipool");
	a
}

/// Storage key of a `Blake2_128Concat` map entry.
fn map_key(pallet: &str, item: &str, key: &[u8]) -> Vec<u8> {
	let mut k = Vec::with_capacity(32 + 16 + key.len());
	k.extend_from_slice(&twox_128(pallet.as_bytes()));
	k.extend_from_slice(&twox_128(item.as_bytes()));
	k.extend_from_slice(&blake2_128(key));
	k.extend_from_slice(key);
	k
}
