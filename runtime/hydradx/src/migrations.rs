#![allow(unused_imports)]
use crate::Vec;
use frame_support::{codec::alloc::vec, traits::OnRuntimeUpgrade, weights::Weight};
pub struct OnRuntimeUpgradeMigration;
use super::Runtime;

impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		pallet_referrals::migration::preregister_parachain_codes::<Runtime>()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		Ok(())
	}
}
