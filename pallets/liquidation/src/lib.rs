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

use codec::decode_from_bytes;
use ethabi::ethereum_types::BigEndianHash;
use evm::{ExitReason, ExitSucceed};
use frame_support::traits::DefensiveOption;
use frame_support::{
	pallet_prelude::*,
	sp_runtime::traits::AccountIdConversion,
	traits::fungibles::{Inspect, Mutate},
	traits::tokens::{Fortitude, Precision, Preservation},
	PalletId,
};
use frame_system::{pallet_prelude::OriginFor, RawOrigin};
use hydradx_traits::evm::Erc20Mapping;
use hydradx_traits::router::Route;
use hydradx_traits::{
	evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM},
	router::{AmmTradeWeights, AmountInAndOut, RouteProvider, RouterT, Trade},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;
use hydration_precompile_utils::evm::writer::{EvmDataReader, EvmDataWriter};
use hydration_precompile_utils::evm::Bytes;
use sp_arithmetic::ArithmeticError;
use sp_core::{crypto::AccountId32, H256, U256};
use sp_std::{vec, vec::Vec};

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
	FlashLoan = "flashLoan(address,address,uint256,bytes)",
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use hydradx_traits::evm::Erc20Mapping;

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
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
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
				ValidTransaction::with_tag_prefix("liquidate_unsigned_call")
					.priority(1_000_000)
					.and_provides([&provide])
					.longevity(3)
					.propagate(false)
					.build()
			};

			match call {
				Call::liquidate { .. } => valid_tx(b"liquidate_unsigned".to_vec()),
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
			debt_to_cover: Balance,
			profit: Balance,
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
			_origin: OriginFor<T>,
			collateral_asset: AssetId,
			debt_asset: AssetId,
			user: EvmAddress,
			debt_to_cover: Balance,
			route: Route<AssetId>,
		) -> DispatchResult {
			log::trace!(target: "liquidation","liquidating debt asset: {:?} for amount: {:?}", debt_asset, debt_to_cover);

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

				let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());

				if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
					log::error!(target: "liquidation", "Flash loan Hollar EVM execution failed - {:?}. Reason: {:?}", exit_reason, value);
					return Err(Error::<T>::LiquidationCallFailed.into());
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

		/// Set the borrowing market contract address.
		#[pallet::call_index(1)]
		#[pallet::weight(<T as Config>::WeightInfo::set_borrowing_contract())]
		pub fn set_borrowing_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResult {
			frame_system::ensure_root(origin)?;

			BorrowingContract::<T>::put(contract);

			Ok(())
		}
	}
}

impl<T: Config> Pallet<T> {
	pub fn account_id() -> T::AccountId {
		PalletId(*b"lqdation").into_account_truncating()
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
		let contract = BorrowingContract::<T>::get();
		let context = CallContext::new_call(contract, liquidator);
		let data = Self::encode_liquidation_call_data(collateral_asset, debt_asset, user, debt_to_cover, false);

		let (exit_reason, value) = T::Evm::call(context, data, U256::zero(), T::GasLimit::get());
		if exit_reason != ExitReason::Succeed(ExitSucceed::Returned) {
			log::error!(target: "liquidation",
						"Evm execution failed. Reason: {:?}", value);
			return Err(Error::<T>::LiquidationCallFailed.into());
		}

		// swap collateral if necessary
		if collateral_asset != debt_asset {
			let collateral_earned = <T as Config>::Currency::balance(collateral_asset, &liquidator_account)
				.checked_sub(collateral_original_balance)
				.defensive_ok_or(ArithmeticError::Underflow)?;

			log::trace!(target: "liquidation",
				"Collateral earned: {:?} for asset: {:?}", collateral_earned, collateral_asset);

			T::Router::sell(
				RawOrigin::Signed(liquidator_account.clone()).into(),
				collateral_asset,
				debt_asset,
				collateral_earned,
				1,
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
				"Profit: {:?} for asset: {:?}", profit, debt_asset);

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
			debt_to_cover,
			profit,
		});

		Ok(())
	}

	/// Liquidates an existing money market position.
	pub fn liquidate_position(liquidator: EvmAddress, loan_amount: Balance, data: &[u8]) -> DispatchResult {
		let (collateral_asset_id, debt_asset_id, user, route) = Self::decode_liquidation_data(data)?;
		log::trace!(target: "liquidation", "collateral_asset_id: {}, debt_asset_id: {}, user: {:?}, route: {:?}", collateral_asset_id, debt_asset_id, user, route);
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
