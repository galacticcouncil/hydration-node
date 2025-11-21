#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::{string::String, vec};

use alloy_primitives::U256;
use alloy_sol_types::{sol, SolCall};
use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::{fungibles::Mutate, tokens::Preservation, Currency};
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, BoundedVec};
use frame_system::pallet_prelude::*;
use sp_core::H160;
use sp_std::vec::Vec;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;
pub mod weights;

#[cfg(test)]
mod tests;

pub use pallet::*;

sol! {
	#[sol(abi)]
	interface IGasFaucet {
		function fund(address to, uint256 amount) external;
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
	pub trait Config: frame_system::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;
		type UpdateOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		#[pallet::constant]
		type MinimumRequestAmount: Get<u128>;
		#[pallet::constant]
		type MaxDispenseAmount: Get<u128>;
		#[pallet::constant]
		type DispenserFee: Get<u128>;

		#[pallet::constant]
		type FeeAsset: Get<AssetId>;
		#[pallet::constant]
		type FaucetAsset: Get<AssetId>;

		#[pallet::constant]
		type TreasuryAddress: Get<Self::AccountId>;

		#[pallet::constant]
		type FaucetAddress: Get<EvmAddress>;

		#[pallet::constant]
		type VaultPalletId: Get<PalletId>;

		#[pallet::constant]
		type MinFaucetEthThreshold: Get<u128>;

		type WeightInfo: crate::WeightInfo;
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
		FaucetBalanceUpdated {
			old_balance_wei: u128,
			new_balance_wei: u128,
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
		AlreadyPaused,
		AlreadyUnpaused,
		AmountTooSmall,
		AmountTooLarge,
		InvalidAddress,
		FaucetBalanceBelowThreshold,
		NotEnoughFunds,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::initialize())]
		pub fn initialize(origin: OriginFor<T>, balance_wei: u128) -> DispatchResult {
			let _ = ensure_signed(origin)?;
			ensure!(DispenserConfig::<T>::get().is_none(), Error::<T>::AlreadyInitialized);

			DispenserConfig::<T>::put(DispenserConfigData {
				init: true,
				paused: false,
			});

			CurrentFaucetBalanceWei::<T>::put(balance_wei);

			Self::deposit_event(Event::Initialized);
			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::request_fund())]
		pub fn request_fund(
			origin: OriginFor<T>,
			to: [u8; 20],
			amount_wei: u128,
			request_id: [u8; 32],
			tx: EvmTransactionParams,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			let pallet_acc = Self::account_id();

			Self::ensure_initialized()?;
			let cfg = DispenserConfig::<T>::get().ok_or(Error::<T>::NotFound)?;
			ensure!(!cfg.paused, Error::<T>::Paused);
			ensure!(to != [0u8; 20], Error::<T>::InvalidAddress);
			ensure!(amount_wei >= T::MinimumRequestAmount::get(), Error::<T>::AmountTooSmall);
			ensure!(amount_wei <= T::MaxDispenseAmount::get(), Error::<T>::AmountTooLarge);

			let observed = CurrentFaucetBalanceWei::<T>::get();
			let needed = T::MinFaucetEthThreshold::get()
				.checked_add(amount_wei)
				.ok_or(Error::<T>::InvalidOutput)?;
			ensure!(observed >= needed, Error::<T>::FaucetBalanceBelowThreshold);

			ensure!(tx.gas_limit > 0, Error::<T>::InvalidOutput);
			ensure!(
				tx.max_fee_per_gas >= tx.max_priority_fee_per_gas,
				Error::<T>::InvalidOutput
			);

			let call = IGasFaucet::fundCall {
				to: alloy_primitives::Address::from_slice(&to),
				amount: U256::from(amount_wei),
			};

			let rlp = pallet_signet::Pallet::<T>::build_evm_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				Some(H160::from(T::FaucetAddress::get())),
				0u128,
				call.abi_encode(),
				tx.nonce,
				tx.gas_limit,
				tx.max_fee_per_gas,
				tx.max_priority_fee_per_gas,
				Vec::new(),
				tx.chain_id,
			)?;

			let mut path = Vec::with_capacity(2 + requester.encoded_size() * 2);
			path.extend_from_slice(b"0x");
			path.extend_from_slice(hex::encode(requester.encode()).as_bytes());

			let req_id = Self::generate_request_id(&pallet_acc, &rlp, 60, 0, &path, b"ecdsa", b"ethereum", b"");

			ensure!(req_id == request_id, Error::<T>::InvalidRequestId);
			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			let fee = T::DispenserFee::get();
			let fee_bal = <T as Config>::Currency::balance(T::FeeAsset::get(), &requester);
			let faucet_bal = <T as Config>::Currency::balance(T::FaucetAsset::get(), &requester);
			ensure!(fee_bal >= fee, Error::<T>::NotEnoughFunds);
			ensure!(faucet_bal >= amount_wei, Error::<T>::NotEnoughFunds);

			<T as Config>::Currency::transfer(
				T::FeeAsset::get(),
				&requester,
				&T::TreasuryAddress::get(),
				fee,
				Preservation::Expendable,
			)?;

			<T as Config>::Currency::transfer(
				T::FaucetAsset::get(),
				&requester,
				&T::TreasuryAddress::get(),
				amount_wei,
				Preservation::Expendable,
			)?;

			let explorer_schema = Vec::<u8>::new();
			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			pallet_signet::Pallet::<T>::sign_respond(
				frame_system::RawOrigin::Signed(pallet_acc.clone()).into(),
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
		#[pallet::weight(<T as pallet::Config>::WeightInfo::pause())]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::ensure_initialized()?;
			DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = true);
			Self::deposit_event(Event::Paused);
			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::unpause())]
		pub fn unpause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::ensure_initialized()?;
			DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = false);
			Self::deposit_event(Event::Unpaused);
			Ok(())
		}

		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_faucet_balance())]
		pub fn set_faucet_balance(origin: OriginFor<T>, balance_wei: u128) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			Self::ensure_initialized()?;
			let old = CurrentFaucetBalanceWei::<T>::get();
			if old == balance_wei {
				return Ok(());
			}
			CurrentFaucetBalanceWei::<T>::put(balance_wei);
			Self::deposit_event(Event::FaucetBalanceUpdated {
				old_balance_wei: old,
				new_balance_wei: balance_wei,
			});
			Ok(())
		}
	}

	// ========================= Helper Functions =========================

	impl<T: Config> Pallet<T> {
		pub fn generate_request_id(
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

pub trait WeightInfo {
	fn initialize() -> Weight;
	fn request_fund() -> Weight;
	fn set_faucet_balance() -> Weight;
	fn pause() -> Weight;
	fn unpause() -> Weight;
}
