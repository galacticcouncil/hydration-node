// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

//! Sponsored meta transactions for native Substrate accounts.
//!
//! A signer authorises a call off-chain; any account may submit it and pays the fee. The call
//! executes under the signer's own origin. The EVM equivalent is the call-permit precompile.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::useless_conversion)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

pub mod weights;

pub use pallet::*;
pub use weights::WeightInfo;

/// Domain tag mixed into every signed payload so a signature cannot be reused elsewhere.
pub const PAYLOAD_TAG: [u8; 12] = *b"hydra-metatx";

/// EIP-712 domain and message types for the EVM signing path.
pub const EIP712_DOMAIN_TYPE: &[u8] = b"EIP712Domain(string name,string version,uint256 chainId)";
pub const EIP712_DOMAIN_NAME: &[u8] = b"Hydration Meta Transaction";
pub const EIP712_DOMAIN_VERSION: &[u8] = b"1";
pub const EIP712_META_TX_TYPE: &[u8] =
	b"MetaTransaction(address from,bytes call,uint256 nonce,uint256 deadline,uint256 specVersion)";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{Encode, FullCodec};
	use frame_support::{
		dispatch::{DispatchErrorWithPostInfo, GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;
	use hydradx_traits::evm::{EvmFeePayerSupport, InspectEvmAccounts};
	use sp_core::{H160, H256};
	use sp_io::hashing::{blake2_256, keccak_256};
	use sp_runtime::traits::{Dispatchable, IdentifyAccount, UniqueSaturatedInto, Verify, Zero};
	use sp_std::boxed::Box;
	use sp_std::vec::Vec;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching call type.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ FullCodec
			+ TypeInfo
			+ From<frame_system::Call<Self>>
			+ IsType<<Self as frame_system::Config>::RuntimeCall>;

		/// Signature type accepted from meta transaction signers.
		type Signature: Parameter + Verify<Signer = Self::Signer>;

		/// Public key type that resolves to an `AccountId`.
		type Signer: IdentifyAccount<AccountId = Self::AccountId> + Parameter;

		/// Resolves an EVM address to the account its calls execute under.
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// Redirects EVM gas to the relayer for the duration of the inner call.
		type EvmFeePayer: EvmFeePayerSupport<AccountId = Self::AccountId>;

		/// EVM chain id, bound into the EIP-712 domain separator.
		type ChainId: Get<u64>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Meta transaction nonce per signer, independent of the system nonce.
	#[pallet::storage]
	#[pallet::getter(fn nonce)]
	pub type Nonces<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u32, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// A meta transaction was executed.
		Dispatched {
			signer: T::AccountId,
			relayer: T::AccountId,
			nonce: u32,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// The signature does not match the signer, the call, this chain or this runtime version.
		InvalidSignature,
		/// The supplied nonce is not the signer's current meta transaction nonce.
		InvalidNonce,
		/// The deadline block has already passed.
		Expired,
		/// The signer's meta transaction nonce cannot be advanced any further.
		NonceExhausted,
		/// The secp256k1 signature does not recover to the given EVM address.
		InvalidEvmSignature,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Execute `call` under `signer`'s origin, charging the submitting relayer for the fee.
		///
		/// A failing inner call is returned as an extrinsic error, which reverts the nonce with it.
		/// Such an intent stays valid until its deadline.
		///
		/// Parameters:
		/// - `origin`: any signed account; it pays the transaction fee.
		/// - `signer`: the account that authorised `call` and under whose origin it runs.
		/// - `call`: the call to execute.
		/// - `nonce`: must equal the signer's current meta transaction nonce.
		/// - `deadline`: last block number at which the signature remains valid.
		/// - `signature`: `signer`'s signature over `signing_payload`.
		///
		/// Emits `Dispatched` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			T::WeightInfo::dispatch_meta_tx().saturating_add(call_weight)
		})]
		pub fn dispatch_meta_tx(
			origin: OriginFor<T>,
			signer: T::AccountId,
			call: Box<<T as Config>::RuntimeCall>,
			nonce: u32,
			deadline: BlockNumberFor<T>,
			signature: T::Signature,
		) -> DispatchResultWithPostInfo {
			let relayer = ensure_signed(origin)?;
			let base_weight = T::WeightInfo::dispatch_meta_tx();

			let next_nonce = Self::admit(&signer, nonce, deadline, base_weight)?;

			let payload = Self::signing_payload(&call, &signer, nonce, deadline);
			if !signature.verify(&payload[..], &signer) {
				return Err(Self::rejected(Error::<T>::InvalidSignature, base_weight));
			}

			Self::execute(relayer, signer, *call, nonce, next_nonce, base_weight)
		}

		/// Execute `call` under the account of an EVM address that authorised it off-chain,
		/// charging the submitting relayer for both the extrinsic fee and any inner EVM gas.
		///
		/// `from` signs an EIP-712 `MetaTransaction` payload with a secp256k1 key, so wallets
		/// that hold no Substrate key can still originate Substrate calls.
		///
		/// Parameters:
		/// - `origin`: any signed account; it pays the transaction fee and the EVM gas.
		/// - `from`: the EVM address that authorised `call`.
		/// - `call`: the call to execute.
		/// - `nonce`: must equal the current meta transaction nonce of `from`'s account.
		/// - `deadline`: last block number at which the signature remains valid.
		/// - `v`: recovery id of the signature, `0` or `1`.
		/// - `r`, `s`: the secp256k1 signature components.
		///
		/// Emits `Dispatched` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight({
			let call_weight = call.get_dispatch_info().call_weight;
			T::WeightInfo::dispatch_evm_meta_tx().saturating_add(call_weight)
		})]
		#[allow(clippy::too_many_arguments)]
		pub fn dispatch_evm_meta_tx(
			origin: OriginFor<T>,
			from: H160,
			call: Box<<T as Config>::RuntimeCall>,
			nonce: u32,
			deadline: BlockNumberFor<T>,
			v: u8,
			r: H256,
			s: H256,
		) -> DispatchResultWithPostInfo {
			let relayer = ensure_signed(origin)?;
			let base_weight = T::WeightInfo::dispatch_evm_meta_tx();
			let signer = T::EvmAccounts::account_id(from);

			let next_nonce = Self::admit(&signer, nonce, deadline, base_weight)?;

			let payload = Self::evm_signing_payload(&call, from, nonce, deadline);
			let mut signature = [0u8; 65];
			signature[..32].copy_from_slice(r.as_bytes());
			signature[32..64].copy_from_slice(s.as_bytes());
			signature[64] = v;

			let public = sp_io::crypto::secp256k1_ecdsa_recover(&signature, &payload)
				.map_err(|_| Self::rejected(Error::<T>::InvalidEvmSignature, base_weight))?;
			if H160::from(H256::from_slice(&keccak_256(&public))) != from {
				return Err(Self::rejected(Error::<T>::InvalidEvmSignature, base_weight));
			}

			Self::execute(relayer, signer, *call, nonce, next_nonce, base_weight)
		}
	}

	impl<T: Config> Pallet<T> {
		fn rejected(error: Error<T>, weight: Weight) -> DispatchErrorWithPostInfo {
			DispatchErrorWithPostInfo {
				post_info: PostDispatchInfo {
					actual_weight: Some(weight),
					pays_fee: Pays::Yes,
				},
				error: error.into(),
			}
		}

		fn admit(
			signer: &T::AccountId,
			nonce: u32,
			deadline: BlockNumberFor<T>,
			weight: Weight,
		) -> Result<u32, DispatchErrorWithPostInfo> {
			if frame_system::Pallet::<T>::block_number() > deadline {
				return Err(Self::rejected(Error::<T>::Expired, weight));
			}
			if Nonces::<T>::get(signer) != nonce {
				return Err(Self::rejected(Error::<T>::InvalidNonce, weight));
			}
			nonce
				.checked_add(1)
				.ok_or_else(|| Self::rejected(Error::<T>::NonceExhausted, weight))
		}

		fn execute(
			relayer: T::AccountId,
			signer: T::AccountId,
			call: <T as Config>::RuntimeCall,
			nonce: u32,
			next_nonce: u32,
			weight: Weight,
		) -> DispatchResultWithPostInfo {
			Nonces::<T>::insert(&signer, next_nonce);

			let previous_payer = T::EvmFeePayer::set_fee_payer(relayer.clone());
			let result = call.dispatch(frame_system::RawOrigin::Signed(signer.clone()).into());
			match previous_payer {
				Some(payer) => {
					let _ = T::EvmFeePayer::set_fee_payer(payer);
				}
				None => {
					let _ = T::EvmFeePayer::clear_fee_payer();
				}
			}

			let inner_weight = match &result {
				Ok(post) => post.actual_weight,
				Err(e) => e.post_info.actual_weight,
			};
			let post_info = PostDispatchInfo {
				actual_weight: inner_weight.map(|w| w.saturating_add(weight)),
				pays_fee: Pays::Yes,
			};

			match result {
				Ok(_) => {
					Self::deposit_event(Event::Dispatched { signer, relayer, nonce });
					Ok(post_info)
				}
				Err(e) => Err(DispatchErrorWithPostInfo {
					post_info,
					error: e.error,
				}),
			}
		}

		fn word(value: u128) -> [u8; 32] {
			let mut word = [0u8; 32];
			word[16..].copy_from_slice(&value.to_be_bytes());
			word
		}

		fn address_word(address: H160) -> [u8; 32] {
			let mut word = [0u8; 32];
			word[12..].copy_from_slice(address.as_bytes());
			word
		}

		/// The 32 bytes an EVM key must sign, as an EIP-712 typed data digest.
		pub fn evm_signing_payload(
			call: &<T as Config>::RuntimeCall,
			from: H160,
			nonce: u32,
			deadline: BlockNumberFor<T>,
		) -> [u8; 32] {
			let mut domain = Vec::with_capacity(128);
			domain.extend_from_slice(&keccak_256(EIP712_DOMAIN_TYPE));
			domain.extend_from_slice(&keccak_256(EIP712_DOMAIN_NAME));
			domain.extend_from_slice(&keccak_256(EIP712_DOMAIN_VERSION));
			domain.extend_from_slice(&Self::word(T::ChainId::get().into()));

			let mut message = Vec::with_capacity(192);
			message.extend_from_slice(&keccak_256(EIP712_META_TX_TYPE));
			message.extend_from_slice(&Self::address_word(from));
			message.extend_from_slice(&keccak_256(&call.encode()));
			message.extend_from_slice(&Self::word(nonce.into()));
			message.extend_from_slice(&Self::word(UniqueSaturatedInto::<u128>::unique_saturated_into(
				deadline,
			)));
			message.extend_from_slice(&Self::word(
				<T as frame_system::Config>::Version::get().spec_version.into(),
			));

			let mut digest = Vec::with_capacity(66);
			digest.extend_from_slice(&[0x19, 0x01]);
			digest.extend_from_slice(&keccak_256(&domain));
			digest.extend_from_slice(&keccak_256(&message));
			keccak_256(&digest)
		}

		/// The 32 bytes a signer must sign, bound to this chain and runtime version.
		pub fn signing_payload(
			call: &<T as Config>::RuntimeCall,
			signer: &T::AccountId,
			nonce: u32,
			deadline: BlockNumberFor<T>,
		) -> [u8; 32] {
			(
				PAYLOAD_TAG,
				call,
				signer,
				nonce,
				deadline,
				frame_system::Pallet::<T>::block_hash(BlockNumberFor::<T>::zero()),
				<T as frame_system::Config>::Version::get().spec_version,
			)
				.using_encoded(blake2_256)
		}
	}
}
