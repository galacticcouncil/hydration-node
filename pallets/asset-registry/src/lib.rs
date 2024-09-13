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
use frame_support::require_transactional;
use frame_support::sp_runtime::traits::CheckedAdd;
use frame_support::traits::tokens::fungibles::{Inspect as FungiblesInspect, Mutate as FungiblesMutate};
use frame_support::traits::Contains;
use frame_system::pallet_prelude::*;
use scale_info::TypeInfo;
use sp_arithmetic::traits::BaseArithmetic;
use sp_runtime::DispatchError;
use sp_std::convert::TryInto;
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

mod benchmarking;
pub mod migration;
mod types;
pub mod weights;

pub use weights::WeightInfo;

pub use types::AssetType;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub use crate::types::{AssetDetails, Balance, Name, Symbol};
use frame_support::storage::with_transaction;
use frame_support::BoundedVec;
use hydradx_traits::evm::EvmAddress;
use hydradx_traits::{
	registry::{Create, Inspect, Mutate},
	AssetKind, BoundErc20,
};
use orml_traits::GetByKey;
use polkadot_xcm::v3::Junction::AccountKey20;
use polkadot_xcm::v3::Junctions::X1;
use polkadot_xcm::v3::MultiLocation;
use sp_runtime::TransactionOutcome;

/// Default value of existential deposit. This value is used if existential deposit wasn't
/// provided.
pub const DEFAULT_ED: Balance = 1;

#[frame_support::pallet]
#[allow(clippy::too_many_arguments)]
pub mod pallet {
	use sp_std::fmt::Debug;

	use super::*;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

	pub type AssetDetailsT<T> = AssetDetails<<T as Config>::StringLimit>;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// The origin which can work with asset-registry.
		type RegistryOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// The origin which can update assets' detail.
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Asset type
		type AssetId: Parameter
			+ Member
			+ Default
			+ Copy
			+ BaseArithmetic
			+ MaybeSerializeDeserialize
			+ MaxEncodedLen
			+ TypeInfo;

		/// Asset location type
		type AssetNativeLocation: Parameter + Member + Default + MaxEncodedLen;

		/// Multi currency mechanism
		type Currency: FungiblesInspect<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>
			+ FungiblesMutate<Self::AccountId>;

		#[pallet::constant]
		type SequentialIdStartAt: Get<Self::AssetId>;

		/// The maximum length of a name or symbol stored on-chain.
		#[pallet::constant]
		type StringLimit: Get<u32> + Debug + PartialEq;

		/// The min length of a name or symbol stored on-chain.
		#[pallet::constant]
		type MinStringLimit: Get<u32> + Debug + PartialEq;

		/// Weight multiplier for `register_external` extrinsic
		#[pallet::constant]
		type RegExternalWeightMultiplier: Get<u64>;

		/// Weight information for the extrinsics
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn integrity_test() {
			assert!(
				T::RegExternalWeightMultiplier::get().ge(&1_u64),
				"`T::RegExternalWeightMultiplier` must be greater than zero."
			);
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Asset ID is not available. This only happens when it reaches the MAX value of given id type.
		NoIdAvailable,

		/// Invalid asset name or symbol.
		AssetNotFound,

		/// Length of name or symbol is less than min. length.
		TooShort,

		/// Asset's symbol can't contain whitespace characters .
		InvalidSymbol,

		/// Asset ID is not registered in the asset-registry.
		AssetNotRegistered,

		/// Asset is already registered.
		AssetAlreadyRegistered,

		/// Incorrect number of assets provided to create shared asset.
		InvalidSharedAssetLen,

		/// Cannot update asset location.
		CannotUpdateLocation,

		/// Selected asset id is out of reserved range.
		NotInReservedRange,

		/// Location already registered with different asset.
		LocationAlreadyRegistered,

		/// Origin is forbidden to set/update value.
		Forbidden,

		/// Balance too low.
		InsufficientBalance,

		/// Sufficient assets can't be changed to insufficient.
		ForbiddenSufficiencyChange,

		/// Asset is already banned.
		AssetAlreadyBanned,

		/// Asset is not banned.
		AssetNotBanned,
	}

	#[pallet::type_value]
	/// Default value of NextAssetId if storage is empty.
	/// 1 is used to offset the native asset with id 0.
	pub fn DefaultNextAssetId<T: Config>() -> T::AssetId {
		1.into()
	}

	#[pallet::storage]
	#[pallet::getter(fn assets)]
	/// Details of an asset.
	pub type Assets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, AssetDetailsT<T>, OptionQuery>;

	#[pallet::storage]
	/// Next available asset id. This is sequential id assigned for each new registered asset.
	pub type NextAssetId<T: Config> = StorageValue<_, T::AssetId, ValueQuery, DefaultNextAssetId<T>>;

	#[pallet::storage]
	#[pallet::getter(fn asset_ids)]
	/// Mapping between asset name and asset id.
	pub type AssetIds<T: Config> =
		StorageMap<_, Blake2_128Concat, BoundedVec<u8, T::StringLimit>, T::AssetId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn locations)]
	/// Native location of an asset.
	pub type AssetLocations<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetId, T::AssetNativeLocation, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn banned_assets)]
	/// Non-native assets which transfer is banned.
	pub type BannedAssets<T: Config> = StorageMap<_, Blake2_128Concat, T::AssetId, (), OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn location_assets)]
	/// Local asset for native location.
	pub type LocationAssets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AssetNativeLocation, T::AssetId, OptionQuery>;

	#[pallet::storage]
	/// Number of accounts that paid existential deposits for insufficient assets.
	/// This storage is used by `SufficiencyCheck`.
	pub type ExistentialDepositCounter<T: Config> = StorageValue<_, u128, ValueQuery>;

	#[allow(clippy::type_complexity)]
	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		//asset_id, name, existential deposit, symbol, decimals, xcm_rate_limit, is_sufficient
		pub registered_assets: Vec<(
			Option<T::AssetId>,
			Option<Name<T::StringLimit>>,
			Balance,
			Option<Symbol<T::StringLimit>>,
			Option<u8>,
			Option<Balance>,
			bool,
		)>,
		pub native_asset_name: Name<T::StringLimit>,
		pub native_existential_deposit: Balance,
		pub native_symbol: Symbol<T::StringLimit>,
		pub native_decimals: u8,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			GenesisConfig::<T> {
				registered_assets: sp_std::vec![],
				native_asset_name: b"HDX".to_vec().try_into().expect("Invalid native asset name!"),
				native_existential_deposit: DEFAULT_ED,
				native_symbol: b"HDX".to_vec().try_into().expect("Invalid native asset symbol!"),
				native_decimals: 12,
			}
		}
	}
	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			with_transaction(|| {
				// Register native asset first
				// It is to make sure that native is registered as any other asset
				let native_asset_id = T::AssetId::from(0);
				AssetIds::<T>::insert(&self.native_asset_name, native_asset_id);
				let details = AssetDetails {
					name: Some(self.native_asset_name.clone()),
					asset_type: AssetType::Token,
					existential_deposit: self.native_existential_deposit,
					xcm_rate_limit: None,
					symbol: Some(self.native_symbol.clone()),
					decimals: Some(self.native_decimals),
					is_sufficient: true,
				};

				Assets::<T>::insert(native_asset_id, details);

				self.registered_assets.iter().for_each(
					|(id, name, ed, symbol, decimals, xcm_rate_limit, is_sufficient)| {
						let details = AssetDetails {
							name: name.clone(),
							asset_type: AssetType::Token,
							existential_deposit: *ed,
							xcm_rate_limit: *xcm_rate_limit,
							symbol: symbol.clone(),
							decimals: *decimals,
							is_sufficient: *is_sufficient,
						};
						let _ = Pallet::<T>::do_register_asset(*id, &details, None).expect("Failed to register asset");
					},
				);

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			})
			.expect("Genesis build failed.")
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub fn deposit_event)]
	pub enum Event<T: Config> {
		/// Existential deposit for insufficinet asset was paid.
		/// `SufficiencyCheck` triggers this event.
		ExistentialDepositPaid {
			who: T::AccountId,
			fee_asset: T::AssetId,
			amount: Balance,
		},

		/// Asset was registered.
		Registered {
			asset_id: T::AssetId,
			asset_name: Option<BoundedVec<u8, T::StringLimit>>,
			asset_type: AssetType,
			existential_deposit: Balance,
			xcm_rate_limit: Option<Balance>,
			symbol: Option<BoundedVec<u8, T::StringLimit>>,
			decimals: Option<u8>,
			is_sufficient: bool,
		},

		/// Asset was updated.
		Updated {
			asset_id: T::AssetId,
			asset_name: Option<BoundedVec<u8, T::StringLimit>>,
			asset_type: AssetType,
			existential_deposit: Balance,
			xcm_rate_limit: Option<Balance>,
			symbol: Option<BoundedVec<u8, T::StringLimit>>,
			decimals: Option<u8>,
			is_sufficient: bool,
		},

		/// Native location set for an asset.
		LocationSet {
			asset_id: T::AssetId,
			location: T::AssetNativeLocation,
		},

		/// Asset was banned.
		AssetBanned { asset_id: T::AssetId },

		/// Asset's ban was removed.
		AssetUnbanned { asset_id: T::AssetId },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Register a new asset.
		///
		/// New asset is given `NextAssetId` - sequential asset id
		///
		/// Asset's id is optional and it can't be used by another asset if it's provided.
		/// Provided `asset_id` must be from within reserved range.
		/// If `asset_id` is `None`, new asset is given id for sequential ids.
		///
		/// Asset's name is optional and it can't be used by another asset if it's provided.
		/// Adds mapping between `name` and assigned `asset_id` so asset id can be retrieved by name too (Note: this approach is used in AMM implementation (xyk))
		///
		/// Emits 'Registered` event when successful.
		#[allow(clippy::too_many_arguments)]
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::register())]
		pub fn register(
			origin: OriginFor<T>,
			asset_id: Option<T::AssetId>,
			name: Option<Name<T::StringLimit>>,
			asset_type: AssetType,
			existential_deposit: Option<Balance>,
			symbol: Option<Symbol<T::StringLimit>>,
			decimals: Option<u8>,
			location: Option<T::AssetNativeLocation>,
			xcm_rate_limit: Option<Balance>,
			is_sufficient: bool,
		) -> DispatchResult {
			T::RegistryOrigin::ensure_origin(origin)?;

			let details = AssetDetails::new(
				name,
				asset_type,
				existential_deposit.unwrap_or(DEFAULT_ED),
				symbol,
				decimals,
				xcm_rate_limit,
				is_sufficient,
			);

			Self::do_register_asset(asset_id, &details, location)?;
			Ok(())
		}

		/// Update registered asset.
		///
		/// All parameteres are optional and value is not updated if param is `None`.
		///
		/// `decimals` - can be update by `UpdateOrigin` only if it wasn't set yet. Only
		/// `RegistryOrigin` can update `decimals` if it was previously set.
		///
		/// `location` - can be updated only by `RegistryOrigin`.
		///
		/// Emits `Updated` event when successful.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update())]
		pub fn update(
			origin: OriginFor<T>,
			asset_id: T::AssetId,
			name: Option<Name<T::StringLimit>>,
			asset_type: Option<AssetType>,
			existential_deposit: Option<Balance>,
			xcm_rate_limit: Option<Balance>,
			is_sufficient: Option<bool>,
			symbol: Option<Symbol<T::StringLimit>>,
			decimals: Option<u8>,
			location: Option<T::AssetNativeLocation>,
		) -> DispatchResult {
			let is_registry_origin = T::RegistryOrigin::ensure_origin(origin.clone()).is_ok();
			if !is_registry_origin {
				T::UpdateOrigin::ensure_origin(origin)?;
			}

			if let Some(n) = name.as_ref() {
				ensure!(n.len() >= T::MinStringLimit::get() as usize, Error::<T>::TooShort);
			}

			Self::validate_symbol(&symbol)?;

			Assets::<T>::try_mutate(asset_id, |maybe_detail| -> DispatchResult {
				let detail = maybe_detail.as_mut().ok_or(Error::<T>::AssetNotFound)?;

				if let Some(new_name) = name.as_ref() {
					ensure!(Self::asset_ids(new_name).is_none(), Error::<T>::AssetAlreadyRegistered);

					if let Some(old_name) = &detail.name {
						AssetIds::<T>::remove(old_name);
					}

					if Some(new_name.clone()) != detail.name {
						AssetIds::<T>::insert(new_name, asset_id);
					}
				};

				detail.name = name.or_else(|| detail.name.clone());
				detail.asset_type = asset_type.unwrap_or(detail.asset_type);
				detail.existential_deposit = existential_deposit.unwrap_or(detail.existential_deposit);
				detail.xcm_rate_limit = xcm_rate_limit.or(detail.xcm_rate_limit);
				detail.symbol = symbol.or_else(|| detail.symbol.clone());

				let suff = is_sufficient.unwrap_or(detail.is_sufficient);
				if detail.is_sufficient != suff {
					//NOTE: Change sufficient -> insufficient require storage migration and is not
					//allowed by extrinsic.
					ensure!(!detail.is_sufficient, Error::<T>::ForbiddenSufficiencyChange);
					detail.is_sufficient = suff;
				}

				if decimals.is_some() {
					if detail.decimals.is_none() {
						detail.decimals = decimals;
					} else {
						//Only highest origin can change decimal if it was set previously.
						ensure!(is_registry_origin, Error::<T>::Forbidden);
						detail.decimals = decimals;
					};
				}

				if let Some(loc) = location {
					//Only highest origin can update location.
					ensure!(is_registry_origin, Error::<T>::Forbidden);

					if let Some(old_location) = AssetLocations::<T>::take(asset_id) {
						LocationAssets::<T>::remove(&old_location);
					}
					Self::do_set_location(asset_id, loc)?;
				}

				Self::deposit_event(Event::Updated {
					asset_id,
					asset_name: detail.name.clone(),
					asset_type: detail.asset_type,
					existential_deposit: detail.existential_deposit,
					xcm_rate_limit: detail.xcm_rate_limit,
					symbol: detail.symbol.clone(),
					decimals: detail.decimals,
					is_sufficient: detail.is_sufficient,
				});

				Ok(())
			})
		}

		//NOTE: call indices 2 and 3 were used by removed extrinsics.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::register_external().saturating_mul(<T as Config>::RegExternalWeightMultiplier::get()))]
		pub fn register_external(origin: OriginFor<T>, location: T::AssetNativeLocation) -> DispatchResult {
			let _ = ensure_signed(origin)?;

			Self::do_register_asset(
				None,
				&AssetDetails::new(None, AssetType::External, DEFAULT_ED, None, None, None, false),
				Some(location),
			)?;

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::ban_asset())]
		pub fn ban_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			ensure!(Assets::<T>::contains_key(asset_id), Error::<T>::AssetNotFound);

			ensure!(
				!BannedAssets::<T>::contains_key(asset_id),
				Error::<T>::AssetAlreadyBanned
			);

			BannedAssets::<T>::insert(asset_id, ());

			Self::deposit_event(Event::AssetBanned { asset_id });
			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::unban_asset())]
		pub fn unban_asset(origin: OriginFor<T>, asset_id: T::AssetId) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			ensure!(BannedAssets::<T>::contains_key(asset_id), Error::<T>::AssetNotBanned);

			BannedAssets::<T>::remove(asset_id);

			Self::deposit_event(Event::AssetUnbanned { asset_id });
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	fn validate_symbol(symbol: &Option<Symbol<T::StringLimit>>) -> Result<(), DispatchError> {
		if let Some(s) = symbol.clone() {
			ensure!(s.len() >= T::MinStringLimit::get() as usize, Error::<T>::TooShort);

			ensure!(
				s.into_inner().iter().all(|c| !char::is_whitespace(*c as char)),
				Error::<T>::InvalidSymbol
			);
		}
		Ok(())
	}

	pub fn next_asset_id() -> Option<T::AssetId> {
		NextAssetId::<T>::get().checked_add(&T::SequentialIdStartAt::get())
	}

	fn do_set_location(asset_id: T::AssetId, location: T::AssetNativeLocation) -> Result<(), DispatchError> {
		ensure!(
			Self::location_assets(&location).is_none(),
			Error::<T>::LocationAlreadyRegistered
		);

		AssetLocations::<T>::insert(asset_id, &location);
		LocationAssets::<T>::insert(&location, asset_id);

		Self::deposit_event(Event::LocationSet { asset_id, location });

		Ok(())
	}

	#[require_transactional]
	fn do_register_asset(
		selected_asset_id: Option<T::AssetId>,
		details: &AssetDetails<T::StringLimit>,
		location: Option<T::AssetNativeLocation>,
	) -> Result<T::AssetId, DispatchError> {
		Self::validate_symbol(&details.symbol)?;

		let asset_id = if let Some(id) = selected_asset_id {
			ensure!(id < T::SequentialIdStartAt::get(), Error::<T>::NotInReservedRange);

			ensure!(!Assets::<T>::contains_key(id), Error::<T>::AssetAlreadyRegistered);

			id
		} else {
			NextAssetId::<T>::mutate(|value| -> Result<T::AssetId, DispatchError> {
				let next_asset_id = *value;
				*value = next_asset_id
					.checked_add(&T::AssetId::from(1))
					.ok_or(Error::<T>::NoIdAvailable)?;

				Ok(next_asset_id
					.checked_add(&T::SequentialIdStartAt::get())
					.ok_or(Error::<T>::NoIdAvailable)?)
			})?
		};

		Assets::<T>::insert(asset_id, details);
		if let Some(name) = details.name.as_ref() {
			ensure!(name.len() >= T::MinStringLimit::get() as usize, Error::<T>::TooShort);
			ensure!(!AssetIds::<T>::contains_key(name), Error::<T>::AssetAlreadyRegistered);
			AssetIds::<T>::insert(name, asset_id);
		}

		if let Some(loc) = location {
			Self::do_set_location(asset_id, loc)?;
		}

		Self::deposit_event(Event::Registered {
			asset_id,
			asset_name: details.name.clone(),
			asset_type: details.asset_type,
			existential_deposit: details.existential_deposit,
			xcm_rate_limit: details.xcm_rate_limit,
			symbol: details.symbol.clone(),
			decimals: details.decimals,
			is_sufficient: details.is_sufficient,
		});

		Ok(asset_id)
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

// Return Existential deposit of an asset
impl<T: Config> GetByKey<T::AssetId, Balance> for Pallet<T> {
	fn get(k: &T::AssetId) -> Balance {
		if let Some(details) = Self::assets(k) {
			details.existential_deposit
		} else {
			// Asset does not exist - not supported
			Balance::max_value()
		}
	}
}

/// Allows querying the XCM rate limit for an asset by its id.
pub struct XcmRateLimitsInRegistry<T>(PhantomData<T>);
/// Allows querying the XCM rate limit for an asset by its id.
/// Both a unknown asset and an unset rate limit will return `None`.
impl<T: Config> GetByKey<T::AssetId, Option<Balance>> for XcmRateLimitsInRegistry<T> {
	fn get(k: &T::AssetId) -> Option<Balance> {
		Pallet::<T>::assets(k).and_then(|details| details.xcm_rate_limit)
	}
}

impl<T: Config> Inspect for Pallet<T> {
	type AssetId = T::AssetId;
	type Location = T::AssetNativeLocation;

	fn is_sufficient(id: Self::AssetId) -> bool {
		match Self::assets(id) {
			Some(a) => a.is_sufficient,
			None => false,
		}
	}

	fn exists(id: Self::AssetId) -> bool {
		Assets::<T>::contains_key(id)
	}

	fn decimals(id: Self::AssetId) -> Option<u8> {
		Self::assets(id).and_then(|a| a.decimals)
	}

	fn asset_type(id: Self::AssetId) -> Option<AssetKind> {
		Self::assets(id).map(|a| a.asset_type.into())
	}

	fn is_banned(id: Self::AssetId) -> bool {
		BannedAssets::<T>::contains_key(id)
	}

	fn asset_name(id: Self::AssetId) -> Option<Vec<u8>> {
		Self::assets(id).and_then(|a| a.name.map(|v| v.into()))
	}

	fn asset_symbol(id: Self::AssetId) -> Option<Vec<u8>> {
		Self::assets(id).and_then(|a| a.symbol.map(|v| v.into()))
	}

	fn existential_deposit(id: Self::AssetId) -> Option<u128> {
		Self::assets(id).map(|a| a.existential_deposit)
	}
}

impl<T: Config> Mutate<Balance> for Pallet<T> {
	type Error = DispatchError;

	fn set_location(asset_id: Self::AssetId, location: T::AssetNativeLocation) -> Result<(), Self::Error> {
		ensure!(Self::exists(asset_id), Error::<T>::AssetNotFound);

		Self::do_set_location(asset_id, location)
	}
}

impl<T: Config> Create<Balance> for Pallet<T> {
	type Error = DispatchError;
	type Name = Name<T::StringLimit>;
	type Symbol = Symbol<T::StringLimit>;

	fn register_asset(
		asset_id: Option<Self::AssetId>,
		name: Option<Self::Name>,
		kind: AssetKind,
		existential_deposit: Option<Balance>,
		symbol: Option<Self::Symbol>,
		decimals: Option<u8>,
		location: Option<Self::Location>,
		xcm_rate_limit: Option<Balance>,
		is_sufficient: bool,
	) -> Result<Self::AssetId, Self::Error> {
		let details = AssetDetails::new(
			name,
			kind.into(),
			existential_deposit.unwrap_or(DEFAULT_ED),
			symbol,
			decimals,
			xcm_rate_limit,
			is_sufficient,
		);

		Self::do_register_asset(asset_id, &details, location)
	}

	fn get_or_register_asset(
		name: Self::Name,
		kind: AssetKind,
		existential_deposit: Option<Balance>,
		symbol: Option<Self::Symbol>,
		decimals: Option<u8>,
		location: Option<Self::Location>,
		xcm_rate_limit: Option<Balance>,
		is_sufficient: bool,
	) -> Result<Self::AssetId, Self::Error> {
		//NOTE: in this case `try_into_bounded()` should never return None.
		match Self::asset_ids(&name) {
			Some(id) => Ok(id),
			None => {
				let details = AssetDetails::new(
					Some(name),
					kind.into(),
					existential_deposit.unwrap_or(DEFAULT_ED),
					symbol,
					decimals,
					xcm_rate_limit,
					is_sufficient,
				);

				Self::do_register_asset(None, &details, location)
			}
		}
	}
}

impl<T> BoundErc20 for Pallet<T>
where
	T: Config,
	T::AssetNativeLocation: Into<MultiLocation>,
{
	fn contract_address(id: Self::AssetId) -> Option<EvmAddress> {
		if Self::asset_type(id)? == AssetKind::Erc20 {
			let location: MultiLocation = Self::asset_to_location(id).unwrap_or_default().into();
			if let X1(AccountKey20 { key, .. }) = location.interior {
				Some(key.into())
			} else {
				Some(Default::default())
			}
		} else {
			None
		}
	}
}

/// Oracle whitelist based on the asset sufficiency.
pub struct OracleWhitelist<T>(PhantomData<T>);
impl<T: Config> Contains<(hydradx_traits::Source, <T as Config>::AssetId, <T as Config>::AssetId)>
	for OracleWhitelist<T>
{
	fn contains(t: &(hydradx_traits::Source, <T as Config>::AssetId, <T as Config>::AssetId)) -> bool {
		Pallet::<T>::is_sufficient(t.1) && Pallet::<T>::is_sufficient(t.2)
	}
}
