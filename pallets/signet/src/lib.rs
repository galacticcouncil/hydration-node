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

const EIP1559_TX_TYPE: u8 = 0x02;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
pub mod tests;

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

		/// Maximum length for chain ID
		#[pallet::constant]
		type MaxChainIdLength: Get<u32>;

		type WeightInfo: WeightInfo;

		/// Maximum length of transaction data
		#[pallet::constant]
		type MaxDataLength: Get<u32>;
	}

	// ========================================
	// Types
	// ========================================

	/// Serialization format enum
	#[derive(Encode, Decode, TypeInfo, Clone, Copy, Debug, PartialEq, Eq)]
	pub enum SerializationFormat {
		Borsh = 0,
		AbiJson = 1,
	}

	/// Affine point for signatures
	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, Eq)]
	pub struct AffinePoint {
		pub x: [u8; 32],
		pub y: [u8; 32],
	}

	/// Signature structure
	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, Eq)]
	pub struct Signature {
		pub big_r: AffinePoint,
		pub s: [u8; 32],
		pub recovery_id: u8,
	}

	/// Error response structure
	#[derive(Encode, Decode, TypeInfo, Debug, Clone, PartialEq, Eq)]
	pub struct ErrorResponse {
		pub request_id: [u8; 32],
		pub error_message: BoundedVec<u8, ConstU32<MAX_ERROR_MESSAGE_LENGTH>>,
	}

	// ========================================
	// Storage
	// ========================================

	/// The admin account that controls this pallet
	#[pallet::storage]
	#[pallet::getter(fn admin)]
	pub type Admin<T: Config> = StorageValue<_, T::AccountId>;

	/// The amount required as deposit for signature requests
	#[pallet::storage]
	#[pallet::getter(fn signature_deposit)]
	pub type SignatureDeposit<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

	/// The CAIP-2 chain identifier
	#[pallet::storage]
	#[pallet::getter(fn chain_id)]
	pub type ChainId<T: Config> = StorageValue<_, BoundedVec<u8, T::MaxChainIdLength>, ValueQuery>;

	// ========================================
	// Events
	// ========================================

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Pallet has been initialized with an admin
		Initialized {
			admin: T::AccountId,
			signature_deposit: BalanceOf<T>,
			chain_id: Vec<u8>,
		},

		/// Signature deposit amount has been updated
		DepositUpdated {
			old_deposit: BalanceOf<T>,
			new_deposit: BalanceOf<T>,
		},

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

		/// Sign-respond request event
		SignRespondRequested {
			sender: T::AccountId,
			transaction_data: Vec<u8>,
			slip44_chain_id: u32,
			key_version: u32,
			deposit: BalanceOf<T>,
			path: Vec<u8>,
			algo: Vec<u8>,
			dest: Vec<u8>,
			params: Vec<u8>,
			explorer_deserialization_format: u8,
			explorer_deserialization_schema: Vec<u8>,
			callback_serialization_format: u8,
			callback_serialization_schema: Vec<u8>,
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

		/// Read response event
		ReadResponded {
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
		/// The pallet has already been initialized
		AlreadyInitialized,
		/// The pallet has not been initialized yet
		NotInitialized,
		/// Unauthorized - caller is not admin
		Unauthorized,
		/// Insufficient funds for withdrawal
		InsufficientFunds,
		/// Invalid transaction data (empty)
		InvalidTransaction,
		/// Arrays must have the same length
		InvalidInputLength,
		/// The chain ID is too long
		ChainIdTooLong,
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
		/// Initialize the pallet with admin, deposit, and chain ID
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::initialize())]
		pub fn initialize(
			origin: OriginFor<T>,
			admin: T::AccountId,
			signature_deposit: BalanceOf<T>,
			chain_id: BoundedVec<u8, T::MaxChainIdLength>,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			ensure!(Admin::<T>::get().is_none(), Error::<T>::AlreadyInitialized);

			Admin::<T>::put(&admin);
			SignatureDeposit::<T>::put(signature_deposit);

			let bounded_chain_id = BoundedVec::<u8, T::MaxChainIdLength>::try_from(chain_id.clone())
				.map_err(|_| Error::<T>::ChainIdTooLong)?;
			ChainId::<T>::put(bounded_chain_id);

			Self::deposit_event(Event::Initialized {
				admin,
				signature_deposit,
				chain_id: chain_id.to_vec(),
			});

			Ok(())
		}

		/// Update the signature deposit amount (admin only)
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::update_deposit())]
		pub fn update_deposit(origin: OriginFor<T>, new_deposit: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().ok_or(Error::<T>::NotInitialized)?;
			ensure!(who == admin, Error::<T>::Unauthorized);

			let old_deposit = SignatureDeposit::<T>::get();
			SignatureDeposit::<T>::put(new_deposit);

			Self::deposit_event(Event::DepositUpdated {
				old_deposit,
				new_deposit,
			});

			Ok(())
		}

		/// Withdraw funds from the pallet account (admin only)
		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::withdraw_funds())]
		pub fn withdraw_funds(origin: OriginFor<T>, recipient: T::AccountId, amount: BalanceOf<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let admin = Admin::<T>::get().ok_or(Error::<T>::NotInitialized)?;
			ensure!(who == admin, Error::<T>::Unauthorized);

			let pallet_account = Self::account_id();
			let pallet_balance = T::Currency::free_balance(&pallet_account);
			ensure!(pallet_balance >= amount, Error::<T>::InsufficientFunds);

			T::Currency::transfer(&pallet_account, &recipient, amount, ExistenceRequirement::AllowDeath)?;

			Self::deposit_event(Event::FundsWithdrawn { amount, recipient });

			Ok(())
		}

		/// Request a signature for a payload
		#[pallet::call_index(3)]
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

			// Ensure initialized
			ensure!(Admin::<T>::get().is_some(), Error::<T>::NotInitialized);

			// Get deposit amount
			let deposit = SignatureDeposit::<T>::get();

			// Transfer deposit from requester to pallet account
			let pallet_account = Self::account_id();
			T::Currency::transfer(&requester, &pallet_account, deposit, ExistenceRequirement::KeepAlive)?;

			// Get chain ID for event (convert BoundedVec to Vec)
			let chain_id = ChainId::<T>::get().to_vec();

			// Emit event
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
		#[pallet::call_index(4)]
		#[pallet::weight(<T as Config>::WeightInfo::sign_respond())]
		pub fn sign_respond(
			origin: OriginFor<T>,
			serialized_transaction: BoundedVec<u8, ConstU32<MAX_TRANSACTION_LENGTH>>,
			slip44_chain_id: u32,
			key_version: u32,
			path: BoundedVec<u8, ConstU32<MAX_PATH_LENGTH>>,
			algo: BoundedVec<u8, ConstU32<MAX_ALGO_LENGTH>>,
			dest: BoundedVec<u8, ConstU32<MAX_DEST_LENGTH>>,
			params: BoundedVec<u8, ConstU32<MAX_PARAMS_LENGTH>>,
			explorer_deserialization_format: SerializationFormat,
			explorer_deserialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>>,
			callback_serialization_format: SerializationFormat,
			callback_serialization_schema: BoundedVec<u8, ConstU32<MAX_SCHEMA_LENGTH>>,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;

			// Ensure initialized
			ensure!(Admin::<T>::get().is_some(), Error::<T>::NotInitialized);

			// Validate transaction data
			ensure!(!serialized_transaction.is_empty(), Error::<T>::InvalidTransaction);

			// Get deposit amount
			let deposit = SignatureDeposit::<T>::get();

			// Transfer deposit from requester to pallet account
			let pallet_account = Self::account_id();
			T::Currency::transfer(&requester, &pallet_account, deposit, ExistenceRequirement::KeepAlive)?;

			// Emit event
			Self::deposit_event(Event::SignRespondRequested {
				sender: requester,
				transaction_data: serialized_transaction.to_vec(),
				slip44_chain_id,
				key_version,
				deposit,
				path: path.to_vec(),
				algo: algo.to_vec(),
				dest: dest.to_vec(),
				params: params.to_vec(),
				explorer_deserialization_format: explorer_deserialization_format as u8,
				explorer_deserialization_schema: explorer_deserialization_schema.to_vec(),
				callback_serialization_format: callback_serialization_format as u8,
				callback_serialization_schema: callback_serialization_schema.to_vec(),
			});

			Ok(())
		}

		/// Respond to signature requests (batch support)
		#[pallet::call_index(5)]
		#[pallet::weight(<T as Config>::WeightInfo::respond(request_ids.len() as u32))]
		pub fn respond(
			origin: OriginFor<T>,
			request_ids: BoundedVec<[u8; 32], ConstU32<MAX_BATCH_SIZE>>,
			signatures: BoundedVec<Signature, ConstU32<MAX_BATCH_SIZE>>,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			// Validate input lengths
			ensure!(request_ids.len() == signatures.len(), Error::<T>::InvalidInputLength);

			// Emit events for each response
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
		#[pallet::call_index(6)]
		#[pallet::weight(<T as Config>::WeightInfo::respond_error(errors.len() as u32))]
		pub fn respond_error(
			origin: OriginFor<T>,
			errors: BoundedVec<ErrorResponse, ConstU32<MAX_BATCH_SIZE>>,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			// Emit error events
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
		#[pallet::call_index(7)]
		#[pallet::weight(<T as Config>::WeightInfo::read_respond())]
		pub fn read_respond(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			serialized_output: BoundedVec<u8, ConstU32<MAX_SERIALIZED_OUTPUT_LENGTH>>,
			signature: Signature,
		) -> DispatchResult {
			let responder = ensure_signed(origin)?;

			// Just emit event
			Self::deposit_event(Event::ReadResponded {
				request_id,
				responder,
				serialized_output: serialized_output.to_vec(),
				signature,
			});

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
		///
		/// # Parameters
		/// - `origin`: The signed origin
		/// - `to_address`: Optional recipient address (None for contract creation)
		/// - `value`: ETH value in wei
		/// - `data`: Transaction data/calldata
		/// - `nonce`: Transaction nonce
		/// - `gas_limit`: Maximum gas units for transaction
		/// - `max_fee_per_gas`: Maximum total fee per gas (base + priority)
		/// - `max_priority_fee_per_gas`: Maximum priority fee (tip) per gas
		/// - `chain_id`: Target EVM chain ID
		///
		/// # Returns
		/// RLP-encoded transaction data with EIP-2718 type prefix (0x02 for EIP-1559)
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

			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
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
