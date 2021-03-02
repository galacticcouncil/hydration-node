#![cfg_attr(not(feature = "std"), no_std)]
use codec::Encode;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	ensure,
	traits::{Currency, Get, Imbalance},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use primitives::Balance;
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
use sp_std::vec::Vec;

pub use traits::*;

mod claims_data;
mod migration;
mod traits;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

mod benchmarking;

use weights::WeightInfo;

pub mod weights;

pub trait Config: frame_system::Config {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type Prefix: Get<&'static [u8]>;
	type WeightInfo: WeightInfo;
	type Currency: Currency<Self::AccountId>;
	// This type is needed to convert from Currency to Balance
	type CurrencyBalance: From<Balance>
		+ Into<<Self::Currency as Currency<<Self as frame_system::Config>::AccountId>>::Balance>;
}

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

decl_storage! {
	trait Store for Module<T: Config> as Claims {
		Claims get(fn claims): map hasher(blake2_128_concat) EthereumAddress => BalanceOf<T>;

		PalletVersion: StorageVersion = StorageVersion::V1EmptyBalances;
	}

	add_extra_genesis {
		config(claims): Vec<(EthereumAddress, BalanceOf<T>)>;

		build(|config: &GenesisConfig<T>| {
			config.claims.iter().for_each(|(eth_address, initial_balance)| {
				Claims::<T>::mutate(eth_address, |amount| *amount += *initial_balance)
			})
		})
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
		Balance = BalanceOf<T>,
	{
		Claimed(AccountId, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
		/// Ethereum signature is not valid
		InvalidEthereumSignature,
		/// Claim is not valid
		NoClaimOrAlreadyClaimed,
		/// Value reached maximum and cannot be incremented further
		BalanceOverflow,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// The Prefix that is used in signed Ethereum messages for this network
		const Prefix: &[u8] = T::Prefix::get();

		/// Claim a balance
		/// Verify an Ethereum signature and deposit the corresponding balance into the account's free balance.
		/// The dispatch origin of this call must match the address in the signed message.
		/// This is feeless transaction.
		#[weight = (<T as Config>::WeightInfo::claim(), DispatchClass::Normal, Pays::No)]
		fn claim(origin, ethereum_signature: EcdsaSignature)  {
			let sender = ensure_signed(origin)?;

			let (balance_due, address) = Self::validate_claim(&sender, &ethereum_signature)?;

			Self::process_claim(sender, balance_due, address)?;
		}

		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			migration::migrate_to_v2::<T>(&claims_data::CLAIMS_DATA)
		}
	}
}

impl<T: Config> Module<T> {
	fn validate_claim(
		who: &T::AccountId,
		signature: &EcdsaSignature,
	) -> Result<(BalanceOf<T>, EthereumAddress), Error<T>> {
		let sender_hex = who.using_encoded(to_ascii_hex);

		let signer = signature.recover(&sender_hex, T::Prefix::get());

		match signer {
			Some(address) => {
				let balance_due = Claims::<T>::get(&address);

				if balance_due == Zero::zero() {
					return Err(Error::<T>::NoClaimOrAlreadyClaimed);
				};
				Ok((balance_due, address))
			}
			None => Err(Error::<T>::InvalidEthereumSignature),
		}
	}

	fn process_claim(dest: T::AccountId, balance_due: BalanceOf<T>, address: EthereumAddress) -> DispatchResult {
		let imbalance = <T::Currency as Currency<T::AccountId>>::deposit_creating(&dest, balance_due);
		ensure!(
			imbalance.peek() != <T::Currency as Currency<T::AccountId>>::PositiveImbalance::zero().peek(),
			Error::<T>::BalanceOverflow
		);

		Claims::<T>::mutate(address, |bal| *bal = Zero::zero());

		Self::deposit_event(RawEvent::Claimed(dest, balance_due));

		Ok(())
	}
}

/// Converts the given binary data into ASCII-encoded hex. It will be twice the length.
fn to_ascii_hex(data: &[u8]) -> Vec<u8> {
	let mut r = Vec::with_capacity(data.len() * 2);
	let mut push_nibble = |n| r.push(if n < 10 { b'0' + n } else { b'a' - 10 + n });
	for &b in data.iter() {
		push_nibble(b / 16);
		push_nibble(b % 16);
	}
	r
}
