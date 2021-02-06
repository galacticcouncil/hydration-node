#![cfg_attr(not(feature = "std"), no_std)]
use codec::{Decode, Encode};
use frame_support::{decl_error, decl_event, decl_module, decl_storage, 
    dispatch::DispatchResult, ensure,
    traits::{Get, Currency},
    weights::Pays
};
use frame_system::ensure_signed;
use orml_traits::MultiCurrencyExtended;
use primitives::{AssetId, Balance};
use sp_io::{hashing::keccak_256, crypto::secp256k1_ecdsa_recover};
use sp_std::vec::Vec;
use sp_runtime::{traits::Zero};
use sp_std::prelude::*;
#[cfg(feature = "std")]
use serde::{Serialize, Deserialize, Serializer, Deserializer};

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub trait Config: frame_system::Config {
    type Event: From<Event<Self>> + Into<<Self as frame_system::Config>::Event>;
    type Currency: MultiCurrencyExtended<Self::AccountId, CurrencyId = AssetId, Balance = Balance, Amount = i128>;
    type Prefix: Get<&'static [u8]>;
}

#[derive(Clone, Copy, PartialEq, Eq, Encode, Decode, Default)]
pub struct EthereumAddress(pub [u8; 20]);

#[cfg(feature = "std")]
impl Serialize for EthereumAddress {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
		let hex: String = rustc_hex::ToHex::to_hex(&self.0[..]);
		serializer.serialize_str(&format!("0x{}", hex))
	}
}

#[cfg(feature = "std")]
impl<'de> Deserialize<'de> for EthereumAddress {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
		let base_string = String::deserialize(deserializer)?;
		let offset = if base_string.starts_with("0x") { 2 } else { 0 };
		let s = &base_string[offset..];
		if s.len() != 40 {
			Err(serde::de::Error::custom("Bad length of Ethereum address (should be 42 including '0x')"))?;
		}
		let raw: Vec<u8> = rustc_hex::FromHex::from_hex(s)
			.map_err(|e| serde::de::Error::custom(format!("{:?}", e)))?;
		let mut r = Self::default();
		r.0.copy_from_slice(&raw);
		Ok(r)
	}
}

#[derive(Encode, Decode, Clone)]
pub struct EcdsaSignature(pub [u8; 65]);

impl PartialEq for EcdsaSignature {
	fn eq(&self, other: &Self) -> bool {
		&self.0[..] == &other.0[..]
	}
}

impl sp_std::fmt::Debug for EcdsaSignature {
	fn fmt(&self, f: &mut sp_std::fmt::Formatter<'_>) -> sp_std::fmt::Result {
		write!(f, "EcdsaSignature({:?})", &self.0[..])
	}
}

#[derive(Encode, Decode, Clone, frame_support::RuntimeDebug, PartialEq)]
pub enum StorageVersion {
	V1EmptyBalances,
	V2AddClaimData,
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
	pub enum Event<T> where AccountId = <T as frame_system::Config>::AccountId {
		HDXClaimed(AccountId),
	}
);

decl_error! {
	pub enum Error for Module<T: Config> {
        InvalidEthereumSignature,
        InvalidStatement,
        SignerHasNoClaim
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
		type Error = Error<T>;

		fn deposit_event() = default;

        /// The Prefix that is used in signed Ethereum messages for this network
        const Prefix: &[u8] = T::Prefix::get();

		#[weight = (0, Pays::No)]
		fn claim(origin, dest: T::AccountId, ethereum_signature: EcdsaSignature)  {
            ensure_signed(origin)?;

            let data = dest.using_encoded(to_ascii_hex);
			let signer = Self::eth_recover(&ethereum_signature, &data, &[][..])
				.ok_or(Error::<T>::InvalidEthereumSignature)?;
            
            Self::process_claim(signer, dest)?;
		}
	}
}

impl<T: Config> Module<T> {
    fn process_claim(signer: EthereumAddress, dest: T::AccountId) -> DispatchResult {
        
        let balance_due = Self::hdxclaims(signer);

        ensure!(balance_due != Zero::zero(), Error::<T>::SignerHasNoClaim);

        HDXClaims::insert(signer, 0);
        
        Self::deposit_event(RawEvent::HDXClaimed(dest));
        Ok(())
    }

    // Constructs the message that Ethereum RPC's `personal_sign` and `eth_sign` would sign.
	fn ethereum_signable_message(what: &[u8], extra: &[u8]) -> Vec<u8> {
		let prefix = T::Prefix::get();
		let mut l = prefix.len() + what.len() + extra.len();
		let mut rev = Vec::new();
		while l > 0 {
			rev.push(b'0' + (l % 10) as u8);
			l /= 10;
		}
		let mut v = b"\x19Ethereum Signed Message:\n".to_vec();
		v.extend(rev.into_iter().rev());
		v.extend_from_slice(&prefix[..]);
		v.extend_from_slice(what);
		v.extend_from_slice(extra);
		v
	}

    // Attempts to recover the Ethereum address from a message signature signed by using
	// the Ethereum RPC's `personal_sign` and `eth_sign`.
	fn eth_recover(s: &EcdsaSignature, what: &[u8], extra: &[u8]) -> Option<EthereumAddress> {
		let msg = keccak_256(&Self::ethereum_signable_message(what, extra));
		let mut res = EthereumAddress::default();
		res.0.copy_from_slice(&keccak_256(&secp256k1_ecdsa_recover(&s.0, &msg).ok()?[..])[12..]);
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