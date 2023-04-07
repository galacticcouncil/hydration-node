use super::*;
use sp_std::marker::PhantomData;

use frame_support::{log, traits::OnRuntimeUpgrade, weights::Weight};
use pallet_asset_registry::{AssetLocations, LocationAssets};
use polkadot_xcm::v3::MultiLocation;

pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		frame_support::log::info!("PreMigrate Duster Pallet start");
		pallet_duster::migration::v1::pre_migrate::<Runtime, Duster>();
		frame_support::log::info!("PreMigrate Duster Pallet end");

		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = Weight::zero();

		frame_support::log::info!("Migrate Scheduler Pallet to v4 start");
		weight = weight.saturating_add(Scheduler::migrate_v1_to_v4());
		frame_support::log::info!("Migrate Scheduler Pallet to v4 end");

		frame_support::log::info!("Migrate Duster Pallet to v1 start");
		weight = weight.saturating_add(pallet_duster::migration::v1::migrate::<Runtime, Duster>(
			get_all_module_accounts(),
			TreasuryAccount::get(),
			TreasuryAccount::get(),
		));
		frame_support::log::info!("Migrate Duster Pallet to v1 end");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		frame_support::log::info!("PostMigrate Duster Pallet start");
		pallet_duster::migration::v1::post_migrate::<Runtime, Duster>();
		frame_support::log::info!("PostMigrate Duster Pallet end");
		Ok(())
	}
}

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq, TypeInfo)]
pub struct AssetLocationV2(pub polkadot_xcm::v2::MultiLocation);

pub struct MigrateRegistryLocationToV3<T>(PhantomData<T>);
impl<T: pallet_asset_registry::Config> OnRuntimeUpgrade for MigrateRegistryLocationToV3<T>
where
	AssetLocation: Into<T::AssetNativeLocation>,
{
	fn on_runtime_upgrade() -> Weight {
		log::info!(
			target: "asset-registry",
			"MigrateRegistryLocationToV3::on_runtime_upgrade: migrating asset locations to v3"
		);

		let mut weight: Weight = Weight::zero();

		let _ = LocationAssets::<T>::clear(u32::MAX, None);

		AssetLocations::<T>::translate(|asset_id, old_location: AssetLocationV2| {
			weight.saturating_accrue(T::DbWeight::get().reads_writes(1, 2));
			let new_multi_loc: MultiLocation = old_location.0.try_into().expect("xcm::v1::MultiLocation");
			let new_location: T::AssetNativeLocation = AssetLocation(new_multi_loc).into();
			LocationAssets::<T>::insert(&new_location, asset_id);
			Some(new_location)
		});

		weight
	}
}
