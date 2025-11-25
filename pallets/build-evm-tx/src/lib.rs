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

use ethereum::AccessListItem;
use signet_rs::{TransactionBuilder, TxBuilder, EVM};
use sp_core::H160;

pub use pallet::*;

#[cfg(test)]
mod tests;

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

			let to_address_bytes = to_address.map(|h| h.0);
			let access_list_converted = convert_access_list(access_list);

			let tx = TransactionBuilder::new::<EVM>()
				.chain_id(chain_id)
				.nonce(nonce)
				.max_priority_fee_per_gas(max_priority_fee_per_gas)
				.max_fee_per_gas(max_fee_per_gas)
				.gas_limit(gas_limit as u128)
				.value(value)
				.input(data)
				.access_list(access_list_converted);

			let tx = if let Some(to) = to_address_bytes { tx.to(to) } else { tx };

			Ok(tx.build().build_for_signing())
		}
	}

	fn convert_access_list(items: Vec<AccessListItem>) -> Vec<([u8; 20], Vec<[u8; 32]>)> {
		items
			.into_iter()
			.map(|item| (item.address.0, item.storage_keys.into_iter().map(|k| k.0).collect()))
			.collect()
	}
}
