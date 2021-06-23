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

use frame_support::{dispatch::DispatchResult, traits::Get};
use primitives::{AssetId, Balance};
use sp_std::marker;

use orml_traits::{GetByKey, MultiCurrency, OnDust};

use sp_runtime::traits::Saturating;

use frame_system::{
	ensure_signed,
	offchain::{CreateSignedTransaction, SendSignedTransaction, Signer},
};

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn offchain_worker(_block_number: <T as frame_system::Config>::BlockNumber) {
			for (who, asset, account) in orml_tokens::Accounts::<T>::iter() {
				let ed = T::MinCurrencyDeposits::get(&asset);
				let total = account.free.saturating_add(account.reserved);

				if total < ed {
					let _ = Self::transfer_dust_signed(&who, asset);
				}
			}
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config + orml_tokens::Config + CreateSignedTransaction<Call<Self>> {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Call: From<Call<Self>>;

		type MinCurrencyDeposits: GetByKey<Self::CurrencyId, Self::Balance>;

		type AuthorityId: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>;

		#[pallet::constant]
		type DustAccount: Get<Self::AccountId>;

		#[pallet::constant]
		type RewardAccount: Get<Self::AccountId>;

		#[pallet::constant]
		type Reward: Get<Self::Balance>;
	}

	#[pallet::error]
	pub enum Error<T> {
		BalanceSufficient,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		Dusted,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight((0, Pays::No))]
		pub fn dust_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			currency_id: T::CurrencyId,
		) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			let (dustable, dust) = Self::is_dustable(&account, currency_id);

			ensure!(dustable, Error::<T>::BalanceSufficient);

			Self::transfer_dust(&account, &T::DustAccount::get(), currency_id, dust)?;

			Self::deposit_event(Event::Dusted);

			// Ignore the result, it fails - no problem.
			let _ = Self::reward_duster(&who, currency_id, dust);

			Ok(().into())
		}
	}
}
impl<T: Config> Pallet<T> {
	fn is_dustable(account: &T::AccountId, currency_id: T::CurrencyId) -> (bool, T::Balance) {
		let ed = T::MinCurrencyDeposits::get(&currency_id);

		let total = <orml_tokens::Pallet<T> as MultiCurrency<T::AccountId>>::total_balance(currency_id, account);

		(total < ed, total)
	}

	fn reward_duster(_duster: &T::AccountId, _currency_id: T::CurrencyId, _dust: T::Balance) -> DispatchResult {
		/*
		let reserve_account = T::RewardAccount::get();
		let reward = T::Reward::get();
		 */

		Ok(())
	}

	fn transfer_dust_signed(from: &T::AccountId, currency_id: T::CurrencyId) -> Result<(), &'static str> {
		let signer = Signer::<T, T::AuthorityId>::any_account();
		if !signer.can_sign() {
			return Err("No local accounts available. Consider adding one via `author_insertKey` RPC.");
		}

		let results = signer.send_signed_transaction(|_account| Call::dust_account(from.clone(), currency_id));

		for (acc, res) in &results {
			match res {
				Ok(()) => {
					println!("Dust moved successfully to [{:?}]", acc.id);
					frame_support::log::info!("Dust moved successfully to [{:?}]", acc.id)
				}
				Err(e) => {
					println!("[{:?}] Failed to submit transaction: {:?}", acc.id, e);
					frame_support::log::error!("[{:?}] Failed to submit transaction: {:?}", acc.id, e)
				}
			}
		}

		Ok(())
	}

	fn transfer_dust(
		from: &T::AccountId,
		dest: &T::AccountId,
		currency_id: T::CurrencyId,
		dust: T::Balance,
	) -> DispatchResult {
		<orml_tokens::Pallet<T> as MultiCurrency<T::AccountId>>::transfer(currency_id, from, dest, dust)
	}
}

impl<T: Config> GetByKey<AssetId, Balance> for Pallet<T>
where
	Balance: From<T::Balance>,
	AssetId: Into<T::CurrencyId>,
{
	fn get(k: &AssetId) -> u128 {
		T::MinCurrencyDeposits::get(&k.clone().into()).into()
	}
}

pub struct TransferDust<T, GetAccountId>(marker::PhantomData<(T, GetAccountId)>);

impl<T, GetAccountId> OnDust<T::AccountId, T::CurrencyId, T::Balance> for TransferDust<T, GetAccountId>
where
	T: Config,
	GetAccountId: Get<T::AccountId>,
{
	fn on_dust(who: &T::AccountId, currency_id: T::CurrencyId, amount: T::Balance) {
		let _ = <Pallet<T>>::transfer_dust(who, &GetAccountId::get(), currency_id, amount);
	}
}

use sp_core::crypto::KeyTypeId;

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
