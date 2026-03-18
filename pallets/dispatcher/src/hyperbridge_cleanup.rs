//! Background cleanup logic for stale ISMP storage entries.
//!
//! Cleans the following storage prefixes across multiple blocks via `on_idle`:
//! - `pallet_ismp::StateCommitments`
//! - `pallet_ismp::StateMachineUpdateTime`
//! - `ismp_parachain::RelayChainStateCommitments`

use codec::{Decode, Encode, MaxEncodedLen};
use sp_core::hashing::twox_128;

/// Stages of the ISMP storage cleanup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Encode, Decode, MaxEncodedLen, scale_info::TypeInfo)]
pub enum Stage {
	/// Cleaning pallet_ismp::StateCommitments
	StateCommitments,
	/// Cleaning pallet_ismp::StateMachineUpdateTime
	StateMachineUpdateTime,
	/// Cleaning ismp_parachain::RelayChainStateCommitments
	RelayChainStateCommitments,
}

impl Stage {
	/// Get the storage prefix for the current stage.
	pub fn storage_prefix(&self) -> [u8; 32] {
		match self {
			Stage::StateCommitments => make_prefix(b"Ismp", b"StateCommitments"),
			Stage::StateMachineUpdateTime => make_prefix(b"Ismp", b"StateMachineUpdateTime"),
			Stage::RelayChainStateCommitments => make_prefix(b"IsmpParachain", b"RelayChainStateCommitments"),
		}
	}

	/// Get the next stage, or `None` if this is the last stage.
	pub fn next(&self) -> Option<Self> {
		match self {
			Stage::StateCommitments => Some(Stage::StateMachineUpdateTime),
			Stage::StateMachineUpdateTime => Some(Stage::RelayChainStateCommitments),
			Stage::RelayChainStateCommitments => None,
		}
	}
}

fn make_prefix(pallet: &[u8], storage: &[u8]) -> [u8; 32] {
	let mut prefix = [0u8; 32];
	prefix[0..16].copy_from_slice(&twox_128(pallet));
	prefix[16..32].copy_from_slice(&twox_128(storage));
	prefix
}

/// Execute one cleanup step for `stage`, deleting at most `limit` keys.
///
/// Returns `(stage_done, keys_deleted)` where `stage_done` is `true` when no
/// keys remain under the stage's prefix.
pub fn do_cleanup_step(stage: Stage, limit: u32) -> (bool, u32) {
	let prefix = stage.storage_prefix();
	match sp_io::storage::clear_prefix(&prefix, Some(limit)) {
		sp_io::KillStorageResult::AllRemoved(n) => (true, n),
		sp_io::KillStorageResult::SomeRemaining(n) => (false, n),
	}
}
