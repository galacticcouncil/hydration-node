use super::*;

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};

#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		pallet_asset_registry::migration::v2::pre_migrate::<Runtime>();
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		pallet_asset_registry::migration::v2::migrate::<Runtime>();

		Weight::zero()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		pallet_asset_registry::migration::v2::post_migrate::<Runtime>();
		Ok(())
	}
}
