#![cfg_attr(not(feature = "std"), no_std)]

use ethereum::{AccessListItem, EIP1559TransactionMessage, TransactionAction};
use frame_support::{
	pallet_prelude::*,
	traits::{Currency, ExistenceRequirement},
	PalletId,
};
use frame_system::pallet_prelude::*;
pub use pallet::*;
use sp_core::{H160, U256};
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec::Vec;

// Type alias for cleaner code
type BalanceOf<T> = <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// Maximum length for paths, algorithms, destinations, and parameters
const MAX_PATH_LENGTH: u32 = 256;
const MAX_ALGO_LENGTH: u32 = 32;
const MAX_DEST_LENGTH: u32 = 64;
const MAX_PARAMS_LENGTH: u32 = 1024;

/// Maximum length for transaction data and serialized outputs
const MAX_TRANSACTION_LENGTH: u32 = 65536; // 64 KB
const MAX_SERIALIZED_OUTPUT_LENGTH: u32 = 65536; // 64 KB

/// Maximum length for schemas and error messages
const MAX_SCHEMA_LENGTH: u32 = 4096; // 4 KB
const MAX_ERROR_MESSAGE_LENGTH: u32 = 1024;

/// Maximum batch sizes
const MAX_BATCH_SIZE: u32 = 100;

/// Hard upper bound for chain ID length (used as BoundedVec bound)
pub const MAX_CHAIN_ID_LENGTH: u32 = 128;

const EIP1559_TX_TYPE: u8 = 0x02;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod types;
pub mod weights;
pub use types::WeightInfo;

#[cfg(test)]
pub mod tests;

#[allow(clippy::too_many_arguments)]
#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Currency for handling deposits and fees
		type Currency: Currency<Self::AccountId>;

		/// The pallet's unique ID for deriving its account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		type WeightInfo: WeightInfo;
	}

	// ========================================
	// Types
	// ========================================

	/// Serialization format enum
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, Copy, Debug, PartialEq, Eq)]
	pub enum SerializationFormat {
		Borsh = 0,
		AbiJson = 1,
	}

	/// Affine point for signatures
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, Debug, PartialEq, Eq)]
	pub struct AffinePoint {
		pub x: [u8; 32],
		pub y: [u8; 32],
	}

	/// Signature structure
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, Debug, PartialEq, Eq)]
	pub struct Signature {
		pub big_r: AffinePoint,
		pub s: [u8; 32],
		pub recovery_id: u8,
	}

	/// Error response structure
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Debug, Clone, PartialEq, Eq)]
	pub struct ErrorResponse {
		pub request_id: [u8; 32],
		pub error_message: BoundedVec<u8, ConstU32<MAX_ERROR_MESSAGE_LENGTH>>,
	}

	/// Signet configuration data.
	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct SignetConfigData<Balance: MaxEncodedLen> {
		/// If `true`, all user-facing requests are blocked.
		pub paused: bool,
		/// Amount required as deposit for signature requests.
		pub signature_deposit: Balance,
		/// Maximum length for chain ID.
		pub max_chain_id_length: u32,
		/// Maximum length for EVM transaction data.
		pub max_evm_data_length: u32,
		/// The CAIP-2 chain identifier.
		pub chain_id: BoundedVec<u8, ConstU32<MAX_CHAIN_ID_LENGTH>>,
	}

	// ========================================
	// Storage
	// ========================================

	/// Global configuration for the signet pallet.
	///
	/// If `None`, the pallet has not been configured yet and cannot be used.
	#[pallet::storage]
	#[pallet::getter(fn signet_config)]
	pub type SignetConfig<T: Config> = StorageValue<_, SignetConfigData<BalanceOf<T>>, OptionQuery>;

	// ========================================
	// Events
	// ========================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Signet configuration has been updated.
		ConfigUpdated {
			signature_deposit: BalanceOf<T>,
			max_chain_id_length: u32,
			max_evm_data_length: u32,
			chain_id: Vec<u8>,
		},

		/// Signet has been paused. No new requests will be accepted.
		Paused,
		/// Signet has been unpaused. New requests are allowed again.
		Unpaused,

		/// Funds have been withdrawn from the pallet
		FundsWithdrawn {
			amount: BalanceOf<T>,
			recipient: T::AccountId,
		},

		/// A signature has been requested
		SignatureRequested {
			sender: T::AccountId,
			payload: [u8; 32],
			key_version: u32,
			deposit: BalanceOf<T>,
			chain_id: Vec<u8>,
			path: Vec<u8>,
			algo: Vec<u8>,
			dest: Vec<u8>,
			params: Vec<u8>,
		},

		/// Sign bidirectional request event
		SignBidirectionalRequested {
			sender: T::AccountId,
			serialized_transaction: Vec<u8>,
			caip2_id: Vec<u8>,
			key_version: u32,
			deposit: BalanceOf<T>,
			path: Vec<u8>,
			algo: Vec<u8>,
			dest: Vec<u8>,
			params: Vec<u8>,
			output_deserialization_schema: Vec<u8>,
			respond_serialization_schema: Vec<u8>,
		},

		/// Signature response event
		SignatureResponded {
			request_id: [u8; 32],
			responder: T::AccountId,
			signature: Signature,
		},

		/// Signature error event
		SignatureError {
			request_id: [u8; 32],
			responder: T::AccountId,
			error: Vec<u8>,
		},

		/// Respond bidirectional event
		RespondBidirectionalEvent {
			request_id: [u8; 32],
			responder: T::AccountId,
			serialized_output: Vec<u8>,
			signature: Signature,
		},
	}

	// ========================================
	// Errors
	// ========================================

	#[pallet::error]
	pub enum Error<T> {
		/// The pallet has not been configured yet
		NotConfigured,
		/// Pallet is paused and cannot process this call.
		Paused,
		/// Insufficient funds for withdrawal
		InsufficientFunds,
		/// Invalid transaction data (empty)
		InvalidTransaction,
		/// Arrays must have the same length
		InvalidInputLength,
		/// Transaction data exceeds maximum allowed length
		DataTooLong,
		/// Invalid address format - must be exactly 20 bytes
		InvalidAddress,
		/// Priority fee cannot exceed max fee per gas (EIP-1559 requirement)
		InvalidGasPrice,
	}

	// ========================================
	// Extrinsics
	// ========================================

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Set or update the signet configuration.
		///
		/// Can be called multiple times to update the configuration.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		/// - `signature_deposit`: Deposit amount for signature requests.
		/// - `max_chain_id_length`: Maximum chain ID length.
		/// - `max_evm_data_length`: Maximum EVM transaction data length.
		/// - `chain_id`: The CAIP-2 chain identifier.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::set_config())]
		pub fn set_config(
			origin: OriginFor<T>,
			signature_deposit: BalanceOf<T>,
			max_chain_id_length: u32,
			max_evm_data_length: u32,
			chain_id: BoundedVec<u8, ConstU32<MAX_CHAIN_ID_LENGTH>>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let paused = SignetConfig::<T>::get().map(|c| c.paused).unwrap_or(false);

			SignetConfig::<T>::put(SignetConfigData {
				paused,
				signature_deposit,
				max_chain_id_length,
				max_evm_data_length,
				chain_id: chain_id.clone(),
			});

			Self::deposit_event(Event::ConfigUpdated {
				signature_deposit,
				max_chain_id_length,
				max_evm_data_length,
				chain_id: chain_id.to_vec(),
			});

			Ok(())
		}

		/// Withdraw funds from the pallet account.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		/// - `recipient`: Account to receive the withdrawn funds.
		/// - `amount`: Amount to withdraw.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_funds())]
		pub fn withdraw_funds(origin: OriginFor<T>, recipient: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;

			let pallet_account = Self::account_id();
			let pallet_balance = T::Currency::free_balance(&pallet_account);
			ensure!(pallet_balance >= amount, Error::<T>::InsufficientFunds);

			T::Currency::transfer(&pallet_account, &recipient, amount, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::FundsWithdrawn { amount, recipient });

			Ok(())
		}

		/// Request a signature for a payload
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::sign())]
		pub fn sign(
			origin: OriginFor<T>,
			payload: [u8; 32],
			key_version: u32,
			path: BoundedVec<u8, ConstU32<MAX_PATH_LENGTH>>,
			algo: BoundedVec<u8, ConstU32<MAX_ALGO_LENGTH>>,
			dest: BoundedVec<u8, ConstU32<MAX_DEST_LENGTH>>,
			params: BoundedVec<u8, ConstU32<MAX_PARAMS_LENGTH>>,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			let config = SignetConfig::<T>::get().ok_or(Error::<T>::NotConfigured)?;
			ensure!(!config.paused, Error::<T>::Paused);

			let deposit = config.signature_deposit;
			let chain_id = config.chain_id.to_vec();

			// Transfer deposit from requester to pallet account
			let pallet_account = Self::account_id();
			T::Currency::transfer(&requester, &pallet_account, deposit, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::SignatureRequested {
				sender: requester,
				payload,
				key_version,
				deposit,
				chain_id,
				path: path.to_vec(),
				algo: algo.to_vec(),
				dest: dest.to_vec(),
				params: params.to_vec(),
			});

			Ok(())
		}

		/// Request a signature for a serialized transaction
		#[pallet::call_index(3)]
		#[pallet::weight(<T as Config>::WeightInfo::sign_bidirectional())]
		pub fn sign_bidirectional(
			origin: OriginFor<T>,
			serialized_transaction: BoundedVec<u8, ConstU32<MAX_TRANSACTION_LENGTH>>,
			caip2_id: BoundedVec<u8, ConstU32<64>>,
			key_version: u32,
			path: BoundedVec<u8, ConstU32<MAX_PATH_LENGTH>>,
			algo: BoundedVec<u8, ConstU32<MAX_ALGO_LENGTH>>,
			dest: BoundedVec<u8, ConstU32<MAX_DEST_LENGTH>>,
			params: BoundedVec<u8, ConstU32<MAX_PARAMS_LENGTH>>,
			output_deserialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>>,
			respond_serialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>>,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			let config = SignetConfig::<T>::get().ok_or(Error::<T>::NotConfigured)?;
			ensure!(!config.paused, Error::<T>::Paused);

			// Validate transaction data
			ensure!(!serialized_transaction.is_empty(), Error::<T>::InvalidTransaction);

			let deposit = config.signature_deposit;

			// Transfer deposit from requester to pallet account
			let pallet_account = Self::account_id();
			T::Currency::transfer(&requester, &pallet_account, deposit, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::SignBidirectionalRequested {
				sender: requester,
				serialized_transaction: serialized_transaction.to_vec(),
				caip2_id: caip2_id.to_vec(),
				key_version,
				deposit,
				path: path.to_vec(),
				algo: algo.to_vec(),
				dest: dest.to_vec(),
				params: params.to_vec(),
				output_deserialization_schema: output_deserialization_schema.to_vec(),
				respond_serialization_schema: respond_serialization_schema.to_vec(),
			});

			Ok(())
		}

		/// Respond to signature requests (batch support)
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::respond())]
		pub fn respond(
			origin: OriginFor<T>,
			request_ids: BoundedVec<[u8; 32], ConstU32<MAX_BATCH_SIZE>>,
			signatures: BoundedVec<Signature, ConstU32<MAX_BATCH_SIZE>>,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			ensure!(request_ids.len() == signatures.len(), Error::<T>::InvalidInputLength);

			for i in 0..request_ids.len() {
				Self::deposit_event(Event::SignatureResponded {
					request_id: request_ids[i],
					responder: responder.clone(),
					signature: signatures[i].clone(),
				});
			}

			Ok(())
		}

		/// Report signature generation errors (batch support)
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::respond_error())]
		pub fn respond_error(
			origin: OriginFor<T>,
			errors: BoundedVec<ErrorResponse, ConstU32<MAX_BATCH_SIZE>>,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			for error in errors {
				Self::deposit_event(Event::SignatureError {
					request_id: error.request_id,
					responder: responder.clone(),
					error: error.error_message.to_vec(),
				});
			}

			Ok(())
		}

		/// Provide a read response with signature
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::respond_bidirectional())]
		pub fn respond_bidirectional(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			serialized_output: BoundedVec<u8, ConstU32<MAX_SERIALIZED_OUTPUT_LENGTH>>,
			signature: Signature,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			Self::deposit_event(Event::RespondBidirectionalEvent {
				request_id,
				responder,
				serialized_output: serialized_output.to_vec(),
				signature,
			});

			Ok(())
		}

		/// Pause the signet so that no new signing requests can be made.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::pause())]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			SignetConfig::<T>::mutate(|maybe_config| {
				if let Some(config) = maybe_config {
					config.paused = true;
				}
			});

			Self::deposit_event(Event::Paused);
			Ok(())
		}

		/// Unpause the signet so that signing requests are allowed again.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		#[pallet::call_index(8)]
		#[pallet::weight(<T as Config>::WeightInfo::unpause())]
		pub fn unpause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			SignetConfig::<T>::mutate(|maybe_config| {
				if let Some(config) = maybe_config {
					config.paused = false;
				}
			});

			Self::deposit_event(Event::Unpaused);
			Ok(())
		}
	}

	// Helper functions
	impl<T: Config> Pallet<T> {
		/// Get the pallet's account ID (where funds are stored)
		pub fn account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Build an EIP-1559 EVM transaction and return the RLP-encoded data
		pub fn build_evm_tx(
			origin: OriginFor<T>,
			to_address: Option<H160>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u64,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			access_list: Vec<AccessListItem>,
			chain_id: u64,
		) -> Result<Vec<u8>, DispatchError> {
			ensure_signed(origin)?;

			let config = SignetConfig::<T>::get().ok_or(Error::<T>::NotConfigured)?;
			ensure!(
				data.len() <= config.max_evm_data_length as usize,
				Error::<T>::DataTooLong
			);
			ensure!(max_priority_fee_per_gas <= max_fee_per_gas, Error::<T>::InvalidGasPrice);

			let action = match to_address {
				Some(addr) => TransactionAction::Call(addr),
				None => TransactionAction::Create,
			};

			let tx_message = EIP1559TransactionMessage {
				chain_id,
				nonce: U256::from(nonce),
				max_priority_fee_per_gas: U256::from(max_priority_fee_per_gas),
				max_fee_per_gas: U256::from(max_fee_per_gas),
				gas_limit: U256::from(gas_limit),
				action,
				value: U256::from(value),
				input: data,
				access_list,
			};

			let mut output = Vec::new();
			output.push(EIP1559_TX_TYPE);
			output.extend_from_slice(&rlp::encode(&tx_message));

			Ok(output)
		}
	}
}
