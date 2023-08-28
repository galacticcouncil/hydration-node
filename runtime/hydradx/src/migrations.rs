use super::*;

use frame_support::{log, traits::OnRuntimeUpgrade, weights::Weight};

pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		log::info!("PreMigrate Transaction Pause Pallet start");
		let tx_pause_state = pallet_transaction_pause::migration::v1::Migration::<Runtime>::pre_upgrade()?;
		log::info!("PreMigrate Transaction Pause Pallet end");

		log::info!("PreMigrate Collator Rewards Pallet start");
		pallet_collator_rewards::migration::v1::pre_migrate::<Runtime>();
		log::info!("PreMigrate Collator Rewards Pallet end");

		log::info!("PreMigrate Genesis History Pallet start");
		pallet_genesis_history::migration::v1::pre_migrate::<Runtime>();
		log::info!("PreMigrate Genesis History Pallet end");

		Ok(tx_pause_state)
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = Weight::zero();

		log::info!("Migrate Transaction Pause Pallet to v1 start");
		weight =
			weight.saturating_add(pallet_transaction_pause::migration::v1::Migration::<Runtime>::on_runtime_upgrade());
		log::info!("Migrate Transaction Pause Pallet to v1 end");

		log::info!("Migrate Collator Rewards Pallet to v1 start");
		weight = weight.saturating_add(pallet_collator_rewards::migration::v1::migrate::<Runtime>());
		log::info!("Migrate Collator Rewards Pallet to v1 end");

		log::info!("Migrate Genesis History Pallet to v1 start");
		weight = weight.saturating_add(pallet_genesis_history::migration::v1::migrate::<Runtime>());
		log::info!("Migrate Genesis History Pallet to v1 end");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), &'static str> {
		log::info!("PostMigrate Transaction Pause Pallet start");
		pallet_transaction_pause::migration::v1::Migration::<Runtime>::post_upgrade(state)?;
		log::info!("PostMigrate Transaction Pause Pallet end");

		log::info!("PostMigrate Collator Rewards Pallet start");
		pallet_collator_rewards::migration::v1::post_migrate::<Runtime>();
		log::info!("PostMigrate Collator Rewards Pallet end");

		log::info!("PostMigrate Genesis History Pallet start");
		pallet_genesis_history::migration::v1::post_migrate::<Runtime>();
		log::info!("PostMigrate Genesis History Pallet end");

		Ok(())
	}
}
