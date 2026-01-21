#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{format, vec};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::traits::Currency;
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, pallet_prelude::Weight, BoundedVec};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{AccountIdConversion, Saturating};
use sp_std::vec::Vec;

const MAX_SERIALIZED_OUTPUT_LENGTH: u32 = 4 * 1024 * 1024; // 4 MB

#[cfg(test)]
mod tests;

pub use pallet::*;

/// Bitcoin testnet vault address (P2WPKH pubkey hash - 20 bytes)
/// Derived from MPC root key + path "root" + sender btcVault pallet account
/// tb1qvakrvjwy5sj8a5q5laf9qulx8wa9cslyl73uwq
pub const TESTNET_VAULT_ADDRESS: [u8; 20] = [
	0x67, 0x6c, 0x36, 0x49, 0xc4, 0xa4, 0x24, 0x7e, 0xd0, 0x14, 0xff, 0x52, 0x50, 0x73, 0xe6, 0x3b, 0xba, 0x5c, 0x43,
	0xe4,
];

/// Bitcoin testnet CAIP-2 chain ID
const BITCOIN_TESTNET_CAIP2: &str = "bip122:000000000933ea01ad0ee984209779ba";

/// Hardcoded root path for withdrawals (shared vault address on Bitcoin)
const HARDCODED_ROOT_PATH: &[u8] = b"root";

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_io::hashing;

	// ========================= Configuration =========================

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type VaultPalletId: Get<PalletId>;

		/// Maximum number of Bitcoin inputs per transaction
		#[pallet::constant]
		type MaxBtcInputs: Get<u32>;

		/// Maximum number of Bitcoin outputs per transaction
		#[pallet::constant]
		type MaxBtcOutputs: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	// ========================= Storage =========================

	/// Global vault configuration
	#[pallet::storage]
	#[pallet::getter(fn vault_config)]
	pub type VaultConfig<T> = StorageValue<_, VaultConfigData, OptionQuery>;

	/// Pending deposits awaiting signature
	#[pallet::storage]
	#[pallet::getter(fn pending_deposits)]
	pub type PendingDeposits<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		[u8; 32], // request_id
		PendingDepositData<T::AccountId>,
		OptionQuery,
	>;

	/// User BTC balances (in satoshis)
	#[pallet::storage]
	#[pallet::getter(fn user_balances)]
	pub type UserBalances<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		u64, // satoshis
		ValueQuery,
	>;

	/// Pending withdrawals awaiting completion
	#[pallet::storage]
	#[pallet::getter(fn pending_withdrawals)]
	pub type PendingWithdrawals<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		[u8; 32], // request_id
		PendingWithdrawalData<T::AccountId>,
		OptionQuery,
	>;

	// ========================= Types =========================

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct VaultConfigData {
		pub mpc_root_signer_address: [u8; 20],
	}

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, MaxEncodedLen)]
	pub struct PendingDepositData<AccountId> {
		pub requester: AccountId,
		pub amount: u64, // satoshis
		pub path: BoundedVec<u8, ConstU32<256>>,
	}

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, MaxEncodedLen)]
	pub struct PendingWithdrawalData<AccountId> {
		pub requester: AccountId,
		pub amount: u64, // satoshis
		pub recipient_script_pubkey: BoundedVec<u8, ConstU32<64>>, // Bitcoin recipient script
	}

	// ========================= Events =========================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		VaultInitialized {
			mpc_address: [u8; 20],
			initialized_by: T::AccountId,
		},
		DepositRequested {
			request_id: [u8; 32],
			requester: T::AccountId,
			amount: u64,
		},
		DepositClaimed {
			request_id: [u8; 32],
			claimer: T::AccountId,
			amount: u64,
		},
		DebugTxid {
			txid: [u8; 32],
		},
		DebugTransaction {
			tx_hex: BoundedVec<u8, ConstU32<1024>>,
			version: u32,
			locktime: u32,
		},
		WithdrawalRequested {
			request_id: [u8; 32],
			requester: T::AccountId,
			amount: u64,
			recipient_script_pubkey: Vec<u8>,
		},
		WithdrawalCompleted {
			request_id: [u8; 32],
			requester: T::AccountId,
			amount: u64,
		},
		WithdrawalFailed {
			request_id: [u8; 32],
			requester: T::AccountId,
			amount: u64,
			refunded: bool,
		},
	}

	// ========================= Errors =========================

	#[pallet::error]
	pub enum Error<T> {
		NotInitialized,
		AlreadyInitialized,
		InvalidRequestId,
		DuplicateRequest,
		DepositNotFound,
		UnauthorizedClaimer,
		InvalidSignature,
		InvalidSigner,
		InvalidOutput,
		TransferFailed,
		Overflow,
		SerializationError,
		PathTooLong,
		PalletAccountNotFunded,
		NoVaultOutput,
		InvalidVaultOutput,
		TooManyInputs,
		TooManyOutputs,
		InsufficientBalance,
		Underflow,
		WithdrawalNotFound,
		InvalidRecipientScript,
	}

	// ========================= Hooks =========================

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	// ========================= Extrinsics =========================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Initialize the vault with MPC signer address
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn initialize(origin: OriginFor<T>, mpc_root_signer_address: [u8; 20]) -> DispatchResult {
			let initializer = ensure_signed(origin)?;
			ensure!(VaultConfig::<T>::get().is_none(), Error::<T>::AlreadyInitialized);

			VaultConfig::<T>::put(VaultConfigData {
				mpc_root_signer_address,
			});

			Self::deposit_event(Event::VaultInitialized {
				mpc_address: mpc_root_signer_address,
				initialized_by: initializer,
			});

			Ok(())
		}

		/// Request to deposit BTC tokens
		/// Note: The pallet account must be funded before calling this
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(100_000, 0))]
		pub fn deposit_btc(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			inputs: Vec<pallet_signet::UtxoInput>,
			outputs: Vec<pallet_signet::BitcoinOutput>,
			lock_time: u32,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			// Ensure vault is initialized
			ensure!(VaultConfig::<T>::get().is_some(), Error::<T>::NotInitialized);

			// Ensure no duplicate request
			ensure!(
				PendingDeposits::<T>::get(&request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			// Convert to bounded vecs
			let bounded_inputs: BoundedVec<pallet_signet::UtxoInput, pallet_signet::MaxBitcoinInputs> =
				inputs.clone().try_into().map_err(|_| Error::<T>::TooManyInputs)?;
			let bounded_outputs: BoundedVec<pallet_signet::BitcoinOutput, pallet_signet::MaxBitcoinOutputs> =
				outputs.clone().try_into().map_err(|_| Error::<T>::TooManyOutputs)?;

			// Get signet deposit amount and pallet account
			let signet_deposit = pallet_signet::Pallet::<T>::signature_deposit();
			let pallet_account = Self::account_id();
			let existential_deposit = <T as pallet_signet::Config>::Currency::minimum_balance();

			// Ensure pallet account has sufficient balance
			let pallet_balance = <T as pallet_signet::Config>::Currency::free_balance(&pallet_account);
			let required_balance = existential_deposit.saturating_add(signet_deposit);
			ensure!(pallet_balance >= required_balance, Error::<T>::PalletAccountNotFunded);

			// Transfer signet deposit from requester to pallet account
			<T as pallet_signet::Config>::Currency::transfer(
				&requester,
				&pallet_account,
				signet_deposit,
				frame_support::traits::ExistenceRequirement::AllowDeath,
			)?;

			// Find vault output and extract deposit amount
			let vault_script_pubkey = Self::create_p2wpkh_script(&TESTNET_VAULT_ADDRESS);
			let deposit_amount = outputs
				.iter()
				.find(|output| output.script_pubkey.to_vec() == vault_script_pubkey)
				.map(|output| output.value)
				.ok_or(Error::<T>::NoVaultOutput)?;

			ensure!(deposit_amount > 0, Error::<T>::InvalidVaultOutput);

			// Use requester account as path
			let path = {
				let encoded = requester.encode();
				format!("0x{}", hex::encode(encoded)).into_bytes()
			};

			// Build PSBT using signet's build_bitcoin_tx
			let psbt_bytes = pallet_signet::Pallet::<T>::build_bitcoin_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				bounded_inputs.clone(),
				bounded_outputs.clone(),
				lock_time,
			)?;

			let txid = pallet_signet::Pallet::<T>::get_bitcoin_txid(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				bounded_inputs,
				bounded_outputs,
				lock_time,
			)?;

			Self::deposit_event(Event::DebugTxid { txid });

			Self::deposit_event(Event::DebugTransaction {
				tx_hex: psbt_bytes.clone().try_into().unwrap_or_default(),
				version: 2,
				locktime: lock_time,
			});

			// Generate and verify request ID
			let computed_request_id = Self::generate_request_id(
				&Self::account_id(),
				&txid,
				BITCOIN_TESTNET_CAIP2,
				0,
				&path,
				b"ecdsa",
				b"bitcoin",
				b"",
			);

			ensure!(computed_request_id == request_id, Error::<T>::InvalidRequestId);

			// Store pending deposit
			PendingDeposits::<T>::insert(
				&request_id,
				PendingDepositData {
					requester: requester.clone(),
					amount: deposit_amount,
					path: path.clone().try_into().map_err(|_| Error::<T>::PathTooLong)?,
				},
			);

			// Create schemas for the response (simple boolean for Bitcoin)
			let explorer_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::SerializationError)?;

			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::SerializationError)?;

			// Call sign_bidirectional from the pallet account
			pallet_signet::Pallet::<T>::sign_bidirectional(
				frame_system::RawOrigin::Signed(Self::account_id()).into(),
				BoundedVec::try_from(psbt_bytes).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(BITCOIN_TESTNET_CAIP2.as_bytes().to_vec())
					.map_err(|_| Error::<T>::SerializationError)?,
				0, // key_version
				BoundedVec::try_from(path).map_err(|_| Error::<T>::PathTooLong)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(b"bitcoin".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::SerializationError)?,
			)?;

			Self::deposit_event(Event::DepositRequested {
				request_id,
				requester,
				amount: deposit_amount,
			});

			Ok(())
		}

		/// Claim deposited BTC tokens after signature verification
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(50_000, 0))]
		pub fn claim_btc(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			serialized_output: BoundedVec<u8, ConstU32<MAX_SERIALIZED_OUTPUT_LENGTH>>,
			signature: pallet_signet::Signature,
		) -> DispatchResult {
			let claimer = ensure_signed(origin)?;

			// Get pending deposit
			let pending = PendingDeposits::<T>::get(&request_id).ok_or(Error::<T>::DepositNotFound)?;

			// Verify claimer is the original requester
			ensure!(pending.requester == claimer, Error::<T>::UnauthorizedClaimer);

			// Get vault config
			let config = VaultConfig::<T>::get().ok_or(Error::<T>::NotInitialized)?;

			// Verify signature
			let message_hash = Self::hash_message(&request_id, &serialized_output);
			Self::verify_signature_from_address(&message_hash, &signature, &config.mpc_root_signer_address)?;

			// Check for error magic prefix
			const ERROR_PREFIX: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

			let success = if serialized_output.len() >= 4 && &serialized_output[..4] == ERROR_PREFIX {
				false
			} else {
				// Decode boolean (Borsh serialized)
				use borsh::BorshDeserialize;
				bool::try_from_slice(&serialized_output).map_err(|_| Error::<T>::InvalidOutput)?
			};

			ensure!(success, Error::<T>::TransferFailed);

			// Update user balance
			UserBalances::<T>::mutate(&claimer, |balance| -> DispatchResult {
				*balance = balance.checked_add(pending.amount).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			// Clean up storage
			PendingDeposits::<T>::remove(&request_id);

			Self::deposit_event(Event::DepositClaimed {
				request_id,
				claimer,
				amount: pending.amount,
			});

			Ok(())
		}

		/// Request to withdraw BTC tokens from the vault
		/// Uses optimistic decrement - balance is deducted immediately
		/// If the withdrawal fails on Bitcoin, it will be refunded in complete_withdraw_btc
		#[pallet::call_index(3)]
		#[pallet::weight(Weight::from_parts(100_000, 0))]
		pub fn withdraw_btc(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			amount: u64,
			recipient_script_pubkey: Vec<u8>,
			inputs: Vec<pallet_signet::UtxoInput>,
			outputs: Vec<pallet_signet::BitcoinOutput>,
			lock_time: u32,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			// Ensure vault is initialized
			ensure!(VaultConfig::<T>::get().is_some(), Error::<T>::NotInitialized);

			// Ensure no duplicate request
			ensure!(
				PendingWithdrawals::<T>::get(&request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			// Validate recipient script (basic sanity check)
			ensure!(!recipient_script_pubkey.is_empty() && recipient_script_pubkey.len() <= 64, Error::<T>::InvalidRecipientScript);

			// Check user has sufficient balance
			let current_balance = UserBalances::<T>::get(&requester);
			ensure!(current_balance >= amount, Error::<T>::InsufficientBalance);

			// Optimistically decrement the balance
			UserBalances::<T>::mutate(&requester, |balance| -> DispatchResult {
				*balance = balance.checked_sub(amount).ok_or(Error::<T>::Underflow)?;
				Ok(())
			})?;

			// Convert to bounded vecs
			let bounded_inputs: BoundedVec<pallet_signet::UtxoInput, pallet_signet::MaxBitcoinInputs> =
				inputs.clone().try_into().map_err(|_| Error::<T>::TooManyInputs)?;
			let bounded_outputs: BoundedVec<pallet_signet::BitcoinOutput, pallet_signet::MaxBitcoinOutputs> =
				outputs.clone().try_into().map_err(|_| Error::<T>::TooManyOutputs)?;

			// Verify one of the outputs goes to the recipient
			let has_recipient_output = outputs.iter().any(|o| o.script_pubkey.to_vec() == recipient_script_pubkey);
			ensure!(has_recipient_output, Error::<T>::InvalidRecipientScript);

			// Get signet deposit amount and pallet account
			let signet_deposit = pallet_signet::Pallet::<T>::signature_deposit();
			let pallet_account = Self::account_id();
			let existential_deposit = <T as pallet_signet::Config>::Currency::minimum_balance();

			// Ensure pallet account has sufficient balance
			let pallet_balance = <T as pallet_signet::Config>::Currency::free_balance(&pallet_account);
			let required_balance = existential_deposit.saturating_add(signet_deposit);
			ensure!(pallet_balance >= required_balance, Error::<T>::PalletAccountNotFunded);

			// Transfer signet deposit from requester to pallet account
			<T as pallet_signet::Config>::Currency::transfer(
				&requester,
				&pallet_account,
				signet_deposit,
				frame_support::traits::ExistenceRequirement::AllowDeath,
			)?;

			// Use hardcoded root path for withdrawals (shared vault address on Bitcoin)
			let path = HARDCODED_ROOT_PATH.to_vec();

			// Build PSBT
			let psbt_bytes = pallet_signet::Pallet::<T>::build_bitcoin_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				bounded_inputs.clone(),
				bounded_outputs.clone(),
				lock_time,
			)?;

			let txid = pallet_signet::Pallet::<T>::get_bitcoin_txid(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				bounded_inputs,
				bounded_outputs,
				lock_time,
			)?;

			// Generate and verify request ID
			let computed_request_id = Self::generate_request_id(
				&Self::account_id(),
				&txid,
				BITCOIN_TESTNET_CAIP2,
				0,
				&path,
				b"ecdsa",
				b"bitcoin",
				b"",
			);

			ensure!(computed_request_id == request_id, Error::<T>::InvalidRequestId);

			// Store pending withdrawal
			PendingWithdrawals::<T>::insert(
				&request_id,
				PendingWithdrawalData {
					requester: requester.clone(),
					amount,
					recipient_script_pubkey: recipient_script_pubkey.clone().try_into().map_err(|_| Error::<T>::InvalidRecipientScript)?,
				},
			);

			// Create schemas for the response
			let explorer_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::SerializationError)?;

			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::SerializationError)?;

			// Call sign_bidirectional from the pallet account
			pallet_signet::Pallet::<T>::sign_bidirectional(
				frame_system::RawOrigin::Signed(Self::account_id()).into(),
				BoundedVec::try_from(psbt_bytes).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(BITCOIN_TESTNET_CAIP2.as_bytes().to_vec())
					.map_err(|_| Error::<T>::SerializationError)?,
				0, // key_version
				BoundedVec::try_from(path).map_err(|_| Error::<T>::PathTooLong)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(b"bitcoin".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::SerializationError)?,
			)?;

			Self::deposit_event(Event::WithdrawalRequested {
				request_id,
				requester,
				amount,
				recipient_script_pubkey,
			});

			Ok(())
		}

		/// Complete a withdrawal after MPC signature verification
		/// If the withdrawal failed on Bitcoin (error prefix or success=false), refund the balance
		#[pallet::call_index(4)]
		#[pallet::weight(Weight::from_parts(50_000, 0))]
		pub fn complete_withdraw_btc(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			serialized_output: BoundedVec<u8, ConstU32<MAX_SERIALIZED_OUTPUT_LENGTH>>,
			signature: pallet_signet::Signature,
		) -> DispatchResult {
			let caller = ensure_signed(origin)?;

			// Get pending withdrawal
			let pending = PendingWithdrawals::<T>::get(&request_id).ok_or(Error::<T>::WithdrawalNotFound)?;

			// Verify caller is the original requester
			ensure!(pending.requester == caller, Error::<T>::UnauthorizedClaimer);

			// Get vault config
			let config = VaultConfig::<T>::get().ok_or(Error::<T>::NotInitialized)?;

			// Verify signature
			let message_hash = Self::hash_message(&request_id, &serialized_output);
			Self::verify_signature_from_address(&message_hash, &signature, &config.mpc_root_signer_address)?;

			// Check for error magic prefix
			const ERROR_PREFIX: [u8; 4] = [0xDE, 0xAD, 0xBE, 0xEF];

			let should_refund = if serialized_output.len() >= 4 && &serialized_output[..4] == ERROR_PREFIX {
				// Error response - refund
				true
			} else {
				// Decode boolean (Borsh serialized)
				use borsh::BorshDeserialize;
				let success = bool::try_from_slice(&serialized_output).map_err(|_| Error::<T>::InvalidOutput)?;
				!success // Refund if not successful
			};

			if should_refund {
				// Refund the balance
				UserBalances::<T>::mutate(&pending.requester, |balance| -> DispatchResult {
					*balance = balance.checked_add(pending.amount).ok_or(Error::<T>::Overflow)?;
					Ok(())
				})?;

				Self::deposit_event(Event::WithdrawalFailed {
					request_id,
					requester: pending.requester.clone(),
					amount: pending.amount,
					refunded: true,
				});
			} else {
				Self::deposit_event(Event::WithdrawalCompleted {
					request_id,
					requester: pending.requester.clone(),
					amount: pending.amount,
				});
			}

			// Clean up storage
			PendingWithdrawals::<T>::remove(&request_id);

			Ok(())
		}
	}

	// ========================= Helper Functions =========================

	impl<T: Config> Pallet<T> {
		fn generate_request_id(
			sender: &T::AccountId,
			txid: &[u8; 32],
			caip2_id: &str,
			key_version: u32,
			path: &[u8],
			algo: &[u8],
			dest: &[u8],
			params: &[u8],
		) -> [u8; 32] {
			use alloy_sol_types::SolValue;
			use sp_core::crypto::Ss58Codec;

			let encoded = sender.encode();
			let mut account_bytes = [0u8; 32];
			let len = encoded.len().min(32);
			account_bytes[..len].copy_from_slice(&encoded[..len]);

			let account_id32 = sp_runtime::AccountId32::from(account_bytes);
			let sender_ss58 = account_id32.to_ss58check_with_version(sp_core::crypto::Ss58AddressFormat::custom(0));

			let encoded = (
				sender_ss58.as_str(),
				txid.as_ref(),
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

		fn hash_message(request_id: &[u8; 32], output: &[u8]) -> [u8; 32] {
			let mut data = Vec::with_capacity(32 + output.len());
			data.extend_from_slice(request_id);
			data.extend_from_slice(output);
			hashing::keccak_256(&data)
		}

		fn verify_signature_from_address(
			message_hash: &[u8; 32],
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
			let recovered_address = &pubkey_hash[12..];

			ensure!(recovered_address == expected_address, Error::<T>::InvalidSigner);

			Ok(())
		}

		fn create_p2wpkh_script(pubkey_hash: &[u8; 20]) -> Vec<u8> {
			let mut script = Vec::with_capacity(22);
			script.push(0x00); // OP_0
			script.push(0x14); // Push 20 bytes
			script.extend_from_slice(pubkey_hash);
			script
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::VaultPalletId::get().into_account_truncating()
		}
	}
}
