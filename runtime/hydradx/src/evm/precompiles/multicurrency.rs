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
use crate::evm::precompiles::revert;
use crate::{
	evm::{
		precompiles::{
			erc20_mapping::HydraErc20Mapping,
			handle::{EvmDataWriter, FunctionModifier, PrecompileHandleExt},
			substrate::RuntimeHelper,
			succeed, Address, Output,
		},
		ExtendedAddressMapping,
	},
	Currencies,
};
use codec::{Encode, EncodeLike};
use frame_support::traits::{ExistenceRequirement, IsType, OriginTrait};
use hydradx_traits::evm::{Erc20Encoding, InspectEvmAccounts};
use hydradx_traits::registry::Inspect as InspectRegistry;
use orml_traits::{MultiCurrency as MultiCurrencyT, MultiCurrency};
use pallet_evm::{AddressMapping, ExitRevert, Log, Precompile, PrecompileFailure, PrecompileHandle, PrecompileResult};
use pallet_synthetic_logs::{encode_u256_be, h160_to_h256, TRANSFER_TOPIC};
use primitive_types::{H160, U256};
use primitives::{AssetId, Balance};
use sp_runtime::traits::Dispatchable;
use sp_std::{marker::PhantomData, vec};

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
			log::debug!(target: "evm", "multicurrency: currency id: {asset_id:?}");

			let selector = handle.read_selector()?;

			handle.check_function_modifier(match selector {
				Function::Transfer => FunctionModifier::NonPayable,
				Function::TransferFrom => FunctionModifier::NonPayable,
				Function::Approve => FunctionModifier::NonPayable,
				_ => FunctionModifier::View,
			})?;

			return match selector {
				Function::Name => Self::name(asset_id, handle),
				Function::Symbol => Self::symbol(asset_id, handle),
				Function::Decimals => Self::decimals(asset_id, handle),
				Function::TotalSupply => Self::total_supply(asset_id, handle),
				Function::BalanceOf => Self::balance_of(asset_id, handle),
				Function::Transfer => Self::transfer(asset_id, handle),
				Function::Allowance => Self::allowance(asset_id, handle),
				Function::Approve => Self::approve(asset_id, handle),
				Function::TransferFrom => Self::transfer_from(asset_id, handle),
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
				log::debug!(target: "evm", "multicurrency: symbol: {name:?}");

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
				log::debug!(target: "evm", "multicurrency: name: {symbol:?}");

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
				log::debug!(target: "evm", "multicurrency: decimals: {decimals:?}");

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

		log::debug!(target: "evm", "multicurrency: totalSupply: {total_issuance:?}");

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

		log::debug!(target: "evm", "multicurrency: balanceOf: {free_balance:?}");

		let encoded = Output::encode_uint::<u128>(free_balance);

		Ok(succeed(encoded))
	}

	fn transfer(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		let to_h160: H160 = input.read::<Address>()?.into();
		let from_h160: H160 = handle.context().caller;
		let amount = input.read::<Balance>()?;

		let origin = ExtendedAddressMapping::into_account_id(from_h160);
		let to = ExtendedAddressMapping::into_account_id(to_h160);

		log::debug!(target: "evm", "multicurrency: transfer from: {origin:?}, to: {to:?}, amount: {amount:?}");

		<pallet_currencies::Pallet<Runtime> as MultiCurrency<Runtime::AccountId>>::transfer(
			asset_id,
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(origin)),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to)),
			amount,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|e| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: e.encode(),
		})?;

		// Emit ERC-20 Transfer log inline so eth tooling sees the event in the
		// real eth tx's logs. The substrate-side orml hooks are suppressed for
		// in-evm context (`is_in_evm()` returns true) so we don't double-emit.
		emit_erc20_transfer_log(handle, asset_id, from_h160, to_h160, amount)?;

		Ok(succeed(EvmDataWriter::new().write(true).build()))
	}

	fn allowance(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		// Parse input
		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		let owner: H160 = input.read::<Address>()?.into();
		let spender: H160 = input.read::<Address>()?.into();

		let allowance =
			if <pallet_evm_accounts::Pallet<Runtime> as InspectEvmAccounts<Runtime::AccountId>>::is_approved_contract(
				spender,
			) {
				Balance::MAX
			} else {
				pallet_evm_accounts::Pallet::<Runtime>::get_allowance(asset_id.into(), owner, spender)
			};

		let encoded = Output::encode_uint::<u128>(allowance);
		Ok(succeed(encoded))
	}

	fn approve(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> PrecompileResult {
		handle.record_cost(RuntimeHelper::<Runtime>::db_read_gas_cost())?;

		let mut input = handle.read_input()?;
		input.expect_arguments(2)?;

		let spender: H160 = input.read::<Address>()?.into();
		let amount: Balance = input.read::<Balance>()?;

		let owner: H160 = handle.context().caller;

		pallet_evm_accounts::Pallet::<Runtime>::set_allowance(asset_id.into(), owner, spender, amount);

		Ok(succeed(EvmDataWriter::new().write(true).build()))
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

		let spender_is_approved =
			<pallet_evm_accounts::Pallet<Runtime> as InspectEvmAccounts<Runtime::AccountId>>::is_approved_contract(
				origin,
			);

		if !spender_is_approved {
			let allowed: Balance = pallet_evm_accounts::Pallet::<Runtime>::get_allowance(asset_id.into(), from, origin);

			if allowed < amount {
				return Err(revert("ERC20: insufficient allowance"));
			}

			// Some ERC-20 tokens treat `type(uint256).max` as an “infinite allowance” and do not decrement it
			// on `transferFrom`. We mirror that behavior: if `allowed == Balance::MAX`, we skip updating the
			// stored allowance; otherwise we decrement by `amount`.
			if allowed != Balance::MAX {
				pallet_evm_accounts::Pallet::<Runtime>::set_allowance(
					asset_id.into(),
					from,
					origin,
					allowed.saturating_sub(amount),
				);
			}
		}

		let from_acc = ExtendedAddressMapping::into_account_id(from);
		let to_acc = ExtendedAddressMapping::into_account_id(to);

		log::debug!(target: "evm", "multicurrency: transferFrom from: {from_acc:?}, to: {to_acc:?}, amount: {amount:?}");

		<pallet_currencies::Pallet<Runtime> as MultiCurrency<Runtime::AccountId>>::transfer(
			asset_id,
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(from_acc)),
			&(<sp_runtime::AccountId32 as Into<Runtime::AccountId>>::into(to_acc)),
			amount,
			ExistenceRequirement::AllowDeath,
		)
		.map_err(|e| PrecompileFailure::Revert {
			exit_status: ExitRevert::Reverted,
			output: e.encode(),
		})?;

		emit_erc20_transfer_log(handle, asset_id, from, to, amount)?;

		Ok(succeed(EvmDataWriter::new().write(true).build()))
	}
}

/// Emit an ERC-20 `Transfer(from, to, amount)` log inline on the precompile
/// handle so it lands in the calling eth transaction's logs. Address used is
/// the precompile's `code_address` (the asset's erc20 contract address).
fn emit_erc20_transfer_log(
	handle: &mut impl PrecompileHandle,
	asset_id: AssetId,
	from: H160,
	to: H160,
	amount: Balance,
) -> Result<(), PrecompileFailure> {
	let _ = asset_id; // contract address is `handle.code_address()`; asset_id reserved for future use
	let topics = vec![TRANSFER_TOPIC, h160_to_h256(from), h160_to_h256(to)];
	let data = encode_u256_be(U256::from(amount)).to_vec();

	// Charge the standard EVM log gas (3 topics, 32 bytes data).
	let log_cost =
		crate::evm::precompiles::costs::log_costs(topics.len(), data.len()).map_err(|_| PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::OutOfGas,
		})?;
	handle.record_cost(log_cost)?;

	let address = handle.code_address();
	let log = Log { address, topics, data };
	handle.log(log.address, log.topics, log.data)?;
	Ok(())
}
