// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]

use codec::HasCompact;
use frame_support::{
	dispatch::DispatchResult,
	ensure,
	traits::{tokens::nonfungibles::*, Get},
	BoundedVec,
};
use frame_system::{ensure_signed, pallet_prelude::BlockNumberFor};
use pallet_uniques::DestroyWitness;

use hydradx_traits::nft::{CreateTypedCollection, ReserveCollectionId};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, StaticLookup, Zero},
	DispatchError,
};
use sp_std::boxed::Box;
pub use types::*;
use weights::WeightInfo;

mod benchmarking;
pub mod migration;
pub mod types;
pub mod weights;

#[cfg(test)]
pub mod mock;

#[cfg(test)]
mod tests;

pub type BoundedVecOfUnq<T> = BoundedVec<u8, <T as pallet_uniques::Config>::StringLimit>;
type CollectionInfoOf<T> = CollectionInfo<<T as Config>::CollectionType, BoundedVecOfUnq<T>>;
pub type ItemInfoOf<T> = ItemInfo<BoundedVec<u8, <T as pallet_uniques::Config>::StringLimit>>;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {

	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_uniques::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type WeightInfo: WeightInfo;
		type NftCollectionId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ AtLeast32BitUnsigned
			+ Into<Self::CollectionId>
			+ From<Self::CollectionId>
			+ MaxEncodedLen;
		type NftItemId: Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ AtLeast32BitUnsigned
			+ Into<Self::ItemId>
			+ From<Self::ItemId>
			+ MaxEncodedLen;
		type CollectionType: Member + Parameter + Default + Copy + MaxEncodedLen;
		type Permissions: NftPermission<Self::CollectionType>;
		/// Collection IDs reserved for runtime up to the following constant
		#[pallet::constant]
		type ReserveCollectionIdUpTo: Get<Self::NftCollectionId>;
	}

	#[pallet::storage]
	#[pallet::getter(fn collections)]
	/// Stores collection info
	pub type Collections<T: Config> = StorageMap<_, Twox64Concat, T::NftCollectionId, CollectionInfoOf<T>>;

	#[pallet::storage]
	#[pallet::getter(fn items)]
	/// Stores item info
	pub type Items<T: Config> =
		StorageDoubleMap<_, Twox64Concat, T::NftCollectionId, Twox64Concat, T::NftItemId, ItemInfoOf<T>>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Creates an NFT collection of the given collection type and sets its metadata.
		/// The collection ID needs to be outside of the range of reserved IDs.
		/// The creation of a collection needs to be enabled in the permissions
		/// for the given collection type.
		///
		/// Parameters:
		/// - `origin`: The owner of the newly created collection.
		/// - `collection_id`: Identifier of a collection.
		/// - `collection_type`: The collection type determines its purpose and usage.
		/// - `metadata`: Arbitrary data about a collection, e.g. IPFS hash or name.
		///
		/// Emits CollectionCreated event
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::create_collection())]
		pub fn create_collection(
			origin: OriginFor<T>,
			collection_id: T::NftCollectionId,
			collection_type: T::CollectionType,
			metadata: BoundedVecOfUnq<T>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			ensure!(!Self::is_id_reserved(collection_id), Error::<T>::IdReserved);
			ensure!(T::Permissions::can_create(&collection_type), Error::<T>::NotPermitted);

			Self::do_create_collection(sender, collection_id, collection_type, metadata)?;

			Ok(())
		}

		/// Mints an NFT in the specified collection and sets its metadata.
		/// Minting of new items needs to be enabled in the permissions
		/// for the given collection type.
		///
		/// Parameters:
		/// - `origin`: The owner of the newly minted NFT.
		/// - `collection_id`: The collection of the asset to be minted.
		/// - `item_id`: The item of the asset to be minted.
		/// - `metadata`: Arbitrary data about an item, e.g. IPFS hash or symbol.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::mint())]
		pub fn mint(
			origin: OriginFor<T>,
			collection_id: T::NftCollectionId,
			item_id: T::NftItemId,
			metadata: BoundedVecOfUnq<T>,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collection_type = Self::collections(collection_id)
				.map(|c| c.collection_type)
				.ok_or(Error::<T>::CollectionUnknown)?;

			ensure!(T::Permissions::can_mint(&collection_type), Error::<T>::NotPermitted);

			let collection_owner = Self::collection_owner(&collection_id).ok_or(Error::<T>::CollectionUnknown)?;
			ensure!(collection_owner == sender, Error::<T>::NotPermitted);

			Self::do_mint(sender, collection_id, item_id, metadata)?;

			Ok(())
		}

		/// Transfers NFT from account A to account B.
		/// Transfers need to be enabled in the permissions for the given collection type.
		///
		/// Parameters:
		/// - `origin`: The NFT owner
		/// - `collection_id`: The collection of the asset to be transferred.
		/// - `item_id`: The instance of the asset to be transferred.
		/// - `dest`: The account to receive ownership of the asset.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::transfer())]
		pub fn transfer(
			origin: OriginFor<T>,
			collection_id: T::NftCollectionId,
			item_id: T::NftItemId,
			dest: <T::Lookup as StaticLookup>::Source,
		) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let dest = T::Lookup::lookup(dest)?;

			let collection_type = Self::collections(collection_id)
				.map(|c| c.collection_type)
				.ok_or(Error::<T>::CollectionUnknown)?;

			ensure!(T::Permissions::can_transfer(&collection_type), Error::<T>::NotPermitted);

			Self::do_transfer(collection_id, item_id, sender, dest)?;

			Ok(())
		}

		/// Removes a token from existence.
		/// Burning needs to be enabled in the permissions for the given collection type.
		///
		/// Parameters:
		/// - `origin`: The NFT owner.
		/// - `collection_id`: The collection of the asset to be burned.
		/// - `item_id`: The instance of the asset to be burned.
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::burn())]
		pub fn burn(origin: OriginFor<T>, collection_id: T::NftCollectionId, item_id: T::NftItemId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collection_type = Self::collections(collection_id)
				.map(|c| c.collection_type)
				.ok_or(Error::<T>::CollectionUnknown)?;

			ensure!(T::Permissions::can_burn(&collection_type), Error::<T>::NotPermitted);

			Self::do_burn(sender, collection_id, item_id)?;

			Ok(())
		}

		/// Removes a collection from existence.
		/// Destroying of collections need to be enabled in the permissions
		/// for the given collection type.
		/// Fails if the collection is not empty.
		///
		/// Parameters:
		/// - `origin`: The collection owner.
		/// - `collection_id`: The identifier of the asset collection to be destroyed.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::destroy_collection())]
		pub fn destroy_collection(origin: OriginFor<T>, collection_id: T::NftCollectionId) -> DispatchResult {
			let sender = ensure_signed(origin)?;

			let collection_type = Self::collections(collection_id)
				.map(|c| c.collection_type)
				.ok_or(Error::<T>::CollectionUnknown)?;

			ensure!(T::Permissions::can_destroy(&collection_type), Error::<T>::NotPermitted);

			Self::do_destroy_collection(sender, collection_id)?;

			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A collection was created
		CollectionCreated {
			owner: T::AccountId,
			collection_id: T::NftCollectionId,
			collection_type: T::CollectionType,
			metadata: BoundedVecOfUnq<T>,
		},
		/// An item was minted
		ItemMinted {
			owner: T::AccountId,
			collection_id: T::NftCollectionId,
			item_id: T::NftItemId,
			metadata: BoundedVecOfUnq<T>,
		},
		/// An item was transferred
		ItemTransferred {
			from: T::AccountId,
			to: T::AccountId,
			collection_id: T::NftCollectionId,
			item_id: T::NftItemId,
		},
		/// An item was burned
		ItemBurned {
			owner: T::AccountId,
			collection_id: T::NftCollectionId,
			item_id: T::NftItemId,
		},
		/// A collection was destroyed
		CollectionDestroyed {
			owner: T::AccountId,
			collection_id: T::NftCollectionId,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Count of items overflown
		NoAvailableItemId,
		/// Count of collections overflown
		NoAvailableCollectionId,
		/// Collection still contains minted tokens
		TokenCollectionNotEmpty,
		/// Collection does not exist
		CollectionUnknown,
		/// Item does not exist
		ItemUnknown,
		/// Operation not permitted
		NotPermitted,
		/// ID reserved for runtime
		IdReserved,
	}
}

impl<T: Config> Pallet<T> {
	fn do_create_collection(
		owner: T::AccountId,
		collection_id: T::NftCollectionId,
		collection_type: T::CollectionType,
		metadata: BoundedVecOfUnq<T>,
	) -> DispatchResult {
		let deposit_info = match T::Permissions::has_deposit(&collection_type) {
			false => (Zero::zero(), true),
			true => (T::CollectionDeposit::get(), false),
		};

		pallet_uniques::Pallet::<T>::do_create_collection(
			collection_id.into(),
			owner.clone(),
			owner.clone(),
			deposit_info.0,
			deposit_info.1,
			pallet_uniques::Event::Created {
				collection: collection_id.into(),
				creator: owner.clone(),
				owner: owner.clone(),
			},
		)?;

		Collections::<T>::insert(
			collection_id,
			CollectionInfo {
				collection_type,
				metadata: metadata.clone(),
			},
		);

		Self::deposit_event(Event::CollectionCreated {
			owner,
			collection_id,
			collection_type,
			metadata,
		});

		Ok(())
	}

	fn do_mint(
		owner: T::AccountId,
		collection_id: T::NftCollectionId,
		item_id: T::NftItemId,
		metadata: BoundedVecOfUnq<T>,
	) -> DispatchResult {
		ensure!(
			Collections::<T>::contains_key(collection_id),
			Error::<T>::CollectionUnknown
		);

		pallet_uniques::Pallet::<T>::do_mint(collection_id.into(), item_id.into(), owner.clone(), |_details| Ok(()))?;

		Items::<T>::insert(
			collection_id,
			item_id,
			ItemInfo {
				metadata: metadata.clone(),
			},
		);

		Self::deposit_event(Event::ItemMinted {
			owner,
			collection_id,
			item_id,
			metadata,
		});

		Ok(())
	}

	/// Transfer NFT from account `from` to `to`.
	/// Fails if `from` is not the NFT owner.
	///
	/// Is a no-op if `from` is the same as `to`.
	fn do_transfer(
		collection_id: T::NftCollectionId,
		item_id: T::NftItemId,
		from: T::AccountId,
		to: T::AccountId,
	) -> DispatchResult {
		if from == to {
			return Ok(());
		}

		let owner = Self::owner(&collection_id, &item_id).ok_or(Error::<T>::ItemUnknown)?;
		ensure!(owner == from, Error::<T>::NotPermitted);

		pallet_uniques::Pallet::<T>::do_transfer(
			collection_id.into(),
			item_id.into(),
			to.clone(),
			|_collection_details, _item_details| {
				Self::deposit_event(Event::ItemTransferred {
					from,
					to,
					collection_id,
					item_id,
				});
				Ok(())
			},
		)
	}

	fn do_burn(owner: T::AccountId, collection_id: T::NftCollectionId, item_id: T::NftItemId) -> DispatchResult {
		let item_owner = Self::owner(&collection_id, &item_id).ok_or(Error::<T>::ItemUnknown)?;
		ensure!(owner == item_owner, Error::<T>::NotPermitted);

		pallet_uniques::Pallet::<T>::do_burn(
			collection_id.into(),
			item_id.into(),
			|_collection_details, _item_details| Ok(()),
		)?;

		Items::<T>::remove(collection_id, item_id);

		Self::deposit_event(Event::ItemBurned {
			owner,
			collection_id,
			item_id,
		});

		Ok(())
	}

	fn do_destroy_collection(
		owner: T::AccountId,
		collection_id: T::NftCollectionId,
	) -> Result<DestroyWitness, DispatchError> {
		let witness = Self::get_destroy_witness(&collection_id).ok_or(Error::<T>::CollectionUnknown)?;

		// witness struct is empty because we don't allow destroying a collection with existing items
		ensure!(witness.items == 0u32, Error::<T>::TokenCollectionNotEmpty);

		let witness =
			pallet_uniques::Pallet::<T>::do_destroy_collection(collection_id.into(), witness, Some(owner.clone()))?;
		Collections::<T>::remove(collection_id);

		Self::deposit_event(Event::CollectionDestroyed { owner, collection_id });
		Ok(witness)
	}
}

impl<T: Config> Inspect<T::AccountId> for Pallet<T> {
	type ItemId = T::NftItemId;
	type CollectionId = T::NftCollectionId;

	/// Returns the owner of `item` of `collection`, or `None` if the item doesn't exist.
	fn owner(collection: &Self::CollectionId, item: &Self::ItemId) -> Option<T::AccountId> {
		pallet_uniques::Pallet::<T>::owner((*collection).into(), (*item).into())
	}

	/// Returns the owner of the `collection`, or `None` if the collection doesn't exist.
	fn collection_owner(collection: &Self::CollectionId) -> Option<T::AccountId> {
		pallet_uniques::Pallet::<T>::collection_owner((*collection).into())
	}

	/// Returns `true` if the `item` of `collection` may be transferred.
	fn can_transfer(collection: &Self::CollectionId, _item: &Self::ItemId) -> bool {
		let maybe_collection_type = Self::collections(collection).map(|c| c.collection_type);

		match maybe_collection_type {
			Some(collection_type) => T::Permissions::can_transfer(&collection_type),
			_ => false,
		}
	}
}

impl<T: Config> InspectEnumerable<T::AccountId> for Pallet<T> {
	type CollectionsIterator = Box<dyn Iterator<Item = <T as Config>::NftCollectionId>>;
	type ItemsIterator = Box<dyn Iterator<Item = <T as Config>::NftItemId>>;
	type OwnedIterator = Box<dyn Iterator<Item = (<T as Config>::NftCollectionId, <T as Config>::NftItemId)>>;
	type OwnedInCollectionIterator = Box<dyn Iterator<Item = <T as Config>::NftItemId>>;

	/// Returns an iterator of the collections in existence.
	fn collections() -> Self::CollectionsIterator {
		Box::new(Collections::<T>::iter_keys())
	}

	/// Returns an iterator of the items of a `collection` in existence.
	fn items(collection: &Self::CollectionId) -> Self::ItemsIterator {
		Box::new(Items::<T>::iter_key_prefix(collection))
	}

	/// Returns an iterator of the items of all collections owned by `who`.
	fn owned(who: &T::AccountId) -> Self::OwnedIterator {
		Box::new(
			pallet_uniques::Pallet::<T>::owned(who)
				.map(|(collection_id, item_id)| (collection_id.into(), item_id.into())),
		)
	}

	/// Returns an iterator of the items of `collection` owned by `who`.
	fn owned_in_collection(collection: &Self::CollectionId, who: &T::AccountId) -> Self::OwnedInCollectionIterator {
		Box::new(
			pallet_uniques::Pallet::<T>::owned_in_collection(
				&(Into::<<T as pallet_uniques::Config>::CollectionId>::into(*collection)),
				who,
			)
			.map(|i| i.into()),
		)
	}
}

impl<T: Config> Destroy<T::AccountId> for Pallet<T> {
	type DestroyWitness = DestroyWitness;

	/// The witness data needed to destroy an item.
	fn get_destroy_witness(collection: &Self::CollectionId) -> Option<Self::DestroyWitness> {
		pallet_uniques::Pallet::<T>::get_destroy_witness(
			&(Into::<<T as pallet_uniques::Config>::CollectionId>::into(*collection)),
		)
	}

	/// Removes a collection from existence.
	/// Destroying of collections is not enforced by the permissions
	/// for the given collection type.
	/// Fails if the collection is not empty and contains items.
	///
	/// Parameters:
	/// - `collection`: The `CollectionId` to be destroyed.
	/// - `witness`: Empty witness data that needs to be provided to complete the operation
	///   successfully.
	/// - `maybe_check_owner`: An optional account id that can be used to authorize the destroy
	///   command. If not provided, we will not do any authorization checks before destroying the
	///   item.
	///
	/// If successful, this function will return empty witness data from the destroyed item.
	fn destroy(
		collection: Self::CollectionId,
		_witness: Self::DestroyWitness,
		maybe_check_owner: Option<T::AccountId>,
	) -> Result<Self::DestroyWitness, DispatchError> {
		let owner = if let Some(check_owner) = maybe_check_owner {
			check_owner
		} else {
			Self::collection_owner(&collection).ok_or(Error::<T>::CollectionUnknown)?
		};

		Self::do_destroy_collection(owner, collection)
	}
}

impl<T: Config> Mutate<T::AccountId> for Pallet<T> {
	/// Mints an NFT in the specified collection and sets its metadata.
	/// The minting permissions are not enforced.
	/// Metadata is set to the default value.
	///
	/// Parameters:
	/// - `collection`: The collection of the asset to be minted.
	/// - `item`: The item of the asset to be minted.
	/// - `who`: The owner of the newly minted NFT.
	fn mint_into(collection: &Self::CollectionId, item: &Self::ItemId, who: &T::AccountId) -> DispatchResult {
		Self::do_mint(who.clone(), *collection, *item, BoundedVec::default())?;

		Ok(())
	}

	/// Removes an item from existence.
	/// The burning permissions are not enforced.
	///
	/// Parameters:
	/// - `collection`: The collection of the asset to be burned.
	/// - `item`: The instance of the asset to be burned.
	/// - `maybe_check_owner`: Optional value.
	fn burn(
		collection: &Self::CollectionId,
		item: &Self::ItemId,
		maybe_check_owner: Option<&T::AccountId>,
	) -> DispatchResult {
		let owner = if let Some(check_owner) = maybe_check_owner {
			check_owner.clone()
		} else {
			Self::owner(collection, item).ok_or(Error::<T>::ItemUnknown)?
		};

		Self::do_burn(owner, *collection, *item)?;

		Ok(())
	}
}

impl<T: Config> Transfer<T::AccountId> for Pallet<T> {
	/// Transfer `item` of `collection` into `destination` account.
	fn transfer(collection: &Self::CollectionId, item: &Self::ItemId, destination: &T::AccountId) -> DispatchResult {
		let owner = Self::owner(collection, item).ok_or(Error::<T>::ItemUnknown)?;

		Self::do_transfer(*collection, *item, owner, destination.clone())
	}
}

impl<T: Config> CreateTypedCollection<T::AccountId, T::NftCollectionId, T::CollectionType, BoundedVecOfUnq<T>>
	for Pallet<T>
{
	/// Creates an NFT collection of the given collection type and sets its metadata.
	/// The collection ID does not need to be outside of the range of reserved IDs.
	/// The permissions for the creation of a collection are not enforced.
	/// Metadata is set to the default value if not provided.
	///
	/// Parameters:
	/// - `owner`: The collection owner.
	/// - `collection_id`: Identifier of a collection.
	/// - `collection_type`: The collection type.
	/// - `metadata`: Optional arbitrary data about a collection, e.g. IPFS hash or name.
	///
	/// Emits CollectionCreated event
	fn create_typed_collection(
		owner: T::AccountId,
		collection_id: T::NftCollectionId,
		collection_type: T::CollectionType,
		metadata: Option<BoundedVecOfUnq<T>>,
	) -> DispatchResult {
		Self::do_create_collection(owner, collection_id, collection_type, metadata.unwrap_or_default())
	}
}

impl<T: Config> ReserveCollectionId<T::NftCollectionId> for Pallet<T> {
	/// Checks if the provided collection ID is within the range of reserved IDs.
	fn is_id_reserved(id: T::NftCollectionId) -> bool {
		id <= T::ReserveCollectionIdUpTo::get()
	}
}
