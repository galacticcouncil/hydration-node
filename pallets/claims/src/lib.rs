#![cfg_attr(not(feature = "std"), no_std)]
use codec::Encode;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	ensure,
	traits::{Currency, Get},
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use orml_utilities::with_transaction_result;
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
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
	type Currency: Currency<Self::AccountId>;
	type Prefix: Get<&'static [u8]>;
	type WeightInfo: WeightInfo;
	type IntoBalance: From<u128> + Into<<Self::Currency as Currency<<Self as frame_system::Config>::AccountId>>::Balance>;
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
		InvalidEthereumSignature,
		NoClaimOrAlreadyClaimed,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// The Prefix that is used in signed Ethereum messages for this network
		const Prefix: &[u8] = T::Prefix::get();

		#[weight = (<T as Config>::WeightInfo::claim(), DispatchClass::Normal, Pays::No)]
		fn claim(origin, ethereum_signature: EcdsaSignature)  {
			let sender = ensure_signed(origin)?;

			let sender_hex = sender.using_encoded(to_ascii_hex);

			let signer = Self::eth_recover(&ethereum_signature, &sender_hex).ok_or(Error::<T>::InvalidEthereumSignature)?;

			Self::process_claim(signer, sender)?;
		}

		fn on_runtime_upgrade() -> frame_support::weights::Weight {
			migration::migrate_to_v2::<T>()
		}
	}
}

impl<T: Config> Module<T> {
	fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
		let balance_due = Claims::<T>::get(&signer);

		ensure!(balance_due != Zero::zero(), Error::<T>::NoClaimOrAlreadyClaimed);

		with_transaction_result(|| {
			Claims::<T>::mutate(signer, |bal| *bal = Zero::zero());
			<T::Currency as Currency<T::AccountId>>::deposit_creating(&dest, balance_due);

			Self::deposit_event(RawEvent::Claimed(dest, balance_due));

			Ok(())
		})
	}

	// Constructs the message that Ethereum RPC's `personal_sign` and `eth_sign` would sign.
	fn ethereum_signable_message(what: &[u8]) -> Vec<u8> {
		let prefix = T::Prefix::get();
		let mut l = prefix.len() + what.len();
		let mut rev = Vec::new();
		while l > 0 {
			rev.push(b'0' + (l % 10) as u8);
			l /= 10;
		}
		let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
		v.extend(rev.into_iter().rev());
		v.extend_from_slice(&prefix[..]);
		v.extend_from_slice(what);
		v
	}

	// Attempts to recover the Ethereum address from a message signature signed by using
	// the Ethereum RPC's `personal_sign` and `eth_sign`.
	fn eth_recover(s: &EcdsaSignature, what: &[u8]) -> Option<EthereumAddress> {
		let msg = keccak_256(&Self::ethereum_signable_message(what));
		let mut res = EthereumAddress::default();
		res.0
			.copy_from_slice(&keccak_256(&secp256k1_ecdsa_recover(&s.0, &msg).ok()?[..])[12..]);
		Some(res)
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
