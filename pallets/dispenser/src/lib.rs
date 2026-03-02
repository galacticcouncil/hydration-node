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
use codec::{Decode, DecodeWithMemTracking, Encode};
use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Inspect;
use frame_support::traits::{fungibles::Mutate, tokens::Preservation};
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, BoundedVec};
use frame_system::pallet_prelude::*;
use primitives::EvmAddress;
use sp_std::vec::Vec;

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
#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, Clone, Debug, PartialEq)]
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

		/// Pallet ID used to derive the pallet's sovereign account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Weight information provider for extrinsics of this pallet.
		type WeightInfo: crate::WeightInfo;
	}

	/*************************** STORAGE ***************************/

	/// Unified dispenser state: pause flag, tracked faucet balance, and all
	/// governance-controlled parameters in a single storage entry.
	///
	/// Must be initialised via `set_faucet_params` before any funding requests
	/// can be made.  `pause`/`unpause`/`set_faucet_balance` will create the
	/// entry with defaults if it does not yet exist.
	#[pallet::storage]
	#[pallet::getter(fn dispenser_config)]
	pub type DispenserConfig<T> = StorageValue<_, DispenserConfigData, OptionQuery>;

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
		/// Faucet parameters have been updated via governance.
		FaucetParamsUpdated {
			/// EVM address of the faucet contract.
			faucet_address: EvmAddress,
			/// Minimum ETH (in wei) that must remain in the faucet.
			min_faucet_threshold: Balance,
			/// Minimum request amount.
			min_request: Balance,
			/// Maximum dispense amount.
			max_dispense: Balance,
			/// Flat fee per request.
			dispenser_fee: Balance,
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
		/// Faucet address or minimum threshold has not been set via governance.
		NotInitialized,
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
		/// - Submits a signing request to SigNet via `pallet_signet::sign_bidirectional`.
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

			// Load full config (includes paused flag, balance, and params).
			let config = DispenserConfig::<T>::get().ok_or(Error::<T>::NotInitialized)?;
			ensure!(!config.paused, Error::<T>::Paused);

			// Basic validation of parameters.
			ensure!(to != EvmAddress::zero(), Error::<T>::InvalidAddress);
			ensure!(amount >= config.min_request, Error::<T>::AmountTooSmall);
			ensure!(amount <= config.max_dispense, Error::<T>::AmountTooLarge);

			// Check tracked faucet balance vs. threshold.
			let observed = config.faucet_balance_wei;
			let min_threshold = config.min_faucet_threshold;
			let needed = min_threshold
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
				to: alloy_primitives::Address::from_slice(to.as_bytes()),
				amount: U256::from(amount),
			};

			// Build EVM transaction bytes using pallet_signet helper.
			let faucet_addr = config.faucet_address;
			let rlp = pallet_signet::Pallet::<T>::build_evm_tx(
				frame_system::RawOrigin::Signed(requester.clone()).into(),
				Some(faucet_addr),
				0u128,
				call.abi_encode(),
				tx.nonce,
				tx.gas_limit,
				tx.max_fee_per_gas,
				tx.max_priority_fee_per_gas,
				Vec::new(),
				tx.chain_id,
			)?;

			let path = SIGNING_PATH.to_vec();

			// CAIP-2 chain ID (e.g., "eip155:1" for Ethereum mainnet)
			let caip2_id = alloc::format!("eip155:{}", tx.chain_id);

			// Derive canonical request ID and compare with user-supplied one.
			let req_id = Self::generate_request_id(&pallet_acc, &rlp, &caip2_id, 0, &path, ECDSA, ETHEREUM, b"");

			ensure!(req_id == request_id, Error::<T>::InvalidRequestId);
			ensure!(
				UsedRequestIds::<T>::get(request_id).is_none(),
				Error::<T>::DuplicateRequest
			);

			// Check balances for fee and faucet asset.
			let fee = config.dispenser_fee;
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

			let output_deserialization_schema = Vec::<u8>::new();
			let respond_serialization_schema =
				serde_json::to_vec(&serde_json::json!("bool")).map_err(|_| Error::<T>::Serialization)?;

			// Submit signing request to SigNet.
			pallet_signet::Pallet::<T>::sign_bidirectional(
				frame_system::RawOrigin::Signed(pallet_acc.clone()).into(),
				BoundedVec::try_from(rlp).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(caip2_id.into_bytes()).map_err(|_| Error::<T>::Serialization)?,
				0,
				BoundedVec::try_from(path).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(ECDSA.to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(ETHEREUM.to_vec()).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(vec![]).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(output_deserialization_schema).map_err(|_| Error::<T>::Serialization)?,
				BoundedVec::try_from(respond_serialization_schema).map_err(|_| Error::<T>::Serialization)?,
			)?;

			// Mark request ID as used and update tracked faucet balance.
			UsedRequestIds::<T>::insert(request_id, ());
			DispenserConfig::<T>::mutate(|s| {
				if let Some(ref mut state) = s {
					state.faucet_balance_wei = state.faucet_balance_wei.saturating_sub(amount);
				}
			});

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
			let mut state = DispenserConfig::<T>::get().unwrap_or_default();
			state.paused = true;
			DispenserConfig::<T>::put(state);
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
			let mut state = DispenserConfig::<T>::get().unwrap_or_default();
			state.paused = false;
			DispenserConfig::<T>::put(state);
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
			let mut state = DispenserConfig::<T>::get().unwrap_or_default();
			let old = state.faucet_balance_wei;
			state.faucet_balance_wei = old + balance_wei;
			let new_balance = state.faucet_balance_wei;
			DispenserConfig::<T>::put(state);
			Self::deposit_event(Event::FaucetBalanceUpdated {
				old_balance_wei: old,
				new_balance_wei: new_balance,
			});
			Ok(())
		}
		/// Set or update all governance-controlled dispenser parameters.
		///
		/// Parameters:
		/// - `origin`: Must satisfy `UpdateOrigin`.
		/// - `faucet_address`: EVM address of the external faucet contract.
		/// - `min_faucet_threshold`: Minimum ETH (in wei) that must remain after a request.
		/// - `min_request`: Minimum amount of faucet asset per request.
		/// - `max_dispense`: Maximum amount of faucet asset per request.
		/// - `dispenser_fee`: Flat fee charged in `FeeAsset` per request.
		#[pallet::call_index(5)]
		#[pallet::weight(<T as pallet::Config>::WeightInfo::set_faucet_params())]
		pub fn set_faucet_params(
			origin: OriginFor<T>,
			faucet_address: EvmAddress,
			min_faucet_threshold: Balance,
			min_request: Balance,
			max_dispense: Balance,
			dispenser_fee: Balance,
		) -> DispatchResult {
			T::UpdateOrigin::ensure_origin(origin)?;
			let mut state = DispenserConfig::<T>::get().unwrap_or_default();
			state.faucet_address = faucet_address;
			state.min_faucet_threshold = min_faucet_threshold;
			state.min_request = min_request;
			state.max_dispense = max_dispense;
			state.dispenser_fee = dispenser_fee;
			DispenserConfig::<T>::put(state);
			Self::deposit_event(Event::FaucetParamsUpdated {
				faucet_address,
				min_faucet_threshold,
				min_request,
				max_dispense,
				dispenser_fee,
			});
			Ok(())
		}
	}

	// ========================= Helper Functions =========================

	impl<T: Config> Pallet<T> {
		/// Derive a deterministic request ID from the given parameters.
		///
		/// The ID is computed as:
		/// - Encode `(sender_ss58, transaction_data, caip2_id, key_version,
		///   path_str, algo_str, dest_str, params_str)` using Solidity's
		///   `abi_encode_packed`.
		/// - Apply `keccak256` to the result.
		///
		/// This mirrors the off-chain logic used by SigNet clients and prevents
		/// clients from supplying arbitrary request IDs.
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
	}

	impl<T: Config> Pallet<T> {
		/// Returns the pallet's sovereign account ID.
		///
		/// This account is derived from `PalletId` and is used as the logical
		/// owner of outbound EVM transactions and SigNet requests.
		pub fn account_id() -> T::AccountId {
			<T as pallet::Config>::PalletId::get().into_account_truncating()
		}
	}
}
