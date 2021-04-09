#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::dispatch::DispatchError;
use frame_support::sp_runtime::traits::{AtLeast32Bit, CheckedAdd, One};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Asset type
		type AssetId: Parameter + Member + Into<u32> + AtLeast32Bit + Default + Copy + MaybeSerializeDeserialize;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Next Asset ID is not available. Happens when it reaches the MAX of given id type.
		NoIdAvailable,
	}

	/// Core Asset Id
	#[pallet::storage]
	#[pallet::getter(fn core_asset_id)]
	pub type CoreAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

	/// Current asset id. Note: This must set so it does not clash with the CoreAssetId!
	#[pallet::storage]
	#[pallet::getter(fn next_asset_id)]
	pub type NextAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

	/// Created assets
	#[pallet::storage]
	#[pallet::getter(fn asset_ids)]
	pub type AssetIds<T: Config> = StorageMap<_, Twox64Concat, Vec<u8>, Option<T::AssetId>, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub core_asset_id: T::AssetId,
		pub next_asset_id: T::AssetId,
		pub asset_ids: Vec<(Vec<u8>, T::AssetId)>,
	}

	#[cfg(feature = "std")]
	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig {
				core_asset_id: Default::default(),
				next_asset_id: Default::default(),
				asset_ids: vec![],
			}
		}
	}
	#[pallet::genesis_build]
	impl<T: Config> GenesisBuild<T> for GenesisConfig<T> {
		fn build(&self) {
			CoreAssetId::<T>::put(self.core_asset_id);
			NextAssetId::<T>::put(self.next_asset_id);
			self.asset_ids.iter().for_each(|(name, asset_id)| {
				AssetIds::<T>::insert(name, Some(asset_id));
			})
		}
	}
}

impl<T: Config> Pallet<T> {
	/// Create assset for given name or return existing AssetId if already exists.
	pub fn get_or_create_asset(name: Vec<u8>) -> Result<T::AssetId, DispatchError> {
		match <AssetIds<T>>::contains_key(&name) {
			true => Ok(<AssetIds<T>>::get(&name).unwrap()),
			false => {
				let asset_id = Self::next_asset_id();
				let next_id = asset_id.checked_add(&One::one()).ok_or(Error::<T>::NoIdAvailable)?;
				<NextAssetId<T>>::put(next_id);
				<AssetIds<T>>::insert(name, Some(asset_id));
				Ok(asset_id)
			}
		}
	}
}
