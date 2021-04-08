#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use sp_std::vec::Vec;
use sp_core::RuntimeDebug;
#[cfg(feature = "std")]
use frame_support::traits::GenesisBuild;
#[cfg(feature = "std")]
use sp_core::bytes;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, PartialOrd, Ord, Default, Encode, Decode, RuntimeDebug, derive_more::From)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Hash))]
pub struct BlockHash(#[cfg_attr(feature = "std", serde(with="bytes"))] pub Vec<u8>);

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Chain {
	pub genesis_hash: BlockHash,
	pub last_block_hash: BlockHash,
}

impl Default for Chain {
	fn default() -> Self {
		Chain { genesis_hash: BlockHash::default(), last_block_hash: BlockHash::default() }
	}
}

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn previous_chain)]
	pub type PreviousChain<T: Config> = StorageValue<_, Chain, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig {
		pub previous_chain: Chain,
	}

	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig {
		fn build(&self) {
			PreviousChain::<T>::put(self.previous_chain.clone());
		}
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			GenesisConfig { previous_chain: { Chain::default() } }
		}
	}

	#[cfg(feature = "std")]
	impl GenesisConfig {
		pub fn build_storage<T: Config>(&self) -> Result<sp_runtime::Storage, String> {
			<Self as frame_support::traits::GenesisBuild<T>>::build_storage(self)
		}

		pub fn assimilate_storage<T: Config>(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
			<Self as frame_support::traits::GenesisBuild<T>>::assimilate_storage(self, storage)
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T:Config> Pallet<T> {}
}
