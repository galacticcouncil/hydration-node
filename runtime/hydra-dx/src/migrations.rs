use super::*;

/// Migrate from `PalletVersion` to the new `StorageVersion`
pub struct MigratePalletVersionToStorageVersion;
impl frame_support::traits::OnRuntimeUpgrade for MigratePalletVersionToStorageVersion {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_support::migrations::migrate_from_pallet_version_to_storage_version::<
            AllPalletsWithSystem,
        >(&RocksDbWeight::get())
    }
}

const COUNCIL_OLD_PREFIX: &str = "Instance1Collective";
/// Migrate from `Instance1Collective` to the new pallet prefix `Council`
pub struct CouncilStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for CouncilStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_collective::migrations::v4::migrate::<Runtime, Council, _>(COUNCIL_OLD_PREFIX)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::pre_migrate::<Council, _>(COUNCIL_OLD_PREFIX);
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::post_migrate::<Council, _>(COUNCIL_OLD_PREFIX);
        Ok(())
    }
}

const TECHNICAL_COMMITTEE_OLD_PREFIX: &str = "Instance2Collective";
/// Migrate from `Instance2Collective` to the new pallet prefix `TechnicalCommittee`
pub struct TechnicalCommitteeStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for TechnicalCommitteeStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_collective::migrations::v4::migrate::<Runtime, TechnicalCommittee, _>(
            TECHNICAL_COMMITTEE_OLD_PREFIX,
        )
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::pre_migrate::<TechnicalCommittee, _>(
            TECHNICAL_COMMITTEE_OLD_PREFIX,
        );
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::post_migrate::<TechnicalCommittee, _>(
            TECHNICAL_COMMITTEE_OLD_PREFIX,
        );
        Ok(())
    }
}

const TIPS_OLD_PREFIX: &str = "Treasury";
/// Migrate pallet-tips from `Treasury` to the new pallet prefix `Tips`
pub struct MigrateTipsPalletPrefix;
impl frame_support::traits::OnRuntimeUpgrade for MigrateTipsPalletPrefix {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_tips::migrations::v4::migrate::<Runtime, Tips, _>(TIPS_OLD_PREFIX)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        pallet_tips::migrations::v4::pre_migrate::<Runtime, Tips, _>(TIPS_OLD_PREFIX);
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        pallet_tips::migrations::v4::post_migrate::<Runtime, Tips, _>(TIPS_OLD_PREFIX);
        Ok(())
    }
}

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
pub struct ToV4OnRuntimeUpgrade;
impl OnRuntimeUpgrade for ToV4OnRuntimeUpgrade {
    fn on_runtime_upgrade() -> Weight {
        let mut weight = 0;
        frame_support::log::info!("MigratePalletVersionToStorageVersion start");
        weight += <MigratePalletVersionToStorageVersion as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("MigratePalletVersionToStorageVersion end");

        frame_support::log::info!("CouncilStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<Council>();
        weight += <CouncilStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("CouncilStoragePrefixMigration end");

        frame_support::log::info!("TechnicalCommitteeStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<TechnicalCommittee>();
        weight +=
            <TechnicalCommitteeStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("TechnicalCommitteeStoragePrefixMigration end");

        frame_support::log::info!("CouncilStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<Council>();
        weight += <CouncilStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("CouncilStoragePrefixMigration end");

        frame_support::log::info!("MigrateTipsPalletPrefix start");
        frame_support::traits::StorageVersion::new(0).put::<Tips>();
        weight += <MigrateTipsPalletPrefix as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("MigrateTipsPalletPrefix end");

        weight
    }
}
