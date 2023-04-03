use super::*;

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
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
