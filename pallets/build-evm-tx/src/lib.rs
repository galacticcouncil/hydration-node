// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use sp_std::vec::Vec;

use ethereum::{AccessListItem, EIP1559TransactionMessage, TransactionAction};
use sp_core::{H160, U256};

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarking;

const EIP1559_TX_TYPE: u8 = 0x02;

#[cfg(test)]
pub mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Maximum length of transaction data
		#[pallet::constant]
		type MaxDataLength: Get<u32>;
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Transaction data exceeds maximum allowed length
		DataTooLong,
		/// Invalid address format - must be exactly 20 bytes
		InvalidAddress,
		/// Priority fee cannot exceed max fee per gas (EIP-1559 requirement)
		InvalidGasPrice,
	}

	impl<T: Config> Pallet<T> {
		/// Build an EIP-1559 EVM transaction and return the RLP-encoded data
		///
		/// # Parameters
		/// - `origin`: The signed origin
		/// - `to_address`: Optional recipient address (None for contract creation)
		/// - `value`: ETH value in wei
		/// - `data`: Transaction data/calldata
		/// - `nonce`: Transaction nonce
		/// - `gas_limit`: Maximum gas units for transaction
		/// - `max_fee_per_gas`: Maximum total fee per gas (base + priority)
		/// - `max_priority_fee_per_gas`: Maximum priority fee (tip) per gas
		/// - `chain_id`: Target EVM chain ID
		///
		/// # Returns
		/// RLP-encoded transaction data with EIP-2718 type prefix (0x02 for EIP-1559)
		pub fn build_evm_tx(
			origin: OriginFor<T>,
			to_address: Option<H160>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u64,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			access_list: Vec<AccessListItem>,
			chain_id: u64,
		) -> Result<Vec<u8>, DispatchError> {
			ensure_signed(origin)?;

			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
			ensure!(max_priority_fee_per_gas <= max_fee_per_gas, Error::<T>::InvalidGasPrice);

			let action = match to_address {
				Some(addr) => TransactionAction::Call(addr),
				None => TransactionAction::Create,
			};

			let tx_message = EIP1559TransactionMessage {
				chain_id,
				nonce: U256::from(nonce),
				max_priority_fee_per_gas: U256::from(max_priority_fee_per_gas),
				max_fee_per_gas: U256::from(max_fee_per_gas),
				gas_limit: U256::from(gas_limit),
				action,
				value: U256::from(value),
				input: data,
				access_list,
			};

			let mut output = Vec::new();
			output.push(EIP1559_TX_TYPE);
			output.extend_from_slice(&rlp::encode(&tx_message));

			Ok(output)
		}
	}
}
