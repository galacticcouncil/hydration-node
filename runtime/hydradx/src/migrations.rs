#![allow(unused_imports)]
use crate::Vec;
use frame_support::{codec::alloc::vec, traits::OnRuntimeUpgrade, weights::Weight};

pub struct OnRuntimeUpgradeMigration;

use crate::Runtime;
use pallet_evm_chain_id::ChainId;

impl OnRuntimeUpgrade for OnRuntimeUpgradeMigration {
	#[cfg(feature = "try-runtime")]
	fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
		Ok(vec![])
	}

	fn on_runtime_upgrade() -> Weight {
		let evm_id: u64 = 222_222u64;
		ChainId::<Runtime>::put(evm_id);
		<Runtime as frame_system::Config>::DbWeight::get().reads_writes(0, 1)
	}

	#[cfg(feature = "try-runtime")]
	fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
		Ok(())
	}
}
