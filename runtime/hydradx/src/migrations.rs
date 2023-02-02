use super::*;

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<(), &'static str> {
		Ok(())
	}

	fn on_runtime_upgrade() -> Weight {
		let mut weight: Weight = Weight::zero();

		frame_support::log::info!("Migrate Uniques Pallet start");
		weight = weight.saturating_add(pallet_uniques::migration::migrate_to_v1::<Runtime, _, Uniques>());
		frame_support::log::info!("Migrate Uniques Pallet end");

		frame_support::log::info!("Migrate Omnipool Pallet start");
		weight = weight.saturating_add(pallet_omnipool::migration::migrate_to_v1::<Runtime, Omnipool>());
		frame_support::log::info!("Migrate Omnipool Pallet end");

		frame_support::log::info!("Migrate Omnipool Pallet to v2 start");
		weight = weight.saturating_add(pallet_omnipool::migration::migrate_to_v2::<Runtime, Omnipool>());
		frame_support::log::info!("Migrate Omnipool Pallet to v2 end");

		frame_support::log::info!("Mingrate Omnipool Liquidity Mining Pallet to v1 start");
		weight = weight.saturating_add(pallet_omnipool_liquidity_mining::migration::migrate_to_v1::<
			Runtime,
			OmnipoolLiquidityMining,
		>());
		frame_support::log::info!("Mingrate Omnipool Liquidity Mining Pallet to v1 end");

		weight
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade() -> Result<(), &'static str> {
		Ok(())
	}
}
