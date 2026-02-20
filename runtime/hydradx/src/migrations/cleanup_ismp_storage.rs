//! Multi-block migration to clean up ISMP storage maps.
//!
//! This migration removes all entries from:
//! - pallet_ismp::StateCommitments
//! - pallet_ismp::StateMachineUpdateTime
//! - ismp_parachain::RelayChainStateCommitments

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{storage::unhashed, weights::WeightMeter};
use sp_core::Get;
use sp_io::hashing::twox_128;

#[cfg(feature = "try-runtime")]
use alloc::vec::Vec;

/// Maximum number of keys to delete per step execution.
const MAX_KEYS_PER_STEP: u32 = 1000;

/// Stages of the cleanup migration.
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
	fn storage_prefix(&self) -> [u8; 32] {
		match self {
			Stage::StateCommitments => {
				let pallet = twox_128(b"Ismp");
				let storage = twox_128(b"StateCommitments");
				let mut prefix = [0u8; 32];
				prefix[0..16].copy_from_slice(&pallet);
				prefix[16..32].copy_from_slice(&storage);
				prefix
			}
			Stage::StateMachineUpdateTime => {
				let pallet = twox_128(b"Ismp");
				let storage = twox_128(b"StateMachineUpdateTime");
				let mut prefix = [0u8; 32];
				prefix[0..16].copy_from_slice(&pallet);
				prefix[16..32].copy_from_slice(&storage);
				prefix
			}
			Stage::RelayChainStateCommitments => {
				let pallet = twox_128(b"IsmpParachain");
				let storage = twox_128(b"RelayChainStateCommitments");
				let mut prefix = [0u8; 32];
				prefix[0..16].copy_from_slice(&pallet);
				prefix[16..32].copy_from_slice(&storage);
				prefix
			}
		}
	}

	/// Get the next stage, or None if this is the last stage.
	fn next(&self) -> Option<Self> {
		match self {
			Stage::StateCommitments => Some(Stage::StateMachineUpdateTime),
			Stage::StateMachineUpdateTime => Some(Stage::RelayChainStateCommitments),
			Stage::RelayChainStateCommitments => None,
		}
	}
}

/// Cursor for tracking migration progress.
#[derive(Debug, Clone, PartialEq, Eq, Encode, Decode, MaxEncodedLen, scale_info::TypeInfo)]
pub struct Cursor {
	/// Current stage of the migration.
	stage: Stage,
}

/// Multi-block migration to clean up ISMP storage.
pub struct CleanupIsmpStorage<T>(sp_std::marker::PhantomData<T>);

impl<T: frame_system::Config> frame_support::migrations::SteppedMigration for CleanupIsmpStorage<T> {
	type Cursor = Cursor;
	type Identifier = [u8; 32];

	fn id() -> Self::Identifier {
		*b"cleanup_ismp_storage_v1_2026____"
	}

	fn max_steps() -> Option<u32> {
		// Allow a reasonable number of steps. With MAX_KEYS_PER_STEP=1000,
		// this allows cleaning up to 10M keys total across all three maps.
		Some(10_000)
	}

	fn step(
		cursor: Option<Self::Cursor>,
		meter: &mut WeightMeter,
	) -> Result<Option<Self::Cursor>, frame_support::migrations::SteppedMigrationError> {
		// Start with the first stage if no cursor is provided.
		let cursor = cursor.unwrap_or(Cursor {
			stage: Stage::StateCommitments,
		});

		let stage = cursor.stage;
		let prefix = stage.storage_prefix();

		// Estimate weight for reading and deleting keys.
		let weight_per_key = T::DbWeight::get().reads_writes(1, 1);
		let max_keys = meter
			.remaining()
			.checked_div_per_component(&weight_per_key)
			.unwrap_or(0)
			.min(MAX_KEYS_PER_STEP as u64) as u32;

		if max_keys == 0 {
			return Err(frame_support::migrations::SteppedMigrationError::InsufficientWeight {
				required: weight_per_key,
			});
		}

		// Collect keys to delete (bounded by max_keys).
		let mut keys_to_delete = alloc::vec::Vec::new();
		let mut keys_remaining = false;

		// Iterate over keys with the given prefix.
		let mut iter = sp_io::storage::next_key(&prefix);
		while let Some(key) = iter {
			// Check if the key still has our prefix.
			if !key.starts_with(&prefix) {
				break;
			}

			keys_to_delete.push(key.clone());

			if keys_to_delete.len() >= max_keys as usize {
				// Check if there are more keys.
				if let Some(next) = sp_io::storage::next_key(&key) {
					if next.starts_with(&prefix) {
						keys_remaining = true;
					}
				}
				break;
			}

			iter = sp_io::storage::next_key(&key);
		}

		// Delete the collected keys.
		let deleted_count = keys_to_delete.len() as u32;
		for key in keys_to_delete {
			unhashed::kill(&key);
		}

		// Charge weight for the operations performed.
		meter.consume(weight_per_key.saturating_mul(deleted_count as u64));

		// Determine the next cursor.
		if keys_remaining {
			// More keys in this stage, continue with the same stage.
			Ok(Some(Cursor { stage }))
		} else {
			// No more keys in this stage, move to the next stage or finish.
			match stage.next() {
				Some(next_stage) => Ok(Some(Cursor { stage: next_stage })),
				None => Ok(None), // Migration complete
			}
		}
	}

	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::TryRuntimeError> {
		use codec::Encode;

		let state_commitments_count = count_keys(&Stage::StateCommitments.storage_prefix());
		let state_machine_update_time_count = count_keys(&Stage::StateMachineUpdateTime.storage_prefix());
		let relay_chain_state_commitments_count = count_keys(&Stage::RelayChainStateCommitments.storage_prefix());

		log::info!(
			"CleanupIsmpStorage pre_upgrade: StateCommitments={}, StateMachineUpdateTime={}, RelayChainStateCommitments={}",
			state_commitments_count,
			state_machine_update_time_count,
			relay_chain_state_commitments_count
		);

		Ok((
			state_commitments_count,
			state_machine_update_time_count,
			relay_chain_state_commitments_count,
		)
			.encode())
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::TryRuntimeError> {
		let state_commitments_count = count_keys(&Stage::StateCommitments.storage_prefix());
		let state_machine_update_time_count = count_keys(&Stage::StateMachineUpdateTime.storage_prefix());
		let relay_chain_state_commitments_count = count_keys(&Stage::RelayChainStateCommitments.storage_prefix());

		log::info!(
			"CleanupIsmpStorage post_upgrade: StateCommitments={}, StateMachineUpdateTime={}, RelayChainStateCommitments={}",
			state_commitments_count,
			state_machine_update_time_count,
			relay_chain_state_commitments_count
		);

		if state_commitments_count > 0 || state_machine_update_time_count > 0 || relay_chain_state_commitments_count > 0
		{
			return Err("CleanupIsmpStorage: Not all keys were deleted".into());
		}

		Ok(())
	}
}

#[cfg(feature = "try-runtime")]
fn count_keys(prefix: &[u8; 32]) -> u32 {
	let mut count = 0u32;
	let mut iter = sp_io::storage::next_key(prefix);
	while let Some(key) = iter {
		if !key.starts_with(prefix) {
			break;
		}
		count += 1;
		iter = sp_io::storage::next_key(&key);
	}
	count
}

#[cfg(test)]
mod tests {
	use super::*;
	use frame_support::{
		migrations::SteppedMigration,
		storage::unhashed,
		weights::{Weight, WeightMeter},
	};
	use sp_io::TestExternalities;

	// Use the actual Runtime from the crate for testing
	type TestRuntime = crate::Runtime;

	fn insert_test_keys(prefix: &[u8; 32], count: u32) {
		for i in 0..count {
			let mut key = prefix.to_vec();
			key.extend_from_slice(&i.to_le_bytes());
			unhashed::put(&key, &i);
		}
	}

	fn count_test_keys(prefix: &[u8; 32]) -> u32 {
		let mut count = 0u32;
		let mut iter = sp_io::storage::next_key(prefix);
		while let Some(key) = iter {
			if !key.starts_with(prefix) {
				break;
			}
			count += 1;
			iter = sp_io::storage::next_key(&key);
		}
		count
	}

	#[test]
	fn test_cleanup_empty_storage() {
		TestExternalities::new_empty().execute_with(|| {
			// Migration should iterate through all stages even when empty
			let mut cursor = None;
			let mut steps = 0;
			loop {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
				steps += 1;
				if cursor.is_none() {
					break;
				}
				assert!(steps <= 3, "Should complete within 3 steps for empty storage");
			}
			assert!(steps <= 3, "Should check all three stages");
		});
	}

	#[test]
	fn test_cleanup_single_stage() {
		TestExternalities::new_empty().execute_with(|| {
			let prefix = Stage::StateCommitments.storage_prefix();
			insert_test_keys(&prefix, 10);

			assert_eq!(count_test_keys(&prefix), 10);

			let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
			let result = CleanupIsmpStorage::<TestRuntime>::step(None, &mut meter);

			// Should move to next stage after cleaning
			assert!(result.is_ok());
			let cursor = result.unwrap();
			assert!(cursor.is_some());

			// Continue until completion
			let mut cursor = cursor;
			let mut steps = 0;
			while cursor.is_some() && steps < 100 {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
				steps += 1;
			}

			assert_eq!(count_test_keys(&prefix), 0, "All keys should be deleted");
		});
	}

	#[test]
	fn test_cleanup_all_stages() {
		TestExternalities::new_empty().execute_with(|| {
			// Populate all three storage maps
			let prefix1 = Stage::StateCommitments.storage_prefix();
			let prefix2 = Stage::StateMachineUpdateTime.storage_prefix();
			let prefix3 = Stage::RelayChainStateCommitments.storage_prefix();

			insert_test_keys(&prefix1, 50);
			insert_test_keys(&prefix2, 75);
			insert_test_keys(&prefix3, 100);

			assert_eq!(count_test_keys(&prefix1), 50);
			assert_eq!(count_test_keys(&prefix2), 75);
			assert_eq!(count_test_keys(&prefix3), 100);

			// Run migration to completion
			let mut cursor = None;
			let mut steps = 0;
			loop {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
				steps += 1;

				if cursor.is_none() {
					break;
				}

				assert!(steps < 1000, "Migration should complete in reasonable steps");
			}

			// Verify all maps are empty
			assert_eq!(count_test_keys(&prefix1), 0);
			assert_eq!(count_test_keys(&prefix2), 0);
			assert_eq!(count_test_keys(&prefix3), 0);
		});
	}

	#[test]
	fn test_bounded_deletion() {
		TestExternalities::new_empty().execute_with(|| {
			let prefix = Stage::StateCommitments.storage_prefix();
			let total_keys = MAX_KEYS_PER_STEP * 3;
			insert_test_keys(&prefix, total_keys);

			assert_eq!(count_test_keys(&prefix), total_keys);

			// Run one step with sufficient weight
			let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
			let initial_remaining = meter.remaining();
			let result = CleanupIsmpStorage::<TestRuntime>::step(None, &mut meter);

			assert!(result.is_ok());
			let cursor = result.unwrap();
			assert!(cursor.is_some(), "Should not complete in one step");

			// Verify that we deleted at most MAX_KEYS_PER_STEP
			let remaining_keys = count_test_keys(&prefix);
			let deleted = total_keys - remaining_keys;
			assert!(
				deleted <= MAX_KEYS_PER_STEP,
				"Should delete at most MAX_KEYS_PER_STEP keys, deleted: {}",
				deleted
			);

			// Verify weight was consumed
			let remaining = meter.remaining();
			assert!(remaining.ref_time() < initial_remaining.ref_time());
		});
	}

	#[test]
	fn test_idempotency() {
		TestExternalities::new_empty().execute_with(|| {
			// Run migration to completion on empty storage
			let mut cursor = None;
			loop {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
				if cursor.is_none() {
					break;
				}
			}

			// Run again - should iterate through all stages and complete again
			let mut cursor = None;
			let mut steps = 0;
			loop {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
				steps += 1;
				if cursor.is_none() {
					break;
				}
				assert!(steps <= 3, "Should complete within 3 steps");
			}
		});
	}

	#[test]
	fn test_insufficient_weight() {
		TestExternalities::new_empty().execute_with(|| {
			let prefix = Stage::StateCommitments.storage_prefix();
			insert_test_keys(&prefix, 10);

			// Provide insufficient weight
			let mut meter = WeightMeter::with_limit(Weight::from_parts(1, 0));
			let result = CleanupIsmpStorage::<TestRuntime>::step(None, &mut meter);

			assert!(matches!(
				result,
				Err(frame_support::migrations::SteppedMigrationError::InsufficientWeight { .. })
			));
		});
	}

	#[test]
	fn test_resume_from_cursor() {
		TestExternalities::new_empty().execute_with(|| {
			let prefix1 = Stage::StateCommitments.storage_prefix();
			let prefix2 = Stage::StateMachineUpdateTime.storage_prefix();

			insert_test_keys(&prefix1, 10);
			insert_test_keys(&prefix2, 10);

			// Start migration
			let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
			let cursor = CleanupIsmpStorage::<TestRuntime>::step(None, &mut meter).unwrap();

			// Continue from cursor
			let mut cursor = cursor;
			while cursor.is_some() {
				let mut meter = WeightMeter::with_limit(Weight::from_parts(1_000_000_000, 0));
				cursor = CleanupIsmpStorage::<TestRuntime>::step(cursor, &mut meter).unwrap();
			}

			// Verify all cleaned
			assert_eq!(count_test_keys(&prefix1), 0);
			assert_eq!(count_test_keys(&prefix2), 0);
		});
	}
}
