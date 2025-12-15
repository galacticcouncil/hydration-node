//! Pallet for requesting Ethereum gas from an external EVM faucet via SigNet.
//!
//! This pallet:
//! - Builds a typed EVM transaction calling an `IGasFaucet::fund` function.
//! - Requests a signature from SigNet using `pallet_signet`.
//! - Charges a fee in a configured asset and collects the requested faucet asset
//!   from the user as collateral.
//! - Tracks faucet ETH balance (in wei) off-chain and prevents requests when
//!   the configured threshold is not met.
//! - Allows governance to pause/unpause requests and update the tracked faucet balance.

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
pub mod types;
pub mod weights;

#[cfg(test)]
pub mod tests;

pub use pallet::*;
pub use types::*;

// Solidity interface for the external EVM gas faucet contract.
//
// The pallet builds a transaction calling `fund(address,uint256)` using this ABI.
sol! {
	#[sol(abi)]
	interface IGasFaucet {
		function fund(address to, uint256 amount) external;
	}
}

/// Parameters required to build an EIP-1559 EVM transaction.
///
/// These values are provided by the caller and used to construct the RLP-encoded
/// transaction which SigNet will sign.
#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq)]
pub struct EvmTransactionParams {
	/// ETH value (in wei) sent with the EVM transaction.
	pub value: u128,
	/// Gas limit for the transaction.
	pub gas_limit: u64,
	/// Maximum total fee per gas (EIP-1559 `maxFeePerGas`).
	pub max_fee_per_gas: u128,
	/// Maximum priority fee per gas (EIP-1559 `maxPriorityFeePerGas`).
	pub max_priority_fee_per_gas: u128,
	/// Nonce of the faucet account on the target EVM chain.
	pub nonce: u64,
	/// Native chain ID (EIP-155) of the target EVM chain.
	pub chain_id: u64,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use sp_runtime::traits::AccountIdConversion;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Pallet configuration trait.
	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_signet::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi-asset fungible currency implementation used for fees and faucet tokens.
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// Minimum amount of faucet asset that can be requested in a single call.
		#[pallet::constant]
		type MinimumRequestAmount: Get<Balance>;

		/// Maximum amount of faucet asset that can be requested in a single call.
		#[pallet::constant]
		type MaxDispenseAmount: Get<Balance>;

		/// Flat fee charged in `FeeAsset` for each faucet request.
		#[pallet::constant]
		type DispenserFee: Get<Balance>;

		/// Asset ID used to charge the faucet request fee.
		/// (HDX - 0)
		#[pallet::constant]
		type FeeAsset: Get<AssetId>;

		/// Asset ID deducted to receive faucet on the destination chain.
		/// (WETH - 20)
		#[pallet::constant]
		type FaucetAsset: Get<AssetId>;

		/// Account that receives the collected dispenser fees and faucet asset.
		#[pallet::constant]
		type FeeDestination: Get<Self::AccountId>;

		/// EVM address of the external gas faucet contract.
		#[pallet::constant]
		type FaucetAddress: Get<EvmAddress>;

		/// Pallet ID used to derive the pallet's sovereign account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Minimum remaining ETH (in wei) that must be available in the faucet
		/// after servicing a request. Requests are rejected if this threshold
		/// would be breached.
		#[pallet::constant]
		type MinFaucetEthThreshold: Get<Balance>;

		/// Weight information provider for extrinsics of this pallet.
		type WeightInfo: crate::WeightInfo;
	}

	/*************************** STORAGE ***************************/

	/// Global configuration for the dispenser.
	///
	/// Currently only tracks whether the pallet is paused. If `None`, defaults
	/// to unpaused.
	#[pallet::storage]
	#[pallet::getter(fn dispenser_config)]
	pub type DispenserConfig<T> = StorageValue<_, DispenserConfigData, OptionQuery>;

	/// Tracked ETH balance (in wei) currently available in the external faucet.
	///
	/// This value is updated manually via governance and is used as a guardrail
	/// to prevent issuing requests that would over-spend the faucet.
	#[pallet::storage]
	#[pallet::getter(fn current_faucet_balance_wei)]
	pub type FaucetBalanceWei<T> = StorageValue<_, Balance, ValueQuery>;

	/// Dispenser configuration data.
	#[derive(Encode, Decode, TypeInfo, Clone, Debug, PartialEq, MaxEncodedLen)]
	pub struct DispenserConfigData {
		/// If `true`, all user-facing requests are blocked.
		pub paused: bool,
	}

	/// Request IDs that have already been used.
	///
	/// This prevents accidental or malicious re-submission of the same request.
	#[pallet::storage]
	pub type UsedRequestIds<T: Config> = StorageMap<_, Blake2_128Concat, Bytes32, (), OptionQuery>;

	/// Pallet events.
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Dispenser has been paused. No new requests will be accepted.
		Paused,
		/// Dispenser has been unpaused. New requests are allowed again.
		Unpaused,
		/// A funding request has been submitted to SigNet.
		///
		/// Note: This indicates the request was formed and submitted, not that
		/// the EVM transaction has been included on the target chain.
		FundRequested {
			/// Unique request ID derived from request parameters.
			request_id: Bytes32,
			/// Account that initiated the request.
			requester: T::AccountId,
			/// Target EVM address to receive ETH.
			to: EvmAddress,
			/// Requested amount of ETH (in wei).
			amount: Balance,
		},
		/// Tracked faucet ETH balance has been updated.
		FaucetBalanceUpdated {
			/// Previous tracked balance (in wei).
			old_balance_wei: Balance,
			/// New tracked balance (in wei).
			new_balance_wei: Balance,
		},
	}

	/// Pallet errors.
	#[pallet::error]
	pub enum Error<T> {
		/// Request ID has already been used.
		DuplicateRequest,
		/// Failed to (de)serialize data.
		Serialization,
		/// Output data did not match the expected format.
		InvalidOutput,
		/// Request ID does not match the derived ID for the provided data.
		InvalidRequestId,
		/// Pallet is paused and cannot process this call.
		Paused,
		/// Requested amount is below the configured minimum.
		AmountTooSmall,
		/// Requested amount exceeds the configured maximum.
		AmountTooLarge,
		/// EVM address parameter is invalid (e.g., zero address).
		InvalidAddress,
		/// Faucet balance would fall below the configured threshold after this request.
		FaucetBalanceBelowThreshold,
		/// Caller does not have enough balance of the fee asset.
		NotEnoughFeeFunds,
		/// Caller does not have enough balance of the faucet asset.
		NotEnoughFaucetFunds,
	}

	/// Dispatchable functions.
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Request ETH from the external faucet for a given EVM address.
		///
		/// This call:
		/// - Verifies amount bounds and EVM transaction parameters.
		/// - Checks the tracked faucet ETH balance against `MinFaucetEthThreshold`.
		/// - Charges the configured fee in `FeeAsset`.
		/// - Transfers the requested faucet asset from the user to `FeeDestination`.
		/// - Builds an EVM transaction calling `IGasFaucet::fund`.
		/// - Submits a signing request to SigNet via `pallet_signet::sign_respond`.
		///
		/// The `request_id` must match the ID derived internally from the inputs,
		/// otherwise the call will fail with `InvalidRequestId`.
		///  Parameters:
		/// - `to`: Target EVM address to receive ETH.
		/// - `amount`: Amount of ETH (in wei) to request.
		/// - `request_id`: Client-supplied request ID; must match derived ID.
		/// - `tx`: Parameters for the EVM transaction submitted to the faucet.
		#[pallet::call_index(0)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::request_fund())]
		pub fn request_fund(
			origin: OriginFor<T>,
			to: EvmAddress,
			amount: Balance,
			request_id: Bytes32,
			tx: EvmTransactionParams,
		) -> DispatchResult {
			let requester = ensure_signed(origin)?;
			let pallet_acc = Self::account_id();

			// Pallet must not be paused.
			Self::ensure_not_paused()?;

			// Basic validation of parameters.
			ensure!(to != [0u8; 20], Error::<T>::InvalidAddress);
			ensure!(amount >= T::MinimumRequestAmount::get(), Error::<T>::AmountTooSmall);
			ensure!(amount <= T::MaxDispenseAmount::get(), Error::<T>::AmountTooLarge);

			// Check tracked faucet balance vs. threshold.
			let observed = FaucetBalanceWei::<T>::get();
			let needed = T::MinFaucetEthThreshold::get()
				.checked_add(amount)
				.ok_or(Error::<T>::InvalidOutput)?;
			ensure!(observed >= needed, Error::<T>::FaucetBalanceBelowThreshold);

			// EIP-1559 fee sanity checks.
			ensure!(tx.gas_limit > 0, Error::<T>::InvalidOutput);
			ensure!(
				tx.max_fee_per_gas >= tx.max_priority_fee_per_gas,
				Error::<T>::InvalidOutput
			);

			// Build the EVM call to the faucet.
			let call = IGasFaucet::fundCall {
				to: alloy_primitives::Address::from_slice(&to),
				amount: U256::from(amount),
			};

			// Build EVM transaction bytes using pallet_signet helper.
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

			// Construct signing path used by SigNet.
			let mut path = Vec::with_capacity(2 + requester.encoded_size() * 2);
			path.extend_from_slice(b"0x");
			path.extend_from_slice(hex::encode(requester.encode()).as_bytes());

			// Derive canonical request ID and compare with user-supplied one.
			let req_id = Self::generate_request_id(&pallet_acc, &rlp, 60, 0, &path, ECDSA, ETHEREUM, b"");
			ensure!(req_id == request_id, Error::<T>::InvalidRequestId);
			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			// Check balances for fee and faucet asset.
			let fee = T::DispenserFee::get();
			let fee_bal = <T as Config>::Currency::balance(T::FeeAsset::get(), &requester);
			let faucet_bal = <T as Config>::Currency::balance(T::FaucetAsset::get(), &requester);
			ensure!(fee_bal >= fee, Error::<T>::NotEnoughFeeFunds);
			ensure!(faucet_bal >= amount, Error::<T>::NotEnoughFaucetFunds);

			// Charge fee.
			<T as Config>::Currency::transfer(
				T::FeeAsset::get(),
				&requester,
				&T::FeeDestination::get(),
				fee,
				Preservation::Expendable,
			)?;

			// Transfer faucet asset collateral.
			<T as Config>::Currency::transfer(
				T::FaucetAsset::get(),
				&requester,
				&T::FeeDestination::get(),
				amount,
				Preservation::Expendable,
			)?;

			let explorer_schema = Vec::<u8>::new();
			let callback_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			// Submit signing request to SigNet.
			pallet_signet::Pallet::<T>::sign_respond(
				frame_system::RawOrigin::Signed(pallet_acc.clone()).into(),
				BoundedVec::<u8, ConstU32<65536>>::try_from(rlp).map_err(|_| Error::<T>::Serialization)?,
				60,
				0,
				BoundedVec::try_from(path).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(ECDSA.to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(ETHEREUM.to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(Vec::new()).map_err(|_| Error::<T>::Serialization)?,
				pallet_signet::SerializationFormat::AbiJson,
				BoundedVec::try_from(explorer_schema).map_err(|_| Error::<T>::Serialization)?,
				pallet_signet::SerializationFormat::Borsh,
				BoundedVec::try_from(callback_schema).map_err(|_| Error::<T>::Serialization)?,
			)?;

			// Mark request ID as used and update tracked faucet balance.
			UsedRequestIds::<T>::insert(request_id, ());
			FaucetBalanceWei::<T>::mutate(|b| *b = b.saturating_sub(amount));

			Self::deposit_event(Event::FundRequested {
				request_id: req_id,
				requester,
				to,
				amount,
			});

			Ok(())
		}

		/// Pause the dispenser so that no new funding requests can be made.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		#[pallet::call_index(2)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::pause())]
		pub fn pause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if DispenserConfig::<T>::get().is_none() {
				DispenserConfig::<T>::put(DispenserConfigData { paused: true });
			} else {
				DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = true);
			};

			Self::deposit_event(Event::Paused);
			Ok(())
		}

		/// Unpause the dispenser so that funding requests are allowed again.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`
		#[pallet::call_index(3)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::unpause())]
		pub fn unpause(origin: OriginFor<T>) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			if DispenserConfig::<T>::get().is_none() {
				DispenserConfig::<T>::put(DispenserConfigData { paused: false });
			} else {
				DispenserConfig::<T>::mutate_exists(|p| p.as_mut().unwrap().paused = false);
			};
			Self::deposit_event(Event::Unpaused);
			Ok(())
		}

		/// Increase the tracked faucet ETH balance (in wei).
		///
		/// This is an accounting helper used to keep `FaucetBalanceWei`
		/// roughly in sync with the real faucet balance on the EVM chain.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		/// - `balance_wei`: Amount (in wei) to add to the currently stored balance.
		#[pallet::call_index(4)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_faucet_balance())]
		pub fn set_faucet_balance(origin: OriginFor<T>, balance_wei: Balance) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			let old = FaucetBalanceWei::<T>::get();
			let new_balance = old + balance_wei;
			FaucetBalanceWei::<T>::put(new_balance);
			Self::deposit_event(Event::FaucetBalanceUpdated {
				old_balance_wei: old,
				new_balance_wei: new_balance,
			});
			Ok(())
		}
	}

	// ========================= Helper Functions =========================

	impl<T: Config> Pallet<T> {
		/// Derive a deterministic request ID from the given parameters.
		///
		/// The ID is computed as:
		/// - Encode `(sender_ss58, transaction_data, slip44_chain_id, key_version,
		///   path_str, algo_str, dest_str, params_str)` using Solidity's
		///   `abi_encode_packed`.
		/// - Apply `keccak256` to the result.
		///
		/// This mirrors the off-chain logic used by SigNet clients and prevents
		/// clients from supplying arbitrary request IDs.

		pub fn generate_request_id(
			sender: &T::AccountId,
			transaction_data: &[u8],
			slip44_chain_id: u32,
			key_version: u32,
			path: &[u8],
			algo: &[u8],
			dest: &[u8],
			params: &[u8],
		) -> Bytes32 {
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
		/// Returns the pallet's sovereign account ID.
		///
		/// This account is derived from `PalletId` and is used as the logical
		/// owner of outbound EVM transactions and SigNet requests.
		pub fn account_id() -> T::AccountId {
			<T as pallet::Config>::PalletId::get().into_account_truncating()
		}

		/// Ensures that the dispenser is not paused.
		///
		/// Returns `Ok(())` if the dispenser is active, otherwise `Error::Paused`.
		#[inline]
		fn ensure_not_paused() -> Result<(), Error<T>> {
			match DispenserConfig::<T>::get() {
				Some(DispenserConfigData { paused: true, .. }) => Err(Error::<T>::Paused),
				_ => Ok(()),
			}
		}
	}
}
