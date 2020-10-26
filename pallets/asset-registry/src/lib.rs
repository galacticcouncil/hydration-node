#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::sp_runtime::traits::{AtLeast32Bit, CheckedAdd, Member, One};
use frame_support::{decl_error, decl_module, decl_storage, dispatch::DispatchError, Parameter};
use frame_system::{self as system};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: system::Trait {
	type AssetId: Parameter + Member + Into<u32> + AtLeast32Bit + Default + Copy;
}

decl_storage! {
	trait Store for Module<T: Trait> as AssetRegistry {
		pub CoreAssetId get(fn core_asset_id) config(): T::AssetId;
		pub NextAssetId get(fn next_asset_id) config(): T::AssetId;
		pub AssetIds get(fn asset_ids) config(): map hasher(twox_64_concat) Vec<u8> => Option<T::AssetId>;
	}
}

decl_error! {
	pub enum Error for Module<T: Trait> {
		NoIdAvailable
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

	}
}

impl<T: Trait> Module<T> {
	pub fn create_asset(name: Vec<u8>) -> Result<T::AssetId, DispatchError> {
		match <AssetIds<T>>::contains_key(&name) {
			true => Ok(<AssetIds<T>>::get(&name).unwrap()),
			false => {
				let asset_id = Self::next_asset_id();
				let next_id = asset_id.checked_add(&One::one()).ok_or(Error::<T>::NoIdAvailable)?;
				<NextAssetId<T>>::put(next_id);
				<AssetIds<T>>::insert(name, asset_id);
				Ok(asset_id)
			}
		}
	}
}
