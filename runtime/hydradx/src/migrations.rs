use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
use sp_std::vec;
use sp_std::vec::*;
pub struct OnRuntimeUpgradeMigration;
impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		Weight::zero()
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		Ok(())
	}
}
