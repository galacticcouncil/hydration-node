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
				Action::Allowance => Self::not_supported(asset_id, handle),
				Action::Approve => Self::not_supported(asset_id, handle),
				Action::TransferFrom => Self::not_supported(asset_id, handle),
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
	fn not_supported(asset_id: AssetId, handle: &mut impl PrecompileHandle) -> EvmResult<PrecompileOutput> {
		Err(PrecompileFailure::Error {
			exit_status: pallet_evm::ExitError::Other("not supported".into()),
		})
	}

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

/*
#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		aca_evm_address, alice, ausd_evm_address, bob, erc20_address_not_exists, lp_aca_ausd_evm_address, new_test_ext,
		Balances, Test,
	};
	use frame_support::assert_noop;
	use hex_literal::hex;

	type MultiCurrencyPrecompile = crate::MultiCurrencyPrecompile<Test>;

	#[test]
	fn name_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// name() -> 0x06fdde03
			let input = hex! {"
				06fdde03
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000005
				4163616c61000000000000000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000017
				4c50204163616c61202d204163616c6120446f6c6c6172000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}
}*/
/*
pub fn read_selector<T>(input: &[u8]) -> EvmResult<T>
where
	T: num_enum::TryFromPrimitive<Primitive = u32>,
{
	if input.len() < 4 {
		return Err(revert("tried to parse selector out of bounds"));
	}

	let mut buffer = [0u8; 4];
	buffer.copy_from_slice(&input[0..4]);
	let selector = T::try_from_primitive(u32::from_be_bytes(buffer)).map_err(|_| {
		log::trace!(
			target: "precompile-utils",
			"Failed to match function selector",
		);
		//TODO: we could maybe include the selector name if possible with similar like type_name::<T>()
		revert("unknown selector")
	})?;

	Ok(selector)
}*/

/*
impl<Runtime> Precompile for MultiCurrencyPrecompile<Runtime>
where
	Runtime: pallet_evm::Config + pallet_currencies::Config,
	pallet_currencies::Pallet<Runtime>: MultiCurrencyT<Runtime::AccountId, CurrencyId = AssetId, Balance = Balance>,
{
	fn execute(input: &[u8], target_gas: Option<u64>, context: &Context, _is_static: bool) -> PrecompileResult {
		let input = Input::<
			Action,
			Runtime::AccountId,
			<Runtime as pallet_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>::new(input, target_gas_limit(target_gas));

		let currency_id =
			Runtime::Erc20InfoMapping::decode_evm_address(context.caller).ok_or_else(|| PrecompileFailure::Revert {
				exit_status: ExitRevert::Reverted,
				output: "invalid currency id".into(),
				cost: target_gas_limit(target_gas).unwrap_or_default(),
			})?;

		//TODO: enable it
		//let gas_cost = Pricer::<Runtime>::cost(&input, currency_id)?;
		let gas_cost = 11;

		if let Some(gas_limit) = target_gas {
			if gas_limit < gas_cost {
				return Err(PrecompileFailure::Error {
					exit_status: ExitError::OutOfGas,
				});
			}
		}

		let action = input.action()?;

		log::debug!(target: "evm", "multicurrency: currency id: {:?}", currency_id);

		/*match action {
			Action::QueryName => {
				let name = Runtime::Erc20InfoMapping::name(currency_id).ok_or_else(|| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Get name failed".into(),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;
				log::debug!(target: "evm", "multicurrency: name: {:?}", name);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes(&name),
					logs: Default::default(),
				})
			}
			Action::QuerySymbol => {
				let symbol =
					Runtime::Erc20InfoMapping::symbol(currency_id).ok_or_else(|| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Get symbol failed".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})?;
				log::debug!(target: "evm", "multicurrency: symbol: {:?}", symbol);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_bytes(&symbol),
					logs: Default::default(),
				})
			}
			Action::QueryDecimals => {
				let decimals =
					Runtime::Erc20InfoMapping::decimals(currency_id).ok_or_else(|| PrecompileFailure::Revert {
						exit_status: ExitRevert::Reverted,
						output: "Get decimals failed".into(),
						cost: target_gas_limit(target_gas).unwrap_or_default(),
					})?;
				log::debug!(target: "evm", "multicurrency: decimals: {:?}", decimals);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(decimals),
					logs: Default::default(),
				})
			}
			Action::QueryTotalIssuance => {
				let total_issuance =
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_issuance(currency_id);
				log::debug!(target: "evm", "multicurrency: total issuance: {:?}", total_issuance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(total_issuance),
					logs: Default::default(),
				})
			}
			Action::QueryBalance => {
				let who = input.account_id_at(1)?;
				let balance = if currency_id == <Runtime as module_transaction_payment::Config>::NativeCurrencyId::get()
				{
					<Runtime as module_evm::Config>::Currency::free_balance(&who)
				} else {
					<Runtime as module_transaction_payment::Config>::MultiCurrency::total_balance(currency_id, &who)
				};
				log::debug!(target: "evm", "multicurrency: who: {:?}, balance: {:?}", who, balance);

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: Output::encode_uint(balance),
					logs: Default::default(),
				})
			}
			Action::Transfer => {
				let from = input.account_id_at(1)?;
				let to = input.account_id_at(2)?;
				let amount = input.balance_at(3)?;
				log::debug!(target: "evm", "multicurrency: transfer from: {:?}, to: {:?}, amount: {:?}", from, to, amount);

				<module_currencies::Pallet<Runtime> as MultiCurrencyT<Runtime::AccountId>>::transfer(
					currency_id,
					&from,
					&to,
					amount,
				)
				.map_err(|e| PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: Output::encode_error_msg("Multicurrency Transfer failed", e),
					cost: target_gas_limit(target_gas).unwrap_or_default(),
				})?;

				Ok(PrecompileOutput {
					exit_status: ExitSucceed::Returned,
					cost: gas_cost,
					output: vec![],
					logs: Default::default(),
				})
			}
		}*/
		Ok(PrecompileOutput {
			exit_status: ExitSucceed::Returned,
			cost: 0,
			output: vec![],
			logs: Default::default(),
		})
	}
}
*/
/*
struct Pricer<R>(PhantomData<R>);

impl<Runtime> Pricer<Runtime>
where
	Runtime:
		module_currencies::Config + module_evm::Config + module_prices::Config + module_transaction_payment::Config,
{
	const BASE_COST: u64 = 200;

	fn cost(
		input: &Input<
			Action,
			Runtime::AccountId,
			<Runtime as module_evm::Config>::AddressMapping,
			Runtime::Erc20InfoMapping,
		>,
		currency_id: CurrencyId,
	) -> Result<u64, PrecompileFailure> {
		let action = input.action()?;

		// Decode CurrencyId from EvmAddress
		let read_currency = InputPricer::<Runtime>::read_currency(currency_id);

		let cost = match action {
			Action::QueryName | Action::QuerySymbol | Action::QueryDecimals => Self::erc20_info(currency_id),
			Action::QueryTotalIssuance => {
				// Currencies::TotalIssuance (r: 1)
				WeightToGas::convert(<Runtime as frame_system::Config>::DbWeight::get().reads(1))
			}
			Action::QueryBalance => {
				let cost = InputPricer::<Runtime>::read_accounts(1);
				// Currencies::Balance (r: 1)
				cost.saturating_add(WeightToGas::convert(
					<Runtime as frame_system::Config>::DbWeight::get().reads(2),
				))
			}
			Action::Transfer => {
				let cost = InputPricer::<Runtime>::read_accounts(2);

				// transfer weight
				let weight = if currency_id == <Runtime as module_transaction_payment::Config>::NativeCurrencyId::get()
				{
					<Runtime as module_currencies::Config>::WeightInfo::transfer_native_currency()
				} else {
					<Runtime as module_currencies::Config>::WeightInfo::transfer_non_native_currency()
				};

				cost.saturating_add(WeightToGas::convert(weight))
			}
		};

		Ok(Self::BASE_COST.saturating_add(read_currency).saturating_add(cost))
	}

	fn dex_share_read_cost(share: DexShare) -> u64 {
		match share {
			DexShare::Erc20(_) | DexShare::ForeignAsset(_) => WeightToGas::convert(Runtime::DbWeight::get().reads(1)),
			_ => Self::BASE_COST,
		}
	}

	fn erc20_info(currency_id: CurrencyId) -> u64 {
		match currency_id {
			CurrencyId::Erc20(_) | CurrencyId::StableAssetPoolToken(_) | CurrencyId::ForeignAsset(_) => {
				WeightToGas::convert(Runtime::DbWeight::get().reads(1))
			}
			CurrencyId::DexShare(symbol_0, symbol_1) => {
				Self::dex_share_read_cost(symbol_0).saturating_add(Self::dex_share_read_cost(symbol_1))
			}
			_ => Self::BASE_COST,
		}
	}
}*/

/*
#[cfg(test)]
mod tests {
	use super::*;

	use crate::precompile::mock::{
		aca_evm_address, alice, ausd_evm_address, bob, erc20_address_not_exists, lp_aca_ausd_evm_address, new_test_ext,
		Balances, Test,
	};
	use frame_support::assert_noop;
	use hex_literal::hex;

	type MultiCurrencyPrecompile = crate::MultiCurrencyPrecompile<Test>;

	#[test]
	fn handles_invalid_currency_id() {
		new_test_ext().execute_with(|| {
			// call with not exists erc20
			let context = Context {
				address: Default::default(),
				caller: erc20_address_not_exists(),
				apparent_value: Default::default(),
			};

			// symbol() -> 0x95d89b41
			let input = hex! {"
				95d89b41
			"};

			assert_noop!(
				MultiCurrencyPrecompile::execute(&input, Some(10_000), &context, false),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "invalid currency id".into(),
					cost: target_gas_limit(Some(10_000)).unwrap(),
				}
			);
		});
	}

	#[test]
	fn name_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// name() -> 0x06fdde03
			let input = hex! {"
				06fdde03
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000005
				4163616c61000000000000000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000017
				4c50204163616c61202d204163616c6120446f6c6c6172000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn symbol_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// symbol() -> 0x95d89b41
			let input = hex! {"
				95d89b41
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				0000000000000000000000000000000000000000000000000000000000000003
				4143410000000000000000000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				0000000000000000000000000000000000000000000000000000000000000020
				000000000000000000000000000000000000000000000000000000000000000b
				4c505f4143415f41555344000000000000000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn decimals_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// decimals() -> 0x313ce567
			let input = hex! {"
				313ce567
			"};

			// Token
			context.caller = aca_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000000000000c
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn total_supply_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// totalSupply() -> 0x18160ddd
			let input = hex! {"
				18160ddd
			"};

			// Token
			context.caller = ausd_evm_address();

			// 2_000_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000077359400
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		});
	}

	#[test]
	fn balance_of_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// balanceOf(address) -> 0x70a08231
			// account
			let input = hex! {"
				70a08231
				000000000000000000000000 1000000000000000000000000000000000000001
			"};

			// Token
			context.caller = aca_evm_address();

			// INITIAL_BALANCE = 1_000_000_000_000
			let expected_output = hex! {"
				00000000000000000000000000000000 0000000000000000000000e8d4a51000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());

			// DexShare
			context.caller = lp_aca_ausd_evm_address();

			let expected_output = hex! {"
				00000000000000000000000000000000 00000000000000000000000000000000
			"};

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, expected_output.to_vec());
		})
	}

	#[test]
	fn transfer_works() {
		new_test_ext().execute_with(|| {
			let mut context = Context {
				address: Default::default(),
				caller: Default::default(),
				apparent_value: Default::default(),
			};

			// transfer(address,address,uint256) -> 0xbeabacc8
			// from
			// to
			// amount
			let input = hex! {"
				beabacc8
				000000000000000000000000 1000000000000000000000000000000000000001
				000000000000000000000000 1000000000000000000000000000000000000002
				00000000000000000000000000000000 00000000000000000000000000000001
			"};

			let from_balance = Balances::free_balance(alice());
			let to_balance = Balances::free_balance(bob());

			// Token
			context.caller = aca_evm_address();

			let resp = MultiCurrencyPrecompile::execute(&input, None, &context, false).unwrap();
			assert_eq!(resp.exit_status, ExitSucceed::Returned);
			assert_eq!(resp.output, [0u8; 0].to_vec());

			assert_eq!(Balances::free_balance(alice()), from_balance - 1);
			assert_eq!(Balances::free_balance(bob()), to_balance + 1);

			// DexShare
			context.caller = lp_aca_ausd_evm_address();
			assert_noop!(
				MultiCurrencyPrecompile::execute(&input, Some(100_000), &context, false),
				PrecompileFailure::Revert {
					exit_status: ExitRevert::Reverted,
					output: "Multicurrency Transfer failed: BalanceTooLow".into(),
					cost: target_gas_limit(Some(100_000)).unwrap(),
				}
			);
		})
	}
}
*/
