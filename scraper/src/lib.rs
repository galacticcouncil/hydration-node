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

/// Slim snapshot filtering.
///
/// We use `sp_io::storage::clear()` inside `execute_with()` instead of filtering raw trie entries
/// directly. The raw snapshot is a Merkle-Patricia trie — removing leaf entries from the raw bytes
/// leaves parent/branch nodes still referencing them, breaking the trie with "Database missing
/// expected key" errors. `sp_io::storage::clear()` goes through Substrate's storage layer which
/// properly updates the trie structure.
mod slim {
	use std::collections::HashSet;

	/// Compute `twox128(a) ++ twox128(b)` as a 32-byte storage prefix.
	pub fn storage_prefix(pallet: &str, item: &str) -> Vec<u8> {
		let mut prefix = Vec::with_capacity(32);
		prefix.extend_from_slice(&sp_io::hashing::twox_128(pallet.as_bytes()));
		prefix.extend_from_slice(&sp_io::hashing::twox_128(item.as_bytes()));
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

	fn read_u32_le(bytes: &[u8], offset: usize) -> u32 {
		let mut buf = [0u8; 4];
		buf.copy_from_slice(&bytes[offset..offset + 4]);
		u32::from_le_bytes(buf)
	}

	/// Extract account from end of key (for single-key maps like System.Account, XYK.PoolAssets).
	/// Key = prefix(32) + blake2_128_concat(account)(48). Account at last 32 bytes.
	fn account_from_key_tail(key: &[u8]) -> Option<[u8; 32]> {
		if key.len() < 32 {
			return None;
		}
		let mut account = [0u8; 32];
		account.copy_from_slice(&key[key.len() - 32..]);
		Some(account)
	}

	/// Extract account from a Blake2_128Concat first key position.
	/// Key = prefix(32) + blake2_128(account)(16) + account(32) + ...rest.
	/// Account is at offset 48..80.
	fn account_from_first_key(key: &[u8]) -> Option<[u8; 32]> {
		if key.len() < 80 {
			return None;
		}
		let mut account = [0u8; 32];
		account.copy_from_slice(&key[48..80]);
		Some(account)
	}

	/// Iterate all storage keys under a given prefix using sp_io.
	fn iter_keys_with_prefix(prefix: &[u8]) -> Vec<Vec<u8>> {
		let mut keys = Vec::new();
		let mut current = sp_io::storage::next_key(prefix);
		while let Some(key) = current {
			if !key.starts_with(prefix) {
				break;
			}
			keys.push(key.clone());
			current = sp_io::storage::next_key(&key);
		}
		keys
	}

	/// Build allow-list by reading storage via sp_io (called inside execute_with).
	pub fn build_allow_list() -> HashSet<[u8; 32]> {
		let mut accounts = HashSet::new();

		// Hardcoded dev accounts
		for a in [
			hex_literal::hex!("d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d"),
			hex_literal::hex!("8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"),
			hex_literal::hex!("90b5ab205c6974c9ea841be688864633dc9ca8a6e38e35e40ef95fd7d98de856"),
			hex_literal::hex!("306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20"),
			hex_literal::hex!("e659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e"),
			hex_literal::hex!("1cbd2d43530a44705ad088af313e18f80b53ef16b36177cd4b77b846f2a5f07c"),
			hex_literal::hex!("aa7e0000000000000000000000000000000aa7e0000000000000000000000000"),
			hex_literal::hex!("aa7e0000000000000000000000000000000aa7e1000000000000000000000000"),
		] {
			accounts.insert(a);
		}

		// EVM dev accounts
		for h160 in [
			hex_literal::hex!("8202C0aF5962B750123CE1A9B12e1C30A4973557"),
			hex_literal::hex!("8aF7764663644989671A71Abe9738a3cF295f384"),
			hex_literal::hex!("C19A2970A13ac19898c47d59Cbd0278D428EBC7c"),
			hex_literal::hex!("222222ff7Be76052e023Ec1a306fCca8F9659D80"),
			hex_literal::hex!("E52567fF06aCd6CBe7BA94dc777a3126e180B6d9"),
		] {
			accounts.insert(evm_truncated_account(&h160));
		}

		// PalletId accounts
		for id in [
			b"py/trsry", b"py/vstng", b"omnipool", b"stblpool", b"staking#", b"PotStake",
			b"OmniWhLM", b"Omni//LM", b"xykLMpID", b"XYK///LM", b"pltbonds", b"referral",
			b"gigahdx!", b"gigarwd!", b"feeproc/", b"py/hsmod", b"py/signt", b"py/fucet",
			b"curreser", b"xcm/alte", b"otcsettl", b"routerex", b"lqdation",
		] {
			accounts.insert(pallet_account(id));
		}

		// XYK pool accounts
		for key in iter_keys_with_prefix(&storage_prefix("XYK", "PoolAssets")) {
			if let Some(account) = account_from_key_tail(&key) {
				accounts.insert(account);
			}
		}

		// Stableswap pool accounts
		for key in iter_keys_with_prefix(&storage_prefix("Stableswap", "Pools")) {
			if key.len() >= 52 {
				let mut hash_input = Vec::with_capacity(7);
				hash_input.extend_from_slice(b"sts");
				hash_input.extend_from_slice(&key[48..52]);
				accounts.insert(sp_io::hashing::blake2_256(&hash_input));
			}
		}

		// LBP pool accounts + owners
		for key in iter_keys_with_prefix(&storage_prefix("LBP", "PoolData")) {
			if let Some(account) = account_from_key_tail(&key) {
				accounts.insert(account);
			}
			if let Some(value) = sp_io::storage::get(&key) {
				if value.len() >= 32 {
					let mut owner = [0u8; 32];
					owner.copy_from_slice(&value[..32]);
					accounts.insert(owner);
				}
			}
		}

		// EVM contract accounts
		for key in iter_keys_with_prefix(&storage_prefix("EVM", "AccountCodes")) {
			if key.len() >= 68 {
				let mut h160 = [0u8; 20];
				h160.copy_from_slice(&key[48..68]);
				accounts.insert(evm_truncated_account(&h160));
			}
		}

		// LM GlobalFarm owners
		for prefix in [
			storage_prefix("OmnipoolWarehouseLM", "GlobalFarm"),
			storage_prefix("XYKWarehouseLM", "GlobalFarm"),
		] {
			for key in iter_keys_with_prefix(&prefix) {
				if let Some(value) = sp_io::storage::get(&key) {
					if value.len() >= 36 {
						let mut owner = [0u8; 32];
						owner.copy_from_slice(&value[4..36]);
						accounts.insert(owner);
					}
				}
			}
		}

		// Dispatcher AaveManagerAccount
		if let Some(value) = sp_io::storage::get(&storage_prefix("Dispatcher", "AaveManagerAccount")) {
			if value.len() >= 32 {
				let mut account = [0u8; 32];
				account.copy_from_slice(&value[..32]);
				accounts.insert(account);
			}
		}

		// Signet Admin
		if let Some(value) = sp_io::storage::get(&storage_prefix("Signet", "Admin")) {
			if value.len() >= 33 && value[0] == 1 {
				let mut account = [0u8; 32];
				account.copy_from_slice(&value[1..33]);
				accounts.insert(account);
			}
		}

		println!("Slim allow-list: {} accounts", accounts.len());
		accounts
	}

	fn should_keep(account: &[u8; 32], allow_list: &HashSet<[u8; 32]>) -> bool {
		if account[..4] == *b"modl" {
			return true;
		}
		allow_list.contains(account)
	}

	/// Clear unwanted storage entries and recalculate issuances.
	/// Must be called inside execute_with().
	pub fn clear_unwanted_entries(allow_list: &HashSet<[u8; 32]>) {
		// (pallet, item, account_is_first_key)
		// account_is_first_key=true: double map where AccountId is the first key (offset 48..80)
		// account_is_first_key=false: single map where AccountId is the only key (last 32 bytes)
		let filterable: [(&str, &str, bool); 6] = [
			("System", "Account", false),
			("Tokens", "Accounts", true),      // DoubleMap<AccountId, CurrencyId>
			("Balances", "Locks", false),       // Map<AccountId>
			("Tokens", "Locks", true),          // DoubleMap<AccountId, CurrencyId>
			("MultiTransactionPayment", "AccountCurrencyMap", false),
			("Vesting", "VestingSchedules", false),
		];

		let mut native_issuance: u128 = 0;
		let mut token_issuances: std::collections::HashMap<u32, u128> = std::collections::HashMap::new();

		let system_account_prefix = storage_prefix("System", "Account");
		let tokens_accounts_prefix = storage_prefix("Tokens", "Accounts");

		for (pallet, item, account_is_first_key) in &filterable {
			let prefix = storage_prefix(pallet, item);
			let keys = iter_keys_with_prefix(&prefix);
			let total = keys.len();
			let mut removed = 0usize;

			for key in keys {
				let account = if *account_is_first_key {
					account_from_first_key(&key)
				} else {
					account_from_key_tail(&key)
				};

				if let Some(account) = account {
					if !should_keep(&account, allow_list) {
						sp_io::storage::clear(&key);
						removed += 1;
					} else if key.starts_with(&system_account_prefix) {
						if let Some(val) = sp_io::storage::get(&key) {
							if val.len() >= 48 {
								let free = read_u128_le(&val, 16);
								let reserved = read_u128_le(&val, 32);
								native_issuance = native_issuance.saturating_add(free).saturating_add(reserved);
							}
						}
					} else if key.starts_with(&tokens_accounts_prefix) {
						// Tokens.Accounts key: prefix(32) + blake2_128(account)(16) + account(32) + twox64(currency)(8) + currency(4) = 92
						// Currency is last 4 bytes
						if key.len() >= 92 {
							let currency_id = read_u32_le(&key, key.len() - 4);
							if let Some(val) = sp_io::storage::get(&key) {
								if val.len() >= 32 {
									let free = read_u128_le(&val, 0);
									let reserved = read_u128_le(&val, 16);
									*token_issuances.entry(currency_id).or_insert(0) += free.saturating_add(reserved);
								}
							}
						}
					}
				}
			}

			if removed > 0 {
				println!("  Removed {removed}/{total} entries from {pallet}.{item}");
			}
		}

		// Recalculate Balances.TotalIssuance
		let balances_ti_key = storage_prefix("Balances", "TotalIssuance");
		sp_io::storage::set(&balances_ti_key, &native_issuance.to_le_bytes());
		println!("Recalculated Balances.TotalIssuance: {native_issuance}");

		// Recalculate Tokens.TotalIssuance for each currency
		let ti_prefix = storage_prefix("Tokens", "TotalIssuance");
		for key in iter_keys_with_prefix(&ti_prefix) {
			if key.len() >= 44 {
				let currency_id = read_u32_le(&key, key.len() - 4);
				if let Some(&new_issuance) = token_issuances.get(&currency_id) {
					sp_io::storage::set(&key, &new_issuance.to_le_bytes());
				}
			}
		}
	}
}

/// Save a slim snapshot: filter out most user accounts, keep only protocol/pool/dev accounts.
/// Uses execute_with() + sp_io::storage to properly modify trie-backed storage.
pub fn save_slim_snapshot<B: BlockT<Hash = H256>>(
	mut ext: sp_state_machine::TestExternalities<frame_support::sp_runtime::traits::HashingFor<B>>,
	header: B::Header,
	path: PathBuf,
) -> Result<(), &'static str> {
	ext.execute_with(|| {
		println!("Building slim allow-list...");
		let allow_list = slim::build_allow_list();

		println!("Clearing unwanted storage entries...");
		slim::clear_unwanted_entries(&allow_list);
	});

	ext.commit_all().map_err(|_| "Failed to commit storage changes")?;

	let state_version = ext.state_version;
	let (raw_storage, storage_root) = ext.into_raw_snapshot();

	println!("Saving slim snapshot with {} trie entries...", raw_storage.len());
	let snapshot = Snapshot::<B>::new(state_version, raw_storage, storage_root, header);
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
