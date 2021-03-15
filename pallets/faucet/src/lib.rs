#![cfg_attr(not(feature = "std"), no_std)]

#![allow(clippy::unused_unit)]

use frame_support::{
	ensure,
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use primitives::{AssetId, Balance};
use sp_std::vec::Vec;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::OriginFor;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::hooks]
	impl<T: Config> Hooks<T::BlockNumber> for Pallet<T> {
		fn on_finalize(_p: T::BlockNumber) {
			Minted::<T>::set(0u8);
		}
	}

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;

		type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		RampageMint(T::AccountId, AssetId, Balance),
		Mint(T::AccountId),
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
	pub struct GenesisConfig {
		pub mint_limit: u8,
		pub rampage: bool,
		pub mintable_currencies: Vec<AssetId>,
	}

	#[cfg(feature = "std")]
	impl Default for GenesisConfig {
		fn default() -> Self {
			GenesisConfig {
				mint_limit: Default::default(),
				rampage: Default::default(),
				mintable_currencies: vec![],
			}
		}
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
		#[pallet::weight((0, DispatchClass::Normal, Pays::No))]
		pub fn rampage_mint(origin: OriginFor<T>, asset: AssetId, amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			ensure!(Self::rampage(), Error::<T>::RampageMintNotAllowed);

			T::Currency::deposit(asset, &who, amount)?;
			Self::deposit_event(Event::RampageMint(who, asset, amount));

			Ok(().into())
		}

		#[pallet::weight((0, DispatchClass::Normal, Pays::No))]
		pub fn mint(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;

			ensure!(Self::minted() < Self::mint_limit(), Error::<T>::MaximumMintLimitReached);

			for i in Self::mintable_currencies() {
				T::Currency::deposit(i, &who, 1_000_000_000_000_000)?;
			}

			Minted::<T>::set(Self::minted() + 1);

			Self::deposit_event(Event::Mint(who));

			Ok(().into())
		}
	}
}
