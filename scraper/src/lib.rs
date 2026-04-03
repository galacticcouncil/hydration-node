#![allow(dead_code)]
#![allow(clippy::type_complexity)]

use codec::{Compact, Decode, Encode};
use frame_support::__private::log;
use frame_support::sp_runtime::{traits::Block as BlockT, StateVersion, Storage};
use futures::StreamExt;
use hydradx::chain_spec::hydradx::parachain_config;
use sc_chain_spec::ChainSpec;
use sp_core::storage::{StorageData, StorageKey};
use sp_core::H256;
use sp_io::TestExternalities;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::{
	fs,
	path::{Path, PathBuf},
	str::FromStr,
};
use substrate_rpc_client::ws_client;
use substrate_rpc_client::StateApi;

/// Compute the storage key prefix for a pallet name using twox_128 hash.
pub fn pallet_storage_prefix(pallet_name: &str) -> [u8; 16] {
	sp_io::hashing::twox_128(pallet_name.as_bytes())
}

pub fn save_blocks_snapshot<Block: Encode>(data: &Vec<Block>, path: &Path) -> Result<(), &'static str> {
	let mut path = path.to_path_buf();
	let encoded = data.encode();
	path.set_extension("blocks");
	fs::write(path, encoded).map_err(|_| "fs::write failed.")?;
	Ok(())
}

pub fn load_blocks_snapshot<Block: Decode>(path: &Path) -> Result<Vec<Block>, &'static str> {
	let mut path = path.to_path_buf();
	path.set_extension("blocks");
	let bytes = fs::read(path).map_err(|_| "fs::read failed.")?;
	Decode::decode(&mut &*bytes).map_err(|_| "decode failed")
}

pub fn hash_of<Block: BlockT>(hash_str: &str) -> Result<Block::Hash, &'static str>
where
	Block::Hash: FromStr,
	<Block::Hash as FromStr>::Err: std::fmt::Debug,
{
	hash_str
		.parse::<<Block as BlockT>::Hash>()
		.map_err(|_| "Could not parse block hash")
}

pub type SnapshotVersion = Compact<u16>;
pub const SNAPSHOT_VERSION: SnapshotVersion = Compact(4);

/// The snapshot that we store on disk.
/// Must match the format from `frame-remote-externalities`.
#[derive(Decode, Encode, Clone)]
pub struct Snapshot<B: BlockT> {
	snapshot_version: SnapshotVersion,
	state_version: StateVersion,
	// <Vec<Key, (Value, MemoryDbRefCount)>>
	raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
	storage_root: B::Hash,
	header: B::Header,
}

impl<B: BlockT> Snapshot<B> {
	pub fn new(
		state_version: StateVersion,
		raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
		storage_root: B::Hash,
		header: B::Header,
	) -> Self {
		Self {
			snapshot_version: SNAPSHOT_VERSION,
			state_version,
			raw_storage,
			storage_root,
			header,
		}
	}

	fn load(path: &PathBuf) -> Result<Snapshot<B>, &'static str> {
		let bytes = fs::read(path).map_err(|_| "fs::read failed.")?;
		// The first item in the SCALE encoded struct bytes is the snapshot version. We decode and
		// check that first, before proceeding to decode the rest of the snapshot.
		let snapshot_version =
			SnapshotVersion::decode(&mut &*bytes).map_err(|_| "Failed to decode snapshot version")?;

		if snapshot_version != SNAPSHOT_VERSION {
			return Err("Unsupported snapshot version detected. Please create a new snapshot.");
		}

		Decode::decode(&mut &*bytes).map_err(|_| "Decode failed")
	}
	fn load_from_bytes(bytes: Vec<u8>) -> Result<Snapshot<B>, &'static str> {
		// The first item in the SCALE encoded struct bytes is the snapshot version. We decode and
		// check that first, before proceeding to decode the rest of the snapshot.
		let snapshot_version =
			SnapshotVersion::decode(&mut &*bytes).map_err(|_| "Failed to decode snapshot version")?;

		if snapshot_version != SNAPSHOT_VERSION {
			return Err("Unsupported snapshot version detected. Please create a new snapshot.");
		}

		Decode::decode(&mut &*bytes).map_err(|_| "Decode failed")
	}
}

pub fn save_externalities<B: BlockT<Hash = H256>>(ext: TestExternalities, path: PathBuf) -> Result<(), &'static str> {
	let state_version = ext.state_version;
	let (raw_storage, storage_root) = ext.into_raw_snapshot();

	// Construct a minimal header for the snapshot format.
	// This is only used in tests; production snapshots use save_slim_snapshot which gets the real header.
	let header = B::Header::decode(&mut frame_support::sp_runtime::traits::TrailingZeroInput::new(&[0u8]))
		.expect("infinite input; qed");

	let snapshot = Snapshot::<B>::new(state_version, raw_storage, storage_root, header);

	let encoded = snapshot.encode();
	fs::write(path, encoded).map_err(|_| "fs::write failed")?;

	Ok(())
}

/// Filter a snapshot file by removing storage entries for excluded pallets.
/// This loads the snapshot, filters out keys matching excluded pallet prefixes,
/// and saves the filtered snapshot back to the same path.
pub fn filter_snapshot_by_excluded_pallets<B: BlockT<Hash = H256>>(
	path: &PathBuf,
	excluded_pallets: &[String],
) -> Result<(), &'static str> {
	if excluded_pallets.is_empty() {
		return Ok(());
	}

	// Compute prefixes for all excluded pallets
	let excluded_prefixes: Vec<[u8; 16]> = excluded_pallets
		.iter()
		.map(|name| pallet_storage_prefix(name))
		.collect();

	// Load the existing snapshot
	let snapshot = Snapshot::<B>::load(path)?;

	let original_count = snapshot.raw_storage.len();

	// Filter out keys that start with any excluded pallet prefix
	let filtered_storage: Vec<(Vec<u8>, (Vec<u8>, i32))> = snapshot
		.raw_storage
		.into_iter()
		.filter(|(key, _)| {
			// Keys in raw_storage have the format: prefix + hash
			// We need to check if the storage key (not the DB key) starts with excluded prefix
			// The actual storage key prefix is at the beginning of the key
			if key.len() >= 16 {
				!excluded_prefixes.iter().any(|prefix| key.starts_with(prefix))
			} else {
				true // Keep keys that are too short to match
			}
		})
		.collect();

	let filtered_count = filtered_storage.len();
	println!(
		"Filtered {} entries (removed {} entries for excluded pallets)",
		filtered_count,
		original_count - filtered_count
	);

	// Create new snapshot with filtered storage
	let filtered_snapshot = Snapshot::<B>::new(
		snapshot.state_version,
		filtered_storage,
		snapshot.storage_root,
		snapshot.header,
	);

	// Save the filtered snapshot
	let encoded = filtered_snapshot.encode();
	fs::write(path, encoded).map_err(|_| "fs::write failed for filtered snapshot")?;

	Ok(())
}

pub fn load_snapshot<B: BlockT<Hash = H256>>(path: PathBuf) -> Result<TestExternalities, &'static str> {
	let Snapshot {
		snapshot_version: _,
		header: _,
		state_version,
		raw_storage,
		storage_root,
	} = Snapshot::<B>::load(&path)?;

	let ext_from_snapshot = TestExternalities::from_raw_snapshot(raw_storage, storage_root, state_version);

	Ok(ext_from_snapshot)
}

pub fn load_snapshot_from_bytes<B: BlockT<Hash = H256>>(bytes: Vec<u8>) -> Result<TestExternalities, &'static str> {
	let Snapshot {
		snapshot_version: _,
		header: _,
		state_version,
		raw_storage,
		storage_root,
	} = Snapshot::<B>::load_from_bytes(bytes)?;

	let ext_from_snapshot = TestExternalities::from_raw_snapshot(raw_storage, storage_root, state_version);
	Ok(ext_from_snapshot)
}

pub fn get_snapshot_from_bytes<B: BlockT<Hash = H256>>(bytes: Vec<u8>) -> Result<Snapshot<B>, &'static str> {
	let s = Snapshot::<B>::load_from_bytes(bytes)?;
	Ok(s)
}

pub fn construct_backend_from_snapshot<B: BlockT<Hash = H256>>(
	snapshot: Snapshot<B>,
) -> Result<(sp_trie::PrefixedMemoryDB<sp_core::Blake2Hasher>, StateVersion, H256), &'static str> {
	let Snapshot {
		snapshot_version: _,
		header: _,
		state_version,
		raw_storage,
		storage_root,
	} = snapshot;
	let mut backend = PrefixedMemoryDB::default();

	for (key, (v, ref_count)) in raw_storage {
		let mut hash = H256::default();
		let hash_len = hash.as_ref().len();

		if key.len() < hash_len {
			log::warn!("Invalid key in `from_raw_snapshot`: {key:?}");
			continue;
		}

		hash.as_mut().copy_from_slice(&key[(key.len() - hash_len)..]);

		// Each time .emplace is called the internal MemoryDb ref count increments.
		// Repeatedly call emplace to initialise the ref count to the correct value.
		for _ in 0..ref_count {
			backend.emplace(hash, (&key[..(key.len() - hash_len)], None), v.clone());
		}
	}
	Ok((backend, state_version, storage_root))
}

pub fn create_externalities_with_backend<B: BlockT<Hash = H256>>(
	backend: sp_trie::PrefixedMemoryDB<sp_core::Blake2Hasher>,
	storage_root: H256,
	state_version: StateVersion,
) -> TestExternalities {
	TestExternalities {
		backend: TrieBackendBuilder::new(backend, storage_root).build(),
		state_version,
		..Default::default()
	}
}

pub fn create_externalities_from_snapshot<B: BlockT<Hash = H256>>(
	snapshot: &Snapshot<B>,
) -> Result<TestExternalities, &'static str> {
	let Snapshot {
		snapshot_version: _,
		header: _,
		state_version,
		raw_storage,
		storage_root,
	} = snapshot;
	let ext_from_snapshot = TestExternalities::from_raw_snapshot(raw_storage.to_vec(), *storage_root, *state_version);
	Ok(ext_from_snapshot)
}

pub fn extend_externalities<B: BlockT>(
	mut ext: TestExternalities,
	execute: impl FnOnce(),
) -> Result<TestExternalities, String> {
	ext.execute_with(execute);
	ext.commit_all()?;
	Ok(ext)
}

pub async fn save_chainspec(
	at: Option<H256>,
	path: PathBuf,
	uri: String,
	excluded_pallets: Vec<String>,
) -> Result<(), &'static str> {
	let rpc = ws_client(uri.clone())
		.await
		.map_err(|_| "Failed to create RPC client")?;

	let mut storage_map = BTreeMap::new();

	let code_key = sp_core::storage::well_known_keys::CODE;
	println!("Fetching WASM code with key: {}", hex::encode(code_key));

	let wasm_code = StateApi::<H256>::storage(&rpc, StorageKey(code_key.to_vec()), at)
		.await
		.map_err(|e| {
			println!("RPC error: {e:?}");
			"Failed to fetch WASM code from chain"
		})?
		.ok_or("WASM code not found in chain state")?;

	println!("Saving WASM code with key: {}", hex::encode(code_key));

	storage_map.insert(code_key.to_vec(), wasm_code.0);

	println!("Reading all storage key-value pairs remotely");
	let all_pairs = fetch_all_storage(uri, at, excluded_pallets)
		.await
		.map_err(|_| "Failed to fetch storage")
		.unwrap();

	for (k, v) in all_pairs {
		storage_map.insert(k.as_ref().to_vec(), v.0.to_vec());
	}
	let storage = Storage {
		top: storage_map,
		children_default: HashMap::new(),
	};

	let mut input_spec = parachain_config().unwrap();
	input_spec.set_storage(storage);

	println!("Generating new chain spec...");
	let json = sc_service::chain_ops::build_spec(&input_spec, true).unwrap();

	fs::write(path, json).map_err(|err| {
		println!("Failed to write chainspec to file {err:?}");
		"Failed to write chainspec file"
	})?;

	Ok(())
}
use futures::stream::{self};
use indicatif::{ProgressBar, ProgressStyle};
use sp_state_machine::TrieBackendBuilder;
use sp_trie::{HashDBT, PrefixedMemoryDB};

const PAGE_SIZE: u32 = 1000; //Limiting as bigger values lead to error when calling PROD RPCs
const CONCURRENCY: usize = 1000;

const ESTIMATED_TOTAL_KEYS: u64 = 350_000;

// Using the StateApi is the only easily working way to fetch all storage entries
// Loading the SNAPSHOT or getting raw storage entries don't help as the keys contain additional hashing data,
// so the keys are difficult to be cleaned up by trimming
// StateApi call performances needed to be improved by using concurrency
pub async fn fetch_all_storage(
	uri: String,
	at: Option<H256>,
	excluded_pallets: Vec<String>,
) -> Result<Vec<(StorageKey, StorageData)>, &'static str> {
	let rpc = Arc::new(ws_client(uri).await.map_err(|_| "Failed to create RPC client")?);

	// Compute prefixes for excluded pallets
	let excluded_prefixes: Vec<[u8; 16]> = excluded_pallets
		.iter()
		.map(|name| pallet_storage_prefix(name))
		.collect();

	if !excluded_pallets.is_empty() {
		println!("Excluding pallets: {excluded_pallets:?}");
	}

	let mut all_pairs = Vec::new();
	let mut start_key: Option<StorageKey> = None;

	let pb = ProgressBar::new(ESTIMATED_TOTAL_KEYS);
	pb.set_style(
		ProgressStyle::with_template("{spinner} [{elapsed_precise}] [{wide_bar}] {pos}/{len}(approx.) keys")
			.unwrap()
			.progress_chars("#>-"),
	);

	loop {
		let keys =
			StateApi::<H256>::storage_keys_paged(&*rpc, Some(StorageKey(vec![])), PAGE_SIZE, start_key.clone(), at)
				.await
				.map_err(|_| "Failed to get keys")?;

		if keys.is_empty() {
			break;
		}

		let forbidden_keys = [
			sp_core::storage::well_known_keys::HEAP_PAGES,
			sp_core::storage::well_known_keys::EXTRINSIC_INDEX,
			sp_core::storage::well_known_keys::INTRABLOCK_ENTROPY,
			sp_core::storage::well_known_keys::CHILD_STORAGE_KEY_PREFIX,
			sp_core::storage::well_known_keys::DEFAULT_CHILD_STORAGE_KEY_PREFIX,
		];

		let excluded_prefixes = excluded_prefixes.clone();
		let fetched: Vec<(StorageKey, StorageData)> = stream::iter(keys.clone())
			.map(|key| {
				let rpc = Arc::clone(&rpc);
				async move {
					match StateApi::<H256>::storage(&*rpc, key.clone(), at).await {
						Ok(Some(value)) => Some((key, value)),
						_ => None,
					}
				}
			})
			.buffer_unordered(CONCURRENCY)
			.inspect(|res| {
				if res.is_some() {
					pb.inc(1);
				}
			})
			.filter_map(futures::future::ready)
			.filter(|(key, _value)| {
				let raw_key = key.as_ref();
				futures::future::ready({
					// Check if key matches any excluded pallet prefix
					let is_excluded =
						raw_key.len() >= 16 && excluded_prefixes.iter().any(|prefix| raw_key.starts_with(prefix));

					!is_excluded
						&& (raw_key == sp_core::storage::well_known_keys::CODE
							|| !forbidden_keys.iter().any(|prefix| raw_key.starts_with(prefix)))
				})
			})
			.collect()
			.await;

		all_pairs.extend(fetched);

		start_key = keys.last().cloned();
	}

	pb.finish_with_message("✅ Done fetching all storage key-value pairs..");
	Ok(all_pairs)
}

mod slim {
	use super::*;
	use std::collections::{HashMap, HashSet};

	/// Compute `twox128(a) ++ twox128(b)` as a 32-byte storage prefix.
	fn storage_prefix(pallet: &str, item: &str) -> [u8; 32] {
		let mut prefix = [0u8; 32];
		prefix[..16].copy_from_slice(&sp_io::hashing::twox_128(pallet.as_bytes()));
		prefix[16..].copy_from_slice(&sp_io::hashing::twox_128(item.as_bytes()));
		prefix
	}

	/// Build a PalletId-derived account: `b"modl" + id_bytes + zero_padding` to 32 bytes.
	fn pallet_account(id: &[u8; 8]) -> [u8; 32] {
		let mut account = [0u8; 32];
		account[..4].copy_from_slice(b"modl");
		account[4..12].copy_from_slice(id);
		account
	}

	/// Build a truncated EVM account: `b"ETH\0" + h160 + 8_zero_bytes`.
	fn evm_truncated_account(h160: &[u8; 20]) -> [u8; 32] {
		let mut account = [0u8; 32];
		account[..4].copy_from_slice(b"ETH\0");
		account[4..24].copy_from_slice(h160);
		account
	}

	/// Read a little-endian u128 from a byte slice.
	fn read_u128_le(bytes: &[u8], offset: usize) -> u128 {
		let mut buf = [0u8; 16];
		buf.copy_from_slice(&bytes[offset..offset + 16]);
		u128::from_le_bytes(buf)
	}

	/// Write a little-endian u128 into a byte vec.
	fn u128_le_bytes(val: u128) -> Vec<u8> {
		val.to_le_bytes().to_vec()
	}

	/// Read a little-endian u32 from a byte slice.
	fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
		let mut buf = [0u8; 4];
		buf.copy_from_slice(&bytes[offset..offset + 4]);
		u32::from_le_bytes(buf)
	}

	/// Extract the last 32 bytes from a key as an account ID.
	fn account_from_key_tail(key: &[u8]) -> Option<[u8; 32]> {
		if key.len() < 32 {
			return None;
		}
		let mut account = [0u8; 32];
		account.copy_from_slice(&key[key.len() - 32..]);
		Some(account)
	}

	/// Build the set of all dev/test accounts.
	fn dev_accounts() -> Vec<[u8; 32]> {
		vec![
			// Alice
			hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"),
			// Bob
			hex_literal::hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"),
			// Charlie
			hex_literal::hex!("90b5ab205c6974c9ea841be688864633dc9ca8a6e38e35e40ef95fd7d98de856"),
			// Dave
			hex_literal::hex!("306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20"),
			// Eve
			hex_literal::hex!("e659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e"),
			// Ferdie
			hex_literal::hex!("1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c"),
		]
	}

	/// Build the set of hardcoded EVM accounts (truncated form).
	fn evm_dev_accounts() -> Vec<[u8; 32]> {
		vec![
			// EVM Alice 0x8202C0aF5962B750123CE1A9B12e1C30A4973557
			evm_truncated_account(&hex_literal::hex!("8202C0aF5962B750123CE1A9B12e1C30A4973557")),
			// EVM Bob 0x8aF7764663644989671A71Abe9738a3cF295f384
			evm_truncated_account(&hex_literal::hex!("8aF7764663644989671A71Abe9738a3cF295f384")),
			// EVM Charlie 0xC19A2970A13ac19898c47d59Cbd0278D428EBC7c
			evm_truncated_account(&hex_literal::hex!("C19A2970A13ac19898c47d59Cbd0278D428EBC7c")),
			// Contract deployer 0x222222ff7Be76052e023Ec1a306fCca8F9659D80
			evm_truncated_account(&hex_literal::hex!("222222ff7Be76052e023Ec1a306fCca8F9659D80")),
			// BorrowingTreasury 0xE52567fF06aCd6CBe7BA94dc777a3126e180B6d9
			evm_truncated_account(&hex_literal::hex!("E52567fF06aCd6CBe7BA94dc777a3126e180B6d9")),
		]
	}

	/// All PalletId-derived accounts.
	fn pallet_accounts() -> Vec<[u8; 32]> {
		[
			b"py/trsry", b"py/vstng", b"omnipool", b"stblpool", b"staking#", b"PotStake",
			b"OmniWhLM", b"Omni//LM", b"xykLMpID", b"XYK///LM", b"pltbonds", b"referral",
			b"gigahdx!", b"gigarwd!", b"feeproc/", b"py/hsmod", b"py/signt", b"py/fucet",
			b"curreser", b"xcm/alte", b"otcsettl", b"routerex", b"lqdation",
		]
		.iter()
		.map(|id| pallet_account(id))
		.collect()
	}

	/// Hardcoded special accounts (AaveManager, EmergencyAdmin).
	fn special_accounts() -> Vec<[u8; 32]> {
		vec![
			// AaveManagerAccount
			hex_literal::hex!("aa7e0000000000000000000000000000000aa7e0000000000000000000000000"),
			// EmergencyAdminAccount
			hex_literal::hex!("aa7e0000000000000000000000000000000aa7e1000000000000000000000000"),
		]
	}

	/// Prefixes for storage items that are per-account and should be filtered.
	struct FilterablePrefixes {
		system_account: [u8; 32],
		tokens_accounts: [u8; 32],
		balances_locks: [u8; 32],
		tokens_locks: [u8; 32],
		multi_tx_payment_account_currency: [u8; 32],
		vesting_schedules: [u8; 32],
	}

	impl FilterablePrefixes {
		fn new() -> Self {
			Self {
				system_account: storage_prefix("System", "Account"),
				tokens_accounts: storage_prefix("Tokens", "Accounts"),
				balances_locks: storage_prefix("Balances", "Locks"),
				tokens_locks: storage_prefix("Tokens", "Locks"),
				multi_tx_payment_account_currency: storage_prefix("MultiTransactionPayment", "AccountCurrencyMap"),
				vesting_schedules: storage_prefix("Vesting", "VestingSchedules"),
			}
		}

	}

	/// Prefixes for storage items from which we extract accounts for the allow-list.
	struct SourcePrefixes {
		xyk_pool_assets: [u8; 32],
		stableswap_pools: [u8; 32],
		lbp_pool_data: [u8; 32],
		evm_account_codes: [u8; 32],
		omni_wh_lm_global_farm: [u8; 32],
		xyk_wh_lm_global_farm: [u8; 32],
		dispatcher_aave_manager: [u8; 32],
		signet_admin: [u8; 32],
	}

	impl SourcePrefixes {
		fn new() -> Self {
			Self {
				xyk_pool_assets: storage_prefix("XYK", "PoolAssets"),
				stableswap_pools: storage_prefix("Stableswap", "Pools"),
				lbp_pool_data: storage_prefix("LBP", "PoolData"),
				evm_account_codes: storage_prefix("EVM", "AccountCodes"),
				omni_wh_lm_global_farm: storage_prefix("OmnipoolWarehouseLM", "GlobalFarm"),
				xyk_wh_lm_global_farm: storage_prefix("XYKWarehouseLM", "GlobalFarm"),
				dispatcher_aave_manager: storage_prefix("Dispatcher", "AaveManagerAccount"),
				signet_admin: storage_prefix("Signet", "Admin"),
			}
		}
	}

	/// Build the complete allow-list by scanning raw storage.
	pub fn build_allow_list(raw_storage: &[(Vec<u8>, (Vec<u8>, i32))]) -> HashSet<[u8; 32]> {
		let mut accounts = HashSet::new();
		let sources = SourcePrefixes::new();

		// Add hardcoded accounts
		for a in dev_accounts() {
			accounts.insert(a);
		}
		for a in evm_dev_accounts() {
			accounts.insert(a);
		}
		for a in pallet_accounts() {
			accounts.insert(a);
		}
		for a in special_accounts() {
			accounts.insert(a);
		}

		// Scan raw storage for dynamic accounts
		for (key, (value, _)) in raw_storage {
			if key.len() < 32 {
				continue;
			}

			let prefix: [u8; 32] = key[..32].try_into().unwrap();

			// XYK pool accounts: last 32 bytes of key = pool AccountId
			if prefix == sources.xyk_pool_assets {
				if let Some(account) = account_from_key_tail(key) {
					accounts.insert(account);
				}
			}
			// Stableswap pool accounts: extract pool_id, compute blake2_256("sts" + pool_id_le)
			else if prefix == sources.stableswap_pools {
				// Key layout: 32-byte prefix + blake2_128(asset_id)(16) + asset_id(4)
				// asset_id (pool_id) is at offset 48..52
				if key.len() >= 52 {
					let pool_id_bytes = &key[48..52];
					let mut hash_input = Vec::with_capacity(7);
					hash_input.extend_from_slice(b"sts");
					hash_input.extend_from_slice(pool_id_bytes);
					let hash = sp_io::hashing::blake2_256(&hash_input);
					accounts.insert(hash);
				}
			}
			// LBP pool accounts: last 32 bytes of key = pool AccountId
			// Also extract owner from value (first 32 bytes of value)
			else if prefix == sources.lbp_pool_data {
				if let Some(account) = account_from_key_tail(key) {
					accounts.insert(account);
				}
				// LBP Pool struct: owner(32 bytes) is the first field
				if value.len() >= 32 {
					let mut owner = [0u8; 32];
					owner.copy_from_slice(&value[..32]);
					accounts.insert(owner);
				}
			}
			// EVM contract accounts: extract H160, convert to truncated substrate account
			else if prefix == sources.evm_account_codes {
				// Key layout: 32-byte prefix + blake2_128(h160)(16) + h160(20)
				// H160 is at offset 48..68
				if key.len() >= 68 {
					let mut h160 = [0u8; 20];
					h160.copy_from_slice(&key[48..68]);
					accounts.insert(evm_truncated_account(&h160));
				}
			}
			// LM GlobalFarm owners: value layout = id(u32, 4 bytes) + owner(AccountId, 32 bytes)
			else if prefix == sources.omni_wh_lm_global_farm || prefix == sources.xyk_wh_lm_global_farm {
				if value.len() >= 36 {
					let mut owner = [0u8; 32];
					owner.copy_from_slice(&value[4..36]);
					accounts.insert(owner);
				}
			}
			// Dispatcher AaveManagerAccount: value is just AccountId (32 bytes)
			else if prefix == sources.dispatcher_aave_manager {
				if value.len() >= 32 {
					let mut account = [0u8; 32];
					account.copy_from_slice(&value[..32]);
					accounts.insert(account);
				}
			}
			// Signet Admin: value is Option<AccountId>, so 0x01 + AccountId(32) if Some
			else if prefix == sources.signet_admin && value.len() >= 33 && value[0] == 1 {
				let mut account = [0u8; 32];
				account.copy_from_slice(&value[1..33]);
				accounts.insert(account);
			}
		}

		// Catch-all: any account starting with b"modl" is a pallet account
		// (this is applied during filtering, not here)

		println!("Slim allow-list: {} accounts", accounts.len());
		accounts
	}

	/// Check if an account should be kept: either in the allow-list or starts with "modl".
	fn should_keep(account: &[u8; 32], allow_list: &HashSet<[u8; 32]>) -> bool {
		// Catch-all: any account starting with b"modl" is a pallet-derived account
		if account[..4] == *b"modl" {
			return true;
		}
		allow_list.contains(account)
	}

	/// Filter raw storage, removing per-account entries not in the allow-list,
	/// and recalculate total issuances.
	pub fn filter_and_fix_issuances(
		raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
		allow_list: &HashSet<[u8; 32]>,
		excluded_pallets: &[String],
	) -> Vec<(Vec<u8>, (Vec<u8>, i32))> {
		let prefixes = FilterablePrefixes::new();
		let balances_total_issuance_prefix = storage_prefix("Balances", "TotalIssuance");
		let tokens_total_issuance_prefix = storage_prefix("Tokens", "TotalIssuance");

		let excluded_pallet_prefixes: Vec<[u8; 16]> = excluded_pallets
			.iter()
			.map(|name| pallet_storage_prefix(name))
			.collect();

		// Accumulate issuances from kept accounts
		let mut native_issuance: u128 = 0;
		let mut token_issuances: HashMap<u32, u128> = HashMap::new();

		let original_count = raw_storage.len();
		let mut removed_counts: HashMap<&str, usize> = HashMap::new();

		let mut filtered: Vec<(Vec<u8>, (Vec<u8>, i32))> = Vec::with_capacity(raw_storage.len());

		for (key, value_and_ref) in raw_storage {
			// First check excluded pallets
			if key.len() >= 16
				&& excluded_pallet_prefixes
					.iter()
					.any(|prefix| key.starts_with(prefix))
			{
				continue;
			}

			if key.len() < 32 {
				filtered.push((key, value_and_ref));
				continue;
			}

			let prefix: [u8; 32] = key[..32].try_into().unwrap();

			// System.Account: extract account, filter, accumulate native issuance
			if prefix == prefixes.system_account {
				if let Some(account) = account_from_key_tail(&key) {
					if should_keep(&account, allow_list) {
						// AccountInfo: nonce(4) + consumers(4) + providers(4) + sufficients(4) + free(16) + reserved(16) + ...
						let val = &value_and_ref.0;
						if val.len() >= 48 {
							let free = read_u128_le(val, 16);
							let reserved = read_u128_le(val, 32);
							native_issuance = native_issuance.saturating_add(free).saturating_add(reserved);
						}
						filtered.push((key, value_and_ref));
					} else {
						*removed_counts.entry("System.Account").or_insert(0) += 1;
					}
				} else {
					filtered.push((key, value_and_ref));
				}
			}
			// Tokens.Accounts: extract account, filter, accumulate token issuance
			else if prefix == prefixes.tokens_accounts {
				if let Some(account) = account_from_key_tail(&key) {
					if should_keep(&account, allow_list) {
						// Extract currency_id: at offset 48..52 (after 32-byte prefix + 16-byte blake2_128 hash)
						if key.len() >= 52 {
							let currency_id = read_u32_le(&key, 48);
							// AccountData: free(16) + reserved(16) + frozen(16)
							let val = &value_and_ref.0;
							if val.len() >= 32 {
								let free = read_u128_le(val, 0);
								let reserved = read_u128_le(val, 16);
								*token_issuances.entry(currency_id).or_insert(0) = token_issuances
									.get(&currency_id)
									.unwrap_or(&0)
									.saturating_add(free)
									.saturating_add(reserved);
							}
						}
						filtered.push((key, value_and_ref));
					} else {
						*removed_counts.entry("Tokens.Accounts").or_insert(0) += 1;
					}
				} else {
					filtered.push((key, value_and_ref));
				}
			}
			// Other filterable per-account storage
			else if prefix == prefixes.balances_locks
				|| prefix == prefixes.tokens_locks
				|| prefix == prefixes.multi_tx_payment_account_currency
				|| prefix == prefixes.vesting_schedules
			{
				if let Some(account) = account_from_key_tail(&key) {
					if should_keep(&account, allow_list) {
						filtered.push((key, value_and_ref));
					} else {
						let label = if prefix == prefixes.balances_locks {
							"Balances.Locks"
						} else if prefix == prefixes.tokens_locks {
							"Tokens.Locks"
						} else if prefix == prefixes.multi_tx_payment_account_currency {
							"MultiTransactionPayment.AccountCurrencyMap"
						} else {
							"Vesting.VestingSchedules"
						};
						*removed_counts.entry(label).or_insert(0) += 1;
					}
				} else {
					filtered.push((key, value_and_ref));
				}
			}
			// Everything else: keep as-is
			else {
				filtered.push((key, value_and_ref));
			}
		}

		// Print removal stats
		let total_removed: usize = removed_counts.values().sum();
		println!(
			"Slim filter: kept {} entries, removed {} entries",
			filtered.len(),
			total_removed
		);
		for (label, count) in &removed_counts {
			println!("  Removed {count} entries from {label}");
		}

		// Recalculate Balances.TotalIssuance (native token)
		let balances_ti_key = balances_total_issuance_prefix.to_vec();
		let mut found_native_ti = false;
		for (key, value_and_ref) in &mut filtered {
			if key.len() == 32 && key[..32] == balances_ti_key[..] {
				let old_val = read_u128_le(&value_and_ref.0, 0);
				value_and_ref.0 = u128_le_bytes(native_issuance);
				println!("Recalculated Balances.TotalIssuance: {old_val} -> {native_issuance}");
				found_native_ti = true;
				break;
			}
		}
		if !found_native_ti && native_issuance > 0 {
			println!("Warning: Balances.TotalIssuance key not found, inserting with value {native_issuance}");
			filtered.push((balances_ti_key, (u128_le_bytes(native_issuance), 1)));
		}

		// Recalculate Tokens.TotalIssuance for each currency
		// Key layout: 32-byte prefix + twox64(currency_id)(8) + currency_id(4) = 44 bytes
		let ti_prefix = tokens_total_issuance_prefix;
		for (key, value_and_ref) in &mut filtered {
			if key.len() >= 44 && key[..32] == ti_prefix {
				// Extract currency_id: last 4 bytes of the key
				let currency_id = read_u32_le(key, key.len() - 4);
				if let Some(&new_issuance) = token_issuances.get(&currency_id) {
					let old_val = read_u128_le(&value_and_ref.0, 0);
					value_and_ref.0 = u128_le_bytes(new_issuance);
					if old_val != new_issuance {
						println!(
							"Recalculated Tokens.TotalIssuance[{currency_id}]: {old_val} -> {new_issuance}"
						);
					}
					// Remove from map so we know which ones were handled
					// (can't remove during iteration, handled by tracking)
				}
			}
		}

		println!(
			"Slim snapshot: {} entries (reduced from {})",
			filtered.len(),
			original_count
		);
		filtered
	}
}

/// Save a slim snapshot: filter out most user accounts, keep only protocol/pool/dev accounts.
/// Takes ownership of the externalities, extracts raw storage, filters in memory, writes once.
pub fn save_slim_snapshot<B: BlockT<Hash = H256>>(
	ext: sp_state_machine::TestExternalities<frame_support::sp_runtime::traits::HashingFor<B>>,
	header: B::Header,
	path: PathBuf,
	excluded_pallets: &[String],
) -> Result<(), &'static str> {
	let state_version = ext.state_version;
	let (raw_storage, storage_root) = ext.into_raw_snapshot();

	println!("Building slim allow-list from {} raw storage entries...", raw_storage.len());

	// Build the allow-list by scanning raw storage keys/values
	let allow_list = slim::build_allow_list(&raw_storage);

	// Filter entries and recalculate issuances
	let filtered_storage = slim::filter_and_fix_issuances(raw_storage, &allow_list, excluded_pallets);

	// Write the slim snapshot
	let snapshot = Snapshot::<B>::new(state_version, filtered_storage, storage_root, header);
	let encoded = snapshot.encode();
	fs::write(&path, encoded).map_err(|_| "fs::write failed for slim snapshot")?;

	println!("Slim snapshot saved to {path:?}");
	Ok(())
}

#[cfg(test)]
mod test {
	use super::*;

	pub const ALICE: [u8; 32] = [4u8; 32];
	pub const BOB: [u8; 32] = [5u8; 32];

	#[cfg(test)]
	/// used in tests to generate TestExternalities
	fn externalities_from_genesis() -> TestExternalities {
		use frame_support::sp_runtime::BuildStorage;

		let mut storage = frame_system::GenesisConfig::<hydradx_runtime::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_balances::GenesisConfig::<hydradx_runtime::Runtime> {
			balances: vec![(hydradx_runtime::AccountId::from(ALICE), 1_000_000_000_000_000)],
			dev_accounts: None,
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		TestExternalities::new(storage)
	}

	#[test]
	fn extend_externalities_should_work() {
		use frame_support::assert_ok;

		let ext = externalities_from_genesis();

		let mut modified_ext = extend_externalities::<hydradx_runtime::Block>(ext, || {
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				0
			);
			assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				1_000_000_000_000,
			));
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				1_000_000_000_000
			);
		})
		.unwrap();

		modified_ext.execute_with(|| {
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				1_000_000_000_000
			);
		});
	}

	#[test]
	fn save_and_load_externalities_should_work() {
		use frame_support::assert_ok;

		let ext = externalities_from_genesis();

		let modified_ext = extend_externalities::<hydradx_runtime::Block>(ext, || {
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				0
			);
			assert_ok!(hydradx_runtime::Balances::transfer_allow_death(
				hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
				BOB.into(),
				1_000_000_000_000,
			));
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				1_000_000_000_000
			);
		})
		.unwrap();

		let path = std::path::PathBuf::from("./SNAPSHOT");

		save_externalities::<hydradx_runtime::Block>(modified_ext, path.clone()).unwrap();

		let mut ext_from_snapshot = load_snapshot::<hydradx_runtime::Block>(path.clone()).unwrap();

		fs::remove_file(path).unwrap();

		ext_from_snapshot.execute_with(|| {
			assert_eq!(
				hydradx_runtime::Balances::free_balance(hydradx_runtime::AccountId::from(BOB)),
				1_000_000_000_000
			);
		});
	}
}
