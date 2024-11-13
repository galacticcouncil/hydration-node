// This file is part of HydraDX.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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

use frame_support::{
	pallet_prelude::*,
	traits::fungibles::{Inspect, Mutate},
	traits::tokens::{Fortitude, Precision, Preservation},
	PalletId,
	sp_runtime::traits::{AccountIdConversion, CheckedConversion},
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor, RawOrigin};
use hydradx_traits::{
	evm::{CallContext, Erc20Mapping, EvmAddress, InspectEvmAccounts, EVM},
	router::{AmmTradeWeights, AmountInAndOut, AssetPair, RouteProvider, RouterT, Trade},
};
use sp_arithmetic::ArithmeticError;
use sp_core::{
	H256, U256,
	crypto::AccountId32,
};
use sp_std::{vec, vec::Vec};
use ethabi::ethereum_types::BigEndianHash;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use evm::{ExitReason, ExitSucceed};
use pallet_evm::GasWeightMapping;

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

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Function {
	LiquidationCall = "liquidationCall(address,address,address,uint256,bool)",
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Multi currency.
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// EVM handler.
		type Evm: EVM<CallResult>;

		/// Router implementation.
		type Router: RouteProvider<AssetId>
			+ RouterT<Self::RuntimeOrigin, AssetId, Balance, Trade<AssetId>, AmountInAndOut<Balance>>;

		/// Money market contract address.
		#[pallet::constant]
		type MoneyMarketContract: Get<EvmAddress>;

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

	#[pallet::event]
	#[pallet::generate_deposit(pub(crate) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Money market position has been liquidated
		Liquidated {
			liquidator: T::AccountId,
			evm_address: EvmAddress,
			debt_asset: AssetId,
			collateral_asset: AssetId,
			debt_to_cover: Balance,
			profit: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// AssetId to EVM address conversion failed
		AssetConversionFailed,
		/// EVM call failed
		EvmExecutionFailed,
		/// Provided route doesn't match the existing route
		InvalidRoute,
		/// Liquidation was not profitable enough to repay flash loan
		NotProfitable
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
			.saturating_add(<T as Config>::RouterWeightInfo::get_route_weight())
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
			let pallet_acc = Self::account_id();

			// `route` needs to be provided in order to calculate correct TX fee.
			// Ensure that the provided route matches the route we get from the router.
			ensure!(
				route
					== T::Router::get_route(AssetPair {
						asset_in: collateral_asset,
						asset_out: debt_asset,
					}),
				Error::<T>::InvalidRoute
			);

			let debt_asset_initial_balance = <T as Config>::Currency::balance(debt_asset, &pallet_acc);
			let collateral_asset_initial_balance = <T as Config>::Currency::balance(collateral_asset, &pallet_acc);

			// mint borrow asset
			<T as Config>::Currency::mint_into(debt_asset, &pallet_acc, debt_to_cover)?;

			// liquidation
			let pallet_evm_account = T::EvmAccounts::evm_address(&pallet_acc);
			let mm_contract_address = T::MoneyMarketContract::get();

			let context = CallContext::new_call(mm_contract_address, pallet_evm_account);
			let collateral_asset_evm_address =
				T::Erc20Mapping::encode_evm_address(collateral_asset).ok_or(Error::<T>::AssetConversionFailed)?;
			let debt_asset_evm_address =
				T::Erc20Mapping::encode_evm_address(debt_asset).ok_or(Error::<T>::AssetConversionFailed)?;
			let data = Self::encode_liquidation_call_data(
				collateral_asset_evm_address,
				debt_asset_evm_address,
				user,
				debt_to_cover,
				false,
			);

			// EVM call
			let value = U256::zero();
			let gas = T::GasLimit::get();
			let (exit_reason, value) = T::Evm::call(context, data, value, gas);
			if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
				log::debug!(target: "liquidation",
					"Evm execution failed. Reason: {:?}", value);
				return Err(Error::<T>::EvmExecutionFailed.into());
			}

			// swap collateral asset for borrow asset
			let collateral_earned = <T as Config>::Currency::balance(collateral_asset, &pallet_acc)
				.checked_sub(collateral_asset_initial_balance)
				.ok_or(ArithmeticError::Overflow)?;
			T::Router::sell(
				RawOrigin::Signed(pallet_acc.clone()).into(),
				collateral_asset,
				debt_asset,
				collateral_earned,
				1,
				route.clone(),
			)?;

			//burn
			let debt_asset_final_balance = <T as Config>::Currency::balance(debt_asset, &pallet_acc);
			let debt_asset_earned = debt_asset_final_balance
				.checked_sub(debt_asset_initial_balance)
				.ok_or(ArithmeticError::Overflow)?;

			// ensure that we got back at least the amount we minted
			let transferable_amount = debt_asset_earned
				.checked_sub(debt_to_cover)
				.ok_or(Error::<T>::NotProfitable)?;

			<T as Config>::Currency::burn_from(
				debt_asset,
				&pallet_acc,
				debt_to_cover,
				Precision::Exact,
				Fortitude::Force,
			)?;

			// transfer frofit to `FeeReceiver`
			<T as Config>::Currency::transfer(
				debt_asset,
				&pallet_acc,
				&T::ProfitReceiver::get(),
				transferable_amount,
				Preservation::Expendable,
			)?;

			Self::deposit_event(Event::Liquidated {
				liquidator: who,
				evm_address: user,
				debt_asset,
				collateral_asset,
				debt_to_cover,
				profit: transferable_amount,
			});

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		PalletId(*b"lqdation").into_account_truncating()
	}

	pub fn encode_liquidation_call_data(
		collateral_asset: EvmAddress,
		debt_asset: EvmAddress,
		user: EvmAddress,
		debt_to_cover: Balance,
		receive_atoken: bool,
	) -> Vec<u8> {
		let mut data = Into::<u32>::into(Function::LiquidationCall).to_be_bytes().to_vec();
		data.extend_from_slice(H256::from(collateral_asset).as_bytes());
		data.extend_from_slice(H256::from(debt_asset).as_bytes());
		data.extend_from_slice(H256::from(user).as_bytes());
		data.extend_from_slice(H256::from_uint(&U256::from(debt_to_cover)).as_bytes());
		let mut buffer = [0u8; 32];
		if receive_atoken {
			buffer[31] = 1;
		}
		data.extend_from_slice(&buffer);

		data
	}

	#[allow(dead_code)]
	fn decode_liquidation_call_data(data: Vec<u8>) -> Option<(EvmAddress, EvmAddress, EvmAddress, Balance, bool)> {
		if data.len() != 164 {
			return None;
		}
		let data = data.clone();

		let function_u32: u32 = u32::from_be_bytes(data[0..4].try_into().ok()?);
		let function = Function::try_from(function_u32).ok()?;
		if function == Function::LiquidationCall {
			let collateral_asset = EvmAddress::from(H256::from_slice(&data[4..36]));
			let debt_asset = EvmAddress::from(H256::from_slice(&data[36..68]));
			let user = EvmAddress::from(H256::from_slice(&data[68..100]));
			let debt_to_cover = Balance::try_from(U256::checked_from(&data[100..132])?).ok()?;
			let receive_atoken = !H256::from_slice(&data[132..164]).is_zero();

			Some((collateral_asset, debt_asset, user, debt_to_cover, receive_atoken))
		} else {
			None
		}
	}
}
