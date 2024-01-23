#![allow(dead_code)]
#![allow(clippy::type_complexity)]

use codec::{Compact, Decode, Encode};
use frame_support::sp_runtime::{
	traits::{Block as BlockT, HashingFor},
	StateVersion,
};
use sp_state_machine::TestExternalities;
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

pub type SnapshotVersion = Compact<u16>;
pub const SNAPSHOT_VERSION: SnapshotVersion = Compact(3);

/// The snapshot that we store on disk.
#[derive(Decode, Encode)]
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
}

pub fn save_externalities<B: BlockT>(ext: TestExternalities<HashingFor<B>>, path: PathBuf) -> Result<(), &'static str> {
	let state_version = ext.state_version;
	let (raw_storage, storage_root) = ext.into_raw_snapshot();

	let snapshot = Snapshot::<B>::new(state_version, B::Hash::default(), raw_storage, storage_root);

	let encoded = snapshot.encode();
	fs::write(path, encoded).map_err(|_| "fs::write failed")?;

	Ok(())
}

pub fn load_snapshot<B: BlockT>(path: PathBuf) -> Result<TestExternalities<HashingFor<B>>, &'static str> {
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

pub fn extend_externalities<B: BlockT>(
	mut ext: TestExternalities<HashingFor<B>>,
	execute: impl FnOnce(),
) -> Result<TestExternalities<HashingFor<B>>, String> {
	ext.execute_with(execute);
	ext.commit_all()?;
	Ok(ext)
}

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];

#[cfg(test)]
/// used in tests to generate TestExternalities
fn externalities_from_genesis() -> TestExternalities<HashingFor<hydradx_runtime::Block>> {
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
		assert_ok!(hydradx_runtime::Balances::transfer(
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
		assert_ok!(hydradx_runtime::Balances::transfer(
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
