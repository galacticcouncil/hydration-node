#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{format, string::String, vec};

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::traits::Currency;
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, pallet_prelude::Weight, BoundedVec};
use frame_system::pallet_prelude::*;
use sp_core::H160;
use sp_runtime::traits::{AccountIdConversion, Saturating, Zero};
use sp_std::vec::Vec;

const MAX_SERIALIZED_OUTPUT_LENGTH: u32 = 65536;

#[cfg(test)]
mod tests;

pub use pallet::*;

pub const SEPOLIA_VAULT_ADDRESS: [u8; 20] = [
	0x00, 0xA4, 0x0C, 0x26, 0x61, 0x29, 0x3d, 0x51, 0x34, 0xE5, 0x3D, 0xa5, 0x29, 0x51, 0xA3, 0xF7, 0x76, 0x78, 0x36,
	0xEf,
];

// ERC20 ABI definition
use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};

sol! {
	#[sol(abi)]
	interface IERC20 {
		function transfer(address to, uint256 amount) external returns (bool);
	}
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_io::hashing;

	// ========================= Configuration =========================

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_build_evm_tx::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type VaultPalletId: Get<PalletId>;
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

	/// User ERC20 balances
	#[pallet::storage]
	#[pallet::getter(fn user_balances)]
	pub type UserBalances<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Blake2_128Concat,
		[u8; 20], // ERC20 address
		u128,
		ValueQuery,
	>;

	// ========================= Types =========================

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct VaultConfigData {
		pub mpc_root_signer_address: [u8; 20],
	}

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, MaxEncodedLen)]
	pub struct PendingDepositData<AccountId> {
		pub requester: AccountId,
		pub amount: u128,
		pub erc20_address: [u8; 20],
		pub path: BoundedVec<u8, ConstU32<256>>,
	}

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq)]
	pub struct EvmTransactionParams {
		pub value: u128,
		pub gas_limit: u64,
		pub max_fee_per_gas: u128,
		pub max_priority_fee_per_gas: u128,
		pub nonce: u64,
		pub chain_id: u64,
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
			erc20_address: [u8; 20],
			amount: u128,
		},
		DepositClaimed {
			request_id: [u8; 32],
			claimer: T::AccountId,
			erc20_address: [u8; 20],
			amount: u128,
		},
	}

	// ========================= Errors =========================

	#[pallet::error]
	pub enum Error<T> {
		NotInitialized,
		AlreadyInitialized,
		InvalidRequestId,
		DepositNotFound,
		UnauthorizedClaimer,
		InvalidSignature,
		InvalidSigner,
		InvalidOutput,
		TransferFailed,
		Overflow,
		InvalidAbi,
		SerializationError,
		PathTooLong,
		PalletAccountNotFunded,
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

		/// Request to deposit ERC20 tokens
		/// Note: The pallet account must be funded before calling this
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(100_000, 0))]
		pub fn deposit_erc20(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			erc20_address: [u8; 20],
			amount: u128,
			tx_params: EvmTransactionParams,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			// Ensure vault is initialized
			ensure!(VaultConfig::<T>::get().is_some(), Error::<T>::NotInitialized);

			// Ensure no duplicate request
			ensure!(
				PendingDeposits::<T>::get(&request_id).is_none(),
				Error::<T>::InvalidRequestId
			);

			// Get signet deposit amount and pallet account
			let signet_deposit = pallet_signet::Pallet::<T>::signature_deposit();
			let pallet_account = Self::account_id();
			let existential_deposit = <T as pallet_signet::Config>::Currency::minimum_balance();

			// Ensure pallet account has sufficient balance
			// It needs at least ED + signet_deposit to transfer signet_deposit while staying alive
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

			// Use requester account as path
			let path = {
				let encoded = requester.encode();
				format!("0x{}", hex::encode(encoded)).into_bytes()
			};

			let recipient = Address::from_slice(&SEPOLIA_VAULT_ADDRESS);
			let call = IERC20::transferCall {
				to: recipient,
				amount: U256::from(amount),
			};

			// Build EVM transaction
			let rlp_encoded = pallet_build_evm_tx::Pallet::<T>::build_evm_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				Some(H160::from(erc20_address)),
				tx_params.value,
				call.abi_encode(),
				tx_params.nonce,
				tx_params.gas_limit,
				tx_params.max_fee_per_gas,
				tx_params.max_priority_fee_per_gas,
				vec![],
				tx_params.chain_id,
			)?;

			// Generate and verify request ID
			let computed_request_id = Self::generate_request_id(
				&Self::account_id(),
				&rlp_encoded,
				60,
				0,
				&path,
				b"ecdsa",
				b"ethereum",
				b"",
			);

			ensure!(computed_request_id == request_id, Error::<T>::InvalidRequestId);

			// Store pending deposit
			PendingDeposits::<T>::insert(
				&request_id,
				PendingDepositData {
					requester: requester.clone(),
					amount,
					erc20_address,
					path: path.clone().try_into().map_err(|_| Error::<T>::PathTooLong)?,
				},
			);

			// Create schemas for the response
			let functions = IERC20::abi::functions();
			let transfer_func = functions
				.get("transfer")
				.and_then(|funcs| funcs.first())
				.ok_or(Error::<T>::InvalidAbi)?;

			let explorer_schema =
				serde_json::to_vec(&transfer_func.outputs).map_err(|_| Error::<T>::SerializationError)?;

			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::SerializationError)?;

			// Call sign_respond from the pallet account
			pallet_signet::Pallet::<T>::sign_respond(
				frame_system::RawOrigin::Signed(Self::account_id()).into(),
				BoundedVec::try_from(rlp_encoded).map_err(|_| Error::<T>::SerializationError)?,
				60,
				0,
				BoundedVec::try_from(path).map_err(|_| Error::<T>::PathTooLong)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(b"ethereum".to_vec()).map_err(|_| Error::<T>::SerializationError)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::SerializationError)?,
				pallet_signet::SerializationFormat::AbiJson,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::SerializationError)?,
				pallet_signet::SerializationFormat::Borsh,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::SerializationError)?,
			)?;

			Self::deposit_event(Event::DepositRequested {
				request_id,
				requester,
				erc20_address,
				amount,
			});

			Ok(())
		}

		/// Claim deposited ERC20 tokens after signature verification
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(50_000, 0))]
		pub fn claim_erc20(
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
			UserBalances::<T>::mutate(&claimer, &pending.erc20_address, |balance| -> DispatchResult {
				*balance = balance.checked_add(pending.amount).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			// Clean up storage
			PendingDeposits::<T>::remove(&request_id);

			Self::deposit_event(Event::DepositClaimed {
				request_id,
				claimer,
				erc20_address: pending.erc20_address,
				amount: pending.amount,
			});

			Ok(())
		}
	}

	// ========================= Helper Functions =========================

	impl<T: Config> Pallet<T> {
		fn generate_request_id(
			sender: &T::AccountId,
			transaction_data: &[u8],
			slip44_chain_id: u32,
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
				transaction_data,
				slip44_chain_id,
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
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::VaultPalletId::get().into_account_truncating()
		}
	}
}
