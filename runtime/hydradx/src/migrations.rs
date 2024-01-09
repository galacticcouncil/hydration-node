use crate::Runtime;
use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use pallet_evm_chain_id::ChainId;
#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

pub struct OnRuntimeUpgradeMigration;

impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
		log::info!("PreMigrate Collator Selection Pallet start");
		let number_of_invulnerables = pallet_collator_selection::migration::v1::MigrateToV1::<Runtime>::pre_upgrade()?;
		log::info!("PreMigrate Collator Selection Pallet end");
		Ok(number_of_invulnerables)
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = Weight::zero();

		log::info!("Migrate Collator Selection Pallet to v1 start");
		weight = weight
			.saturating_add(pallet_collator_selection::migration::v1::MigrateToV1::<Runtime>::on_runtime_upgrade());
		log::info!("Migrate Collator Selection Pallet to v1 end");

		log::info!("Migrate Unknown Tokens Pallet to v2 start");
		weight = weight.saturating_add(orml_unknown_tokens::Migration::<Runtime>::on_runtime_upgrade());
		log::info!("Migrate Unknown Tokens Pallet to v2 end");

		let evm_id: u64 = 222_222u64;
		ChainId::<Runtime>::put(evm_id);
		weight = weight.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().reads_writes(0, 1));

		weight = weight.saturating_add(pallet_referrals::migration::preregister_parachain_codes::<Runtime>());

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		log::info!("PostMigrate Collator Selection Pallet start");
		let migration_result = pallet_collator_selection::migration::v1::MigrateToV1::<Runtime>::post_upgrade(state);
		log::info!("PostMigrate Collator Selection Pallet end");

		migration_result
	}
}
