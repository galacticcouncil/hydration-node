use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
#[cfg(feature = "try-runtime")]
use sp_std::prelude::*;

pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, sp_runtime::DispatchError> {
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		Weight::zero()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), sp_runtime::DispatchError> {
		Ok(())
	}
}
