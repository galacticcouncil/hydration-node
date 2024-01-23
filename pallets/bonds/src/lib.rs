// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Bonds pallet
//!
//! ## Overview
//!
//! This pallet provides functionality to issue fungible bonds.
//! Once the bonds are mature, they can be redeemed for the underlying asset.
//! The pallet uses `Time` trait to get the timestamp of the last block, normally provided by the timestamp pallet.
//!
//! ## Issuing of new bonds
//!
//! * When issuing new bonds, new nameless asset of the `AssetType::Bond` type is registered for the bonds.
//! * New amount of bonds is issued when the underlying asset and maturity matches already registered bonds.
//! * It's possible to create multiple bonds for the same underlying asset.
//! * Bonds can be issued for all available asset types permitted by `AssetTypeWhitelist`.
//! * The existential deposit of the bonds is the same as of the underlying asset.
//! * A user receives the same amount of bonds as the amount of the underlying asset he provided, minus the protocol fee.
//! * Maturity of bonds is represented using the Unix time in milliseconds.
//! * Underlying assets are stored in the pallet account until redeemed.
//! * Protocol fee is applied to the amount of the underlying asset and transferred to the fee receiver.
//! * It's possible to issue new bonds for bonds that are already mature.
//!
//! ## Redeeming of new bonds
//! * Bonds can be both partially or fully redeemed.
//! * The amount of the underlying asset an account receives is 1:1 to the `amount` of the bonds redeemed.
//! * Anyone who holds the bonds is able to redeem them.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	ensure,
	pallet_prelude::{DispatchResult, Get},
	sp_runtime::{
		traits::{AccountIdConversion, AtLeast32BitUnsigned, CheckedAdd, CheckedSub},
		DispatchError, Permill, Saturating,
	},
	traits::{Contains, Time},
	PalletId,
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use sp_core::MaxEncodedLen;
use sp_std::vec::Vec;

use hydradx_traits::{AssetKind, CreateRegistry, Registry};
use orml_traits::{GetByKey, MultiCurrency};
use primitives::{AssetId, Moment};

#[cfg(test)]
mod tests;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarks;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Balance type.
		type Balance: Parameter
			+ Member
			+ Copy
			+ PartialOrd
			+ MaybeSerializeDeserialize
			+ Default
			+ CheckedAdd
			+ CheckedSub
			+ AtLeast32BitUnsigned
			+ MaxEncodedLen
			+ From<u128>;

		/// Multi currency mechanism.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetId, Balance = Self::Balance>;

		/// Asset Registry mechanism - used to register bonds in the asset registry.
		type AssetRegistry: Registry<AssetId, Vec<u8>, Self::Balance, DispatchError>
			+ CreateRegistry<AssetId, Self::Balance, Error = DispatchError>;

		/// Provider for existential deposits of assets.
		type ExistentialDeposits: GetByKey<AssetId, Self::Balance>;

		/// Provider for the current timestamp.
		type TimestampProvider: Time<Moment = Moment>;

		/// The pallet id, used for deriving its sovereign account ID.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// The origin which can issue new bonds.
		type IssueOrigin: EnsureOrigin<Self::RuntimeOrigin, Success = Self::AccountId>;

		/// Asset types that are permitted to be used as underlying assets.
		type AssetTypeWhitelist: Contains<AssetKind>;

		/// Protocol fee.
		#[pallet::constant]
		type ProtocolFee: Get<Permill>;

		/// Protocol fee receiver.
		#[pallet::constant]
		type FeeReceiver: Get<Self::AccountId>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::storage]
	/// Registered bond ids.
	/// Maps (underlying asset ID, maturity) -> bond ID
	#[pallet::getter(fn bond_id)]
	pub(super) type BondIds<T: Config> = StorageMap<_, Blake2_128Concat, (AssetId, Moment), AssetId>;

	#[pallet::storage]
	/// Registered bonds.
	/// Maps bond ID -> (underlying asset ID, maturity)
	#[pallet::getter(fn bond)]
	pub(super) type Bonds<T: Config> = StorageMap<_, Blake2_128Concat, AssetId, (AssetId, Moment)>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A bond asset was registered
		TokenCreated {
			issuer: T::AccountId,
			asset_id: AssetId,
			bond_id: AssetId,
			maturity: Moment,
		},
		/// New bond were issued
		Issued {
			issuer: T::AccountId,
			bond_id: AssetId,
			amount: T::Balance,
			fee: T::Balance,
		},
		/// Bonds were redeemed
		Redeemed {
			who: T::AccountId,
			bond_id: AssetId,
			amount: T::Balance,
		},
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// Bond not registered
		NotRegistered,
		/// Bond is not mature
		NotMature,
		/// Maturity not long enough
		InvalidMaturity,
		/// Asset type not allowed for underlying asset
		DisallowedAsset,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Issue new fungible bonds.
		/// New asset id is registered and assigned to the bonds.
		/// The number of bonds the issuer receives is 1:1 to the `amount` of the underlying asset
		/// minus the protocol fee.
		/// The bond asset is registered with the empty string for the asset name,
		/// and with the same existential deposit as of the underlying asset.
		/// Bonds can be redeemed for the underlying asset once mature.
		/// Protocol fee is applied to the amount, and transferred to `T::FeeReceiver`.
		/// When issuing new bonds with the underlying asset and maturity that matches existing bonds,
		/// new amount of these existing bonds is issued, instead of registering new bonds.
		/// It's possible to issue new bonds for bonds that are already mature.
		///
		/// Parameters:
		/// - `origin`: issuer of new bonds, needs to be `T::IssueOrigin`
		/// - `asset_id`: underlying asset id
		/// - `amount`: the amount of the underlying asset
		/// - `maturity`: Unix time in milliseconds, when the bonds will be mature.
		///
		/// Emits `BondTokenCreated` event when successful and new bonds were registered.
		/// Emits `BondsIssued` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::issue())]
		pub fn issue(origin: OriginFor<T>, asset_id: AssetId, amount: T::Balance, maturity: Moment) -> DispatchResult {
			let who = T::IssueOrigin::ensure_origin(origin)?;

			ensure!(
				T::AssetTypeWhitelist::contains(&T::AssetRegistry::retrieve_asset_type(asset_id)?),
				Error::<T>::DisallowedAsset
			);

			let fee = T::ProtocolFee::get().mul_ceil(amount);
			let amount_without_fee = amount.saturating_sub(fee);
			let pallet_account = Self::pallet_account_id();

			let bond_id = match BondIds::<T>::get((asset_id, maturity)) {
				Some(bond_id) => bond_id,
				None => {
					// register new bonds
					ensure!(maturity >= T::TimestampProvider::now(), Error::<T>::InvalidMaturity);

					let ed = T::ExistentialDeposits::get(&asset_id);

					let bond_id = <T::AssetRegistry as CreateRegistry<AssetId, T::Balance>>::create_asset(
						&Self::bond_name(asset_id, maturity),
						AssetKind::Bond,
						ed,
					)?;

					Bonds::<T>::insert(bond_id, (asset_id, maturity));
					BondIds::<T>::insert((asset_id, maturity), bond_id);

					Self::deposit_event(Event::TokenCreated {
						issuer: who.clone(),
						asset_id,
						bond_id,
						maturity,
					});

					bond_id
				}
			};

			T::Currency::transfer(asset_id, &who, &pallet_account, amount_without_fee)?;
			T::Currency::transfer(asset_id, &who, &T::FeeReceiver::get(), fee)?;
			T::Currency::deposit(bond_id, &who, amount_without_fee)?;

			Self::deposit_event(Event::Issued {
				issuer: who,
				bond_id,
				amount: amount_without_fee,
				fee,
			});

			Ok(())
		}

		/// Redeem bonds for the underlying asset.
		/// The amount of the underlying asset the `origin` receives is 1:1 to the `amount` of the bonds.
		/// Anyone who holds the bonds is able to redeem them.
		/// Bonds can be both partially or fully redeemed.
		///
		/// Parameters:
		/// - `origin`: account id
		/// - `asset_id`: bond asset id
		/// - `amount`: the amount of the bonds to redeem for the underlying asset
		///
		/// Emits `BondsRedeemed` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::redeem())]
		pub fn redeem(origin: OriginFor<T>, bond_id: AssetId, amount: T::Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let (underlying_asset_id, maturity) = Self::bond(bond_id).ok_or(Error::<T>::NotRegistered)?;

			let now = T::TimestampProvider::now();
			ensure!(now >= maturity, Error::<T>::NotMature);

			T::Currency::withdraw(bond_id, &who, amount)?;

			let pallet_account = Self::pallet_account_id();
			T::Currency::transfer(underlying_asset_id, &pallet_account, &who, amount)?;

			Self::deposit_event(Event::Redeemed { who, bond_id, amount });

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	/// The account ID of the bonds pallet.
	///
	/// This actually does computation. If you need to keep using it, then make sure you cache the
	/// value and only call this once.
	pub fn pallet_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Return bond token name
	pub fn bond_name(asset_id: AssetId, when: Moment) -> Vec<u8> {
		let mut buf: Vec<u8> = Vec::new();

		buf.extend_from_slice(&asset_id.to_le_bytes());
		buf.extend_from_slice(b".");
		buf.extend_from_slice(&when.to_le_bytes());

		buf
	}
}
