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

use ethabi::ethereum_types::BigEndianHash;
use evm::{ExitReason, ExitSucceed};
use frame_support::sp_runtime::offchain::http;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::AccountIdConversion,
	traits::fungibles::{Inspect, Mutate},
	traits::tokens::{Fortitude, Precision, Preservation},
	PalletId,
};
use frame_support::traits::DefensiveOption;
use frame_system::{
	ensure_signed,
	pallet_prelude::{BlockNumberFor, OriginFor},
	RawOrigin,
	offchain::{SendTransactionTypes, SubmitTransaction},
};
use hydradx_traits::{
	evm::{CallContext, Erc20Mapping, EvmAddress, InspectEvmAccounts, EVM},
	router::{AssetPair, AmmTradeWeights, AmountInAndOut, RouteProvider, RouterT, Trade},
	registry::Inspect as AssetRegistryInspect,
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;
use serde::Deserialize;
use sp_arithmetic::ArithmeticError;
use sp_core::{crypto::AccountId32, offchain::Duration, H160, H256, U256};
use sp_std::{vec, vec::Vec, cmp::Ordering, boxed::Box};

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

pub type Balance = u128;
pub type AssetId = u32;
pub type CallResult = (ExitReason, Vec<u8>);

pub const MAX_LIQUIDATIONS: u32 = 5;
pub const UNSIGNED_TXS_PRIORITY: u64 = 1_000_000;

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
}

#[derive(Clone, Encode, Decode, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BorrowerDataDetails<AccountId> {
	pub total_collateral_base: f32,
	pub total_debt_base: f32,
	pub available_borrows_base: f32,
	pub current_liquidation_threshold: f32,
	pub ltv: f32,
	pub health_factor: f32,
	pub updated: u64,
	pub account: AccountId,
	pub pool: H160,
}

#[derive(Clone, Encode, Decode, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BorrowerData<AccountId> {
	pub last_global_update: u32,
	pub last_update: u32,
	pub borrowers: Vec<(H160, BorrowerDataDetails<AccountId>)>,
}

#[frame_support::pallet]
pub mod pallet {
	use frame_support::sp_runtime::offchain::storage::StorageValueRef;
	use super::*;
	use frame_support::traits::DefensiveOption;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + SendTransactionTypes<Call<Self>> where <Self as frame_system::Config>::AccountId: AsRef<[u8; 32]> , <Self as frame_system::Config>::AccountId: frame_support::traits::IsType<frame_support::sp_runtime::AccountId32>{
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

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
	}

	#[pallet::type_value]
	pub fn DefaultBorrowingContract() -> EvmAddress {
		EvmAddress::from_slice(hex_literal::hex!("1b02E051683b5cfaC5929C25E84adb26ECf87B38").as_slice())
	}

	/// Borrowing market contract address
	#[pallet::storage]
	pub type BorrowingContract<T: Config> = StorageValue<_, EvmAddress, ValueQuery, DefaultBorrowingContract>;

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
		where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>, {
		type Call = Call<T>;

		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
					// receiving unsigned transaction from network - disallow
					return InvalidTransaction::Call.into();
				}
				TransactionSource::Local => {}   // produced by off-chain worker
				TransactionSource::InBlock => {} // some other node included it in a block
			};

			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("settle-otc-with-router")
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides([&provide])
					.longevity(3)
					.propagate(false)
					.build()
			};

			match call {
				Call::liquidate_unsigned { .. } => valid_tx(b"settle_otc_order".to_vec()),
				Call::dummy_send { .. } => valid_tx(b"settle_otc_order".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> where <T as frame_system::Config>::AccountId: AsRef<[u8; 32]> , <T as frame_system::Config>::AccountId: frame_support::traits::IsType<frame_support::sp_runtime::AccountId32>{
		/// Money market position has been liquidated
		Liquidated {
			liquidator: T::AccountId,
			evm_address: EvmAddress,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			debt_to_cover: Balance,
			profit: Balance,
		},
		DummyReceived,
		DummySend,
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
	}

	#[pallet::call]
	impl<T: Config> Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		/// Liquidates an existing money market position.
		///
		/// Performs a flash loan to get funds to pay for the debt.
		/// Received collateral is swapped and the profit is transferred to `FeeReceiver`.
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
		#[pallet::weight(<T as Config>::WeightInfo::liquidate()
			.saturating_add(<T as Config>::RouterWeightInfo::sell_weight(route))
			.saturating_add(<T as Config>::GasWeightMapping::gas_to_weight(<T as Config>::GasLimit::get(), true))
		)]
		pub fn liquidate(
			origin: OriginFor<T>,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Vec<Trade<AssetId>>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::liquidate_inner(Some(who), collateral_asset, debt_asset, user, debt_to_cover, route)
		}

		/// Set the borrowing market contract address.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_borrowing_contract())]
		pub fn set_borrowing_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResult {
			frame_system::ensure_root(origin)?;

			BorrowingContract::<T>::put(contract);

			Ok(())
		}

		#[pallet::call_index(2)]
		#[pallet::weight(<T as Config>::WeightInfo::liquidate()
			.saturating_add(<T as Config>::RouterWeightInfo::sell_weight(route))
			.saturating_add(<T as Config>::GasWeightMapping::gas_to_weight(<T as Config>::GasLimit::get(), true))
		)]
		pub fn liquidate_unsigned(
			_origin: OriginFor<T>,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Vec<Trade<AssetId>>,
		) -> DispatchResult {
			Self::liquidate_inner(None, collateral_asset, debt_asset, user, debt_to_cover, route)
		}

		#[pallet::call_index(3)]
		#[pallet::weight(Weight::zero()
		)]
		pub fn dummy_received(
			origin: OriginFor<T>,
			debt_to_cover: crate::Balance,
		) -> DispatchResult {
			Self::deposit_event(Event::DummyReceived);

			Ok(())
		}

		#[pallet::call_index(4)]
		#[pallet::weight(Weight::zero()
		)]
		pub fn dummy_send(
			origin: OriginFor<T>,
			debt_to_cover: crate::Balance,
		) -> DispatchResult {
			Self::deposit_event(Event::DummySend);
			Ok(())
		}
	}
}

impl<T: Config> Pallet<T>
	where
	T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>, {
	pub fn account_id() -> T::AccountId {
		PalletId(*b"lqdation").into_account_truncating()
	}

	pub fn liquidate_inner(
		maybe_signed_by: Option<T::AccountId>,
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		debt_to_cover: Balance,
		route: Vec<Trade<AssetId>>,
	) -> DispatchResult {
		let pallet_acc = Self::account_id();

		let debt_original_balance = <T as Config>::Currency::balance(debt_asset, &pallet_acc);
		let collateral_original_balance = <T as Config>::Currency::balance(collateral_asset, &pallet_acc);

		// mint debt asset
		<T as Config>::Currency::mint_into(debt_asset, &pallet_acc, debt_to_cover)?;

		// liquidation call
		let pallet_address = T::EvmAccounts::evm_address(&pallet_acc);
		let contract = BorrowingContract::<T>::get();

		let context = CallContext::new_call(contract, pallet_address);
		let data = Self::encode_liquidation_call_data(collateral_asset, debt_asset, user, debt_to_cover, false);

		let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: "liquidation",
					"Evm execution failed. Reason: {:?}", value);
			return Err(Error::<T>::LiquidationCallFailed.into());
		}

		// swap collateral if necessary
		if collateral_asset != debt_asset {
			let collateral_earned = <T as Config>::Currency::balance(collateral_asset, &pallet_acc)
				.checked_sub(collateral_original_balance)
				.defensive_ok_or(ArithmeticError::Underflow)?;
			T::Router::sell(
				RawOrigin::Signed(pallet_acc.clone()).into(),
				collateral_asset,
				debt_asset,
				collateral_earned,
				1,
				route,
			)?;
		}

		// burn debt and transfer profit
		let debt_gained = <T as Config>::Currency::balance(debt_asset, &pallet_acc)
			.checked_sub(debt_original_balance)
			.ok_or(Error::<T>::NotProfitable)?;

		let profit = debt_gained
			.checked_sub(debt_to_cover)
			.ok_or(Error::<T>::NotProfitable)?;

		<T as Config>::Currency::burn_from(
			debt_asset,
			&pallet_acc,
			debt_to_cover,
			Preservation::Expendable,
			Precision::Exact,
			Fortitude::Force,
		)?;

		<T as Config>::Currency::transfer(
			debt_asset,
			&pallet_acc,
			&T::ProfitReceiver::get(),
			profit,
			Preservation::Expendable,
		)?;

		Self::deposit_event(Event::Liquidated {
			liquidator: maybe_signed_by.unwrap_or(pallet_acc),
			evm_address: user,
			collateral_asset,
			debt_asset,
			debt_to_cover,
			profit,
		});

		Ok(())
	}

	pub fn encode_liquidation_call_data(
		collateral_asset: AssetId,
		debt_asset: AssetId,
		user: EvmAddress,
		debt_to_cover: Balance,
		receive_atoken: bool,
	) -> Vec<u8> {
		let collateral_address = T::Erc20Mapping::encode_evm_address(collateral_asset);
		let debt_asset_address = T::Erc20Mapping::encode_evm_address(debt_asset);
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

	pub fn process_borrowers_data(oracle_data: BorrowerData<T::AccountId>) -> Vec<(H160, BorrowerDataDetails<T::AccountId>)> {
		let mut borrowers = oracle_data.borrowers.clone();
		// remove elements with HF == 1
		borrowers.retain(|b| b.1.health_factor > 0.0 && b.1.health_factor < 1.0);
		borrowers.sort_by(|a, b| a.1.health_factor.partial_cmp(&b.1.health_factor).unwrap_or(Ordering::Equal));
		borrowers.truncate(borrowers.len().min(MAX_LIQUIDATIONS as usize));
		borrowers
	}

	/// Parse the borrower data from the given JSON string .
	///
	/// Returns `None` when parsing failed or `Some(BorrowerData)` when parsing is successful.
	fn parse_borrowers_data(oracle_data_str: &str) -> Option<BorrowerData<T::AccountId>> {
		serde_json::from_str(oracle_data_str).ok()
	}

	pub fn parse_oracle_transaction(eth_tx: pallet_ethereum::Transaction) -> Option<Vec<(AssetPair<AssetId>, U256)>> {
		let legacy_transaction = match eth_tx {
			pallet_ethereum::Transaction::Legacy(legacy_transaction) => legacy_transaction,
			_ => return None,
		};

		let decoded = ethabi::decode(
			&[
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::String)),
				ethabi::ParamType::Array(Box::new(ethabi::ParamType::Uint(32))),
			],
			&legacy_transaction.input[4..],// first 4 bytes are function selector
		).ok()?;

		let mut dai_oracle_data = Vec::new();

		if decoded.len() == 2 {
			for (asset_str, price) in sp_std::iter::zip(decoded[0].clone().into_array()?.iter(), decoded[1].clone().into_array()?.iter()) {
				dai_oracle_data.push((asset_str.clone().into_string()?, price.clone().into_uint()?));

			}
		};

		let mut result = Vec::new();
		for i in 0..dai_oracle_data.len() {
			let asset_str: Vec<&str> = dai_oracle_data[i].0.split("/").collect::<Vec<_>>().clone();
			if asset_str.len() != 2 {
				return None;
			}
			let asset_id_a = <T as Config>::AssetRegistry::asset_id(asset_str[0]);
			// remove \0 null-terminator from the string
			let asset_id_b = <T as Config>::AssetRegistry::asset_id(&asset_str[1][0..asset_str[1].len() - 1]);

			if let (Some(asset_id_a), Some(asset_id_b)) = (asset_id_a, asset_id_b) {
				result.push((AssetPair::new(asset_id_a, asset_id_b), dai_oracle_data[i].1.clone()));
			} else {
				return None;
			}
		}

		Some(result)
	}
}


