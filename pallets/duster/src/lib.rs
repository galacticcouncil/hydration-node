// This file is part of HydraDX.

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
#![allow(clippy::unused_unit)]
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::manual_inspect)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

pub mod migration;
pub mod weights;
pub use crate::weights::WeightInfo;
use frame_support::sp_runtime::DispatchError;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::{dispatch::DispatchResult, ensure, traits::Contains, traits::Get};
use frame_system::ensure_signed;
use hydradx_traits::evm::Erc20Inspect;
use hydradx_traits::evm::Erc20OnDust;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use orml_traits::GetByKey;
use sp_runtime::traits::Zero;

use sp_std::convert::TryInto;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

type Balance = u128;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
	use sp_std::vec::Vec;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(2);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn whitelisted)]
	/// Accounts excluded from dusting.
	pub type AccountWhitelist<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type
		type AssetId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;

		/// Currency for transfers
		type MultiCurrency: Inspect<Self::AccountId, AssetId = Self::AssetId, Balance = Balance>
			+ Mutate<Self::AccountId>;

		/// Existential deposit required to keep an account.
		type ExistentialDeposit: GetByKey<Self::AssetId, Balance>;

		/// The origin which can manage whiltelist.
		type WhitelistUpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Erc20 support to dust AToken balances
		type Erc20Support: hydradx_traits::evm::Erc20Inspect<Self::AssetId>
			+ hydradx_traits::evm::Erc20OnDust<Self::AccountId, Self::AssetId>;

		/// Whitelist for dust removal, used to check if an account should be excluded from dusting
		type DustRemovalWhitelist: Contains<Self::AccountId>;

		/// Treasury account, which receives the dust.
		#[pallet::constant]
		type TreasuryAccountId: Get<Self::AccountId>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub account_whitelist: Vec<T::AccountId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			self.account_whitelist.iter().for_each(|account_id| {
				AccountWhitelist::<T>::insert(account_id, ());
			});
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account is excluded from dusting.
		AccountWhitelisted,

		/// Account is not present in the non-dustable list.
		AccountNotWhitelisted,

		/// The balance is zero.
		ZeroBalance,

		/// The balance was not fully dusted, there is some leftover on the account. Normally, it should never happen.
		NonZeroBalance,

		/// The balance is sufficient to keep account open.
		BalanceSufficient,

		/// Reserve account is not set.
		ReserveAccountNotSet,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Account dusted.
		Dusted { who: T::AccountId, amount: Balance },

		/// Account added to non-dustable list.
		Added { who: T::AccountId },

		/// Account removed from non-dustable list.
		Removed { who: T::AccountId },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dust specified account.
		/// IF account balance is < min. existential deposit of given currency, and account is allowed to
		/// be dusted, the remaining balance is transferred to treasury account.
		///
		/// In case of AToken, we perform an erc20 dust, which does a wihtdraw all then supply atoken on behalf of the dust receiver
		///
		/// The transaction fee is returned back in case of successful dusting.
		///
		/// Treasury account can never be dusted.
		///
		/// Emits `Dusted` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::dust_account())]
		pub fn dust_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			currency_id: T::AssetId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			ensure!(
				Self::whitelisted(&account).is_none()
					&& !T::DustRemovalWhitelist::contains(&account)
					&& account != T::TreasuryAccountId::get(),
				Error::<T>::AccountWhitelisted
			);

			let ed = T::ExistentialDeposit::get(&currency_id);
			let dust = T::MultiCurrency::total_balance(currency_id, &account.clone());
			ensure!(dust < ed, Error::<T>::BalanceSufficient);

			ensure!(!dust.is_zero(), Error::<T>::ZeroBalance);

			let dust_dest_account = T::TreasuryAccountId::get();

			if T::Erc20Support::is_atoken(currency_id) {
				//Temporarily adding the account to whitelist to prevent ED error when AToken is withdrawn from contract
				Self::add_account(&account)?;
				T::Erc20Support::on_dust(&account, &dust_dest_account, currency_id)?;
				Self::remove_account(&account)?;
			} else {
				Self::transfer_dust(&account, &dust_dest_account, currency_id, dust)?;
			}

			//Sanity check that account is fully dusted
			let leftover = T::MultiCurrency::total_balance(currency_id, &account);
			ensure!(leftover.is_zero(), Error::<T>::NonZeroBalance);

			Self::deposit_event(Event::Dusted {
				who: account,
				amount: dust,
			});

			Ok(Pays::No.into())
		}

		/// Add account to list of whitelist accounts. Account which are excluded from dusting.
		/// If such account should be dusted - `AccountWhitelisted` error is returned.
		/// Only root can perform this action.
		///
		/// Emits `Added` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::whitelist_account())]
		pub fn whitelist_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			T::WhitelistUpdateOrigin::ensure_origin(origin)?;

			AccountWhitelist::<T>::insert(&account, ());

			Self::deposit_event(Event::Added { who: account });

			Ok(())
		}

		/// Remove account from list of whitelist accounts. That means account can be dusted again.
		///
		/// Emits `Removed` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_from_whitelist())]
		pub fn remove_from_whitelist(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			T::WhitelistUpdateOrigin::ensure_origin(origin)?;

			AccountWhitelist::<T>::mutate(&account, |maybe_account| -> DispatchResult {
				ensure!(!maybe_account.is_none(), Error::<T>::AccountNotWhitelisted);

				*maybe_account = None;

				Ok(())
			})?;

			Self::deposit_event(Event::Removed { who: account });

			Ok(())
		}
	}
}
impl<T: Config> Pallet<T> {
	/// Transfer dust amount to selected DustAccount ( usually treasury)
	fn transfer_dust(
		from: &T::AccountId,
		dest: &T::AccountId,
		currency_id: T::AssetId,
		dust: Balance,
	) -> DispatchResult {
		T::MultiCurrency::transfer(currency_id, from, dest, dust, Preservation::Expendable).map(|_| Ok(()))?
	}
}

use orml_traits::currency::OnDust;

use sp_std::marker::PhantomData;
pub struct DusterWhitelist<T>(PhantomData<T>);

impl<T: Config> OnDust<T::AccountId, T::AssetId, Balance> for Pallet<T> {
	fn on_dust(who: &T::AccountId, currency_id: T::AssetId, amount: Balance) {
		let _ = Self::transfer_dust(who, &T::TreasuryAccountId::get(), currency_id, amount);
	}
}

impl<T: Config> Contains<T::AccountId> for DusterWhitelist<T> {
	fn contains(t: &T::AccountId) -> bool {
		AccountWhitelist::<T>::contains_key(t)
	}
}

impl<T: Config> DustRemovalAccountWhitelist<T::AccountId> for Pallet<T> {
	type Error = DispatchError;

	fn add_account(account: &T::AccountId) -> Result<(), Self::Error> {
		AccountWhitelist::<T>::insert(account, ());
		Ok(())
	}

	fn remove_account(account: &T::AccountId) -> Result<(), Self::Error> {
		AccountWhitelist::<T>::mutate(account, |maybe_account| -> Result<(), DispatchError> {
			ensure!(!maybe_account.is_none(), Error::<T>::AccountNotWhitelisted);

			*maybe_account = None;

			Ok(())
		})
	}
}
