#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{decl_module, decl_storage, decl_event, decl_error, dispatch, traits::Get};
use frame_system::ensure_signed;

use primitives::{AssetId, Balance};

use orml_traits::{MultiCurrency, MultiCurrencyExtended};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Trait: frame_system::Trait {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Trait>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;
}

decl_storage! {
	trait Store for Module<T: Trait> as TemplateModule {
	}
}

decl_event!(
	pub enum Event<T> where
		AccountId = <T as frame_system::Trait>::AccountId,
		AssetId = AssetId,
		Balance = Balance
	{
		Mint(AccountId, AssetId, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Trait> {
	}
}

decl_module! {
	pub struct Module<T: Trait> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = 10_000 + T::DbWeight::get().writes(1)]
		pub fn mint(origin, asset: AssetId, amount: Balance) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			T::Currency::deposit(asset, &who, amount)?;
			Self::deposit_event(RawEvent::Mint(who, asset, amount));
			Ok(())
		}
	}
}
