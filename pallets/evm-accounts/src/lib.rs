// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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

//! # EVM accounts pallet
//!
//! ## Terminology
//!
//! * **Truncated address:** * A substrate address created from an EVM address by prefixing it with "ETH\0" and appending with eight 0 bytes.
//! * **Full Substrate address:** * Original 32 bytes long native address (not a truncated address).
//! * **EVM address:** * First 20 bytes of a Substrate address.
//!
//! ## Overview
//!
//! The pallet allows users to bind their Substrate account to the EVM address.
//! The purpose of this pallet is to make interaction with the EVM easier.
//! Binding an address is not necessary for interacting with the EVM.
//!
//! Without binding, we are unable to get the original Substrate address from the EVM address inside
//! of the EVM. Inside of the EVM, we have access only to the EVM address (first 20 bytes of a Substrate account).
//! In this case we create and use a truncated version of the original Substrate address that called the EVM.
//! The original and truncated address are two different Substrate addresses.
//!
//! With binding, we store the last 12 bytes of the Substrate address. Then we can get the original
//! Substrate address by concatenating these 12 bytes stored in the storage to the EVM address.
//!
//! ### Dispatchable Functions
//!
//! * `bind_evm_address` - Binds a Substrate address to EVM address.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::ensure;
use frame_support::pallet_prelude::{DispatchResult, Get};
use sp_core::{
	crypto::{AccountId32, ByteArray},
	H160, U256,
};

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod benchmarking;
pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

pub type Balance = u128;
pub type EvmAddress = H160;
pub type AccountIdLast12Bytes = [u8; 12];

pub trait EvmNonceProvider {
	fn get_nonce(evm_address: H160) -> U256;
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// EVM nonce provider.
		type EvmNonceProvider: EvmNonceProvider;

		/// Fee multiplier for the binding of addresses.
		#[pallet::constant]
		type FeeMultiplier: Get<u32>;

		/// Weight information for extrinsic in this pallet.
		type WeightInfo: WeightInfo;
	}

	/// Maps an EVM address to the last 12 bytes of a substrate account.
	#[pallet::storage]
	#[pallet::getter(fn account)]
	pub(super) type AccountExtension<T: Config> = StorageMap<_, Blake2_128Concat, EvmAddress, AccountIdLast12Bytes>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Binding was created.
		Bound { account: T::AccountId, address: EvmAddress },
	}

	#[pallet::error]
	#[cfg_attr(test, derive(PartialEq, Eq))]
	pub enum Error<T> {
		/// EVM Account's nonce is not zero
		TruncatedAccountAlreadyUsed,
		/// Address is already bound
		AddressAlreadyBound,
		/// Bound address cannot be used
		BoundAddressCannotBeUsed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		T::AccountId: frame_support::traits::IsType<AccountId32>,
	{
		fn integrity_test() {
			// implementation of this pallet expects that EvmAddress is 20 bytes and AccountId is 32 bytes long.
			// If this is not true, `copy_from_slice` might panic.
			assert_eq!(
				EvmAddress::len_bytes(),
				20,
				"EVM Address is expected to be 20 bytes long."
			);
			assert_eq!(AccountId32::LEN, 32, "AccountId is expected to be 32 bytes long.");
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + frame_support::traits::IsType<AccountId32>,
	{
		/// Binds a Substrate address to EVM address.
		/// After binding, the EVM is able to convert an EVM address to the original Substrate address.
		/// Without binding, the EVM converts an EVM address to a truncated Substrate address, which doesn't correspond
		/// to the origin address.
		///
		/// Binding an address is not necessary for interacting with the EVM.
		///
		/// Parameters:
		/// - `origin`: Substrate account binding an address
		///
		/// Emits `EvmAccountBound` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::bind_evm_address().saturating_mul(<T as Config>::FeeMultiplier::get() as u64))]
		pub fn bind_evm_address(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let evm_address = Self::evm_address(&who);

			// This check is not necessary. It prevents binding the same address multiple times.
			// Without this check binding the address second time can have pass or fail, depending
			// on the nonce. So it's better to prevent any confusion and throw an error when address is
			// already bound.
			ensure!(
				!AccountExtension::<T>::contains_key(evm_address),
				Error::<T>::AddressAlreadyBound
			);

			let nonce = T::EvmNonceProvider::get_nonce(evm_address);
			ensure!(nonce.is_zero(), Error::<T>::TruncatedAccountAlreadyUsed);

			let mut last_12_bytes: [u8; 12] = [0; 12];
			last_12_bytes.copy_from_slice(&who.as_ref()[20..32]);

			<AccountExtension<T>>::insert(evm_address, last_12_bytes);

			Self::deposit_event(Event::Bound {
				account: who,
				address: evm_address,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T>
where
	T::AccountId: frame_support::traits::IsType<AccountId32>,
{
	/// get the EVM address from the substrate address.
	pub fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress {
		let acc = account_id.as_ref();
		EvmAddress::from_slice(&acc[..20])
	}

	/// Get the truncated address from the EVM address.
	pub fn truncated_account_id(evm_address: EvmAddress) -> T::AccountId {
		let mut data: [u8; 32] = [0u8; 32];
		data[0..4].copy_from_slice(b"ETH\0");
		data[4..24].copy_from_slice(&evm_address[..]);
		AccountId32::from(data).into()
	}

	/// Return the Substrate address bound to the EVM account. If not bound, returns `None`.
	pub fn bound_account_id(evm_address: EvmAddress) -> Option<T::AccountId> {
		let Some(last_12_bytes) = AccountExtension::<T>::get(evm_address) else {
			return None;
		};
		let mut data: [u8; 32] = [0u8; 32];
		data[..20].copy_from_slice(evm_address.0.as_ref());
		data[20..32].copy_from_slice(&last_12_bytes);
		Some(AccountId32::from(data).into())
	}

	/// Get the Substrate address from the EVM address.
	/// Returns the truncated version of the address if the address wasn't bind.
	pub fn account_id(evm_address: EvmAddress) -> T::AccountId {
		Self::bound_account_id(evm_address).unwrap_or_else(|| Self::truncated_account_id(evm_address))
	}
}
