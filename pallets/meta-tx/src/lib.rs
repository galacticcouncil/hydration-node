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

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use codec::{Encode, FullCodec};
	use frame_support::{
		dispatch::{GetDispatchInfo, PostDispatchInfo},
		pallet_prelude::*,
	};
	use frame_system::pallet_prelude::*;
	use sp_io::hashing::blake2_256;
	use sp_runtime::traits::{Dispatchable, IdentifyAccount, Verify, Zero};
	use sp_std::boxed::Box;

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
		/// A meta transaction was executed. `result` is the outcome of the inner call.
		Dispatched {
			signer: T::AccountId,
			relayer: T::AccountId,
			nonce: u32,
			result: DispatchResult,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Execute `call` under `signer`'s origin, charging the submitting relayer for the fee.
		///
		/// The nonce is consumed even when the inner call fails, so a rejected intent cannot be
		/// replayed. The inner outcome is reported in the `Dispatched` event rather than as an
		/// extrinsic error.
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

			ensure!(
				frame_system::Pallet::<T>::block_number() <= deadline,
				Error::<T>::Expired
			);
			ensure!(Nonces::<T>::get(&signer) == nonce, Error::<T>::InvalidNonce);

			let payload = Self::signing_payload(&call, &signer, nonce, deadline);
			ensure!(signature.verify(&payload[..], &signer), Error::<T>::InvalidSignature);

			Nonces::<T>::insert(&signer, nonce.saturating_add(1));

			let result = call.dispatch(frame_system::RawOrigin::Signed(signer.clone()).into());
			let inner_weight = match &result {
				Ok(post) => post.actual_weight,
				Err(e) => e.post_info.actual_weight,
			};

			Self::deposit_event(Event::Dispatched {
				signer,
				relayer,
				nonce,
				result: result.map(|_| ()).map_err(|e| e.error),
			});

			Ok(PostDispatchInfo {
				actual_weight: inner_weight.map(|w| w.saturating_add(T::WeightInfo::dispatch_meta_tx())),
				pays_fee: Pays::Yes,
			})
		}
	}

	impl<T: Config> Pallet<T> {
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
