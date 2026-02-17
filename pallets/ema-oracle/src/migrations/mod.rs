use frame_support::traits::StorageVersion;

pub mod v1;
pub mod v2;

/// The in-code storage version.
pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);
