// This file is part of pallet-asset-registry.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use frame_support::sp_runtime::traits::CheckedAdd;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::DispatchError;
use sp_std::convert::TryInto;
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;
pub mod migration;
mod types;
pub mod weights;

use weights::WeightInfo;

pub use types::AssetType;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub use crate::types::{AssetDetails, AssetMetadata};
use frame_support::BoundedVec;
use hydradx_traits::{AssetKind, CreateRegistry, InspectRegistry, Registry, ShareTokenRegistry};

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::types::Metadata;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	pub type AssetDetailsT<T> =
		AssetDetails<<T as Config>::AssetId, <T as Config>::Balance, BoundedVec<u8, <T as Config>::StringLimit>>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin which can work with asset-registry.
		type RegistryOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Asset type
		type AssetId: Parameter
			+ Member
			+ Default
			+ Copy
			+ BaseArithmetic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Balance type
		type Balance: Parameter
			+ Member
			+ AtLeast32BitUnsigned
			+ Default
			+ Copy
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen;

		/// Asset location type
		type AssetNativeLocation: Parameter + Member + Default + MaxEncodedLen;

		/// The maximum length of a name or symbol stored on-chain.
		type StringLimit: Get<u32>;

		#[pallet::constant]
		type SequentialIdStartAt: Get<Self::AssetId>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeAssetId: Get<Self::AssetId>;

		/// Weight information for the extrinsics
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset ID is not available. This only happens when it reaches the MAX value of given id type.
		NoIdAvailable,

		/// Invalid asset name or symbol.
		AssetNotFound,

		/// Invalid asset name or symbol.
		TooLong,

		/// Asset ID is not registered in the asset-registry.
		AssetNotRegistered,

		/// Asset is already registered.
		AssetAlreadyRegistered,

		/// Incorrect number of assets provided to create shared asset.
		InvalidSharedAssetLen,

		/// Cannot update asset location
		CannotUpdateLocation,

		/// Selected asset id is out of reserved range.
		NotInReservedRange,

		/// Location already registered with different asset
		LocationAlreadyRegistered,
	}

	#[pallet::storage]
	#[pallet::getter(fn assets)]
	/// Details of an asset.
	pub type Assets<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, AssetDetailsT<T>, OptionQuery>;

	#[pallet::storage]
	/// Next available asset id. This is sequential id assigned for each new registered asset.
	pub type NextAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asset_ids)]
	/// Mapping between asset name and asset id.
	pub type AssetIds<T: Config> =
		StorageMap<_, Blake2_128Concat, BoundedVec<u8, T::StringLimit>, T::AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn locations)]
	/// Native location of an asset.
	pub type AssetLocations<T: Config> = StorageMap<_, Twox64Concat, T::AssetId, T::AssetNativeLocation, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn location_assets)]
	/// Local asset for native location.
	pub type LocationAssets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetNativeLocation, T::AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn asset_metadata)]
	/// Metadata of an asset.
	pub type AssetMetadataMap<T: Config> =
		StorageMap<_, Twox64Concat, T::AssetId, AssetMetadata<BoundedVec<u8, T::StringLimit>>, OptionQuery>;

	#[allow(clippy::type_complexity)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		pub registered_assets: Vec<(Vec<u8>, T::Balance, Option<T::AssetId>)>,
		pub native_asset_name: Vec<u8>,
		pub native_existential_deposit: T::Balance,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig::<T> {
				registered_assets: sp_std::vec![],
				native_asset_name: b"HDX".to_vec(),
				native_existential_deposit: Default::default(),
			}
		}
	}
	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			// Register native asset first
			// It is to make sure that native is registered as any other asset
			let native_asset_name = Pallet::<T>::to_bounded_name(self.native_asset_name.to_vec())
				.map_err(|_| panic!("Invalid native asset name!"))
				.unwrap();

			AssetIds::<T>::insert(&native_asset_name, T::NativeAssetId::get());
			let details = AssetDetails {
				name: native_asset_name,
				asset_type: AssetType::Token,
				existential_deposit: self.native_existential_deposit,

				xcm_rate_limit: None,
			};

			Assets::<T>::insert(T::NativeAssetId::get(), details);

			self.registered_assets.iter().for_each(|(name, ed, id)| {
				let bounded_name = Pallet::<T>::to_bounded_name(name.to_vec())
					.map_err(|_| panic!("Invalid asset name!"))
					.unwrap();
				let _ = Pallet::<T>::register_asset(bounded_name, AssetType::Token, *ed, *id, None)
					.map_err(|_| panic!("Failed to register asset"));
			})
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Asset was registered.
		Registered {
			asset_id: T::AssetId,
			asset_name: BoundedVec<u8, T::StringLimit>,
			asset_type: AssetType<T::AssetId>,
		},

		/// Asset was updated.
		Updated {
			asset_id: T::AssetId,
			asset_name: BoundedVec<u8, T::StringLimit>,
			asset_type: AssetType<T::AssetId>,
			existential_deposit: T::Balance,
			xcm_rate_limit: Option<T::Balance>,
		},

		/// Metadata set for an asset.
		MetadataSet {
			asset_id: T::AssetId,
			symbol: BoundedVec<u8, T::StringLimit>,
			decimals: u8,
		},

		/// Native location set for an asset.
		LocationSet {
			asset_id: T::AssetId,
			location: T::AssetNativeLocation,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a new asset.
		///
		/// Asset is identified by `name` and the name must not be used to register another asset.
		///
		/// New asset is given `NextAssetId` - sequential asset id
		///
		/// Adds mapping between `name` and assigned `asset_id` so asset id can be retrieved by name too (Note: this approach is used in AMM implementation (xyk))
		///
		/// Emits 'Registered` event when successful.
		#[allow(clippy::too_many_arguments)]
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			name: Vec<u8>,
			asset_type: AssetType<T::AssetId>,
			existential_deposit: T::Balance,
			asset_id: Option<T::AssetId>,
			metadata: Option<Metadata>,
			location: Option<T::AssetNativeLocation>,
			xcm_rate_limit: Option<T::Balance>,
		) -> DispatchResult {
			T::RegistryOrigin::ensure_origin(origin)?;

			let bounded_name = Self::to_bounded_name(name)?;

			ensure!(
				Self::asset_ids(&bounded_name).is_none(),
				Error::<T>::AssetAlreadyRegistered
			);

			let asset_id =
				Self::register_asset(bounded_name, asset_type, existential_deposit, asset_id, xcm_rate_limit)?;

			if let Some(meta) = metadata {
				let symbol = Self::to_bounded_name(meta.symbol)?;
				AssetMetadataMap::<T>::insert(
					asset_id,
					AssetMetadata {
						symbol: symbol.clone(),
						decimals: meta.decimals,
					},
				);

				Self::deposit_event(Event::MetadataSet {
					asset_id,
					symbol,
					decimals: meta.decimals,
				});
			}

			if let Some(loc) = location {
				ensure!(asset_id != T::NativeAssetId::get(), Error::<T>::CannotUpdateLocation);
				ensure!(
					Self::location_assets(&loc).is_none(),
					Error::<T>::LocationAlreadyRegistered
				);
				AssetLocations::<T>::insert(asset_id, &loc);
				LocationAssets::<T>::insert(&loc, asset_id);

				Self::deposit_event(Event::LocationSet {
					asset_id,
					location: loc,
				});
			}

			Ok(())
		}

		/// Update registered asset.
		///
		/// Updates also mapping between name and asset id if provided name is different than currently registered.
		///
		/// Emits `Updated` event when successful.

		// TODO: No tests
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			name: Vec<u8>,
			asset_type: AssetType<T::AssetId>,
			existential_deposit: Option<T::Balance>,
			xcm_rate_limit: Option<T::Balance>,
		) -> DispatchResult {
			T::RegistryOrigin::ensure_origin(origin)?;

			Assets::<T>::try_mutate(asset_id, |maybe_detail| -> DispatchResult {
				let detail = maybe_detail.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				let bounded_name = Self::to_bounded_name(name)?;

				if bounded_name != detail.name {
					// Make sure that there is no such name already registered
					ensure!(
						Self::asset_ids(&bounded_name).is_none(),
						Error::<T>::AssetAlreadyRegistered
					);

					// update also name map - remove old one first
					AssetIds::<T>::remove(&detail.name);
					AssetIds::<T>::insert(&bounded_name, asset_id);
				}

				detail.name = bounded_name.clone();
				detail.asset_type = asset_type;
				detail.existential_deposit = existential_deposit.unwrap_or(detail.existential_deposit);
				detail.xcm_rate_limit = xcm_rate_limit;

				Self::deposit_event(Event::Updated {
					asset_id,
					asset_name: bounded_name,
					asset_type,
					existential_deposit: detail.existential_deposit,
					xcm_rate_limit: detail.xcm_rate_limit,
				});

				Ok(())
			})
		}

		/// Set metadata for an asset.
		///
		/// - `asset_id`: Asset identifier.
		/// - `symbol`: The exchange symbol for this asset. Limited in length by `StringLimit`.
		/// - `decimals`: The number of decimals this asset uses to represent one unit.
		///
		/// Emits `MetadataSet` event when successful.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::set_metadata())]
		pub fn set_metadata(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			symbol: Vec<u8>,
			decimals: u8,
		) -> DispatchResult {
			T::RegistryOrigin::ensure_origin(origin)?;

			ensure!(Self::assets(asset_id).is_some(), Error::<T>::AssetNotFound);

			let b_symbol = Self::to_bounded_name(symbol)?;

			let metadata = AssetMetadata::<BoundedVec<u8, T::StringLimit>> {
				symbol: b_symbol.clone(),
				decimals,
			};

			AssetMetadataMap::<T>::insert(asset_id, metadata);

			Self::deposit_event(Event::MetadataSet {
				asset_id,
				symbol: b_symbol,
				decimals,
			});

			Ok(())
		}

		/// Set asset native location.
		///
		/// Adds mapping between native location and local asset id and vice versa.
		///
		/// Mainly used in XCM.
		///
		/// Emits `LocationSet` event when successful.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::set_location())]
		pub fn set_location(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			location: T::AssetNativeLocation,
		) -> DispatchResult {
			T::RegistryOrigin::ensure_origin(origin)?;

			ensure!(asset_id != T::NativeAssetId::get(), Error::<T>::CannotUpdateLocation);
			ensure!(Self::assets(asset_id).is_some(), Error::<T>::AssetNotRegistered);
			ensure!(
				Self::location_assets(&location).is_none(),
				Error::<T>::LocationAlreadyRegistered
			);

			if let Some(old_location) = AssetLocations::<T>::take(asset_id) {
				LocationAssets::<T>::remove(&old_location);
			}
			AssetLocations::<T>::insert(asset_id, &location);
			LocationAssets::<T>::insert(&location, asset_id);

			Self::deposit_event(Event::LocationSet { asset_id, location });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn next_asset_id() -> Option<T::AssetId> {
		NextAssetId::<T>::get().checked_add(&T::SequentialIdStartAt::get())
	}

	/// Convert Vec<u8> to BoundedVec so it respects the max set limit, otherwise return TooLong error
	pub fn to_bounded_name(name: Vec<u8>) -> Result<BoundedVec<u8, T::StringLimit>, Error<T>> {
		name.try_into().map_err(|_| Error::<T>::TooLong)
	}

	/// Register new asset.
	///
	/// Does not perform any  check whether an asset for given name already exists. This has to be prior to calling this function.
	pub fn register_asset(
		name: BoundedVec<u8, T::StringLimit>,
		asset_type: AssetType<T::AssetId>,
		existential_deposit: T::Balance,
		selected_asset_id: Option<T::AssetId>,
		xcm_rate_limit: Option<T::Balance>,
	) -> Result<T::AssetId, DispatchError> {
		let asset_id = if let Some(selected_id) = selected_asset_id {
			ensure!(
				selected_id < T::SequentialIdStartAt::get(),
				Error::<T>::NotInReservedRange
			);

			ensure!(
				!Assets::<T>::contains_key(selected_id),
				Error::<T>::AssetAlreadyRegistered
			);

			selected_id
		} else {
			NextAssetId::<T>::mutate(|value| -> Result<T::AssetId, DispatchError> {
				// Check if current id does not clash with CORE ASSET ID.
				// If yes, just skip it and use next one, otherwise use it.
				// Note: this way we prevent accidental clashes with native asset id, so no need to set next asset id to be > next asset id
				let next_asset_id = if *value == T::NativeAssetId::get() {
					value
						.checked_add(&T::AssetId::from(1))
						.ok_or(Error::<T>::NoIdAvailable)?
				} else {
					*value
				};

				*value = next_asset_id
					.checked_add(&T::AssetId::from(1))
					.ok_or(Error::<T>::NoIdAvailable)?;

				Ok(next_asset_id
					.checked_add(&T::SequentialIdStartAt::get())
					.ok_or(Error::<T>::NoIdAvailable)?)
			})?
		};

		AssetIds::<T>::insert(&name, asset_id);

		let details = AssetDetails {
			name: name.clone(),
			asset_type,
			existential_deposit,
			xcm_rate_limit,
		};

		// Store the details
		Assets::<T>::insert(asset_id, details);

		// Increase asset id to be assigned for following asset.

		Self::deposit_event(Event::Registered {
			asset_id,
			asset_name: name,
			asset_type,
		});

		Ok(asset_id)
	}

	/// Create asset for given name or return existing AssetId if such asset already exists.
	pub fn get_or_create_asset(
		name: Vec<u8>,
		asset_type: AssetType<T::AssetId>,
		existential_deposit: T::Balance,
		asset_id: Option<T::AssetId>,
	) -> Result<T::AssetId, DispatchError> {
		let bounded_name: BoundedVec<u8, T::StringLimit> = Self::to_bounded_name(name)?;

		if let Some(asset_id) = AssetIds::<T>::get(&bounded_name) {
			Ok(asset_id)
		} else {
			Self::register_asset(bounded_name, asset_type, existential_deposit, asset_id, None)
		}
	}

	/// Return location for given asset.
	pub fn asset_to_location(asset_id: T::AssetId) -> Option<T::AssetNativeLocation> {
		Self::locations(asset_id)
	}

	/// Return asset for given loation.
	pub fn location_to_asset(location: T::AssetNativeLocation) -> Option<T::AssetId> {
		Self::location_assets(location)
	}
}

impl<T: Config> Registry<T::AssetId, Vec<u8>, T::Balance, DispatchError> for Pallet<T> {
	fn exists(asset_id: T::AssetId) -> bool {
		Assets::<T>::contains_key(asset_id)
	}

	fn retrieve_asset(name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		let bounded_name = Self::to_bounded_name(name.clone())?;
		if let Some(asset_id) = AssetIds::<T>::get(bounded_name) {
			Ok(asset_id)
		} else {
			Err(Error::<T>::AssetNotFound.into())
		}
	}

	fn retrieve_asset_type(asset_id: T::AssetId) -> Result<AssetKind, DispatchError> {
		let asset_details =
			Assets::<T>::get(asset_id).ok_or_else(|| Into::<DispatchError>::into(Error::<T>::AssetNotFound))?;
		Ok(asset_details.asset_type.into())
	}

	fn create_asset(name: &Vec<u8>, existential_deposit: T::Balance) -> Result<T::AssetId, DispatchError> {
		Self::get_or_create_asset(name.clone(), AssetType::Token, existential_deposit, None)
	}
}

impl<T: Config> ShareTokenRegistry<T::AssetId, Vec<u8>, T::Balance, DispatchError> for Pallet<T> {
	fn retrieve_shared_asset(name: &Vec<u8>, _assets: &[T::AssetId]) -> Result<T::AssetId, DispatchError> {
		Self::retrieve_asset(name)
	}

	fn create_shared_asset(
		name: &Vec<u8>,
		assets: &[T::AssetId],
		existential_deposit: T::Balance,
	) -> Result<T::AssetId, DispatchError> {
		ensure!(assets.len() == 2, Error::<T>::InvalidSharedAssetLen);
		Self::get_or_create_asset(
			name.clone(),
			AssetType::PoolShare(assets[0], assets[1]),
			existential_deposit,
			None,
		)
	}
}

use orml_traits::GetByKey;
use sp_arithmetic::traits::Bounded;

// Return Existential deposit of an asset
impl<T: Config> GetByKey<T::AssetId, T::Balance> for Pallet<T> {
	fn get(k: &T::AssetId) -> T::Balance {
		if let Some(details) = Self::assets(k) {
			details.existential_deposit
		} else {
			// Asset does not exist - not supported
			T::Balance::max_value()
		}
	}
}

/// Allows querying the XCM rate limit for an asset by its id.
pub struct XcmRateLimitsInRegistry<T>(PhantomData<T>);
/// Allows querying the XCM rate limit for an asset by its id.
/// Both a unknown asset and an unset rate limit will return `None`.
impl<T: Config> GetByKey<T::AssetId, Option<T::Balance>> for XcmRateLimitsInRegistry<T> {
	fn get(k: &T::AssetId) -> Option<T::Balance> {
		Pallet::<T>::assets(k).and_then(|details| details.xcm_rate_limit)
	}
}

impl<T: Config> CreateRegistry<T::AssetId, T::Balance> for Pallet<T> {
	type Error = DispatchError;

	fn create_asset(name: &[u8], kind: AssetKind, existential_deposit: T::Balance) -> Result<T::AssetId, Self::Error> {
		let bounded_name: BoundedVec<u8, T::StringLimit> = Self::to_bounded_name(name.to_vec())?;
		Pallet::<T>::register_asset(bounded_name, kind.into(), existential_deposit, None, None)
	}
}

impl<T: Config> InspectRegistry<T::AssetId> for Pallet<T> {
	fn exists(asset_id: T::AssetId) -> bool {
		Assets::<T>::contains_key(asset_id)
	}

	fn decimals(asset_id: T::AssetId) -> Option<u8> {
		Some(AssetMetadataMap::<T>::get(asset_id)?.decimals)
	}

	fn asset_name(asset_id: T::AssetId) -> Option<Vec<u8>> {
		let asset = Assets::<T>::get(asset_id)?;
		Some(asset.name.into_inner())
	}

	fn asset_symbol(asset_id: T::AssetId) -> Option<Vec<u8>> {
		let asset_metadata = AssetMetadataMap::<T>::get(asset_id)?;
		Some(asset_metadata.symbol.into_inner())
	}
}
