#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::PalletId;
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AccountIdConversion, Saturating};
use sp_std::vec::Vec;

pub mod benchmarking;
pub mod types;
pub mod weights;

#[cfg(test)]
pub mod tests;

pub use pallet::*;
pub use types::*;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::dispatch::DispatchResult;
	use frame_support::traits::Currency;
	use sp_io::hashing;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type PalletId: Get<PalletId>;

		#[pallet::constant]
		type BitcoinCaip2: Get<&'static str>;

		#[pallet::constant]
		type MpcRootSignerAddress: Get<[u8; 20]>;

		#[pallet::constant]
		type VaultPubkeyHash: Get<[u8; 20]>;

		#[pallet::constant]
		type KeyVersion: Get<u32>;

		type WeightInfo: crate::WeightInfo;
	}

	#[pallet::storage]
	#[pallet::getter(fn pallet_config)]
	pub type PalletConfig<T> = StorageValue<_, PalletConfigData, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn pending_deposits)]
	pub type PendingDeposits<T: Config> =
		StorageMap<_, Blake2_128Concat, Bytes32, PendingDepositData<T::AccountId>, OptionQuery>;

	#[pallet::storage]
	pub type UsedRequestIds<T: Config> = StorageMap<_, Blake2_128Concat, Bytes32, (), OptionQuery>;

	#[pallet::storage]
	pub type PendingWithdrawals<T: Config> =
		StorageMap<_, Blake2_128Concat, Bytes32, PendingWithdrawalData<T::AccountId>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn user_balances)]
	pub type UserBalances<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, u64, ValueQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Paused,
		Unpaused,
		DepositRequested {
			request_id: Bytes32,
			requester: T::AccountId,
			amount_sats: u64,
			txid: Bytes32,
		},
		DepositClaimed {
			request_id: Bytes32,
			claimer: T::AccountId,
			amount_sats: u64,
		},
		WithdrawalRequested {
			request_id: Bytes32,
			requester: T::AccountId,
			amount_sats: u64,
		},
		WithdrawalCompleted {
			request_id: Bytes32,
			requester: T::AccountId,
			amount_sats: u64,
		},
		WithdrawalFailed {
			request_id: Bytes32,
			requester: T::AccountId,
			amount_sats: u64,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		Paused,
		DuplicateRequest,
		InvalidRequestId,
		DepositNotFound,
		UnauthorizedClaimer,
		InvalidSignature,
		InvalidSigner,
		Serialization,
		InvalidOutput,
		TransferFailed,
		Overflow,
		PathTooLong,
		PalletAccountNotFunded,
		NoVaultOutput,
		InvalidVaultOutput,
		SerializationError,
		InsufficientBalance,
		WithdrawalNotFound,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::pause())]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			<T as pallet::Config>::UpdateOrigin::ensure_origin(origin)?;
			if PalletConfig::<T>::get().is_none() {
				PalletConfig::<T>::put(PalletConfigData { paused: true });
			} else {
				PalletConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = true);
			};
			Self::deposit_event(Event::Paused);
			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::unpause())]
		pub fn unpause(origin: OriginFor<T>) -> DispatchResult {
			<T as pallet::Config>::UpdateOrigin::ensure_origin(origin)?;
			if PalletConfig::<T>::get().is_none() {
				PalletConfig::<T>::put(PalletConfigData { paused: false });
			} else {
				PalletConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = false);
			};
			Self::deposit_event(Event::Unpaused);
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::request_deposit())]
		pub fn request_deposit(
			origin: OriginFor<T>,
			request_id: Bytes32,
			inputs: BoundedVec<pallet_signet::UtxoInput, T::MaxInputs>,
			outputs: BoundedVec<pallet_signet::BitcoinOutput, T::MaxOutputs>,
			lock_time: u32,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			Self::ensure_not_paused()?;

			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);
			ensure!(
				PendingDeposits::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			let pallet_acc = Self::account_id();

			let signet_deposit = pallet_signet::Pallet::<T>::signature_deposit();
			let existential_deposit = <T as pallet_signet::Config>::Currency::minimum_balance();
			let pallet_balance = <T as pallet_signet::Config>::Currency::free_balance(&pallet_acc);
			let required_balance = existential_deposit.saturating_add(signet_deposit);
			ensure!(pallet_balance >= required_balance, Error::<T>::PalletAccountNotFunded);

			let vault_script = Self::create_p2wpkh_script(&T::VaultPubkeyHash::get());
			let deposit_amount = outputs
				.iter()
				.find(|o| o.script_pubkey.to_vec() == vault_script)
				.map(|o| o.value)
				.ok_or(Error::<T>::NoVaultOutput)?;
			ensure!(deposit_amount > 0, Error::<T>::InvalidVaultOutput);

			let path = Self::build_path(&requester);

			let psbt_bytes = pallet_signet::Pallet::<T>::build_bitcoin_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				inputs.clone(),
				outputs.clone(),
				lock_time,
			)?;

			let txid = pallet_signet::Pallet::<T>::get_txid(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				inputs,
				outputs,
				lock_time,
			)?;

			let derived = Self::generate_request_id(
				&pallet_acc,
				txid.as_ref(),
				T::BitcoinCaip2::get(),
				T::KeyVersion::get(),
				&path,
				ECDSA,
				BITCOIN,
				b"",
			);

			ensure!(derived == request_id, Error::<T>::InvalidRequestId);

			<T as pallet_signet::Config>::Currency::transfer(
				&requester,
				&pallet_acc,
				signet_deposit,
				frame_support::traits::ExistenceRequirement::AllowDeath,
			)?;

			PendingDeposits::<T>::insert(
				request_id,
				PendingDepositData {
					requester: requester.clone(),
					amount_sats: deposit_amount,
					txid,
					path: path.clone().try_into().map_err(|_| Error::<T>::PathTooLong)?,
				},
			);

			let explorer_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;
			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			pallet_signet::Pallet::<T>::sign_bidirectional(
				frame_system::RawOrigin::Signed(Self::account_id()).into(),
				BoundedVec::try_from(psbt_bytes).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(T::BitcoinCaip2::get().as_bytes().to_vec())
					.map_err(|_| Error::<T>::SerializationError)?,
				0,
				BoundedVec::try_from(path).map_err(|_| Error::<T>::PathTooLong)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(b"bitcoin".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::SerializationError)?,
			)?;

			UsedRequestIds::<T>::insert(request_id, ());

			Self::deposit_event(Event::DepositRequested {
				request_id,
				requester,
				amount_sats: deposit_amount,
				txid,
			});

			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::claim_deposit())]
		pub fn claim_deposit(
			origin: OriginFor<T>,
			request_id: Bytes32,
			serialized_output: BoundedVec<u8, ConstU32<{ MAX_SERIALIZED_OUTPUT_LENGTH }>>,
			signature: pallet_signet::Signature,
		) -> DispatchResult {
			let claimer = ensure_signed(origin)?;

			let pending = PendingDeposits::<T>::get(request_id).ok_or(Error::<T>::DepositNotFound)?;
			ensure!(pending.requester == claimer, Error::<T>::UnauthorizedClaimer);

			#[cfg(not(any(feature = "runtime-benchmarks", test)))]
			{
				let msg_hash = Self::hash_message(&request_id, &serialized_output);
				Self::verify_signature_from_address(&msg_hash, &signature, &T::MpcRootSignerAddress::get())?;
			}
			#[cfg(any(feature = "runtime-benchmarks", test))]
			let _ = &signature;

			let success = Self::decode_success(&serialized_output)?;
			ensure!(success, Error::<T>::TransferFailed);

			UserBalances::<T>::mutate(&claimer, |bal| -> DispatchResult {
				*bal = bal.checked_add(pending.amount_sats).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			PendingDeposits::<T>::remove(request_id);

			Self::deposit_event(Event::DepositClaimed {
				request_id,
				claimer,
				amount_sats: pending.amount_sats,
			});

			Ok(())
		}

		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_btc())]
		pub fn withdraw_btc(
			origin: OriginFor<T>,
			request_id: Bytes32,
			amount: u64,
			recipient_script: BoundedVec<u8, pallet_signet::types::MaxScriptLength>,
			inputs: BoundedVec<pallet_signet::UtxoInput, T::MaxInputs>,
			outputs: BoundedVec<pallet_signet::BitcoinOutput, T::MaxOutputs>,
			lock_time: u32,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			Self::ensure_not_paused()?;

			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);
			ensure!(
				PendingWithdrawals::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			let pallet_acc = Self::account_id();

			let signet_deposit = pallet_signet::Pallet::<T>::signature_deposit();
			let existential_deposit = <T as pallet_signet::Config>::Currency::minimum_balance();
			let pallet_balance = <T as pallet_signet::Config>::Currency::free_balance(&pallet_acc);
			let required_balance = existential_deposit.saturating_add(signet_deposit);
			ensure!(pallet_balance >= required_balance, Error::<T>::PalletAccountNotFunded);

			// Verify recipient output exists in the transaction
			let has_recipient = outputs
				.iter()
				.any(|o| o.script_pubkey.to_vec() == recipient_script.to_vec());
			ensure!(has_recipient, Error::<T>::InvalidOutput);

			let txid = pallet_signet::Pallet::<T>::get_txid(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				inputs.clone(),
				outputs.clone(),
				lock_time,
			)?;

			let path = WITHDRAWAL_PATH;

			let derived = Self::generate_request_id(
				&pallet_acc,
				txid.as_ref(),
				T::BitcoinCaip2::get(),
				T::KeyVersion::get(),
				path,
				ECDSA,
				BITCOIN,
				b"",
			);

			ensure!(derived == request_id, Error::<T>::InvalidRequestId);

			// Optimistically decrement balance
			UserBalances::<T>::mutate(&requester, |bal| -> DispatchResult {
				*bal = bal.checked_sub(amount).ok_or(Error::<T>::InsufficientBalance)?;
				Ok(())
			})?;

			<T as pallet_signet::Config>::Currency::transfer(
				&requester,
				&pallet_acc,
				signet_deposit,
				frame_support::traits::ExistenceRequirement::AllowDeath,
			)?;

			PendingWithdrawals::<T>::insert(
				request_id,
				PendingWithdrawalData {
					requester: requester.clone(),
					amount_sats: amount,
				},
			);

			let psbt_bytes = pallet_signet::Pallet::<T>::build_bitcoin_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				inputs,
				outputs,
				lock_time,
			)?;

			let explorer_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;
			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			pallet_signet::Pallet::<T>::sign_bidirectional(
				frame_system::RawOrigin::Signed(Self::account_id()).into(),
				BoundedVec::try_from(psbt_bytes).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(T::BitcoinCaip2::get().as_bytes().to_vec())
					.map_err(|_| Error::<T>::SerializationError)?,
				0,
				BoundedVec::try_from(path.to_vec()).map_err(|_| Error::<T>::PathTooLong)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(b"bitcoin".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::SerializationError)?,
			)?;

			UsedRequestIds::<T>::insert(request_id, ());

			Self::deposit_event(Event::WithdrawalRequested {
				request_id,
				requester,
				amount_sats: amount,
			});

			Ok(())
		}

		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::complete_withdraw_btc())]
		pub fn complete_withdraw_btc(
			origin: OriginFor<T>,
			request_id: Bytes32,
			serialized_output: BoundedVec<u8, ConstU32<{ MAX_SERIALIZED_OUTPUT_LENGTH }>>,
			signature: pallet_signet::Signature,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			let pending = PendingWithdrawals::<T>::get(request_id).ok_or(Error::<T>::WithdrawalNotFound)?;
			ensure!(pending.requester == caller, Error::<T>::UnauthorizedClaimer);

			#[cfg(not(any(feature = "runtime-benchmarks", test)))]
			{
				let msg_hash = Self::hash_message(&request_id, &serialized_output);
				Self::verify_signature_from_address(&msg_hash, &signature, &T::MpcRootSignerAddress::get())?;
			}
			#[cfg(any(feature = "runtime-benchmarks", test))]
			let _ = &signature;

			let is_error = serialized_output.len() >= 4 && serialized_output[..4] == ERROR_PREFIX;

			if is_error {
				// Refund the user
				UserBalances::<T>::mutate(&pending.requester, |bal| -> DispatchResult {
					*bal = bal.checked_add(pending.amount_sats).ok_or(Error::<T>::Overflow)?;
					Ok(())
				})?;

				PendingWithdrawals::<T>::remove(request_id);

				Self::deposit_event(Event::WithdrawalFailed {
					request_id,
					requester: pending.requester,
					amount_sats: pending.amount_sats,
				});
			} else {
				let success = Self::decode_success(&serialized_output)?;
				if success {
					PendingWithdrawals::<T>::remove(request_id);

					Self::deposit_event(Event::WithdrawalCompleted {
						request_id,
						requester: pending.requester,
						amount_sats: pending.amount_sats,
					});
				} else {
					// BTC transaction failed, refund
					UserBalances::<T>::mutate(&pending.requester, |bal| -> DispatchResult {
						*bal = bal.checked_add(pending.amount_sats).ok_or(Error::<T>::Overflow)?;
						Ok(())
					})?;

					PendingWithdrawals::<T>::remove(request_id);

					Self::deposit_event(Event::WithdrawalFailed {
						request_id,
						requester: pending.requester,
						amount_sats: pending.amount_sats,
					});
				}
			}

			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		#[inline]
		pub(crate) fn ensure_not_paused() -> Result<(), Error<T>> {
			match PalletConfig::<T>::get() {
				Some(PalletConfigData { paused: true, .. }) => Err(Error::<T>::Paused),
				_ => Ok(()),
			}
		}

		fn build_path(who: &T::AccountId) -> Vec<u8> {
			let enc = who.encode();
			let mut path = Vec::with_capacity(2 + enc.len() * 2);
			path.extend_from_slice(b"0x");
			path.extend_from_slice(hex::encode(enc).as_bytes());
			path
		}

		fn sender_ss58(sender: &T::AccountId) -> alloc::string::String {
			use sp_core::crypto::Ss58Codec;

			let encoded = sender.encode();
			let mut bytes = [0u8; 32];
			let len = encoded.len().min(32);
			bytes[..len].copy_from_slice(&encoded[..len]);

			let account_id32 = sp_runtime::AccountId32::from(bytes);
			account_id32.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(0))
		}

		#[allow(clippy::too_many_arguments)]
		pub fn generate_request_id(
			sender: &T::AccountId,
			transaction_data: &[u8],
			caip2_id: &str,
			key_version: u32,
			path: &[u8],
			algo: &[u8],
			dest: &[u8],
			params: &[u8],
		) -> Bytes32 {
			use alloy_sol_types::SolValue;

			let sender_ss58 = Self::sender_ss58(sender);

			let encoded = (
				sender_ss58.as_str(),
				transaction_data,
				caip2_id,
				key_version,
				core::str::from_utf8(path).unwrap_or(""),
				core::str::from_utf8(algo).unwrap_or(""),
				core::str::from_utf8(dest).unwrap_or(""),
				core::str::from_utf8(params).unwrap_or(""),
			)
				.abi_encode_packed();

			sp_io::hashing::keccak_256(&encoded)
		}

		#[allow(dead_code)]
		fn hash_message(request_id: &Bytes32, output: &[u8]) -> Bytes32 {
			let mut data = Vec::with_capacity(32 + output.len());
			data.extend_from_slice(request_id);
			data.extend_from_slice(output);
			hashing::keccak_256(&data)
		}

		#[allow(dead_code)]
		fn verify_signature_from_address(
			message_hash: &Bytes32,
			signature: &pallet_signet::Signature,
			expected_address: &[u8; 20],
		) -> DispatchResult {
			ensure!(signature.recovery_id < 4, Error::<T>::InvalidSignature);

			let mut sig_bytes = [0u8; 65];
			sig_bytes[..32].copy_from_slice(&signature.big_r.x);
			sig_bytes[32..64].copy_from_slice(&signature.s);
			sig_bytes[64] = signature.recovery_id;

			let pubkey = sp_io::crypto::secp256k1_ecdsa_recover(&sig_bytes, message_hash)
				.map_err(|_| Error::<T>::InvalidSignature)?;

			let pubkey_hash = hashing::keccak_256(&pubkey);
			let recovered = &pubkey_hash[12..];

			ensure!(recovered == expected_address, Error::<T>::InvalidSigner);
			Ok(())
		}

		pub(crate) fn create_p2wpkh_script(pubkey_hash: &[u8; 20]) -> Vec<u8> {
			let mut script = Vec::with_capacity(22);
			script.push(0x00);
			script.push(0x14);
			script.extend_from_slice(pubkey_hash);
			script
		}

		pub(crate) fn decode_success(out: &[u8]) -> Result<bool, Error<T>> {
			use borsh::BorshDeserialize;
			bool::try_from_slice(out).map_err(|_| Error::<T>::InvalidOutput)
		}

		pub fn account_id() -> T::AccountId {
			<T as Config>::PalletId::get().into_account_truncating()
		}
	}
}
