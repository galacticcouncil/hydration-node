#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use sp_std::vec::Vec;
use sp_core::RuntimeDebug;

#[cfg(feature = "std")]
use sp_core::bytes;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, PartialOrd, Ord, Encode, Decode, RuntimeDebug, derive_more::From)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize, Default, Hash))]
pub struct BlockHash(#[cfg_attr(feature = "std", serde(with="bytes"))] pub Vec<u8>);

#[derive(Debug, Encode, Decode, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Chain {
	pub genesis_hash: BlockHash,
	pub last_block_hash: BlockHash,
}

#[cfg(feature = "std")]
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
	pub type PreviousChain<T> = StorageValue<_, Chain>;

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

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::call]
	impl<T:Config> Pallet<T> {}
}
