// This file is part of Acala.

// Copyright (C) 2020-2023 Acala Foundation.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use super::super::WeightToGas;
//use super::input::{Input, InputT, Output};
use frame_support::{
	log,
	traits::{Currency, Get},
};
//use module_currencies::WeightInfo;
/*use module_evm::{
	precompiles::Precompile,
	runner::state::{PrecompileFailure, PrecompileOutput, PrecompileResult},
	Context, ExitError, ExitRevert, ExitSucceed,
};*/

//use module_support::Erc20InfoMapping as Erc20InfoMappingT;
use codec::{alloc, Decode, Encode, EncodeLike, MaxEncodedLen};
use frame_support::traits::{IsType, OriginTrait};
use frame_system::pallet_prelude::OriginFor;
use frame_system::Origin;
//use input::Erc20InfoMappingT;
use crate::evm::precompile::handle::{EvmDataWriter, PrecompileHandleExt};
use crate::evm::precompile::substrate::RuntimeHelper;
use crate::evm::precompile::{succeed, Address, Erc20Mapping, EvmAddress, EvmResult, FungibleTokenId, Output};
use crate::evm::ExtendedAddressMapping;
use crate::Currencies;
use crate::NativeAssetId;
use hydradx_traits::RegistryQueryForEvm;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use orml_traits::{MultiCurrency as MultiCurrencyT, MultiCurrency};
use pallet_evm::{
	AddressMapping, Context, ExitError, ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle,
	PrecompileOutput,
};
use primitive_types::H160;
use primitives::{AssetId, Balance};
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::crypto::AccountId32;
use sp_runtime::traits::Dispatchable;
use sp_runtime::{traits::Convert, RuntimeDebug};
use sp_std::{marker::PhantomData, prelude::*};

//TODO: copied from runtime/common/recompile
pub fn target_gas_limit(target_gas: Option<u64>) -> Option<u64> {
	target_gas.map(|x| x.saturating_div(10).saturating_mul(9)) // 90%
}

#[module_evm_utility_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Action {
	Name = "name()",
	Symbol = "symbol()",
	Decimals = "decimals()",
	TotalSupply = "totalSupply()",
	BalanceOf = "balanceOf(address)",
	Allowance = "allowance(address,address)",
	Transfer = "transfer(address,uint256)",
	Approve = "approve(address,uint256)",
	TransferFrom = "transferFrom(address,address,uint256)",
}
pub struct MultiCurrencyPrecompile<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for MultiCurrencyPrecompile<Runtime>
where
	Runtime: Erc20Mapping
		+ frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: core::convert::From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId: core::convert::From<sp_runtime::AccountId32>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
{
	fn execute(handle: &mut impl PrecompileHandle) -> pallet_evm::PrecompileResult {
		let address = handle.code_address();
		if let Some(asset_id) = Runtime::decode_evm_address(address) {
			log::debug!(target: "evm", "multicurrency: currency id: {:?}", asset_id);

			let selector = match handle.read_selector() {
				Ok(selector) => selector,
				Err(e) => return Err(e),
			};

			//TODO: check function modifier

			return match selector {
				Action::Name => Self::name(asset_id, handle),
				Action::Symbol => Self::symbol(asset_id, handle),
				Action::Decimals => Self::decimals(asset_id, handle),
				Action::TotalSupply => Self::total_supply(asset_id, handle),
				Action::BalanceOf => Self::balance_of(asset_id, handle),
				Action::Transfer => Self::transfer(asset_id, handle),
				Action::Allowance => Self::allowance(asset_id, handle),
				Action::Approve => Self::not_supported(asset_id, handle),
				Action::TransferFrom => Self::transfer_from(asset_id, handle),
			};
		}
		Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid currency id".into(),
		})
	}
}

impl<Runtime> MultiCurrencyPrecompile<Runtime>
where
	Runtime: Erc20Mapping
		+ frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<Runtime as pallet_asset_registry::Config>::AssetId: core::convert::From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId: core::convert::From<sp_runtime::AccountId32>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
{
	fn name(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::retrieve_asset_name(asset_id.into()) {
			Ok(name) => {
				log::debug!(target: "evm", "multicurrency: symbol: {:?}", name);

				let encoded = Output::encode_bytes(name.as_slice());

				Ok(succeed(encoded))
			}
			Err(_) => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn symbol(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::retrieve_asset_symbol(asset_id.into()) {
			Ok(symbol) => {
				log::debug!(target: "evm", "multicurrency: name: {:?}", symbol);

				let encoded = Output::encode_bytes(symbol.as_slice());

				Ok(succeed(encoded))
			}
			Err(_) => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn decimals(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::retrieve_asset_decimals(asset_id.into()) {
			Ok(decimals) => {
				log::debug!(target: "evm", "multicurrency: decimals: {:?}", decimals);

				let encoded = Output::encode_uint::<u8>(decimals.into());

				Ok(succeed(encoded))
			}
			Err(_) => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn total_supply(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		let total_issuance = Currencies::total_issuance(asset_id);

		log::debug!(target: "evm", "multicurrency: totalSupply: {:?}", total_issuance);

		let encoded = Output::encode_uint::<u128>(total_issuance.into());

		Ok(succeed(encoded))
	}

	fn balance_of(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;

		let owner: H160 = input.read::<Address>()?.into();
		let who: Runtime::AccountId = ExtendedAddressMapping::into_account_id(owner).into(); //TODO: use pallet?

		let free_balance = Currencies::free_balance(asset_id, &who);

		log::debug!(target: "evm", "multicurrency: balanceOf: {:?}", free_balance);

		let encoded = Output::encode_uint::<u128>(free_balance.into());

		Ok(succeed(encoded))
	}

	fn transfer(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		let to: H160 = input.read::<Address>()?.into();
		let amount = input.read::<Balance>()?;

		let origin = ExtendedAddressMapping::into_account_id(handle.context().caller);
		let to = ExtendedAddressMapping::into_account_id(to);

		log::debug!(target: "evm", "multicurrency: transfer from: {:?}, to: {:?}, amount: {:?}", origin, to, amount);

		<pallet_currencies::Pallet<Runtime> as MultiCurrency<Runtime::AccountId>>::transfer(
			asset_id.into(),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(origin)),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to)),
			amount.into(),
		)
		.map_err(|e| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: Into::<&str>::into(e).as_bytes().to_vec(),
		})?;

		Ok(succeed(EvmDataWriter::new().write(true).build()))
	}

	fn allowance(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(2)?;

		//As approve is not supported yet, we always return 0
		let encoded = Output::encode_uint::<u128>(0);

		Ok(succeed(encoded))
	}

	fn transfer_from(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		//TODO: DOUBLE CHECK WITH SOMEONE IF THIS IS THE CORRECT WAY TO PREVENT MALICIOUS TRANSFER
		let from: H160 = input.read::<Address>()?.into();
		if !handle.context().caller.eq(&from) {
			return Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("not supported".into()).into(),
			});
		}
		let to: H160 = input.read::<Address>()?.into();
		let amount = input.read::<Balance>()?;

		let origin = ExtendedAddressMapping::into_account_id(from);
		let to = ExtendedAddressMapping::into_account_id(to);

		log::debug!(target: "evm", "multicurrency: transferFrom from: {:?}, to: {:?}, amount: {:?}", origin, to, amount);

		<pallet_currencies::Pallet<Runtime> as MultiCurrency<Runtime::AccountId>>::transfer(
			asset_id.into(),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(origin)),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to)),
			amount.into(),
		)
		.map_err(|e| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: Into::<&str>::into(e).as_bytes().to_vec(),
		})?;

		Ok(succeed(EvmDataWriter::new().write(true).build()))
	}

	fn not_supported(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		Err(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("not supported".into()),
		})
	}
}

fn get_account_id<T: frame_system::Config>(address: &EvmAddress) -> T::AccountId
where
	T::AccountId: IsType<AccountId32>,
{
	let mut data: [u8; 32] = [0u8; 32];
	data[0..4].copy_from_slice(b"evm:");
	data[4..24].copy_from_slice(&address[..]);

	AccountId32::from(data).into()
}
