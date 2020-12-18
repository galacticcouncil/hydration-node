#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{
	decl_error, decl_event, decl_module, decl_storage, dispatch,
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;

use primitives::{AssetId, Balance};

use orml_traits::{MultiCurrency, MultiCurrencyExtended};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Config: frame_system::Config {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;
}

decl_storage! {
	trait Store for Module<T: Config> as TemplateModule {
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
		AssetId = AssetId,
		Balance = Balance,
	{
		Mint(AccountId, AssetId, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		#[weight = (10_000, DispatchClass::Normal, Pays::No)]
		pub fn mint(origin, asset: AssetId, amount: Balance) -> dispatch::DispatchResult {
			let who = ensure_signed(origin)?;
			T::Currency::deposit(asset, &who, amount)?;
			Self::deposit_event(RawEvent::Mint(who, asset, amount));
			Ok(())
		}
	}
}
