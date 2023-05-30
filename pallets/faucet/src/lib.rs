// This file is part of pallet-faucet.

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

use frame_support::ensure;
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

type AssetId = u32;
type Balance = u128;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::OriginFor;

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
        fn on_finalize(_p: T::BlockNumber) {
            Minted::<T>::set(0u8);
        }
    }

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(crate) fn deposit_event)]
    pub enum Event<T: Config> {
        RampageMint {
            account_id: T::AccountId,
            asset_id: AssetId,
            amount: Balance,
        },
        Mint {
            account_id: T::AccountId,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        RampageMintNotAllowed,
        MaximumMintLimitReached,
    }
    #[pallet::storage]
    #[pallet::getter(fn minted)]
    pub type Minted<T: Config> = StorageValue<_, u8, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn mint_limit)]
    pub type MintLimit<T: Config> = StorageValue<_, u8, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rampage)]
    pub type Rampage<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn mintable_currencies)]
    pub type MintableCurrencies<T: Config> = StorageValue<_, Vec<AssetId>, ValueQuery>;

    #[pallet::genesis_config]
    #[derive(Default)]
    pub struct GenesisConfig {
        pub mint_limit: u8,
        pub rampage: bool,
        pub mintable_currencies: Vec<AssetId>,
    }

    #[cfg(feature = "std")]
    impl GenesisConfig {
        /// Direct implementation to not break dependency
        pub fn build_storage<T: Config>(&self) -> Result<sp_runtime::Storage, String> {
            <Self as frame_support::traits::GenesisBuild<T>>::build_storage(self)
        }

        /// Direct implementation to not break dependency
        pub fn assimilate_storage<T: Config>(&self, storage: &mut sp_runtime::Storage) -> Result<(), String> {
            <Self as frame_support::traits::GenesisBuild<T>>::assimilate_storage(self, storage)
        }
    }

    #[pallet::genesis_build]
    impl<T: Config> GenesisBuild<T> for GenesisConfig {
        fn build(&self) {
            MintLimit::<T>::put(self.mint_limit);
            Rampage::<T>::put(self.rampage);
            MintableCurrencies::<T>::put(self.mintable_currencies.clone());
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight((0, DispatchClass::Normal, Pays::No))]
        pub fn rampage_mint(origin: OriginFor<T>, asset: AssetId, amount: Balance) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            ensure!(Self::rampage(), Error::<T>::RampageMintNotAllowed);

            T::Currency::deposit(asset, &who, amount)?;
            Self::deposit_event(Event::RampageMint {
                account_id: who,
                asset_id: asset,
                amount,
            });

            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight((0, DispatchClass::Normal, Pays::No))]
        pub fn mint(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            ensure!(Self::minted() < Self::mint_limit(), Error::<T>::MaximumMintLimitReached);

            for i in Self::mintable_currencies() {
                T::Currency::deposit(i, &who, 1_000_000_000_000_000)?;
            }

            Minted::<T>::set(Self::minted() + 1);

            Self::deposit_event(Event::Mint { account_id: who });

            Ok(().into())
        }
    }
}
