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
use sp_runtime::traits::Zero;
pub use crate::weights::WeightInfo;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::fungibles::Mutate;
use frame_support::{dispatch::DispatchResult, ensure, traits::Contains, traits::Get};
use hydradx_traits::evm::ATokenDuster;
use orml_traits::{
	arithmetic::{Signed, SimpleArithmetic},
	GetByKey, MultiCurrency, MultiCurrencyExtended,
};

use frame_system::ensure_signed;

use sp_std::convert::{TryFrom, TryInto};

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

type Balance = u128;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use crate::weights::WeightInfo;
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::AtLeast32BitUnsigned;
	use frame_support::traits::tokens::{Fortitude, Preservation};
	use frame_system::pallet_prelude::{BlockNumberFor, OriginFor};
	use sp_std::vec::Vec;

	/// The current storage version.
	const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn blacklisted)]
	/// Accounts excluded from dusting.
	pub type AccountBlacklist<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, (), OptionQuery>;

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Asset type
		type CurrencyId: Parameter + Member + Copy + MaybeSerializeDeserialize + Ord;

		/// Currency for transfers
		type MultiCurrency: Inspect<Self::AccountId, AssetId = Self::CurrencyId, Balance = Balance>
			+ Mutate<Self::AccountId>;

		/// The minimum amount required to keep an account.
		type MinCurrencyDeposits: GetByKey<Self::CurrencyId, Balance>;

		/// Native Asset Id
		#[pallet::constant]
		type NativeCurrencyId: Get<Self::CurrencyId>;

		/// The origin which can manage whiltelist.
		type BlacklistUpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Duster for accounts with AToken dusts
		type ATokenDuster: hydradx_traits::evm::ATokenDuster<Self::AccountId, Self::CurrencyId>;

		/// Default account for `dust_account` in genesis config.
		#[pallet::constant]
		type TreasuryAccountId: Get<Self::AccountId>;

		/// Weight information for extrinsics in this module.
		type WeightInfo: WeightInfo;
	}

	#[pallet::genesis_config]
	#[derive(frame_support::DefaultNoBound)]
	pub struct GenesisConfig<T: Config> {
		pub account_blacklist: Vec<T::AccountId>,
		pub dust_account: Option<T::AccountId>,
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			self.account_blacklist.iter().for_each(|account_id| {
				AccountBlacklist::<T>::insert(account_id, ());
			});
		}
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Account is excluded from dusting.
		AccountBlacklisted,

		/// Account is not present in the non-dustable list.
		AccountNotBlacklisted,

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
		Dusted { who: T::AccountId, amount: Balance},

		/// Account added to non-dustable list.
		Added { who: T::AccountId },

		/// Account removed from non-dustable list.
		Removed { who: T::AccountId },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Dust specified account.
		/// IF account balance is < min. existential deposit of given currency, and account is allowed to
		/// be dusted, the remaining balance is transferred to selected account (usually treasury).
		///
		/// In case of AToken, the dusting is performed via ATokenDuster dependency, which does a wihtdraw all then supply atoken on behalf of the dust receiver
		///
		/// The transaction fee is returned back in case of sccessful dusting.
		///
		/// Emits `Dusted` event when successful.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::dust_account())]
		pub fn dust_account(
			origin: OriginFor<T>,
			account: T::AccountId,
			currency_id: T::CurrencyId,
		) -> DispatchResultWithPostInfo {
			ensure_signed(origin)?;

			ensure!(Self::blacklisted(&account).is_none(), Error::<T>::AccountBlacklisted);

			let (dustable, dust) = Self::is_dustable(&account, currency_id);
			ensure!(!dust.is_zero(), Error::<T>::ZeroBalance);

			ensure!(dustable, Error::<T>::BalanceSufficient);

			// Error should never occur here
			let dust_dest_account = T::TreasuryAccountId::get();

			if T::ATokenDuster::is_atoken(currency_id) {
				//Temporarily adding the account to blacklist to prevent ED error when AToken is withdrawn from contract
				Self::add_account(&account)?;
				T::ATokenDuster::dust_account(&account, &dust_dest_account, currency_id)?;
				Self::remove_account(&account)?;
			} else {
				Self::transfer_dust(&account, &dust_dest_account, currency_id, dust)?;
			}

			//Sanity check that account is fully dusted
			let leftover =
				T::MultiCurrency::reducible_balance(currency_id, &account, Preservation::Expendable, Fortitude::Polite);
			ensure!(leftover.is_zero(), Error::<T>::NonZeroBalance);

			Self::deposit_event(Event::Dusted {
				who: account,
				amount: dust,
			});

			Ok(Pays::No.into())
		}

		/// Add account to list of non-dustable account. Account whihc are excluded from udsting.
		/// If such account should be dusted - `AccountBlacklisted` error is returned.
		/// Only root can perform this action.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::add_nondustable_account())]
		pub fn add_nondustable_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			T::BlacklistUpdateOrigin::ensure_origin(origin)?;

			AccountBlacklist::<T>::insert(&account, ());

			Self::deposit_event(Event::Added { who: account });

			Ok(())
		}

		/// Remove account from list of non-dustable accounts. That means account can be dusted again.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::remove_nondustable_account())]
		pub fn remove_nondustable_account(origin: OriginFor<T>, account: T::AccountId) -> DispatchResult {
			T::BlacklistUpdateOrigin::ensure_origin(origin)?;

			AccountBlacklist::<T>::mutate(&account, |maybe_account| -> DispatchResult {
				ensure!(!maybe_account.is_none(), Error::<T>::AccountNotBlacklisted);

				*maybe_account = None;

				Ok(())
			})?;

			Self::deposit_event(Event::Removed { who: account });

			Ok(())
		}
	}
}
impl<T: Config> Pallet<T> {
	/// Check is account's balance is below minimum deposit.
	fn is_dustable(account: &T::AccountId, currency_id: T::CurrencyId) -> (bool, Balance) {
		let ed = T::MinCurrencyDeposits::get(&currency_id);

		let total = T::MultiCurrency::total_balance(currency_id, account);

		(total < ed, total)
	}

	/// Transfer dust amount to selected DustAccount ( usually treasury)
	fn transfer_dust(
		from: &T::AccountId,
		dest: &T::AccountId,
		currency_id: T::CurrencyId,
		dust: Balance,
	) -> DispatchResult {
		T::MultiCurrency::transfer(currency_id, from, dest, dust, Preservation::Expendable).map(|_| Ok(()))?
	}
}

use orml_traits::currency::OnDust;

use sp_std::marker::PhantomData;
pub struct DusterWhitelist<T>(PhantomData<T>);

impl<T: Config> OnDust<T::AccountId, T::CurrencyId, Balance> for Pallet<T> {
	fn on_dust(who: &T::AccountId, currency_id: T::CurrencyId, amount: Balance) {
		let _ = Self::transfer_dust(who, &T::TreasuryAccountId::get(), currency_id, amount);
	}
}

impl<T: Config> Contains<T::AccountId> for DusterWhitelist<T> {
	fn contains(t: &T::AccountId) -> bool {
		AccountBlacklist::<T>::contains_key(t)
	}
}

use frame_support::sp_runtime::DispatchError;
use frame_support::traits::tokens::Preservation;
use hydradx_traits::pools::DustRemovalAccountWhitelist;

impl<T: Config> DustRemovalAccountWhitelist<T::AccountId> for Pallet<T> {
	type Error = DispatchError;

	fn add_account(account: &T::AccountId) -> Result<(), Self::Error> {
		AccountBlacklist::<T>::insert(account, ());
		Ok(())
	}

	fn remove_account(account: &T::AccountId) -> Result<(), Self::Error> {
		AccountBlacklist::<T>::mutate(account, |maybe_account| -> Result<(), DispatchError> {
			ensure!(!maybe_account.is_none(), Error::<T>::AccountNotBlacklisted);

			*maybe_account = None;

			Ok(())
		})
	}
}
