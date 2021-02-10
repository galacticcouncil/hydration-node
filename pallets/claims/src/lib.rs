#![cfg_attr(not(feature = "std"), no_std)]
use codec::Encode;
use frame_support::{
	decl_error, decl_event, decl_module, decl_storage,
	dispatch::DispatchResult,
	ensure,
	traits::Get,
	weights::{DispatchClass, Pays},
};
use frame_system::ensure_signed;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use orml_utilities::with_transaction_result;
use primitives::{Amount, AssetId, Balance, CORE_ASSET_ID};
use sp_io::{crypto::secp256k1_ecdsa_recover, hashing::keccak_256};
use sp_runtime::traits::Zero;
use sp_std::prelude::*;
use sp_std::vec::Vec;
pub use traits::*;

mod traits;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub trait Config: frame_system::Config {
	type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
	type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = Amount>;
	type Prefix: Get<&'static [u8]>;
}

decl_storage! {
	trait Store for Module<T: Config> as Claims {
		HDXClaims get(fn hdxclaims): map hasher(blake2_128_concat) EthereumAddress => Balance;

		PalletVersion: StorageVersion = StorageVersion::V1EmptyBalances;
	}

	add_extra_genesis {
		config(claims): Vec<(EthereumAddress, Balance)>;

		build(|config: &GenesisConfig| {
			config.claims.iter().for_each(|(eth_address, initial_balance)| {
				HDXClaims::mutate(eth_address, | amount | *amount += *initial_balance)
			})
		})
	}
}

decl_event!(
	pub enum Event<T>
	where
		AccountId = <T as frame_system::Config>::AccountId,
		Balance = Balance,
	{
		HDXClaimed(AccountId, Balance),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
		InvalidEthereumSignature,
		InvalidStatement,
		NoClaimOrAlreadyClaimed,
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

		/// The Prefix that is used in signed Ethereum messages for this network
		const Prefix: &[u8] = T::Prefix::get();

		#[weight = (0, DispatchClass::Normal, Pays::No)]
		fn claim(origin, ethereum_signature: EcdsaSignature)  {
			let sender = ensure_signed(origin)?;

			let sender_hex = sender.using_encoded(to_ascii_hex);

			let signer = Self::eth_recover(&ethereum_signature, &sender_hex).ok_or(Error::<T>::InvalidEthereumSignature)?;

			Self::process_claim(signer, sender)?;
		}
	}
}

impl<T: Config> Module<T> {
	fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
		// TODO: Fix multicurrency support and separate checks for not matching addresses and already claimed
		let balance_due = HDXClaims::get(&signer);

		ensure!(balance_due != Zero::zero(), Error::<T>::NoClaimOrAlreadyClaimed);

		with_transaction_result(|| {
			HDXClaims::insert(signer, 0);
			T::Currency::deposit(CORE_ASSET_ID, &dest, balance_due)?;
			Ok(())
		})?;

		Self::deposit_event(RawEvent::HDXClaimed(dest, balance_due));
		Ok(())
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

pub mod migration {
	use super::*;

	pub fn migrate_to_v2<T: Config>() -> frame_support::weights::Weight {
		if PalletVersion::get() == StorageVersion::V1EmptyBalances {
			frame_support::debug::info!(" >>> Adding xHDX claims to the storage");
			// put code inserting the struct data here
			PalletVersion::put(StorageVersion::V2AddClaimData);
			T::DbWeight::get().reads_writes(2, 3)
		} else {
			frame_support::debug::info!(" >>> Unused migration");
			0
		}
	}
}
