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

use crate::{
	evm::{Permill, U256},
	BaseFee, Block, Ethereum, Runtime, RuntimeCall, UncheckedExtrinsic, Vec, EVM, H160,
};
use fp_rpc::{
	runtime_decl_for_ConvertTransactionRuntimeApi::ConvertTransactionRuntimeApi,
	runtime_decl_for_EthereumRuntimeRPCApi::EthereumRuntimeRPCApi, TransactionStatus,
};
use pallet_ethereum::Transaction as EthereumTransaction;
use pallet_evm::{Account as EVMAccount, FeeCalculator, Runner};
use sp_core::{Get, H256};
use sp_runtime::traits::{Block as BlockT, UniqueSaturatedInto};

// Frontier APIs
impl EthereumRuntimeRPCApi<Block> for Runtime {
	fn chain_id() -> u64 {
		<Runtime as pallet_evm::Config>::ChainId::get()
	}

	fn account_basic(address: H160) -> EVMAccount {
		let (account, _) = EVM::account_basic(&address);
		account
	}

	fn gas_price() -> U256 {
		let (gas_price, _) = <Runtime as pallet_evm::Config>::FeeCalculator::min_gas_price();
		gas_price
	}

	fn account_code_at(address: H160) -> Vec<u8> {
		EVM::account_codes(address)
	}

	fn author() -> H160 {
		<pallet_evm::Pallet<Runtime>>::find_author()
	}

	fn storage_at(address: H160, index: U256) -> H256 {
		let mut tmp = [0u8; 32];
		index.to_big_endian(&mut tmp);
		EVM::account_storages(address, H256::from_slice(&tmp[..]))
	}

	fn call(
		from: H160,
		to: H160,
		data: Vec<u8>,
		value: U256,
		gas_limit: U256,
		max_fee_per_gas: Option<U256>,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		estimate: bool,
		access_list: Option<Vec<(H160, Vec<H256>)>>,
	) -> Result<pallet_evm::CallInfo, sp_runtime::DispatchError> {
		let mut config = <Runtime as pallet_evm::Config>::config().clone();
		config.estimate = estimate;

		let is_transactional = false;
		let validate = true;
		<Runtime as pallet_evm::Config>::Runner::call(
			from,
			to,
			data,
			value,
			gas_limit.unique_saturated_into(),
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list.unwrap_or_default(),
			is_transactional,
			validate,
			&config,
		)
		.map_err(|err| err.error.into())
	}

	fn create(
		_from: H160,
		_data: Vec<u8>,
		_value: U256,
		_gas_limit: U256,
		_max_fee_per_gas: Option<U256>,
		_max_priority_fee_per_gas: Option<U256>,
		_nonce: Option<U256>,
		_estimate: bool,
		_access_list: Option<Vec<(H160, Vec<H256>)>>,
	) -> Result<pallet_evm::CreateInfo, sp_runtime::DispatchError> {
		Err(sp_runtime::DispatchError::Other(
			"Creating contracts is not currently supported",
		))
	}

	fn current_transaction_statuses() -> Option<Vec<TransactionStatus>> {
		Ethereum::current_transaction_statuses()
	}

	fn current_block() -> Option<pallet_ethereum::Block> {
		Ethereum::current_block()
	}

	fn current_receipts() -> Option<Vec<pallet_ethereum::Receipt>> {
		Ethereum::current_receipts()
	}

	fn current_all() -> (
		Option<pallet_ethereum::Block>,
		Option<Vec<pallet_ethereum::Receipt>>,
		Option<Vec<TransactionStatus>>,
	) {
		(
			Ethereum::current_block(),
			Ethereum::current_receipts(),
			Ethereum::current_transaction_statuses(),
		)
	}

	fn extrinsic_filter(xts: Vec<<Block as BlockT>::Extrinsic>) -> Vec<EthereumTransaction> {
		xts.into_iter()
			.filter_map(|xt| match xt.0.function {
				RuntimeCall::Ethereum(pallet_ethereum::Call::transact { transaction }) => Some(transaction),
				_ => None,
			})
			.collect::<Vec<EthereumTransaction>>()
	}

	fn elasticity() -> Option<Permill> {
		Some(BaseFee::elasticity())
	}

	fn gas_limit_multiplier_support() {}
}

impl ConvertTransactionRuntimeApi<Block> for Runtime {
	fn convert_transaction(transaction: EthereumTransaction) -> <Block as BlockT>::Extrinsic {
		UncheckedExtrinsic::new_unsigned(pallet_ethereum::Call::<Runtime>::transact { transaction }.into())
	}
}
