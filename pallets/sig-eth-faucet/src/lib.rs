#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{format, string::String, vec};

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::traits::{Currency, fungibles::Mutate, tokens::Preservation};
use frame_support::{dispatch::DispatchResult, BoundedVec};
use frame_system::pallet_prelude::*;
use sp_core::H160;
use sp_io::hashing;
use sp_std::vec::Vec;

#[cfg(test)]
mod tests;

pub use pallet::*;

sol! {
	#[sol(abi)]
	interface IGasFaucet {
		function fund(address to, uint256 amount, bytes32 requestId) external;
	}
}

pub type Balance = u128;
pub type AssetId = u32;

pub type EvmAddress = [u8; 20];

pub type BalanceOf<T> =
<<T as pallet_signet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq)]
pub struct EvmTransactionParams {
	pub value: u128,
	pub gas_limit: u64,
	pub max_fee_per_gas: u128,
	pub max_priority_fee_per_gas: u128,
	pub nonce: u64,
	pub chain_id: u64,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_build_evm_tx::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		type MinimumRequestAmount: Get<BalanceOf<Self>>;

		type MaxDispenseAmount: Get<u128>;

		type DispenserFee: Get<u128>;

		type FeeAsset: Get<AssetId>;

		type FaucetAsset: Get<AssetId>;

		type TreasuryAddress: Get<Self::AccountId>;

		type FaucetAddress: Get<EvmAddress>;

		type MPCRootSigner: Get<EvmAddress>;

		type VaultPalletId: Get<Self::AccountId>;
	}

	#[pallet::storage]
	pub type Pending<T: Config> =
		StorageMap<_, Blake2_128Concat, [u8; 32], PendingData<T::AccountId>, OptionQuery>;

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct PendingData<AccountId> {
		pub requester: AccountId,
		pub pay_amount: u128,
		pub to: [u8; 20]
	}

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq)]
	pub struct EvmTx {
		pub value: u128,
		pub gas_limit: u64,
		pub max_fee_per_gas: u128,
		pub max_priority_fee_per_gas: u128,
		pub nonce: u64,
		pub chain_id: u64,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Initialized,
		FundRequested {
			request_id: [u8; 32],
			requester: T::AccountId,
			to: [u8; 20],
			amount_wei: u128,
		},
		FundSucceeded {
			request_id: [u8; 32],
		},
		FundFailed {
			request_id: [u8; 32],
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		AlreadyInitialized,
		DuplicateRequest,
		Serialization,
		NotFound,
		InvalidOutput,
		InvalidSignature,
		InvalidSigner
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn initialize(
			origin: OriginFor<T>
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			Self::deposit_event(Event::Initialized);
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(100_000)]
		pub fn request_fund(
			origin: OriginFor<T>,
			to: [u8; 20],
			amount_wei: u128,
			tx: EvmTransactionParams,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			let pallet_id = Self::account_id();

			// deducting fee
			<T as Config>::Currency::transfer(
				T::FeeAsset::get(),
				&requester,
				&T::TreasuryAddress::get(),
				T::DispenserFee::get(),
				Preservation::Expendable,
			)?;

			// deducting asset
			<T as Config>::Currency::transfer(
				T::FaucetAsset::get(),
				&requester,
				&pallet_id,
				amount_wei,
				Preservation::Expendable,
			)?;

			let call = IGasFaucet::fundCall {
				to: Address::from_slice(&to),
				amount: U256::from(amount_wei),
				requestId: alloy_primitives::FixedBytes([0u8; 32]),
			};

			let rlp = pallet_build_evm_tx::Pallet::<T>::build_evm_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				Some(H160::from(T::FaucetAddress::get())),
				0u128,
				call.abi_encode(),
				tx.nonce,
				tx.gas_limit,
				tx.max_fee_per_gas,
				tx.max_priority_fee_per_gas,
				vec![],
				tx.chain_id,
			)?;

			let path: Vec<u8> = requester.encode();
			let req_id = Self::generate_request_id(&requester, &rlp, 60, 0, &path, b"ecdsa", b"ethereum", b"");

			Pending::<T>::try_mutate(req_id, |slot| -> DispatchResult {
				ensure!(slot.is_none(), Error::<T>::DuplicateRequest);
				*slot = Some(PendingData {
					requester: requester.clone(),
					pay_amount: amount_wei,
					to,
				});
				Ok(())
			})?;

			let explorer_schema = Vec::<u8>::new();
			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			pallet_signet::Pallet::<T>::sign_respond(
				frame_system::RawOrigin::Signed(pallet_id.clone()).into(),
				BoundedVec::<u8, ConstU32<65536>>::try_from(rlp).map_err(|_| Error::<T>::Serialization)?,
				60,
				0,
				BoundedVec::try_from(path).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(b"ecdsa".to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(b"ethereum".to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(Vec::new()).map_err(|_| Error::<T>::Serialization)?,
				pallet_signet::SerializationFormat::AbiJson,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::Serialization)?,
				pallet_signet::SerializationFormat::Borsh,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::Serialization)?,
			)?;

			Self::deposit_event(Event::FundRequested {
				request_id: req_id,
				requester,
				to,
				amount_wei,
			});

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(50_000)]
		pub fn respond_fund(
			origin: OriginFor<T>,
			request_id: [u8; 32],
			serialized_output: BoundedVec<u8, ConstU32<65536>>,
			signature: pallet_signet::Signature,
		) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			let pending = Pending::<T>::take(&request_id).ok_or(Error::<T>::NotFound)?;

			let hash = Self::hash_message(&request_id, &serialized_output);
			Self::verify_signature_from_address(&hash, &signature, &T::MPCRootSigner::get())?;

			let ok = {
				use borsh::BorshDeserialize;
				bool::try_from_slice(&serialized_output).map_err(|_| Error::<T>::InvalidOutput)?
			};

			if ok {
				<T as Config>::Currency::transfer(
					T::FaucetAsset::get(),
					&Self::account_id(),
					&T::TreasuryAddress::get(),
					pending.pay_amount,
					Preservation::Expendable,
				)?;
				Self::deposit_event(Event::FundSucceeded { request_id });
				Ok(())
			} else {
				<T as Config>::Currency::transfer(
					T::FaucetAsset::get(),
					&Self::account_id(),
					&pending.requester,
					pending.pay_amount,
					Preservation::Expendable,
				)?;
				Self::deposit_event(Event::FundFailed {
					request_id,
				});
				Ok(())
			}
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
			T::VaultPalletId::get()
		}
	}
}
