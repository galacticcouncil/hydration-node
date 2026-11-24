// This file is part of HydraDX.
// Copyright (C) 2020-2024  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Liquidation (Money market) pallet
//!
//! ## Description
//! The pallet uses mechanism similar to a flash loan to liquidate a MM position.
//!
//! ## Notes
//! The pallet requires the money market contract to be deployed and enabled.
//!
//! ## Dispatchable functions
//! * `liquidate` - Liquidates an existing MM position. Performs flash loan to get funds.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::useless_conversion)]

use codec::{decode_from_bytes, Encode};
use ethabi::ethereum_types::BigEndianHash;
use evm::{ExitReason, ExitSucceed};
use frame_support::{
	dispatch::{DispatchErrorWithPostInfo, DispatchResultWithPostInfo, PostDispatchInfo},
	pallet_prelude::*,
	sp_runtime::{traits::AccountIdConversion, SaturatedConversion, Saturating},
	traits::{
		fungibles::{Inspect, Mutate},
		tokens::{Fortitude, Precision, Preservation},
		DefensiveOption,
	},
	PalletId,
};
use frame_system::{
	ensure_none,
	pallet_prelude::{BlockNumberFor, OriginFor},
	RawOrigin,
};
use hydradx_traits::evm::CallResult;
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::gigahdx::Seize;
use hydradx_traits::{
	evm::{CallContext, InspectEvmAccounts, EVM},
	router::{AmmTradeWeights, AmountInAndOut, Route, RouteProvider, RouterT, Trade},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_broadcast::types::ExecutionType;
use pallet_evm::GasWeightMapping;
use precompile_utils::evm::{
	writer::{EvmDataReader, EvmDataWriter},
	Bytes,
};
use primitives::EvmAddress;
use sp_arithmetic::ArithmeticError;
use sp_core::{crypto::AccountId32, H256, U256};
use sp_runtime::traits::Convert;
use sp_std::{vec, vec::Vec};
#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod traits;

pub mod weights;
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

// EVM gas used by the running liquidation. Not a storage value: a reverted flash loan rolls storage
// back, and the gas its callback burned must still be counted.
environmental::environmental!(evm_gas_used: u64);

pub type Balance = u128;
pub type AssetId = u32;
pub type Priority = u64;

//NOTE: `u64::max - 1` is set in /node/src/tx_priority.json oracles' updates.
//We don't want to frontrun oracle updates so these should be kept in sync.
pub const MAX_UNSIGNED_LIQUIDATION_PRIORITY: Priority = u64::MAX - 2;
// Blocks an unsigned liquidation may name as its last valid one, counted from the next block. Keeps
// the expiry a property of the chain instead of trusting the node to pick a near block. Well above
// the one block a worker asks for: a collator's pool validates against a view that can trail its
// best block by a few blocks while it authors, and the bound must not reject the liquidation there.
pub const MAX_UNSIGNED_LIQUIDATION_LIFETIME: u32 = 10;
//NOTE: base unsigned liquidation tx priority. `unsigned_priority` param is added on top of this
//and it should represent collateral at risk(max. 10_000_000.0[BASE]).
const BASE_UNSIGNED_LIQUIDATION_PRIORITY: Priority = MAX_UNSIGNED_LIQUIDATION_PRIORITY - 10_000_000;
//NOTE: the polkadot-sdk fork caps signed user tx priority at `MAX_USER_TX_PRIORITY`
//(`substrate/frame/transaction-payment/src/lib.rs`, `u64::MAX - 1_000_000_000`), which is below
//`BASE_UNSIGNED_LIQUIDATION_PRIORITY` — users cannot frontrun liquidations with a tip.
//Keep the two in sync if either changes.

#[module_evm_utility_macro::generate_function_selector]
#[derive(Debug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
	FlashLoan = "flashLoan(address,address,uint256,bytes)",
	Borrow = "borrow(address,uint256,uint256,uint16,address)",
	Repay = "repay(address,uint256,uint256,address)",
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	pub use crate::traits::GigaHdxSupport;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_broadcast::Config {
		/// Multi currency.
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// EVM handler.
		type Evm: EVM<CallResult>;

		/// Router implementation.
		type Router: RouteProvider<AssetId>
			+ RouterT<Self::RuntimeOrigin, AssetId, Balance, Trade<AssetId>, AmountInAndOut<Balance>>;

		/// EVM address converter.
		type EvmAccounts: InspectEvmAccounts<Self::AccountId>;

		/// Mapping between AssetId and ERC20 address.
		type Erc20Mapping: Erc20Mapping<AssetId>;

		/// Gas to Weight conversion.
		type GasWeightMapping: GasWeightMapping;

		/// The gas limit for the execution of the liquidation call.
		#[pallet::constant]
		type GasLimit: Get<u64>;

		/// Account who receives the profit.
		#[pallet::constant]
		type ProfitReceiver: Get<Self::AccountId>;

		/// Router weight information.
		type RouterWeightInfo: AmmTradeWeights<Trade<AssetId>>;

		/// Weight information for the extrinsics.
		type WeightInfo: WeightInfo;

		// Support for HOLLAR liquidations.
		/// Asset ID of Hollar
		#[pallet::constant]
		type HollarId: Get<AssetId>;

		/// Flash minter contract address and flash loan receiver address.
		type FlashMinter: Get<Option<(EvmAddress, EvmAddress)>>;

		type EvmErrorDecoder: Convert<CallResult, DispatchError>;

		/// The origin which can update transaction priorities, allowed signers and call addresses
		/// for the liquidation worker.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Single integration seam for the protocol-funded gigahdx
		/// liquidation path: asset/account wiring, the live pool-contract
		/// lookup, the seize ledger ops and the conviction-lock release.
		/// Wired to `pallet_gigahdx` / `pallet_gigahdx_rewards` in the runtime.
		type GigaHdx: crate::traits::GigaHdxSupport<Self::AccountId>;

		/// Size of the storage proof recorded so far in the block, `None` where proof recording
		/// is off. Lets a liquidation be charged the proof it produced instead of its reservation.
		type RecordedProofSize: Get<Option<u64>>;
	}

	#[pallet::type_value]
	pub fn DefaultBorrowingContract() -> EvmAddress {
		EvmAddress::from_slice(hex_literal::hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38").as_slice())
	}

	/// Borrowing market contract address
	#[pallet::storage]
	#[pallet::getter(fn borrowing_contract)]
	pub type BorrowingContract<T: Config> = StorageValue<_, EvmAddress, ValueQuery, DefaultBorrowingContract>;

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
					// receiving unsigned transaction from network - disallow
					return InvalidTransaction::Call.into();
				}
				TransactionSource::Local => {}   // produced by offchain worker
				TransactionSource::InBlock => {} // some other node included it in a block
			};

			fn valid_tx(provides: impl Encode, priority: Priority, longevity: u64) -> TransactionValidity {
				ValidTransaction::with_tag_prefix("liquidate_unsigned")
					.priority(
						BASE_UNSIGNED_LIQUIDATION_PRIORITY
							.saturating_add(priority)
							.min(MAX_UNSIGNED_LIQUIDATION_PRIORITY),
					)
					.and_provides(provides)
					.longevity(longevity)
					.propagate(false)
					.build()
			}

			match call {
				// Legacy call — byte-identical to the pre-multi-MM release (no priority
				// param): unsigned submissions get the base priority, exactly as before.
				Call::liquidate { user, .. } => valid_tx(user, 0, 1),
				// (user, pool): the same user underwater in two markets must not produce
				// mutually-replacing transactions.
				Call::liquidate_with_pool {
					user,
					pool,
					unsigned_priority,
					..
				} => valid_tx((user, pool), unsigned_priority.unwrap_or(0), 1),
				Call::submit_liquidation {
					user,
					pool,
					priority,
					valid_until,
					..
				} => {
					// The block this liquidation would go into: the pool validates on top of the
					// best block with the number already advanced, a block being built has its own.
					let including_block = frame_system::Pallet::<T>::block_number();
					if including_block > *valid_until {
						return InvalidTransaction::Stale.into();
					}
					if *valid_until >= including_block.saturating_add(MAX_UNSIGNED_LIQUIDATION_LIFETIME.into()) {
						return InvalidTransaction::Call.into();
					}
					// The pool counts longevity from the block it validates on, one before
					// `including_block`, and drops the transaction once it runs out.
					let longevity = valid_until
						.saturating_sub(including_block)
						.saturated_into::<u64>()
						.saturating_add(1);
					valid_tx((user, pool), *priority, longevity)
				}
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Money market position has been liquidated
		Liquidated {
			user: EvmAddress,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			profit: Balance,
		},
		/// A gigahdx Money Market position was liquidated by the protocol.
		/// `hdx_seized` is the matching HDX moved from the borrower to
		/// the gigahdx liquidation account; `gigahdx_seized` is the aToken
		/// amount transferred out of the borrower's gigahdx position.
		GigaHdxLiquidated {
			user: EvmAddress,
			debt_repaid: Balance,
			hdx_seized: Balance,
			gigahdx_seized: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// AssetId to EVM address conversion failed
		AssetConversionFailed,
		/// Liquidation call failed
		LiquidationCallFailed,
		/// Provided route doesn't match the existing route
		InvalidRoute,
		/// Liquidation was not profitable enough to repay flash loan
		NotProfitable,
		/// Flash minter contract address not set. It is required for Hollar liquidations.
		FlashMinterNotSet,
		/// Invalid liquidation data provided
		InvalidLiquidationData,
		/// Gigahdx liquidation only supports HOLLAR debt.
		UnsupportedDebtAsset,
		/// Borrower has no active gigahdx position.
		NoGigaHdxPosition,
		/// Realizing the borrower's accrued gigahdx yield before the seize failed.
		RealizeYieldFailed,
		/// The gigahdx liquidation account is not bound to its EVM address, so
		/// the seized aToken would land in a different account than the ledger.
		LiquidationAccountNotBound,
		/// Selective vote clearing failed before seize.
		ClearVotingLocksFailed,
		/// Treasury's HOLLAR borrow against the GIGAHDX pool reverted.
		BorrowFailed,
		/// Final state move (HDX transfer + lock refresh) failed.
		SeizeFailed,
		/// GIGAHDX pool contract address not set in `pallet_gigahdx`.
		GigaHdxPoolNotSet,
		/// Borrower has no `debt_asset` debt on the GIGAHDX pool, so there is
		/// nothing to liquidate.
		NoPoolDebt,
		/// Repaying the unconsumed protocol-borrowed HOLLAR surplus reverted.
		RepayFailed,
		/// The `pool` provided to `liquidate_with_pool` is not the pool this liquidation
		/// would execute against.
		PoolAddressMismatch,
		/// The execution context stack was not as left by the collateral sale.
		UnbalancedExecutionContext,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		/// Liquidates an existing money market position.
		/// Can be both signed and unsigned.
		///
		/// Performs a flash loan to get funds to pay for the debt.
		/// Received collateral is swapped and the profit is transferred to `FeeReceiver`.
		/// Omnipool hops of the collateral sale pay only the base asset and protocol fees and no slip fee.
		///
		/// Permissionless and caller-supplied `route` are intentional. This call exists
		/// only to keep the money market solvent: the goal is that the position is
		/// liquidated, not that the protocol captures the surplus. A caller can pick a
		/// `route` that prices the collateral so that little or no profit reaches
		/// `FeeReceiver` — that is acceptable. It is not an exploit: anyone can already
		/// liquidate the same position by calling Aave's `liquidationCall` directly and
		/// keep the entire bonus themselves. So reports framing the open origin or the
		/// attacker-chosen route as a fund-redirection vulnerability are out of scope by
		/// design; the only invariant that matters here is that unhealthy positions can
		/// always be closed.
		///
		/// Parameters:
		/// - `origin`: Signed origin.
		/// - `collateral_asset`: Asset ID used as collateral in the MM position.
		/// - `debt_asset`: Asset ID used as debt in the MM position.
		/// - `user`: EVM address of the MM position that we want to liquidate.
		/// - `debt_to_cover`: Amount of debt we want to liquidate.
		/// - `route`: The route we trade against. Required for the fee calculation.
		///
		/// Emits `Liquidated` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(Pallet::<T>::liquidation_weight(*collateral_asset, *debt_asset, *user, route))]
		pub fn liquidate(
			_origin: OriginFor<T>,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Route<AssetId>,
		) -> DispatchResultWithPostInfo {
			let declared = Self::liquidation_weight(collateral_asset, debt_asset, user, &route);
			Self::with_weight_refund(declared, Self::evm_gas_budgets(collateral_asset, debt_asset), || {
				Self::do_liquidate(collateral_asset, debt_asset, user, debt_to_cover, route)
			})
		}

		/// Set the borrowing market contract address.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_borrowing_contract())]
		pub fn set_borrowing_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;

			BorrowingContract::<T>::put(contract);

			Ok(())
		}

		/// Liquidates an existing money market position, addressing the market's pool explicitly.
		///
		/// Behaves exactly like `liquidate`; the extra `pool` parameter is a consistency
		/// assertion, not a router: it must equal the pool contract the liquidation path
		/// resolves on its own (the gigahdx pool for GIGAHDX-collateral positions, the
		/// borrowing contract otherwise), or the call fails with `PoolAddressMismatch`.
		/// This lets a multi-money-market worker state which market a decision was made
		/// against and guarantees the liquidation can never execute against a different one.
		/// Omnipool hops of the collateral sale pay only the base asset and protocol fees and no slip fee.
		///
		/// Unlike `liquidate`, this call is not publicly dispatchable: the origin must be
		/// none, and `ValidateUnsigned` rejects externally received transactions, so only
		/// a collator's own liquidation worker can submit it. The public permissionless
		/// path remains `liquidate`.
		///
		/// Parameters:
		/// - `origin`: Must be none (unsigned transaction).
		/// - `pool`: EVM address of the money-market pool this liquidation targets.
		/// - `collateral_asset`: Asset ID used as collateral in the MM position.
		/// - `debt_asset`: Asset ID used as debt in the MM position.
		/// - `user`: EVM address of the MM position that we want to liquidate.
		/// - `debt_to_cover`: Amount of debt we want to liquidate.
		/// - `route`: The route we trade against. Required for the fee calculation.
		/// - `unsigned_priority`: Optional priority added on top of `BASE_UNSIGNED_LIQUIDATION_PRIORITY` for
		/// unsigned liquidation extrinsics.
		///
		/// Emits `Liquidated` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(Pallet::<T>::unsigned_liquidation_weight(*collateral_asset, *debt_asset, *user, route))]
		pub fn liquidate_with_pool(
			origin: OriginFor<T>,
			pool: EvmAddress,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Route<AssetId>,
			_unsigned_priority: Option<Priority>,
		) -> DispatchResultWithPostInfo {
			Self::do_unsigned_liquidation(origin, pool, collateral_asset, debt_asset, user, debt_to_cover, route)
		}

		/// Liquidates an existing money market position on behalf of the protocol, as an unsigned
		/// transaction that expires.
		///
		/// Only a collator's own liquidation worker can submit it: the origin must be none and
		/// `ValidateUnsigned` rejects externally received transactions. The public permissionless
		/// path is `liquidate`.
		///
		/// The transaction is valid up to block `valid_until` and is rejected as stale after it,
		/// in the transaction pool and at inclusion, so a liquidation decided on an old state can
		/// never be executed later. Supersedes `liquidate_with_pool`, which never expires.
		/// Omnipool hops of the collateral sale pay only the base asset and protocol fees and no slip fee.
		///
		/// Parameters:
		/// - `origin`: Must be none (unsigned transaction).
		/// - `pool`: EVM address of the money-market pool the liquidation was decided against. Must
		/// be the pool the liquidation executes on, or the call fails with `PoolAddressMismatch`.
		/// - `collateral_asset`: Asset ID used as collateral in the MM position.
		/// - `debt_asset`: Asset ID used as debt in the MM position.
		/// - `user`: EVM address of the MM position that we want to liquidate.
		/// - `debt_to_cover`: Amount of debt we want to liquidate.
		/// - `route`: The route we trade against. Required for the fee calculation.
		/// - `priority`: Priority added on top of `BASE_UNSIGNED_LIQUIDATION_PRIORITY`.
		/// - `valid_until`: Last block the transaction can be included in. At most
		/// `MAX_UNSIGNED_LIQUIDATION_LIFETIME` blocks ahead.
		///
		/// Emits `Liquidated` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(Pallet::<T>::unsigned_liquidation_weight(*collateral_asset, *debt_asset, *user, route))]
		pub fn submit_liquidation(
			origin: OriginFor<T>,
			pool: EvmAddress,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Route<AssetId>,
			_priority: Priority,
			_valid_until: BlockNumberFor<T>,
		) -> DispatchResultWithPostInfo {
			Self::do_unsigned_liquidation(origin, pool, collateral_asset, debt_asset, user, debt_to_cover, route)
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		PalletId(*b"lqdation").into_account_truncating()
	}

	/// Full `GasLimit` budgets a liquidation can spend in the EVM.
	pub fn evm_gas_budgets(collateral_asset: AssetId, debt_asset: AssetId) -> u64 {
		if collateral_asset == T::GigaHdx::gigahdx_asset_id() {
			// borrow, liquidationCall and surplus repay, plus one for the adapter's debt-read views.
			4
		} else if debt_asset == T::HollarId::get() {
			// The flash loan, and the liquidationCall its callback runs on a gas budget of its own.
			2
		} else {
			1
		}
	}

	/// Declared weight of a liquidation: the `liquidate()` benchmark, the substrate work of its
	/// branch (gigahdx seize and vote clearance, or the router sell) and its EVM gas budgets.
	pub fn liquidation_weight(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		route: &[Trade<AssetId>],
	) -> Weight {
		let branch_weight = if collateral_asset == T::GigaHdx::gigahdx_asset_id() {
			T::GigaHdx::seize_weight().saturating_add(T::GigaHdx::clear_weight_for(user))
		} else {
			T::RouterWeightInfo::sell_weight(route).saturating_add(T::DbWeight::get().reads_writes(1, 1))
		};
		<T as Config>::WeightInfo::liquidate()
			.saturating_add(branch_weight)
			.saturating_add(Self::evm_gas_weight(Self::evm_gas_budgets(
				collateral_asset,
				debt_asset,
			)))
	}

	/// `liquidation_weight` plus the read of the pool the liquidation is checked against.
	pub fn unsigned_liquidation_weight(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		route: &[Trade<AssetId>],
	) -> Weight {
		Self::liquidation_weight(collateral_asset, debt_asset, user, route).saturating_add(T::DbWeight::get().reads(1))
	}

	fn evm_gas_weight(budgets: u64) -> Weight {
		T::GasWeightMapping::gas_to_weight(T::GasLimit::get(), true).saturating_mul(budgets)
	}

	/// Runs `liquidation` and reports the weight it used: ref time less the EVM gas left unspent,
	/// proof size as measured by the block's proof recorder. Applies to failed liquidations too.
	fn with_weight_refund(
		declared: Weight,
		gas_budgets: u64,
		liquidation: impl FnOnce() -> DispatchResult,
	) -> DispatchResultWithPostInfo {
		let proof_before = T::RecordedProofSize::get();
		let mut gas_used = 0u64;
		let result = evm_gas_used::using(&mut gas_used, liquidation);

		let unused_gas = T::GasLimit::get().saturating_mul(gas_budgets).saturating_sub(gas_used);
		let unused_ref_time = T::GasWeightMapping::gas_to_weight(unused_gas, true)
			.ref_time()
			.min(Self::evm_gas_weight(gas_budgets).ref_time());
		let mut actual_weight = declared.saturating_sub(Weight::from_parts(unused_ref_time, 0));
		// Gas says nothing about proof size here: the executor runs without proof metering.
		if let (Some(before), Some(after)) = (proof_before, T::RecordedProofSize::get()) {
			*actual_weight.proof_size_mut() = after.saturating_sub(before).min(declared.proof_size());
		}

		let post_info = PostDispatchInfo {
			actual_weight: Some(actual_weight),
			pays_fee: Pays::Yes,
		};
		match result {
			Ok(()) => Ok(post_info),
			Err(error) => Err(DispatchErrorWithPostInfo { post_info, error }),
		}
	}

	/// EVM call on the liquidation gas budget; the gas it used counts against the weight refund.
	fn evm_call(context: CallContext, data: Vec<u8>) -> CallResult {
		let result = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());
		// A call that never reached the EVM reports no gas. Its cost is unknown: count the budget.
		let used = if result.gas_used.is_zero() {
			T::GasLimit::get()
		} else {
			u64::try_from(result.gas_used).unwrap_or(u64::MAX)
		};
		evm_gas_used::with(|total| *total = total.saturating_add(used));
		result
	}

	/// Shared body of `liquidate_with_pool` and `submit_liquidation`.
	fn do_unsigned_liquidation(
		origin: OriginFor<T>,
		pool: EvmAddress,
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		debt_to_cover: Balance,
		route: Route<AssetId>,
	) -> DispatchResultWithPostInfo
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		let declared = Self::unsigned_liquidation_weight(collateral_asset, debt_asset, user, &route);
		Self::with_weight_refund(declared, Self::evm_gas_budgets(collateral_asset, debt_asset), || {
			ensure_none(origin)?;

			let expected = if collateral_asset == T::GigaHdx::gigahdx_asset_id() {
				T::GigaHdx::pool_contract().ok_or(Error::<T>::GigaHdxPoolNotSet)?
			} else {
				Self::borrowing_contract()
			};
			ensure!(pool == expected, Error::<T>::PoolAddressMismatch);

			Self::do_liquidate(collateral_asset, debt_asset, user, debt_to_cover, route)
		})
	}

	/// Shared body of every liquidation call.
	fn do_liquidate(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		debt_to_cover: Balance,
		route: Route<AssetId>,
	) -> DispatchResult
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		log::trace!(target: "liquidation","liquidating debt asset: {debt_asset:?} for amount: {debt_to_cover:?}");

		if collateral_asset == T::GigaHdx::gigahdx_asset_id() {
			// Protocol-funded gigahdx liquidation: treasury borrows HOLLAR,
			// repays the borrower's debt via Aave's liquidationCall with
			// `receiveAToken=true`, then matching HDX is seized from the
			// borrower's substrate wallet and re-locked under the
			// liquidation account. `route` is unused on this path.
			//
			// Routing is unconditional on the collateral: the gigahdx reserve lists
			// HOLLAR as its ONLY borrowable asset, so GIGAHDX collateral always
			// implies a HOLLAR-debt position. `liquidate_gigahdx`'s
			// `debt_asset == HollarId` check is therefore a fail-closed guard, not a
			// router. A gigahdx position can only be seized through this path (the
			// locked aToken needs the `on_pre_seize`/`on_seize` dance), so it must
			// never fall through to the generic path below. If that reserve is ever
			// configured with another borrowable asset, `liquidate_gigahdx` must be
			// extended to handle it.
			let _ = route;
			return Self::liquidate_gigahdx(debt_asset, user, debt_to_cover);
		}

		if debt_asset == T::HollarId::get() {
			let (flash_minter, loan_receiver) = T::FlashMinter::get().ok_or(Error::<T>::FlashMinterNotSet)?;
			let pallet_address = T::EvmAccounts::evm_address(&Self::account_id());
			let context = CallContext::new_call(flash_minter, pallet_address);
			let hollar_address = T::Erc20Mapping::asset_address(T::HollarId::get());

			let liquidation_data = Self::encode_liquidation_data(collateral_asset, debt_asset, user, &route);

			let data = EvmDataWriter::new_with_selector(Function::FlashLoan)
				.write(loan_receiver)
				.write(hollar_address)
				.write(debt_to_cover)
				.write(Bytes(liquidation_data))
				.build();

			let call_result = Self::evm_call(context, data);

			if call_result.exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
				log::info!(target: "liquidation", "Flash loan Hollar EVM execution failed - {:?}. Reason: {:?}", call_result.exit_reason, call_result.value);
				return Err(T::EvmErrorDecoder::convert(call_result));
			}
		} else {
			let pallet_acc = Self::account_id();
			<T as Config>::Currency::mint_into(debt_asset, &pallet_acc, debt_to_cover)?;
			let pallet_address = T::EvmAccounts::evm_address(&pallet_acc);

			Self::liquidate_position_internal(
				pallet_address,
				collateral_asset,
				debt_asset,
				debt_to_cover,
				user,
				route.clone(),
			)?;

			let _ = <T as Config>::Currency::burn_from(
				debt_asset,
				&pallet_acc,
				debt_to_cover,
				Preservation::Expendable,
				Precision::Exact,
				Fortitude::Force,
			)?;
		}

		Ok(())
	}

	/// Protocol-funded liquidation of a GIGAHDX-collateral position.
	/// See `liquidate` for the dispatch wiring.
	#[frame_support::transactional]
	fn liquidate_gigahdx(debt_asset: AssetId, user: EvmAddress, debt_to_cover: Balance) -> DispatchResult
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		ensure!(debt_asset == T::HollarId::get(), Error::<T>::UnsupportedDebtAsset);

		// The adapter's debt-read views are not metered here, so their budget is never refunded.
		evm_gas_used::with(|total| *total = total.saturating_add(T::GasLimit::get()));

		let borrower = T::EvmAccounts::account_id(user);

		// Fold accrued yield into the borrower's locked stake first, so the
		// snapshot reflects the position's true HDX value: a drained-stake
		// borrower (`Stakes.hdx == 0`, `gigahdx > 0`) would otherwise snapshot
		// `orig_hdx = 0` and the pro-rata `seize_hdx` below would round to zero.
		//
		// Best-effort: a gigapot shortfall (`GigapotInsufficient`) must NOT block
		// the liquidation. Realizing yield is only an optimisation for the seize;
		// on failure we proceed with the un-incremented snapshot (a smaller
		// `seize_hdx`) rather than reverting and leaving the underwater position
		// and its bad debt in place. Liquidation outranks the yield fold.
		if let Err(e) = T::GigaHdx::realize_yield(&borrower) {
			log::warn!(target: "liquidation", "gigahdx: realize_yield failed, proceeding without it: {e:?}");
		}

		let (orig_hdx, orig_gigahdx) =
			T::GigaHdx::snapshot_stake(&borrower).map_err(|_| Error::<T>::NoGigaHdxPosition)?;
		ensure!(orig_gigahdx > 0, Error::<T>::NoGigaHdxPosition);

		// Zero `Stakes[borrower].gigahdx` so the lock-manager precompile
		// lets Aave's internal aToken transfer through. Conviction votes
		// stay intact; `pyconvot` is trimmed surgically below.
		T::GigaHdx::on_pre_seize(&borrower).map_err(|_| Error::<T>::SeizeFailed)?;

		let pool = T::GigaHdx::pool_contract().ok_or(Error::<T>::GigaHdxPoolNotSet)?;
		// One account runs the whole protocol-funded liquidation: the
		// liquidation account borrows the HOLLAR from the *main* money market
		// (against collateral the treasury keeps it topped up with), runs
		// `liquidationCall` on the GIGAHDX pool, and — via `receiveAToken=true`
		// — directly receives the seized aToken. It ends holding the seized
		// GIGAHDX plus the HOLLAR debt; no intermediate treasury hop.
		let borrowing_pool = Self::borrowing_contract();
		let liq_account = T::GigaHdx::liquidation_account();
		let liq_evm = T::EvmAccounts::evm_address(&liq_account);
		// The seized aToken (an EVM-bridged ERC20) is credited to whatever
		// account `liq_evm` maps back to. It must be `liq_account` itself so
		// the balance matches the `on_seize` ledger update — only guaranteed
		// when the liquidation account is bound to its EVM address.
		ensure!(
			T::EvmAccounts::account_id(liq_evm) == liq_account,
			Error::<T>::LiquidationAccountNotBound
		);
		let gigahdx_asset = T::GigaHdx::gigahdx_asset_id();
		let st_hdx = T::GigaHdx::sthdx_asset_id();

		// Clamp the protocol-funded borrow to the borrower's actual debt on
		// the GIGAHDX pool. Without this an attacker-chosen `debt_to_cover`
		// borrows unbounded HOLLAR onto the liquidation account while Aave's
		// `liquidationCall` only ever consumes the close-factor slice.
		let hollar_address = T::Erc20Mapping::asset_address(debt_asset);
		let borrower_debt = T::GigaHdx::borrower_pool_debt(&borrower, debt_asset)?;
		let capped = debt_to_cover.min(borrower_debt);
		ensure!(capped > 0, Error::<T>::NoPoolDebt);

		// Liquidation account borrows the clamped HOLLAR from the main money market.
		let borrow_data = Self::encode_borrow_call_data(hollar_address, capped, liq_evm);
		let borrow_ctx = CallContext::new_call(borrowing_pool, liq_evm);
		let borrow_result = Self::evm_call(borrow_ctx, borrow_data);
		if borrow_result.exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::info!(target: "liquidation", "gigahdx: liquidation-account HOLLAR borrow reverted: {:?}", borrow_result.value);
			return Err(Error::<T>::BorrowFailed.into());
		}

		// Aave liquidationCall — collateral asset is the *underlying* (stHDX),
		// not the aToken. `receiveAToken=true` delivers the seized position
		// straight to `liq_account`, where it keeps earning yield.
		let gigahdx_balance_before = <T as Config>::Currency::balance(gigahdx_asset, &liq_account);
		// HOLLAR held after the borrow; the drop across `liquidationCall` is
		// exactly what Aave consumed (close-factor bounded).
		let hollar_before = <T as Config>::Currency::balance(debt_asset, &liq_account);
		let liq_data = Self::encode_liquidation_call_data(st_hdx, debt_asset, user, capped, true);
		let liq_ctx = CallContext::new_call(pool, liq_evm);
		let liq_result = Self::evm_call(liq_ctx, liq_data);
		if liq_result.exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::info!(target: "liquidation", "gigahdx: liquidationCall reverted: {:?}", liq_result.value);
			return Err(T::EvmErrorDecoder::convert(liq_result));
		}
		let gigahdx_balance_after = <T as Config>::Currency::balance(gigahdx_asset, &liq_account);
		let actual_seized_atoken = gigahdx_balance_after
			.checked_sub(gigahdx_balance_before)
			.ok_or(Error::<T>::LiquidationCallFailed)?;
		ensure!(actual_seized_atoken > 0, Error::<T>::LiquidationCallFailed);
		let hollar_after = <T as Config>::Currency::balance(debt_asset, &liq_account);
		let consumed = hollar_before
			.checked_sub(hollar_after)
			.ok_or(Error::<T>::LiquidationCallFailed)?;
		ensure!(consumed > 0, Error::<T>::LiquidationCallFailed);

		// Pro-rata HDX matching the seized aToken portion. Rounding-down: the
		// protocol takes the floor; residue stays with the borrower.
		let seize_hdx = sp_runtime::helpers_128bit::multiply_by_rational_with_rounding(
			orig_hdx,
			actual_seized_atoken,
			orig_gigahdx,
			sp_runtime::Rounding::Down,
		)
		.ok_or(Error::<T>::SeizeFailed)?;

		// Remove conviction votes no longer backed by the borrower's residual
		// stake so the protocol doesn't carry unbacked governance weight; the
		// `remove_vote` hook also drops the matching `UserVoteRecord`, shrinking
		// the borrower's lazily-derived unstake commitment.
		let residual_hdx = orig_hdx.saturating_sub(seize_hdx);
		T::GigaHdx::clear_conflicting_votes(&borrower, residual_hdx).map_err(|_| Error::<T>::ClearVotingLocksFailed)?;

		// `receiveAToken=true` already delivered the seized aToken to
		// `liq_account`; `on_seize` just reconciles the gigahdx ledger
		// (borrower → liq_account) and refreshes locks.
		T::GigaHdx::on_seize(&borrower, &liq_account, seize_hdx, actual_seized_atoken, orig_gigahdx)
			.map_err(|_| Error::<T>::SeizeFailed)?;

		// Repay the HOLLAR the liquidation account borrowed but Aave did not
		// consume, so the protocol carries debt only for what actually
		// cleared the borrower (and is matched by the seized aToken).
		let surplus = capped.saturating_sub(consumed);
		if surplus > 0 {
			let repay_data = Self::encode_repay_call_data(hollar_address, surplus, liq_evm);
			let repay_ctx = CallContext::new_call(borrowing_pool, liq_evm);
			let repay_result = Self::evm_call(repay_ctx, repay_data);
			if repay_result.exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
				log::info!(target: "liquidation", "gigahdx: surplus HOLLAR repay reverted: {:?}", repay_result.value);
				return Err(Error::<T>::RepayFailed.into());
			}
		}

		Self::deposit_event(Event::GigaHdxLiquidated {
			user,
			debt_repaid: consumed,
			hdx_seized: seize_hdx,
			gigahdx_seized: actual_seized_atoken,
		});
		Ok(())
	}

	/// Encode an AAVE Pool `borrow`/`repay` call. Both share the leading
	/// `(asset, amount, interestRateMode=2, ...)` words; `borrow` carries an extra
	/// `referralCode=0` word before `onBehalfOf`. `interestRateMode = 2` is variable-rate.
	fn encode_pool_debt_call(
		selector: Function,
		asset: EvmAddress,
		amount: Balance,
		on_behalf_of: EvmAddress,
		with_referral: bool,
	) -> Vec<u8> {
		let mut data = Into::<u32>::into(selector).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(asset).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(amount)).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(2u8)).as_bytes()); // variable rate
		if with_referral {
			data.extend_from_slice(H256::from_uint(&U256::from(0u8)).as_bytes()); // referral
		}
		data.extend_from_slice(H256::from(on_behalf_of).as_bytes());
		data
	}

	/// Encode an AAVE Pool `borrow(asset, amount, interestRateMode=2, referralCode=0, onBehalfOf)` call.
	pub fn encode_borrow_call_data(asset: EvmAddress, amount: Balance, on_behalf_of: EvmAddress) -> Vec<u8> {
		Self::encode_pool_debt_call(Function::Borrow, asset, amount, on_behalf_of, true)
	}

	/// Encode an AAVE Pool `repay(asset, amount, interestRateMode=2, onBehalfOf)` call.
	pub fn encode_repay_call_data(asset: EvmAddress, amount: Balance, on_behalf_of: EvmAddress) -> Vec<u8> {
		Self::encode_pool_debt_call(Function::Repay, asset, amount, on_behalf_of, false)
	}

	pub fn encode_liquidation_call_data(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		debt_to_cover: Balance,
		receive_atoken: bool,
	) -> Vec<u8> {
		let collateral_address = T::Erc20Mapping::asset_address(collateral_asset);
		let debt_asset_address = T::Erc20Mapping::asset_address(debt_asset);
		let mut data = Into::<u32>::into(Function::LiquidationCall).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(collateral_address).as_bytes());
		data.extend_from_slice(H256::from(debt_asset_address).as_bytes());
		data.extend_from_slice(H256::from(user).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(debt_to_cover)).as_bytes());
		let mut buffer = [0u8; 32];
		if receive_atoken {
			buffer[31] = 1;
		}
		data.extend_from_slice(&buffer);

		data
	}

	fn liquidate_position_internal(
		liquidator: EvmAddress,
		collateral_asset: AssetId,
		debt_asset: AssetId,
		debt_to_cover: Balance,
		user: EvmAddress,
		route: Route<AssetId>,
	) -> DispatchResult {
		let liquidator_account = T::EvmAccounts::account_id(liquidator);
		let debt_original_balance =
			<T as Config>::Currency::balance(debt_asset, &liquidator_account).saturating_sub(debt_to_cover);
		let collateral_original_balance = <T as Config>::Currency::balance(collateral_asset, &liquidator_account);
		let contract = Self::borrowing_contract();
		let context = CallContext::new_call(contract, liquidator);
		let data = Self::encode_liquidation_call_data(collateral_asset, debt_asset, user, debt_to_cover, false);

		let call_result = Self::evm_call(context, data);
		if call_result.exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::info!(target: "liquidation",
						"Evm execution failed. Reason: {:?}", call_result.value);
			return Err(T::EvmErrorDecoder::convert(call_result));
		}

		// swap collateral if necessary
		if collateral_asset != debt_asset {
			let collateral_earned = <T as Config>::Currency::balance(collateral_asset, &liquidator_account)
				.checked_sub(collateral_original_balance)
				.defensive_ok_or(ArithmeticError::Underflow)?;

			log::trace!(target: "liquidation",
				"Collateral earned: {collateral_earned:?} for asset: {collateral_asset:?}");

			Self::sell_collateral(
				liquidator_account.clone(),
				collateral_asset,
				debt_asset,
				collateral_earned,
				route,
			)?;
		}

		// burn debt and transfer profit
		let debt_gained = <T as Config>::Currency::balance(debt_asset, &liquidator_account)
			.checked_sub(debt_original_balance)
			.ok_or(Error::<T>::NotProfitable)?;

		let profit = debt_gained
			.checked_sub(debt_to_cover)
			.ok_or(Error::<T>::NotProfitable)?;

		log::trace!(target: "liquidation",
				"Profit: {profit:?} for asset: {debt_asset:?}");

		<T as Config>::Currency::transfer(
			debt_asset,
			&liquidator_account,
			&T::ProfitReceiver::get(),
			profit,
			Preservation::Expendable,
		)?;

		Self::deposit_event(Event::Liquidated {
			user,
			collateral_asset,
			debt_asset,
			profit,
		});

		Ok(())
	}

	/// Omnipool hops of this sale pay base fees only; see `LiquidationCollateralSale` in the runtime.
	#[frame_support::transactional]
	fn sell_collateral(
		liquidator_account: T::AccountId,
		collateral_asset: AssetId,
		debt_asset: AssetId,
		amount: Balance,
		route: Route<AssetId>,
	) -> DispatchResult {
		let id = pallet_broadcast::Pallet::<T>::add_to_context(ExecutionType::Liquidation)?;

		T::Router::sell(
			RawOrigin::Signed(liquidator_account).into(),
			collateral_asset,
			debt_asset,
			amount,
			1,
			route,
		)?;

		// A stray `Liquidation` entry would exempt later router trades in the block.
		ensure!(
			pallet_broadcast::Pallet::<T>::get_context().last() == Some(&ExecutionType::Liquidation(id)),
			Error::<T>::UnbalancedExecutionContext
		);
		pallet_broadcast::Pallet::<T>::remove_from_context()
	}

	/// Liquidates an existing money market position.
	pub fn liquidate_position(liquidator: EvmAddress, loan_amount: Balance, data: &[u8]) -> DispatchResult {
		let (collateral_asset_id, debt_asset_id, user, route) = Self::decode_liquidation_data(data)?;
		log::trace!(target: "liquidation", "collateral_asset_id: {collateral_asset_id}, debt_asset_id: {debt_asset_id}, user: {user:?}, route: {route:?}");
		Self::liquidate_position_internal(liquidator, collateral_asset_id, debt_asset_id, loan_amount, user, route)
	}

	/// Encodes the liquidation data to be used in the EVM call to FlashLoan precompile.
	fn encode_liquidation_data(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		route: &Route<AssetId>,
	) -> Vec<u8> {
		let mut data = EvmDataWriter::new()
			.write(1u8)
			.write(collateral_asset)
			.write(debt_asset)
			.write(user)
			.write(route.len() as u32);

		for r in route.iter() {
			data = data.write(Bytes(r.encode()));
		}

		data.build()
	}

	/// Decodes the liquidation data from the EVM call to FlashLoan precompile.
	fn decode_liquidation_data(data: &[u8]) -> Result<(AssetId, AssetId, EvmAddress, Route<AssetId>), Error<T>> {
		// Expected bytes are:
		// - action (u8) - 1 for liquidation
		// - collateral asset id
		// - debt asset id
		// - user address
		// - route length
		// - route entry ( Trade type )

		let mut reader = EvmDataReader::new(data);
		let action: u8 = reader.read().map_err(|_| Error::<T>::FlashMinterNotSet)?;
		ensure!(action == 1, Error::<T>::InvalidLiquidationData);

		let collateral_asset_id: AssetId = reader.read().map_err(|_| Error::<T>::InvalidLiquidationData)?;
		let debt_asset_id: AssetId = reader.read().map_err(|_| Error::<T>::InvalidLiquidationData)?;
		let user: EvmAddress = reader.read().map_err(|_| Error::<T>::InvalidLiquidationData)?;
		let route_len: u32 = reader.read().map_err(|_| Error::<T>::InvalidLiquidationData)?;

		let mut route = vec![];
		for _ in 0..route_len {
			let entry: Bytes = reader.read().map_err(|_| Error::<T>::InvalidLiquidationData)?;
			let entry = entry.as_bytes().to_vec();
			let s = decode_from_bytes::<Trade<AssetId>>(entry.clone().into())
				.map_err(|_| Error::<T>::InvalidLiquidationData)?;
			route.push(s);
		}

		Ok((collateral_asset_id, debt_asset_id, user, Route::truncate_from(route)))
	}
}

impl<T> Get<EvmAddress> for Pallet<T>
where
	T: Config,
{
	fn get() -> EvmAddress {
		Self::borrowing_contract()
	}
}
