use super::*;
use frame_support::log;
use frame_support::traits::{Get, GetStorageVersion, PalletInfoAccess, StorageVersion};

/// Migrate the pallet storage to v1.
pub fn migrate_to_v1<T: Config, P: GetStorageVersion + PalletInfoAccess>() -> frame_support::weights::Weight {
	let on_chain_storage_version = <P as GetStorageVersion>::on_chain_storage_version();
	log::info!(
		target: "runtime::omnipool",
		"Running migration storage v1 for omnipool with storage version {:?}",
		on_chain_storage_version,
	);

	if on_chain_storage_version < 1 {
		HubAssetImbalance::<T>::set(SimpleImbalance::default());
		StorageVersion::new(1).put::<P>();
		log::info!(
			target: "runtime::omnipool",
			"Running migration storage v1 for omnipool with storage version {:?} was complete",
			on_chain_storage_version,
		);
		// calculate and return migration weights
		T::DbWeight::get().reads_writes(1u64, 1u64)
	} else {
		log::warn!(
			target: "runtime::omnipool",
			"Attempted to apply migration to v1 but failed because storage version is {:?}",
			on_chain_storage_version,
		);
		T::DbWeight::get().reads(1)
	}
}
