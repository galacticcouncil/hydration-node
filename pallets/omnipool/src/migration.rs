pub mod v2 {
	use crate::{Config, Pallet};
	use frame_support::pallet_prelude::{Get, StorageVersion, Weight};
	const TARGET: &'static str = "runtime::omnipool";

	pub fn pre_migrate<T: Config>() {
		assert_eq!(StorageVersion::get::<Pallet<T>>(), 1, "Storage version too high.");

		log::info!(
			target: TARGET,
			"V2 migration: removing imbalance - PRE checks successful!"
		);
	}

	pub fn migrate<T: Config<AssetId = u32>>() -> Weight {
		if StorageVersion::get::<Pallet<T>>() != 1 {
			log::info!(
				target: TARGET,
				"v2 migration - Incorrect pallet version."
			);
			return T::DbWeight::get().reads_writes(1, 0);
		}

		log::info!(
			target: TARGET,
			"Omnipool V2 - removing imbalance"
		);

		let mut reads = 0;
		let mut writes = 0;

		let mut weight = Weight::zero();

		//TODO: Implement the migration logic here
		// how to remove storage when it was deleted, huh ?
		// should we keep it for now and remove it later ?

		weight
	}

	pub fn post_migrate<T: Config>() {
		log::info!(
			target: TARGET,
			"Omnipool V2 - balance removed"
		);
	}
}
