// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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
#![allow(clippy::unused_unit)]

use codec::{Decode, Encode};
use frame_support::pallet_prelude::*;
use frame_support::BoundedBTreeMap;
use frame_support::{
	dispatch::{DispatchError, DispatchResult},
	ensure,
	sp_runtime::{
		traits::{DispatchInfoOf, SignedExtension},
		transaction_validity::{InvalidTransaction, TransactionValidity, TransactionValidityError, ValidTransaction},
	},
	traits::{Currency, Get, Imbalance, IsSubType},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use primitives::Balance;
use scale_info::TypeInfo;
use sp_runtime::traits::Saturating;
use sp_runtime::{traits::Zero, ModuleError};
use sp_std::{marker::PhantomData, prelude::*, vec::Vec};
use weights::WeightInfo;

use orml_traits::LockIdentifier;
use orml_traits::MultiCurrency;
use orml_traits::MultiLockableCurrency;
use polkadot_xcm::prelude::*;
use xcm_executor::traits::Convert;
use xcm_executor::traits::TransactAsset;
use xcm_executor::Assets;

mod benchmarking;
mod traits;
pub use traits::*;
pub mod weights;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(Clone, Default, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct AssetVolume {
	asset_in: u128,
	asset_out: u128,
}

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;
use primitives::constants::currency::UNITS;

//TODO: Use spot price provider or existential deposit fall back
pub const MAX_VOLUME_LIMIT: u128 = 10_000 * UNITS;

pub const LOCK_ID: LockIdentifier = *b"XCMlimit";

pub type CurrencyId = u32;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;
	use orml_traits::{MultiCurrency, MultiLockableCurrency};
	use xcm_executor::traits::Convert;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Prefix: Get<&'static [u8]>;

		type WeightInfo: WeightInfo;

		//TODO: do we need reserve or lock?
		type Currency: MultiLockableCurrency<Self::AccountId>;

		type LocationToAccountIdConverter: Convert<MultiLocation, Self::AccountId>;

		type CurrencyIdConverter: Convert<MultiAsset, CurrencyId>;

		type AssetTransactor: TransactAsset;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Event(),
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Error
		TestError,
	}

	/// Asset id storage for each shared token
	#[pallet::storage]
	#[pallet::getter(fn volume)]
	pub type VolumePerAsset<T: Config> = StorageMap<_, Blake2_128Concat, MultiLocation, AssetVolume, ValueQuery>;

	/// TODO: figure out actual max number of btree entries
	#[pallet::storage]
	#[pallet::getter(fn locked_assets)]
	pub type LockedAssets<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, BoundedBTreeMap<CurrencyId, Balance, ConstU32<20>>, ValueQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight((<T as Config>::WeightInfo::claim(), DispatchClass::Normal, Pays::No))]
		pub fn asd(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let sender = ensure_signed(origin)?;

			Ok(().into())
		}
	}
}

impl<T: Config> TransactAsset for Pallet<T>
where
	<<T as pallet::Config>::Currency as MultiCurrency<<T as frame_system::Config>::AccountId>>::CurrencyId: From<u32>,
{
	/// Ensure that `check_in` will result in `Ok`.
	///
	/// When composed as a tuple, all type-items are called and at least one must result in `Ok`.
	fn can_check_in(_origin: &MultiLocation, _what: &MultiAsset) -> XcmResult {
		T::AssetTransactor::can_check_in(_origin, _what)
	}

	fn check_in(_origin: &MultiLocation, _what: &MultiAsset) {
		T::AssetTransactor::check_in(_origin, _what)
	}
	fn check_out(_dest: &MultiLocation, _what: &MultiAsset) {
		T::AssetTransactor::check_out(_dest, _what)
	}

	/// Deposit the `what` asset into the account of `who`.
	///
	/// Implementations should return `XcmError::FailedToTransactAsset` if deposit failed.
	fn deposit_asset(what: &MultiAsset, who: &MultiLocation) -> XcmResult {
		let asset_in_volume = Pallet::<T>::track_volume_in(what);

		let acc = T::LocationToAccountIdConverter::convert_ref(who)
			.map_err(|_| XcmError::FailedToTransactAsset("Failed to convert account id"))?;
		let currency_id = T::CurrencyIdConverter::convert_ref(what)
			.map_err(|_| XcmError::FailedToTransactAsset("Failed to convert currency id"))?;
		let prev_amount = T::Currency::total_balance(currency_id.into(), &acc);
		let res = T::AssetTransactor::deposit_asset(what, who);
		let new_amount = T::Currency::total_balance(currency_id.into(), &acc);

		let account_delta = new_amount
			.saturating_sub(prev_amount)
			.try_into()
			.map_err(|_| XcmError::FailedToTransactAsset("Failed to convert to balance"))?;
		if res.is_ok() && asset_in_volume >= MAX_VOLUME_LIMIT {
			let amount = Pallet::<T>::amount(what);
			let mut locked_assets = LockedAssets::<T>::get(&acc);
			let prev_amount = locked_assets.get(&currency_id).unwrap_or(&0);
			let new_lock_amount = prev_amount.saturating_add(account_delta);
			locked_assets
				.try_insert(currency_id, new_lock_amount)
				.map_err(|_| XcmError::FailedToTransactAsset("Failed to insert locked asset"))?;
			LockedAssets::<T>::insert(&acc, locked_assets);
			let lock_amount = new_lock_amount
				.try_into()
				.map_err(|_| XcmError::FailedToTransactAsset("Failed to convert to balance"))?;
			let id = currency_id.into();

			//TODO: Only lock amount when bigger than the existential deposit
			T::Currency::set_lock(LOCK_ID, id, &acc, lock_amount).map_err(|e| {
				dbg!(e);
				XcmError::FailedToTransactAsset("Failed to set lock")
			})?;
		}

		res
	}

	/// Withdraw the given asset from the consensus system. Return the actual asset(s) withdrawn,
	/// which should always be equal to `_what`.
	///
	/// Implementations should return `XcmError::FailedToTransactAsset` if withdraw failed.
	fn withdraw_asset(what: &MultiAsset, who: &MultiLocation) -> Result<Assets, XcmError> {
		Pallet::<T>::track_volume_out(what);
		T::AssetTransactor::withdraw_asset(what, who)
	}

	/// Move an `asset` `from` one location in `to` another location.
	///
	/// Returns `XcmError::FailedToTransactAsset` if transfer failed.
	///
	/// ## Notes
	/// This function is meant to only be implemented by the type implementing `TransactAsset`, and
	/// not be called directly. Most common API usages will instead call `transfer_asset`, which in
	/// turn has a default implementation that calls `internal_transfer_asset`. As such, **please
	/// do not call this method directly unless you know what you're doing**.
	fn internal_transfer_asset(
		asset: &MultiAsset,
		from: &MultiLocation,
		to: &MultiLocation,
	) -> Result<Assets, XcmError> {
		match (from, to) {
			(
				MultiLocation {
					interior: X1(Parachain(_id)),
					..
				},
				_,
			) => Pallet::<T>::track_volume_in(asset),
			(
				_,
				MultiLocation {
					interior: X1(Parachain(_id)),
					..
				},
			) => Pallet::<T>::track_volume_out(asset),
			_ => todo!(),
		};
		T::AssetTransactor::internal_transfer_asset(asset, from, to)
	}
}

impl<T: Config> Pallet<T> {
	fn amount(asset: &MultiAsset) -> u128 {
		match asset {
			MultiAsset {
				id: Concrete(loc),
				fun: Fungible(amount),
			} => *amount,
			_ => todo!(),
		}
	}

	fn track_volume_in(asset: &MultiAsset) -> u128 {
		match asset {
			MultiAsset {
				id: Concrete(loc),
				fun: Fungible(amount),
			} => VolumePerAsset::<T>::mutate(loc, |volume| {
				volume.asset_in += amount;
				volume.asset_in
			}),
			_ => todo!(),
		}
	}

	fn track_volume_out(asset: &MultiAsset) -> u128 {
		match asset {
			MultiAsset {
				id: Concrete(loc),
				fun: Fungible(amount),
			} => VolumePerAsset::<T>::mutate(loc, |volume| {
				volume.asset_out += amount;
				volume.asset_out
			}),
			_ => todo!(),
		}
	}
}
