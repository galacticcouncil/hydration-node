//! Classification of accounts into economic actors vs protocol plumbing.
//!
//! The mirror side of every trade is a pool/protocol account and must never count as a
//! profiter. `modl`-prefixed (PalletId-derived) accounts are excluded structurally; XYK pair
//! and stableswap pool accounts are Blake2 hashes, so their sets are enumerated on-chain by the
//! mapper and handed in here as plain data. Core owns the *classification*; the chain owns the
//! *enumeration*.

use crate::{AccountId, AssetId, EvmAddress};
use std::collections::HashSet;

/// An account's EVM identity for matching against EVM-log actors (Aave legs). An `ETH\0`-embedded
/// account carries its H160 at bytes 4..24; a native account maps to its first 20 bytes (the
/// truncated mapping). Explicit evm-accounts bindings are resolved by the mapper, not here.
pub fn evm_address(account: &AccountId) -> EvmAddress {
	let mut h = [0u8; 20];
	if account.starts_with(b"ETH\0") {
		h.copy_from_slice(&account[4..24]);
	} else {
		h.copy_from_slice(&account[..20]);
	}
	h
}

/// A flow-graph entity node — the classification result for an account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entity {
	pub key: String,
	pub label: String,
	pub kind: &'static str, // "pool" | "protocol" | "user"
}

impl Entity {
	pub fn new(key: impl Into<String>, label: impl Into<String>, kind: &'static str) -> Self {
		Entity {
			key: key.into(),
			label: label.into(),
			kind,
		}
	}
}

/// Known protocol accounts whose deltas are structural, not profit. Built by the mapper
/// (`ProtocolAccounts::fetch` on the chain side) from on-chain registries.
#[derive(Clone, Debug, Default)]
pub struct ProtocolAccounts {
	pub derived: HashSet<AccountId>,
	/// `(pool_id, pool_account)` for every stableswap pool — leak scoring needs the id↔account map.
	pub stable_pools: Vec<(AssetId, AccountId)>,
	pub xyk_accounts: HashSet<AccountId>,
	pub omnipool_account: AccountId,
}

impl ProtocolAccounts {
	/// `PalletId`-derived accounts are `b"modl" ++ pallet_id ++ …`; catches Omnipool, treasury,
	/// staking, DCA, OTC, router, fee-processor, referrals, … structurally.
	pub fn is_pallet_account(acc: &AccountId) -> bool {
		acc.starts_with(b"modl")
	}

	pub fn contains(&self, acc: &AccountId) -> bool {
		Self::is_pallet_account(acc) || self.derived.contains(acc)
	}

	pub fn stable_pools(&self) -> &[(AssetId, AccountId)] {
		&self.stable_pools
	}

	/// The pool account behind a stableswap SHARE asset (share asset id == pool id).
	pub fn stable_pool_account_of(&self, asset: AssetId) -> Option<&AccountId> {
		self.stable_pools.iter().find(|(id, _)| *id == asset).map(|(_, a)| a)
	}

	pub fn is_xyk(&self, acc: &AccountId) -> bool {
		self.xyk_accounts.contains(acc)
	}

	pub fn omnipool_account(&self) -> &AccountId {
		&self.omnipool_account
	}

	/// Resolve an account to a flow-graph entity. EVM-derived (`b"ETH\0" ++ h160 ++ …`) accounts
	/// are the Aave / strategy-token money-market contracts trades route through; a real EVM *user*
	/// who signs is re-labelled elsewhere (the flow layer, which knows the signer).
	pub fn entity_of(&self, acc: &AccountId) -> Entity {
		if acc == &self.omnipool_account {
			return Entity::new("omnipool", "Omnipool", "pool");
		}
		if let Some((id, _)) = self.stable_pools.iter().find(|(_, a)| a == acc) {
			return Entity::new(format!("ss:{id}"), format!("Stableswap {id}"), "pool");
		}
		if self.xyk_accounts.contains(acc) {
			return Entity::new("xyk", "XYK", "pool");
		}
		if acc.starts_with(b"modl") {
			return name_pallet(&acc[4..12]);
		}
		if acc.starts_with(b"ETH\0") {
			return Entity::new("moneymarket", "Money Market", "pool");
		}
		Entity::new("users", "Users", "user")
	}
}

/// Map an 8-byte PalletId to a named entity. Trailing NULs are trimmed for the key.
fn name_pallet(pid: &[u8]) -> Entity {
	let trimmed: Vec<u8> = pid.iter().copied().take_while(|&b| b != 0).collect();
	let name = String::from_utf8_lossy(&trimmed).to_string();
	let (key, label): (&str, &str) = match name.as_str() {
		"omnipool" => ("omnipool", "Omnipool"),
		"py/trsry" => ("treasury", "Treasury"),
		"staking#" => ("staking", "Staking"),
		"dca_____" => ("dca", "DCA"),
		"otc_____" => ("otc", "OTC"),
		"router__" => ("router", "Router"),
		"gigahdx#" => ("gigahdx", "GigaHDX"),
		"gigarwd!" => ("gigarwd", "Giga Rewards"),
		"feeproc#" => ("feeproc", "Fee Processor"),
		_ => return Entity::new(format!("modl:{name}"), name.clone(), "protocol"),
	};
	Entity::new(key, label, "protocol")
}

#[cfg(test)]
mod tests {
	use super::*;

	fn modl(pallet: &[u8]) -> AccountId {
		let mut a = [0u8; 32];
		a[..4].copy_from_slice(b"modl");
		a[4..4 + pallet.len()].copy_from_slice(pallet);
		a
	}

	#[test]
	fn contains_should_exclude_all_pallet_accounts_when_modl_prefixed() {
		let pa = ProtocolAccounts::default();
		assert!(pa.contains(&modl(b"omnipool")));
		assert!(pa.contains(&modl(b"py/trsry")));
		assert!(!pa.contains(&[7u8; 32]));
	}

	#[test]
	fn entity_of_should_label_evm_derived_as_money_market_when_eth_prefixed() {
		let pa = ProtocolAccounts::default();
		let mut acc = [0u8; 32];
		acc[..4].copy_from_slice(b"ETH\0");
		assert_eq!(pa.entity_of(&acc).kind, "pool");
		assert_eq!(pa.entity_of(&acc).key, "moneymarket");
	}

	#[test]
	fn entity_of_should_return_users_when_plain_account() {
		let pa = ProtocolAccounts::default();
		let e = pa.entity_of(&[9u8; 32]);
		assert_eq!(e.kind, "user");
		assert_eq!(e.key, "users");
	}

	#[test]
	fn stable_pool_account_of_should_resolve_share_asset_to_pool_account() {
		let pa = ProtocolAccounts {
			stable_pools: vec![(222, [1u8; 32])],
			..Default::default()
		};
		assert_eq!(pa.stable_pool_account_of(222), Some(&[1u8; 32]));
		assert_eq!(pa.stable_pool_account_of(999), None);
	}
}
