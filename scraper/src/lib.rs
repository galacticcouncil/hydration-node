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
pub const SNAPSHOT_VERSION: SnapshotVersion = Compact(3);

/// The snapshot that we store on disk.
#[derive(Decode, Encode, Clone)]
pub struct Snapshot<B: BlockT> {
	snapshot_version: SnapshotVersion,
	state_version: StateVersion,
	block_hash: B::Hash,
	// <Vec<Key, (Value, MemoryDbRefCount)>>
	raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
	storage_root: B::Hash,
}

impl<B: BlockT> Snapshot<B> {
	pub fn new(
		state_version: StateVersion,
		block_hash: B::Hash,
		raw_storage: Vec<(Vec<u8>, (Vec<u8>, i32))>,
		storage_root: B::Hash,
	) -> Self {
		Self {
			snapshot_version: SNAPSHOT_VERSION,
			state_version,
			block_hash,
			raw_storage,
			storage_root,
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

	let snapshot = Snapshot::<B>::new(state_version, B::Hash::default(), raw_storage, storage_root);

	let encoded = snapshot.encode();
	fs::write(path, encoded).map_err(|_| "fs::write failed")?;

	Ok(())
}

pub fn load_snapshot<B: BlockT<Hash = H256>>(path: PathBuf) -> Result<TestExternalities, &'static str> {
	let Snapshot {
		snapshot_version: _,
		block_hash: _,
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
		block_hash: _,
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
		block_hash: _,
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
		block_hash: _,
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

pub async fn save_chainspec(at: Option<H256>, path: PathBuf, uri: String) -> Result<(), &'static str> {
	let rpc = ws_client(uri.clone())
		.await
		.map_err(|_| "Failed to create RPC client")?;

	let mut storage_map = BTreeMap::new();

	let code_key = sp_core::storage::well_known_keys::CODE;
	println!("Fetching WASM code with key: {}", hex::encode(code_key));

	let wasm_code = StateApi::<H256>::storage(&rpc, StorageKey(code_key.to_vec()), at)
		.await
		.map_err(|e| {
			println!("RPC error: {:?}", e);
			"Failed to fetch WASM code from chain"
		})?
		.ok_or("WASM code not found in chain state")?;

	println!("Saving WASM code with key: {}", hex::encode(code_key));

	storage_map.insert(code_key.to_vec(), wasm_code.0);

	println!("Reading all storage key-value pairs remotely");
	let all_pairs = fetch_all_storage(uri, at)
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
		println!("Failed to write chainspec to file {:?}", err);
		"Failed to write chainspec file"
	})?;

	Ok(())
}
use futures::stream::{self};
use indicatif::{ProgressBar, ProgressStyle};
use sp_state_machine::{TrieBackend, TrieBackendBuilder};
use sp_trie::{HashDBT, PrefixedMemoryDB};

const PAGE_SIZE: u32 = 1000; //Limiting as bigger values lead to error when calling PROD RPCs
const CONCURRENCY: usize = 1000;

const ESTIMATED_TOTAL_KEYS: u64 = 350_000;

// Using the StateApi is the only easily working way to fetch all storage entries
// Loading the SNAPSHOT or getting raw storage entries don't help as the keys contain additional hashing data,
// so the keys are difficult to be cleaned up by trimming
// StateApi call performances needed to be improved by using concurrency
pub async fn fetch_all_storage(uri: String, at: Option<H256>) -> Result<Vec<(StorageKey, StorageData)>, &'static str> {
	let rpc = Arc::new(ws_client(uri).await.map_err(|_| "Failed to create RPC client")?);

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
					raw_key == sp_core::storage::well_known_keys::CODE
						|| !forbidden_keys.iter().any(|prefix| raw_key.starts_with(prefix))
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
