use super::*;

pub mod v2 {
	use super::*;
	use crate::types::Balance;
	use crate::{Config, Pallet};
	use frame_support::pallet_prelude::{
		Decode, Encode, Get, MaxEncodedLen, RuntimeDebug, StorageVersion, ValueQuery, Weight,
	};
	use frame_support::storage_alias;

	const TARGET: &str = "runtime::omnipool";

	#[derive(Clone, Copy, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo, Default)]
	pub struct SimpleImbalance<Balance: Default> {
		pub value: Balance,
		pub negative: bool,
	}
	#[storage_alias]
	type HubAssetImbalance<T: Config> = StorageValue<Pallet<T>, SimpleImbalance<Balance>, ValueQuery>;

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
				"v2 migration - Incorrect storage version."
			);
			return T::DbWeight::get().reads_writes(1, 0);
		}

		log::info!(
			target: TARGET,
			"Omnipool V2 - removing imbalance"
		);
		HubAssetImbalance::<T>::kill();
		StorageVersion::new(2).put::<Pallet<T>>();
		T::DbWeight::get().reads_writes(2, 2)
	}

	pub fn post_migrate<T: Config>() {
		log::info!(
			target: TARGET,
			"Omnipool V2 - imbalance removed"
		);
	}
}
