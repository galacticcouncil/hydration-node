//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  /
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/
//     .?P!.?GY7:   .. .    ^?PP^:JP~
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

use crate::evm::erc20_currency::Function;
use crate::evm::precompiles::erc20_mapping::is_asset_address;
use crate::evm::precompiles::revert;
use crate::{
	evm::{
		precompiles::{
			dynamic::DynamicPrecompile,
			erc20_mapping::HydraErc20Mapping,
			handle::{EvmDataWriter, FunctionModifier, PrecompileHandleExt},
			substrate::RuntimeHelper,
			succeed, Address, Output,
		},
		ExtendedAddressMapping,
	},
	Currencies,
};
use codec::EncodeLike;
use evm::executor::stack::IsPrecompileResult;
use frame_support::traits::{IsType, OriginTrait};
use hydradx_traits::evm::{Erc20Encoding, InspectEvmAccounts};
use hydradx_traits::registry::Inspect as InspectRegistry;
use orml_traits::{MultiCurrency as MultiCurrencyT, MultiCurrency};
use pallet_evm::{AddressMapping, ExitRevert, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult};
use primitive_types::H160;
use primitives::{AssetId, Balance};
use sp_runtime::traits::Dispatchable;
use sp_std::marker::PhantomData;
use sp_std::vec;

pub struct MultiCurrencyPrecompile<Runtime>(PhantomData<Runtime>);

impl<Runtime> Precompile for MultiCurrencyPrecompile<Runtime>
where
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config
		+ pallet_evm_accounts::Config,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
{
	fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		let address = handle.code_address();
		if let Some(asset_id) = HydraErc20Mapping::decode_evm_address(address) {
			log::debug!(target: "evm", "multicurrency: currency id: {:?}", asset_id);

			let selector = match handle.read_selector() {
				Ok(selector) => selector,
				Err(e) => return Err(e),
			};

			handle.check_function_modifier(match selector {
				Function::Transfer => FunctionModifier::NonPayable,
				Function::TransferFrom => FunctionModifier::NonPayable,
				_ => FunctionModifier::View,
			})?;

			return match selector {
				Function::Name => Self::name(asset_id, handle),
				Function::Symbol => Self::symbol(asset_id, handle),
				Function::Decimals => Self::decimals(asset_id, handle),
				Function::TotalSupply => Self::total_supply(asset_id, handle),
				Function::BalanceOf => Self::balance_of(asset_id, handle),
				Function::Transfer => Self::transfer(asset_id, handle),
				Function::Allowance => Self::allowance(handle),
				Function::Approve => Self::not_supported(),
				Function::TransferFrom => Self::transfer_from(asset_id, handle),
			};
		}
		Err(PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: "invalid currency id".into(),
		})
	}
}

impl<Runtime> DynamicPrecompile for MultiCurrencyPrecompile<Runtime> {
	fn is_precompile(address: H160, _gas: u64) -> IsPrecompileResult {
		if is_asset_address(address) {
			IsPrecompileResult::Answer {
				is_precompile: true,
				extra_cost: 0,
			}
		} else {
			IsPrecompileResult::Answer {
				is_precompile: false,
				extra_cost: 0,
			}
		}
	}
}

impl<Runtime> MultiCurrencyPrecompile<Runtime>
where
	Runtime: frame_system::Config
		+ pallet_evm::Config
		+ pallet_asset_registry::Config
		+ pallet_currencies::Config
		+ pallet_evm_accounts::Config,
	AssetId: EncodeLike<<Runtime as pallet_asset_registry::Config>::AssetId>,
	<Runtime as pallet_asset_registry::Config>::AssetId: From<AssetId>,
	Currencies: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	pallet_currencies::Pallet<Runtime>: MultiCurrency<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
	<Runtime as frame_system::Config>::AccountId:
		From<sp_runtime::AccountId32> + IsType<sp_runtime::AccountId32> + AsRef<[u8; 32]>,
	<<Runtime as frame_system::Config>::RuntimeCall as Dispatchable>::RuntimeOrigin: OriginTrait,
{
	fn name(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::asset_name(asset_id.into()) {
			Some(name) => {
				log::debug!(target: "evm", "multicurrency: symbol: {:?}", name);

				let encoded = Output::encode_bytes(name.as_slice());

				Ok(succeed(encoded))
			}
			None => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn symbol(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::asset_symbol(asset_id.into()) {
			Some(symbol) => {
				log::debug!(target: "evm", "multicurrency: name: {:?}", symbol);

				let encoded = Output::encode_bytes(symbol.as_slice());

				Ok(succeed(encoded))
			}
			None => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn decimals(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		match <pallet_asset_registry::Pallet<Runtime>>::decimals(asset_id.into()) {
			Some(decimals) => {
				log::debug!(target: "evm", "multicurrency: decimals: {:?}", decimals);

				let encoded = Output::encode_uint::<u8>(decimals);

				Ok(succeed(encoded))
			}
			None => Err(PrecompileFailure::Error {
				exit_status: pallet_evm::ExitError::Other("Non-existing asset.".into()),
			}),
		}
	}

	fn total_supply(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let input = handle.read_input()?;
		input.expect_arguments(0)?;

		let total_issuance = Currencies::total_issuance(asset_id);

		log::debug!(target: "evm", "multicurrency: totalSupply: {:?}", total_issuance);

		let encoded = Output::encode_uint::<u128>(total_issuance);

		Ok(succeed(encoded))
	}

	fn balance_of(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(1)?;

		let owner: H160 = input.read::<Address>()?.into();
		let who: Runtime::AccountId = ExtendedAddressMapping::into_account_id(owner).into(); //TODO: use pallet?

		let free_balance = Currencies::free_balance(asset_id, &who);

		log::debug!(target: "evm", "multicurrency: balanceOf: {:?}", free_balance);

		let encoded = Output::encode_uint::<u128>(free_balance);

		Ok(succeed(encoded))
	}

	fn transfer(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
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
			asset_id,
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(origin)),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to)),
			amount,
		)
		.map_err(|e| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: Into::<&str>::into(e).as_bytes().to_vec(),
		})?;

		Ok(succeed(EvmDataWriter::new().write(true).build()))
	}

	fn allowance(handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		let _owner: H160 = input.read::<Address>()?.into();
		let spender: H160 = input.read::<Address>()?.into();

		let allowance =
			if <pallet_evm_accounts::Pallet<Runtime> as InspectEvmAccounts<Runtime::AccountId>>::is_approved_contract(
				spender,
			) {
				u128::MAX
			} else {
				0
			};

		let encoded = Output::encode_uint::<u128>(allowance);
		Ok(succeed(encoded))
	}

	fn transfer_from(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(3)?;

		let origin: H160 = handle.context().caller;
		let from: H160 = input.read::<Address>()?.into();
		let to: H160 = input.read::<Address>()?.into();
		let amount = input.read::<Balance>()?;

		let from = ExtendedAddressMapping::into_account_id(from);
		let to = ExtendedAddressMapping::into_account_id(to);

		log::debug!(target: "evm", "multicurrency: transferFrom from: {:?}, to: {:?}, amount: {:?}", from, to, amount);

		if <pallet_evm_accounts::Pallet<Runtime> as InspectEvmAccounts<Runtime::AccountId>>::is_approved_contract(
			origin,
		) {
			<pallet_currencies::Pallet<Runtime> as MultiCurrency<Runtime::AccountId>>::transfer(
				asset_id,
				&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(from)),
				&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to)),
				amount,
			)
			.map_err(|e| PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: Into::<&str>::into(e).as_bytes().to_vec(),
			})?;

			Ok(succeed(EvmDataWriter::new().write(true).build()))
		} else {
			Err(revert("Not approved contract"))
		}
	}

	fn not_supported() -> PrecompileResult {
		Err(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("not supported".into()),
		})
	}
}
