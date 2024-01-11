#![allow(dead_code)]

use codec::{Decode, Encode};
use frame_remote_externalities::TestExternalities;
use frame_support::sp_runtime::{traits::Block as BlockT, StateVersion};
use sp_core::storage::{
	well_known_keys::is_default_child_storage_key, ChildInfo, ChildType, PrefixedStorageKey, StorageData, StorageKey,
};
use sp_state_machine::Backend;
use std::{
	fs,
	path::{Path, PathBuf},
	str::FromStr,
};

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

type KeyValue = (StorageKey, StorageData);
type TopKeyValues = Vec<KeyValue>;
type ChildKeyValues = Vec<(ChildInfo, Vec<KeyValue>)>;

fn load_top_keys(ext: &TestExternalities) -> TopKeyValues {
	let pairs = ext
		.backend
		.pairs()
		.iter()
		.map(|e| (StorageKey(e.clone().0), StorageData(e.clone().1)))
		.collect();
	pairs
}

fn load_child_keys(ext: &TestExternalities, top_kv: &[KeyValue]) -> Result<ChildKeyValues, &'static str> {
	let child_roots = top_kv
		.iter()
		.filter_map(|(k, _)| is_default_child_storage_key(k.as_ref()).then(|| k.clone()))
		.collect::<Vec<_>>();

	let mut child_kv = vec![];

	for prefixed_top_key in child_roots {
		let storage_key = PrefixedStorageKey::new(prefixed_top_key.as_ref().to_vec());
		let child_info = match ChildType::from_prefixed_key(&storage_key) {
			Some((ChildType::ParentKeyId, storage_key)) => ChildInfo::new_default(storage_key),
			None => return Err("load_child_keys failed."),
		};
		let child_keys: Vec<StorageKey> = ext
			.backend
			.child_keys(&child_info, &StorageKey(vec![]).0)
			.into_iter()
			.map(StorageKey)
			.collect();

		let mut child_kv_inner = vec![];
		for key in child_keys {
			let child_info = match ChildType::from_prefixed_key(&storage_key) {
				Some((ChildType::ParentKeyId, storage_key)) => ChildInfo::new_default(storage_key),
				None => return Err("load_child_keys failed."),
			};
			let value = match ext.backend.child_storage(&child_info, &key.0) {
				Ok(Some(value)) => value,
				_ => return Err("load_child_keys failed."),
			};
			child_kv_inner.push((key, StorageData(value)));
		}

		let prefixed_top_key = PrefixedStorageKey::new(prefixed_top_key.clone().0);
		let un_prefixed = match ChildType::from_prefixed_key(&prefixed_top_key) {
			Some((ChildType::ParentKeyId, storage_key)) => storage_key,
			None => return Err("load_child_keys failed."),
		};

		child_kv.push((ChildInfo::new_default(un_prefixed), child_kv_inner));
	}

	Ok(child_kv)
}

pub fn extend_externalities(mut ext: TestExternalities, execute: impl FnOnce()) -> Result<TestExternalities, String> {
	ext.execute_with(execute);
	ext.commit_all()?;
	Ok(ext)
}

/// The snapshot that we store on disk.
#[derive(Decode, Encode)]
struct Snapshot<B: BlockT> {
	state_version: StateVersion,
	block_hash: B::Hash,
	top: TopKeyValues,
	child: ChildKeyValues,
}

pub fn save_externalities<Block: BlockT>(ext: TestExternalities, path: PathBuf) -> Result<(), &'static str> {
	let top_kv = load_top_keys(&ext);
	let child_kv = load_child_keys(&ext, &top_kv)?;

	let snapshot = Snapshot::<Block> {
		state_version: ext.state_version,
		block_hash: Block::Hash::default(),
		top: top_kv,
		child: child_kv,
	};
	let encoded = snapshot.encode();

	fs::write(path, encoded).map_err(|_| "fs::write failed")?;
	Ok(())
}

pub fn load_snapshot<Block: BlockT>(path: PathBuf) -> Result<TestExternalities, &'static str> {
	let bytes = fs::read(path).map_err(|_| "fs::read failed.")?;
	let snapshot: Snapshot<Block> = Decode::decode(&mut &*bytes).map_err(|_| "decode failed")?;

	let mut ext_from_snapshot =
		TestExternalities::new_with_code_and_state(Default::default(), Default::default(), Default::default());

	for (k, v) in snapshot.top {
		// skip writing the child root data.
		if is_default_child_storage_key(k.as_ref()) {
			continue;
		}
		ext_from_snapshot.insert(k.0, v.0);
	}

	for (info, key_values) in snapshot.child {
		for (k, v) in key_values {
			ext_from_snapshot.insert_child(info.clone(), k.0, v.0);
		}
	}

	Ok(ext_from_snapshot)
}

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];

#[cfg(test)]
/// used in tests to generate TestExternalities
fn externalities_from_genesis() -> TestExternalities {
	let mut storage = frame_system::GenesisConfig::default()
		.build_storage::<hydradx_runtime::Runtime>()
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

	let mut modified_ext = extend_externalities(ext, || {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
			0
		);
		assert_ok!(hydradx_runtime::Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			1_000_000_000_000,
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
			1_000_000_000_000
		);
	})
	.unwrap();

	modified_ext.execute_with(|| {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
			1_000_000_000_000
		);
	});
}

#[test]
fn save_and_load_externalities_should_work() {
	use frame_support::assert_ok;

	let ext = externalities_from_genesis();

	let modified_ext = extend_externalities(ext, || {
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
			0
		);
		assert_ok!(hydradx_runtime::Balances::transfer(
			hydradx_runtime::RuntimeOrigin::signed(ALICE.into()),
			BOB.into(),
			1_000_000_000_000,
		));
		assert_eq!(
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
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
			hydradx_runtime::Balances::free_balance(&hydradx_runtime::AccountId::from(BOB)),
			1_000_000_000_000
		);
	});
}
