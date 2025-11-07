#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{format, string::String, vec};

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::traits::{fungibles::Mutate, tokens::Preservation, Currency};
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, BoundedVec};
use frame_system::offchain::{SendTransactionTypes, SigningTypes, SubmitTransaction};
use frame_system::pallet_prelude::*;
use sp_core::crypto::KeyTypeId;
use sp_core::H160;
use sp_io::hashing;
use sp_runtime::offchain::storage::StorageValueRef;
use sp_runtime::offchain::{
	storage_lock::{BlockAndTime, StorageLock},
	Duration,
};
use sp_runtime::traits::SaturatedConversion;
use sp_runtime::traits::Zero;
use sp_std::vec::Vec;

const POLL_EVERY_BLOCKS: u32 = 100;
const THROTTLE_BLOCKS: u32 = 10;
const OCW_LAST_SEND_KEY: &[u8] = b"gfas/last_send";

use log::{debug, error, info, trace, warn};
const LOG_TARGET: &str = "pallet-dispenser";

#[cfg(test)]
mod tests;

pub use pallet::*;

sol! {
	#[sol(abi)]
	interface IGasFaucet {
		function fund(address to, uint256 amount) external;
	}
}

pub const KEY_TYPE: KeyTypeId = KeyTypeId(*b"btc!");
pub mod crypto {
	use super::KEY_TYPE;
	use sp_runtime::{
		app_crypto::{app_crypto, sr25519},
		MultiSignature, MultiSigner,
	};
	app_crypto!(sr25519, KEY_TYPE);

	pub struct DispenserAuthId;

	impl frame_system::offchain::AppCrypto<MultiSigner, MultiSignature> for DispenserAuthId {
		type RuntimeAppPublic = Public;
		type GenericSignature = sp_core::sr25519::Signature;
		type GenericPublic = sp_core::sr25519::Public;
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
	use sp_runtime::traits::AccountIdConversion;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config:
		frame_system::Config
		+ SigningTypes
		+ SendTransactionTypes<Self::RuntimeCall>
		+ pallet_build_evm_tx::Config
		+ pallet_signet::Config
	{
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		// Multi-currency (HDX fees + wETH faucet asset)
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		// Minimum/Maximum & Fee (use u128 to match Balance/wei)
		#[pallet::constant]
		type MinimumRequestAmount: Get<u128>;
		#[pallet::constant]
		type MaxDispenseAmount: Get<u128>;
		#[pallet::constant]
		type DispenserFee: Get<u128>;

		// fee asset (HDX) & faucet asset (wETH)
		#[pallet::constant]
		type FeeAsset: Get<AssetId>;
		#[pallet::constant]
		type FaucetAsset: Get<AssetId>;

		// Treasury account for fee settlement
		#[pallet::constant]
		type TreasuryAddress: Get<Self::AccountId>;

		// Ethereum config
		#[pallet::constant]
		type FaucetAddress: Get<EvmAddress>;
		#[pallet::constant]
		type MPCRootSigner: Get<EvmAddress>;

		// PalletId for internal account
		#[pallet::constant]
		type VaultPalletId: Get<PalletId>;

		#[pallet::constant]
		type MinFaucetEthThreshold: Get<u128>;

		type AuthorityId: frame_system::offchain::AppCrypto<Self::Public, Self::Signature>;
	}

	/*************************** STORAGE ***************************/

	#[pallet::storage]
	#[pallet::getter(fn dispenser_config)]
	pub type DispenserConfig<T> = StorageValue<_, DispenserConfigData, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn current_faucet_balance_wei)]
	pub type CurrentFaucetBalanceWei<T> = StorageValue<_, u128, ValueQuery>;

	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct DispenserConfigData {
		pub init: bool,
		pub paused: bool,
	}

	#[pallet::storage]
	pub type UsedRequestIds<T: Config> = StorageMap<_, Blake2_128Concat, [u8; 32], (), OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Initialized,
		Paused,
		Unpaused,
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
		InvalidSigner,
		InvalidRequestId,
		Paused,
		AmountTooSmall,
		AmountTooLarge,
		InvalidAddress,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(10_000)]
		pub fn initialize(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			ensure!(DispenserConfig::<T>::get().is_none(), Error::<T>::AlreadyInitialized);

			DispenserConfig::<T>::put(DispenserConfigData {
				init: true,
				paused: false,
			});

			Self::deposit_event(Event::Initialized);
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(100_000)]
		pub fn request_fund(
			origin: OriginFor<T>,
			to: [u8; 20],
			amount_wei: u128,
			request_id: [u8; 32],
			tx: EvmTransactionParams,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			let pallet_id = Self::account_id();

			Self::ensure_initialized()?;

			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			ensure!(!DispenserConfig::<T>::get().unwrap().paused, Error::<T>::Paused);
			ensure!(amount_wei >= T::MinimumRequestAmount::get(), Error::<T>::AmountTooSmall);
			ensure!(amount_wei <= T::MaxDispenseAmount::get(), Error::<T>::AmountTooLarge);

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
				&T::TreasuryAddress::get(),
				amount_wei,
				Preservation::Expendable,
			)?;

			let call = IGasFaucet::fundCall {
				to: Address::from_slice(&to),
				amount: U256::from(amount_wei),
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

			let path = {
				let encoded = requester.encode();
				let path_str = format!("0x{}", hex::encode(&encoded));
				path_str.into_bytes()
			};

			let sender_for_id = Self::account_id();
			let sender_scale = sender_for_id.encode();

			let req_id = Self::generate_request_id(&sender_for_id, &rlp, 60, 0, &path, b"ecdsa", b"ethereum", b"");

			ensure!(req_id == request_id, Error::<T>::InvalidRequestId);

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

			UsedRequestIds::<T>::insert(request_id, ());

			Self::deposit_event(Event::FundRequested {
				request_id: req_id,
				requester,
				to,
				amount_wei,
			});

			Ok(())
		}

		#[pallet::call_index(3)]
		#[pallet::weight(10_000)]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = true);
			Self::deposit_event(Event::Paused);
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(10_000)]
		pub fn unpause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = false);
			Self::deposit_event(Event::Unpaused);
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(10_000)]
		pub fn submit_balance_unsigned(origin: OriginFor<T>, balance_wei: u128) -> DispatchResult {
			ensure_none(origin)?;

			CurrentFaucetBalanceWei::<T>::put(balance_wei);

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

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T> {
		type Call = Call<T>;

		fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			use sp_runtime::transaction_validity::{InvalidTransaction, ValidTransaction};

			match call {
				Call::submit_balance_unsigned { balance_wei } => {
					let now = <frame_system::Pallet<T>>::block_number();
					Ok(ValidTransaction {
						priority: 1_000_000,
						requires: vec![],
						provides: vec![(b"gfas/bal", now, *balance_wei).encode()],
						longevity: 64,
						propagate: true,
					})
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		<T as frame_system::offchain::SendTransactionTypes<<T as frame_system::Config>::RuntimeCall>>::OverarchingCall:
			From<Call<T>>,
	{
		fn offchain_worker(n: BlockNumberFor<T>) {
			log::info!(target: LOG_TARGET, "started offchain worker.....");

			// let mut last_send_ref = StorageValueRef::persistent(OCW_LAST_SEND_KEY);

			let mut should_send = true;

			// if let Ok(Some(last_block)) = last_send_ref.get::<u32>() {
			// 	let now: u32 = n.saturated_into::<u32>();
			// 	if now.saturating_sub(last_block) < THROTTLE_BLOCKS {
			// 		should_send = false;
			// 	}
			// }

			if !should_send {
				return;
			}

			let next_wei: u128 = Self::mock_balance_for(n);

			let prev_wei = CurrentFaucetBalanceWei::<T>::get();
			if prev_wei == next_wei {
				return;
			}

			let call = Call::<T>::submit_balance_unsigned { balance_wei: next_wei };

			log::info!(target: LOG_TARGET, "wei {:?}", next_wei);

			let _ = frame_system::offchain::SubmitTransaction::<T, <T as frame_system::Config>::RuntimeCall>
    ::submit_unsigned_transaction(call.into())
    .map(|_| {
        let now: u32 = n.saturated_into::<u32>();
        let mut last_send_ref = StorageValueRef::persistent(OCW_LAST_SEND_KEY);
        let _ = last_send_ref.set(&now);

			log::info!(target: LOG_TARGET, "signature sent");
    });
		}
	}

	impl<T: Config> Pallet<T> {
		#[inline]
		fn mock_balance_for(n: BlockNumberFor<T>) -> u128 {
			let seed = (n.encode(), b"gfas/mock").using_encoded(sp_io::hashing::blake2_256);
			let x = (seed[0] as u128 % 10) + 1;
			x * 1_000_000_000_000_000_000u128
		}
	}

	impl<T: Config> Pallet<T> {
		pub fn account_id() -> T::AccountId {
			T::VaultPalletId::get().into_account_truncating()
		}

		#[inline]
		fn ensure_initialized() -> Result<(), Error<T>> {
			match DispenserConfig::<T>::get() {
				Some(DispenserConfigData { init: true, .. }) => Ok(()),
				_ => Err(Error::<T>::NotFound),
			}
		}
	}
}
