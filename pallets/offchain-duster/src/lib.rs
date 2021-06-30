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
#![allow(clippy::upper_case_acronyms)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use orml_traits::GetByKey;

use frame_system::offchain::{CreateSignedTransaction, SendSignedTransaction, Signer};

use frame_support::sp_runtime::offchain::storage_lock::{StorageLock, Time};
use frame_support::sp_runtime::offchain::Duration;
use orml_utilities::OffchainErr;
use sp_core::crypto::KeyTypeId;
use sp_runtime::traits::Saturating;

pub use pallet_duster::Call as DusterCall;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub const OFFCHAIN_WORKER_LOCK: &[u8] = b"hydradx/offchain-duster/lock/";
pub const LOCK_DURATION: u64 = 100;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::log;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::BlockNumberFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		<T as orml_tokens::Config>::Balance: From<<T as pallet_duster::Config>::Balance>,
		<T as pallet_duster::Config>::CurrencyId: From<<T as orml_tokens::Config>::CurrencyId>,
	{
		fn offchain_worker(block_number: <T as frame_system::Config>::BlockNumber) {
			if let Err(e) = Self::_offchain_worker() {
				log::info!(
					target: "duster offchain worker",
					"failed to run offchain worker at {:?}: {:?}",
					block_number,
					e,
				);
			} else {
				log::debug!(
					target: "duster offchain worker",
					"offchain worker at block: {:?} completed!",
					block_number,
				);
			}
		}
	}

	#[pallet::config]
	pub trait Config:
		frame_system::Config + orml_tokens::Config + pallet_duster::Config + CreateSignedTransaction<DusterCall<Self>>
	{
		type AuthorityId: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>;
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {}
}
impl<T: Config> Pallet<T> {

	fn _offchain_worker() -> Result<(), OffchainErr>
	where
		<T as orml_tokens::Config>::Balance: From<<T as pallet_duster::Config>::Balance>,
		<T as pallet_duster::Config>::CurrencyId: From<<T as orml_tokens::Config>::CurrencyId>,
	{
		if !sp_io::offchain::is_validator() {
			return Err(OffchainErr::NotValidator);
		}

		let lock_expiration = Duration::from_millis(LOCK_DURATION);
		let mut lock = StorageLock::<'_, Time>::with_deadline(&OFFCHAIN_WORKER_LOCK, lock_expiration);
		let mut guard = lock.try_lock().map_err(|_| OffchainErr::OffchainLock)?;

		for (who, asset, account) in orml_tokens::Accounts::<T>::iter() {
			let ed = T::MinCurrencyDeposits::get(&asset.into());
			let total = account.free.saturating_add(account.reserved);

			if total < ed.into() {
				let _ = Self::dust_account_signed(&who, asset.into());
			}

			guard.extend_lock().map_err(|_| OffchainErr::OffchainLock)?;
		}

		guard.forget();

		Ok(())
	}

	fn dust_account_signed(
		from: &T::AccountId,
		currency_id: <T as pallet_duster::Config>::CurrencyId,
	) -> Result<(), &'static str> {
		let signer = Signer::<T, T::AuthorityId>::any_account();
		if !signer.can_sign() {
			return Err("No local accounts available. Consider adding one via `author_insertKey` RPC.");
		}

		let results = signer.send_signed_transaction(|_account| DusterCall::dust_account(from.clone(), currency_id));

		for (acc, res) in &results {
			match res {
				Ok(()) => {
					frame_support::log::info!("Dust moved successfully to [{:?}]", acc.id)
				}
				Err(e) => {
					frame_support::log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e)
				}
			}
		}

		Ok(())
	}
}

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"dust");

pub mod crypto {
	use super::KEY_TYPE;
	use sp_core::sr25519::Signature as Sr25519Signature;
	use sp_runtime::app_crypto::{app_crypto, sr25519};
	use sp_runtime::{traits::Verify, MultiSignature, MultiSigner};

	app_crypto!(sr25519, KEY_TYPE);

	pub struct TestAuthId;
	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = Sr25519Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}

	//implemented for mock runtime in test
	impl frame_system::offchain::AppCrypto<<Sr25519Signature as Verify>::Signer, Sr25519Signature> for TestAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
	}
}
