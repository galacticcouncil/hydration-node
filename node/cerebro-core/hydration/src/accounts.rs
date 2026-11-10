//! Enumerate on-chain pool accounts to fill core's `ProtocolAccounts` classification sets.
//! XYK pair and stableswap pool accounts are Blake2 hashes (not `modl`-prefixed), so they must be
//! read from the registries; everything `modl`-prefixed is classified structurally in core.

use crate::transport::Transport;
use cerebro_core::accounts::ProtocolAccounts;
use cerebro_core::{AccountId, AssetId};
use sp_core::hashing::{blake2_256, twox_128};
use sp_core::H256;
use std::collections::HashSet;

/// Build the protocol-account sets from chain state at `at`.
pub fn fetch<T: Transport>(api: &T, at: H256) -> Result<ProtocolAccounts, String> {
	let mut derived = HashSet::new();
	let mut stable_pools = Vec::new();
	let mut xyk_accounts = HashSet::new();

	// XYK pair accounts: from `ShareToken` keys (Blake2_128Concat, account = last 32 bytes).
	for key in api.storage_keys(&prefix("XYK", "ShareToken"), at.0)? {
		if key.len() >= 32 {
			let raw: [u8; 32] = key[key.len() - 32..].try_into().expect("checked len; qed");
			derived.insert(raw);
			xyk_accounts.insert(raw);
		}
	}

	// Stableswap pool accounts: from `Pools` keys → blake2_256("sts" ++ pool_id_le).
	for key in api.storage_keys(&prefix("Stableswap", "Pools"), at.0)? {
		if key.len() >= 4 {
			let idb: [u8; 4] = key[key.len() - 4..].try_into().expect("checked len; qed");
			let pool_id = u32::from_le_bytes(idb);
			let acc = stableswap_pool_account(pool_id);
			derived.insert(acc);
			stable_pools.push((pool_id, acc));
		}
	}

	let omnipool_account = crate::snapshot::omnipool_account();
	derived.insert(omnipool_account);

	Ok(ProtocolAccounts {
		derived,
		stable_pools,
		xyk_accounts,
		omnipool_account,
	})
}

fn stableswap_pool_account(pool_id: AssetId) -> AccountId {
	let mut name = b"sts".to_vec();
	name.extend_from_slice(&pool_id.to_le_bytes());
	blake2_256(&name)
}

fn prefix(pallet: &str, item: &str) -> Vec<u8> {
	let mut k = Vec::with_capacity(32);
	k.extend_from_slice(&twox_128(pallet.as_bytes()));
	k.extend_from_slice(&twox_128(item.as_bytes()));
	k
}
