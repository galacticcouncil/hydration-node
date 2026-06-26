// Copyright (C) 2020-2026  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! # Lazy-Executor Pallet
//!
//! Queues a resolved intent's `ForwardAction` and later executes it as a best-effort EVM message
//! call: pushes the resolved output to the named contract, then invokes its receiver interface. The
//! whole push+call runs in one storage transaction and rolls back on revert / wrong ack, leaving the
//! owner's funds untouched.

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, MaxEncodedLen};
use evm::ExitReason;
use frame_support::{
	dispatch::{DispatchClass, DispatchInfo, GetDispatchInfo, Pays, PostDispatchInfo},
	pallet_prelude::{RuntimeDebug, TypeInfo},
	storage::{with_transaction, TransactionOutcome},
	traits::{ExistenceRequirement::AllowDeath, Get},
	transactional,
	weights::Weight,
};
use frame_system::{offchain::SubmitTransaction, pallet_prelude::*};
use hydradx_traits::{
	evm::{CallContext, CallResult, Erc20Mapping, InspectEvmAccounts, EVM},
	lazy_executor::{ForwardAction, Source},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::MultiCurrency;
use pallet_evm::GasWeightMapping;
use pallet_transaction_payment::OnChargeTransaction;
use precompile_utils::evm::{writer::EvmDataWriter, Bytes};
use primitives::{AssetId, Balance};
use sp_core::U256;
use sp_runtime::{
	traits::{Convert, Dispatchable, One},
	DispatchError, DispatchResult,
};
use sp_std::vec::Vec;

pub use pallet::*;
pub mod weights;
pub use weights::WeightInfo;

#[cfg(test)]
mod tests;

pub type CallId = u128;
type BalanceOf<T> = <<T as pallet_transaction_payment::Config>::OnChargeTransaction as OnChargeTransaction<T>>::Balance;

/// A queued forward together with the owner whose funds back it.
#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, MaxEncodedLen, TypeInfo)]
pub struct StoredForward<AccountId> {
	pub owner: AccountId,
	pub action: ForwardAction,
}

const NO_TIP: u32 = 0;
//Encoded call's length offset for additional extrinsic's data in bytes.
//4(length) + 1(version&type) + 32(signer) + 65(signature) + 16(tip) + 40(signedExtras) + 16(tip)
//NOTE: this is approximate number
const CALL_LEN_OFFSET: u32 = 158;
const LOG_TARGET: &str = "runtime::pallet-lazy-executor";
pub(crate) const OCW_TAG_PREFIX: &str = "lazy-executor-dispatch-top";

/// ABI selectors of the receiver interface invoked on a forward.
#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	Execute = "execute(address,uint256,address,uint256,address,uint256,bytes)",
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::{
		dispatch::{DispatchInfo, DispatchResult},
		pallet_prelude::{TransactionSource, TransactionValidity, ValueQuery, *},
	};
	use hydradx_traits::CreateBare;

	#[pallet::config]
	pub trait Config:
		CreateBare<Call<Self>>
		+ frame_system::Config
		+ pallet_transaction_payment::Config<RuntimeCall = <Self as pallet::Config>::RuntimeCall>
	where
		<Self as frame_system::Config>::AccountId: AsRef<[u8; 32]>,
	{
		/// The aggregated call type. Used only for the unsigned `dispatch_top` extrinsic and the
		/// transaction-payment fee machinery — queued forwards are no longer arbitrary calls.
		type RuntimeCall: Parameter
			+ Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, Info = DispatchInfo, PostInfo = PostDispatchInfo>
			+ GetDispatchInfo
			+ From<frame_system::Call<Self>>;

		/// Multi currency, used to push the resolved output to the receiver contract.
		type Currency: MultiCurrency<Self::AccountId, CurrencyId = AssetId, Balance = Balance>;

		/// EVM handler.
		type Evm: EVM<CallResult>;

		/// EVM address converter.
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// Mapping between AssetId and ERC20 address.
		type Erc20Mapping: Erc20Mapping<AssetId>;

		/// Gas to Weight conversion.
		type GasWeightMapping: GasWeightMapping;

		/// The gas limit for the forwarded EVM call.
		#[pallet::constant]
		type GasLimit: Get<u64>;

		/// Decoder turning an EVM `CallResult` into a `DispatchError` for the emitted event.
		type EvmErrorDecoder: Convert<CallResult, DispatchError>;

		/// Configuration for unsigned transaction priority
		#[pallet::constant]
		type UnsignedPriority: Get<TransactionPriority>;

		/// Configuration for unsigned transaction longevity
		#[pallet::constant]
		type UnsignedLongevity: Get<TransactionLongevity>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::type_value]
	pub(super) fn DefaultMaxTxPerBlock() -> u16 {
		10_u16
	}

	#[pallet::type_value]
	pub(super) fn DefaultMaxCallWeight() -> Weight {
		// Must accommodate a forward's `dispatch_top` weight = base + GasWeightMapping(GasLimit).
		// At WEIGHT_PER_GAS = 25_000 a multi-million-gas forward is tens of G ref_time; this cap
		// (a generous fraction of the 2000G block) stays well below the block limit.
		Weight::from_parts(1_000_000_000_000_u64, 5_000_000)
	}

	#[pallet::storage]
	#[pallet::getter(fn max_txs_per_block)]
	pub(super) type MaxTxPerBlock<T: Config> = StorageValue<_, u16, ValueQuery, DefaultMaxTxPerBlock>;

	#[pallet::storage]
	#[pallet::getter(fn max_weight_per_call)]
	//max weight of the `dispatch_top`. (Inner call's weight should be included)
	pub(super) type MaxCallWeight<T: Config> = StorageValue<_, Weight, ValueQuery, DefaultMaxCallWeight>;

	#[pallet::storage]
	#[pallet::getter(fn next_call_id)]
	pub(super) type Sequencer<T: Config> = StorageValue<_, CallId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn dispatch_next_id)]
	pub(super) type DispatchNextId<T: Config> = StorageValue<_, CallId, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn call_queue)]
	pub(super) type CallQueue<T: Config> = StorageMap<_, Blake2_128Concat, CallId, StoredForward<T::AccountId>>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config>
	where
		<T as frame_system::Config>::AccountId: AsRef<[u8; 32]>,
	{
		/// Forward was queued for execution.
		Queued {
			id: CallId,
			src: Source,
			who: T::AccountId,
			fees: BalanceOf<T>,
		},

		/// Forward was executed. `result` is `Ok` only on EVM success with the correct ack.
		Executed { id: CallId, result: DispatchResult },
	}

	#[pallet::error]
	pub enum Error<T> {
		/// `id` reached max. value.
		IdOverflow,

		/// Arithmetic or type conversion overflow
		Overflow,

		/// User failed to pay fees for future execution.
		FailedToPayFees,

		/// Failed to deposit collected fees.
		FailedToDepositFees,

		/// Queue is empty.
		EmptyQueue,

		/// Forward's weight is bigger than max allowed weight.
		Overweight,

		/// The forwarded EVM call reverted or returned a wrong/missing ack.
		ForwardFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]>,
	{
		fn offchain_worker(block_number: BlockNumberFor<T>) {
			log::debug!(target: LOG_TARGET, "run offchain worker on block: {block_number:?}");

			let mut next_id = Self::dispatch_next_id();
			for _ in 0..Self::max_txs_per_block() {
				next_id = if let Some(n) = next_id.checked_add(1_u128) {
					n
				} else {
					log::debug!(target: LOG_TARGET, "queue is empty");
					break;
				};

				if CallQueue::<T>::contains_key(next_id) {
					let call = Call::dispatch_top { id: next_id };
					let tx = T::create_bare(call.into());
					if let Err(e) = SubmitTransaction::<T, Call<T>>::submit_transaction(tx) {
						debug_assert!(false, "laxy-executorn: failed to submit dispatch_top transaction");
						log::error!(target: LOG_TARGET, "to submit dispatch_top call, err: {e:?}");
					}
				} else {
					break;
				}
			}
		}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]>,
	{
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, unsigned_call: &self::Call<T>) -> TransactionValidity {
			if let Call::dispatch_top { id } = unsigned_call {
				// discard call not coming from the local node
				match source {
					TransactionSource::Local | TransactionSource::InBlock => { /* allowed */ }
					_ => {
						return InvalidTransaction::Call.into();
					}
				}

				ensure!(
					CallQueue::<T>::contains_key(Self::dispatch_next_id()),
					InvalidTransaction::Call
				);

				return ValidTransaction::with_tag_prefix(OCW_TAG_PREFIX)
					.priority(T::UnsignedPriority::get())
					.and_provides(id)
					.longevity(T::UnsignedLongevity::get())
					.propagate(false)
					.build();
			}

			InvalidTransaction::Call.into()
		}
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]>,
	{
		/// Extrinsic executes the top forward from the queue.
		///
		/// This is called from OCW.
		///
		/// Emits:
		/// - `Executed` when successful
		#[pallet::call_index(1)]
		#[pallet::weight(
			<T as pallet::Config>::WeightInfo::dispatch_top_base_weight()
				.saturating_add(<T as pallet::Config>::GasWeightMapping::gas_to_weight(
					<T as pallet::Config>::GasLimit::get(),
					false,
				))
		)]
		pub fn dispatch_top(origin: OriginFor<T>, _id: u128) -> DispatchResult {
			ensure_none(origin)?;

			DispatchNextId::<T>::try_mutate(|id| {
				let StoredForward { owner, action } = CallQueue::<T>::take(*id).ok_or(Error::<T>::EmptyQueue)?;

				let result = Self::execute_forward(owner, action);

				Self::deposit_event(Event::Executed { id: *id, result });

				*id = id.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

				Ok(())
			})
		}
	}
}

impl<T: Config> Pallet<T>
where
	T::AccountId: AsRef<[u8; 32]>,
{
	/// Function adds a forward to the queue for future execution.
	///
	/// This function also charges fees for future execution and fails if `origin` can't pay them.
	#[transactional]
	pub fn add_to_queue(src: Source, owner: T::AccountId, action: ForwardAction) -> Result<(), DispatchError> {
		let call_weight = <T as Config>::WeightInfo::dispatch_top_base_weight().saturating_add(
			<T as Config>::GasWeightMapping::gas_to_weight(<T as Config>::GasLimit::get(), false),
		);

		if call_weight.any_gt(Self::max_weight_per_call()) {
			return Err(Error::<T>::Overweight.into());
		}

		let info = DispatchInfo {
			call_weight,
			extension_weight: Default::default(),
			class: DispatchClass::Normal,
			pays_fee: Pays::Yes,
		};

		let len = Call::<T>::dispatch_top { id: u128::MAX }
			.encoded_size()
			.saturating_add(CALL_LEN_OFFSET.try_into().map_err(|_| Error::<T>::Overflow)?);

		let fees = pallet_transaction_payment::Pallet::<T>::compute_fee(
			len.try_into().map_err(|_| Error::<T>::Overflow)?,
			&info,
			NO_TIP.into(),
		);

		// `OnChargeTransaction` is keyed by a call; the queued forward is not a call, so a benign
		// no-op call stands in for the fee accounting only.
		let fee_call = <T as Config>::RuntimeCall::from(frame_system::Call::remark { remark: Vec::new() });

		let already_withdrawn = <T as pallet_transaction_payment::Config>::OnChargeTransaction::withdraw_fee(
			&owner,
			&fee_call,
			&info,
			fees,
			NO_TIP.into(),
		)
		.map_err(|_| Error::<T>::FailedToPayFees)?;

		<T as pallet_transaction_payment::Config>::OnChargeTransaction::correct_and_deposit_fee(
			&owner,
			&info,
			&PostDispatchInfo {
				actual_weight: Some(call_weight),
				pays_fee: Pays::Yes,
			},
			fees,
			NO_TIP.into(),
			already_withdrawn,
		)
		.map_err(|_| Error::<T>::FailedToDepositFees)?;

		let call_id = Self::get_next_call_id()?;
		CallQueue::<T>::insert(
			call_id,
			StoredForward {
				owner: owner.clone(),
				action,
			},
		);

		Self::deposit_event(Event::Queued {
			id: call_id,
			src,
			who: owner,
			fees,
		});
		Ok(())
	}

	/// Pushes the resolved output to the receiver contract and invokes `execute(...)` in one
	/// transactional scope. Rolls the push back on revert / wrong ack, leaving the owner whole.
	fn execute_forward(owner: T::AccountId, action: ForwardAction) -> DispatchResult {
		let owner_evm = T::EvmAccounts::evm_address(&owner);
		let dest = T::EvmAccounts::account_id(action.contract);

		let ForwardAction {
			contract,
			intent_id,
			asset_in,
			amount_in,
			asset_out,
			amount_out,
			data,
		} = action;

		let calldata = EvmDataWriter::new_with_selector(Function::Execute)
			.write(owner_evm)
			.write(U256::from(intent_id))
			.write(T::Erc20Mapping::asset_address(asset_in))
			.write(U256::from(amount_in))
			.write(T::Erc20Mapping::asset_address(asset_out))
			.write(U256::from(amount_out))
			.write(Bytes(data.into_inner()))
			.build();

		with_transaction(|| {
			// 1. push: credit the contract's mapped account so its ERC20 balanceOf reflects it.
			if let Err(e) = T::Currency::transfer(asset_out, &owner, &dest, amount_out, AllowDeath) {
				return TransactionOutcome::Rollback(Err(e));
			}

			// 2. EVM message call as the owner (msg.sender == tx.origin == owner).
			let ctx = CallContext::new_call(contract, owner_evm);
			let res = T::Evm::call(ctx, calldata, U256::zero(), <T as Config>::GasLimit::get());

			// 3. commit only on EVM success with the correct ack; otherwise roll the push back.
			let succeeded = matches!(res.exit_reason, ExitReason::Succeed(_));
			if succeeded && Self::ack_ok(&res.value) {
				TransactionOutcome::Commit(Ok(()))
			} else if succeeded {
				TransactionOutcome::Rollback(Err(Error::<T>::ForwardFailed.into()))
			} else {
				TransactionOutcome::Rollback(Err(T::EvmErrorDecoder::convert(res)))
			}
		})
	}

	/// The receiver acknowledges by returning `execute`'s own selector as a `bytes4` (left-aligned
	/// in the 32-byte ABI return word).
	fn ack_ok(value: &[u8]) -> bool {
		let expected = Into::<u32>::into(Function::Execute).to_be_bytes();
		value.len() >= 4 && value[..4] == expected
	}

	fn get_next_call_id() -> Result<CallId, DispatchError> {
		Sequencer::<T>::try_mutate(|current_val| {
			let ret = *current_val;
			*current_val = current_val.checked_add(One::one()).ok_or(Error::<T>::IdOverflow)?;

			Ok(ret)
		})
	}
}

impl<T: Config> hydradx_traits::lazy_executor::Mutate<T::AccountId> for Pallet<T>
where
	T::AccountId: AsRef<[u8; 32]>,
{
	type Error = DispatchError;

	fn queue(src: Source, origin: T::AccountId, forward: ForwardAction) -> Result<(), Self::Error> {
		Self::add_to_queue(src, origin, forward)
	}
}
