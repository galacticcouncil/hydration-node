//! Decode one extrinsic's storage writes into core's [`StateDelta`]. Keys are named from the
//! runtime's own storage metadata, so a write outside the decode table is still reported by name.

use cerebro_core::model::StateDelta;
use cerebro_core::{AccountId, AssetId, Balance};
use codec::Decode;
use frame_support::traits::StorageInfoTrait;
use hydradx_runtime::{AllPalletsWithSystem, Runtime};
use std::collections::HashMap;
use std::sync::OnceLock;

/// One storage write: the value before and after it, `None` meaning absent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
	pub key: Vec<u8>,
	pub pre: Option<Vec<u8>>,
	pub post: Option<Vec<u8>>,
}

type SystemAccount =
	frame_system::AccountInfo<<Runtime as frame_system::Config>::Nonce, <Runtime as frame_system::Config>::AccountData>;

fn items() -> &'static HashMap<[u8; 32], String> {
	static ITEMS: OnceLock<HashMap<[u8; 32], String>> = OnceLock::new();
	// Some pallets size their items from runtime parameters, which read storage.
	ITEMS.get_or_init(|| {
		sp_io::TestExternalities::default()
			.execute_with(AllPalletsWithSystem::storage_info)
			.into_iter()
			.filter_map(|i| {
				let prefix: [u8; 32] = i.prefix.get(..32)?.try_into().ok()?;
				let name = format!(
					"{}.{}",
					String::from_utf8_lossy(&i.pallet_name),
					String::from_utf8_lossy(&i.storage_name)
				);
				Some((prefix, name))
			})
			.collect()
	})
}

const DECODED: [&str; 13] = [
	"System.Account",
	"Tokens.Accounts",
	"Tokens.TotalIssuance",
	"Balances.TotalIssuance",
	"Omnipool.Assets",
	"Stableswap.Pools",
	"Stableswap.PoolPegs",
	"XYK.TotalLiquidity",
	"XYK.PoolAssets",
	"LBP.PoolData",
	"HSM.Collaterals",
	"HSM.HollarAmountReceived",
	"EVM.AccountStorages",
];

/// Whether [`decode`] reads anything from `key`; the node resolves only these keys.
pub fn decoded(key: &[u8]) -> bool {
	static PREFIXES: OnceLock<Vec<[u8; 32]>> = OnceLock::new();
	let prefixes = PREFIXES.get_or_init(|| {
		DECODED
			.iter()
			.filter_map(|name| name.split_once('.'))
			.map(|(pallet, item)| {
				let mut p = [0u8; 32];
				p[..16].copy_from_slice(&sp_core::twox_128(pallet.as_bytes()));
				p[16..].copy_from_slice(&sp_core::twox_128(item.as_bytes()));
				p
			})
			.collect()
	});
	key.get(..32).is_some_and(|p| prefixes.iter().any(|d| d == p))
}

/// `Pallet.Item` of a storage key, or the key itself for well-known and unregistered keys.
pub fn item_name(key: &[u8]) -> String {
	key.get(..32)
		.and_then(|p| items().get(p))
		.cloned()
		.unwrap_or_else(|| match std::str::from_utf8(key) {
			Ok(s) if s.starts_with(':') => s.to_string(),
			_ => format!("0x{}", hex::encode(&key[..key.len().min(32)])),
		})
}

/// Map keys after the 32-byte prefix, read through their concat hashers.
fn account_at(rest: &[u8], offset: usize) -> Option<AccountId> {
	rest.get(offset..offset + 32)?.try_into().ok()
}

fn asset_at(rest: &[u8], offset: usize) -> Option<AssetId> {
	Some(u32::from_le_bytes(rest.get(offset..offset + 4)?.try_into().ok()?))
}

/// Both sides decoded with `f`; an absent side is `absent`. `None` if a present side fails.
fn both<T: Copy>(c: &Change, absent: T, f: impl Fn(&[u8]) -> Option<T>) -> Option<(T, T)> {
	let side = |v: &Option<Vec<u8>>| match v {
		Some(raw) => f(raw),
		None => Some(absent),
	};
	Some((side(&c.pre)?, side(&c.post)?))
}

fn native_balance(raw: &[u8]) -> Option<Balance> {
	let info = SystemAccount::decode(&mut &raw[..]).ok()?;
	Some(info.data.free.saturating_add(info.data.reserved))
}

fn token_balance(raw: &[u8]) -> Option<Balance> {
	let data = orml_tokens::AccountData::<Balance>::decode(&mut &raw[..]).ok()?;
	Some(data.free.saturating_add(data.reserved))
}

fn balance_value(raw: &[u8]) -> Option<Balance> {
	Balance::decode(&mut &raw[..]).ok()
}

pub fn decode(changes: &[Change]) -> StateDelta {
	let mut d = StateDelta::default();
	for c in changes {
		let name = item_name(&c.key);
		let rest = c.key.get(32..).unwrap_or_default();
		let decoded = match name.as_str() {
			"System.Account" => account_at(rest, 16)
				.zip(both(c, 0, native_balance))
				.map(|(who, v)| d.balances.insert((who, cerebro_core::NATIVE_ASSET), v))
				.map(drop),
			"Tokens.Accounts" => account_at(rest, 16)
				.zip(asset_at(rest, 56))
				.zip(both(c, 0, token_balance))
				.map(|((who, asset), v)| d.balances.insert((who, asset), v))
				.map(drop),
			"Tokens.TotalIssuance" => asset_at(rest, 8)
				.zip(both(c, 0, balance_value))
				.map(|(asset, v)| d.issuance.insert(asset, v))
				.map(drop),
			"Balances.TotalIssuance" => both(c, 0, balance_value)
				.map(|v| d.issuance.insert(cerebro_core::NATIVE_ASSET, v))
				.map(drop),
			"Omnipool.Assets" => asset_at(rest, 16).map(|a| d.omnipool_assets.insert(a)).map(drop),
			"Stableswap.Pools" | "Stableswap.PoolPegs" => {
				asset_at(rest, 16).map(|p| d.stable_pools.insert(p)).map(drop)
			}
			"XYK.TotalLiquidity" | "XYK.PoolAssets" => account_at(rest, 16).map(|a| d.xyk_pools.insert(a)).map(drop),
			"LBP.PoolData" | "HSM.Collaterals" | "HSM.HollarAmountReceived" => {
				d.unmodelled.insert(name.clone());
				Some(())
			}
			"EVM.AccountStorages" => rest
				.get(16..36)
				.and_then(|addr| addr.try_into().ok())
				.zip(rest.get(52..84).and_then(|s| s.try_into().ok()))
				.map(|slot| d.evm_slots.insert(slot))
				.map(drop),
			_ => {
				*d.other.entry(name.clone()).or_default() += 1;
				Some(())
			}
		};
		if decoded.is_none() {
			d.undecodable.insert(name);
		}
	}
	d
}

#[cfg(test)]
mod tests {
	use super::*;
	use codec::Encode;
	use std::collections::{BTreeMap, BTreeSet};

	const WHO: AccountId = [7; 32];

	fn tokens_key(who: AccountId, asset: AssetId) -> Vec<u8> {
		orml_tokens::Accounts::<Runtime>::hashed_key_for(primitives::AccountId::from(who), asset)
	}

	fn token(free: Balance, reserved: Balance) -> Vec<u8> {
		orml_tokens::AccountData::<Balance> {
			free,
			reserved,
			frozen: 0,
		}
		.encode()
	}

	#[test]
	fn decode_should_read_free_plus_reserved_when_token_account_changes() {
		let c = Change {
			key: tokens_key(WHO, 5),
			pre: Some(token(100, 10)),
			post: Some(token(40, 10)),
		};

		assert_eq!(decode(&[c]).balances, BTreeMap::from([((WHO, 5), (110, 50))]));
	}

	#[test]
	fn decode_should_read_zero_when_token_account_is_created() {
		let c = Change {
			key: tokens_key(WHO, 5),
			pre: None,
			post: Some(token(40, 0)),
		};

		assert_eq!(decode(&[c]).balances, BTreeMap::from([((WHO, 5), (0, 40))]));
	}

	#[test]
	fn decode_should_read_native_balance_when_system_account_changes() {
		let key = frame_system::Account::<Runtime>::hashed_key_for(primitives::AccountId::from(WHO));
		let info = |free: Balance| {
			let mut i = SystemAccount::default();
			i.data.free = free;
			i.data.reserved = 1;
			i.encode()
		};
		let c = Change {
			key,
			pre: Some(info(10)),
			post: Some(info(30)),
		};

		assert_eq!(decode(&[c]).balances, BTreeMap::from([((WHO, 0), (11, 31))]));
	}

	#[test]
	fn decode_should_read_issuance_when_token_total_issuance_changes() {
		let c = Change {
			key: orml_tokens::TotalIssuance::<Runtime>::hashed_key_for(102),
			pre: Some(1_000u128.encode()),
			post: Some(1_500u128.encode()),
		};

		assert_eq!(decode(&[c]).issuance, BTreeMap::from([(102, (1_000, 1_500))]));
	}

	#[test]
	fn decode_should_mark_stable_pool_when_pool_info_changes() {
		let c = Change {
			key: pallet_stableswap::Pools::<Runtime>::hashed_key_for(102),
			pre: None,
			post: Some(vec![0]),
		};

		assert_eq!(decode(&[c]).stable_pools, BTreeSet::from([102]));
	}

	#[test]
	fn decode_should_mark_omnipool_asset_when_asset_state_changes() {
		let c = Change {
			key: pallet_omnipool::Assets::<Runtime>::hashed_key_for(5),
			pre: Some(vec![1]),
			post: Some(vec![2]),
		};

		assert_eq!(decode(&[c]).omnipool_assets, BTreeSet::from([5]));
	}

	#[test]
	fn decode_should_read_contract_and_slot_when_evm_storage_changes() {
		let contract = sp_core::H160::repeat_byte(3);
		let slot = sp_core::H256::repeat_byte(9);
		let c = Change {
			key: pallet_evm::AccountStorages::<Runtime>::hashed_key_for(contract, slot),
			pre: None,
			post: Some(vec![1]),
		};

		assert_eq!(decode(&[c]).evm_slots, BTreeSet::from([(contract.0, slot.0)]));
	}

	#[test]
	fn decode_should_count_write_by_name_when_item_is_not_decoded() {
		let c = Change {
			key: [sp_core::twox_128(b"System"), sp_core::twox_128(b"Number")].concat(),
			pre: Some(1u32.encode()),
			post: Some(2u32.encode()),
		};

		assert_eq!(decode(&[c]).other, BTreeMap::from([("System.Number".to_string(), 1)]));
	}

	#[test]
	fn decoded_should_name_only_registered_items() {
		let registered: BTreeSet<&str> = items().values().map(String::as_str).collect();

		assert_eq!(DECODED.iter().filter(|n| !registered.contains(*n)).count(), 0);
	}

	#[test]
	fn decoded_should_accept_token_account_when_key_is_decoded_item() {
		assert!(decoded(&tokens_key(WHO, 5)));
	}

	#[test]
	fn decoded_should_reject_events_when_key_is_not_decoded_item() {
		assert!(!decoded(
			&[sp_core::twox_128(b"System"), sp_core::twox_128(b"Events")].concat()
		));
	}

	#[test]
	fn decode_should_report_item_when_value_does_not_decode() {
		let c = Change {
			key: tokens_key(WHO, 5),
			pre: Some(vec![1, 2]),
			post: Some(token(1, 0)),
		};

		let d = decode(&[c]);

		assert_eq!(d.undecodable, BTreeSet::from(["Tokens.Accounts".to_string()]));
		assert_eq!(d.balances, BTreeMap::new());
	}
}
