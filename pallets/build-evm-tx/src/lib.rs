#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use sp_std::vec::Vec;

use alloy_consensus::{TxEip1559, TxType};
use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_rlp::Encodable;

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
			to_address: Option<Vec<u8>>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u64,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			chain_id: u64,
		) -> Result<Vec<u8>, DispatchError> {
			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);

			ensure!(max_priority_fee_per_gas <= max_fee_per_gas, Error::<T>::InvalidGasPrice);

			let to = match to_address {
				Some(addr) => {
					let address = Address::try_from(addr.as_slice()).map_err(|_| Error::<T>::InvalidAddress)?;
					TxKind::Call(address)
				}
				None => TxKind::Create,
			};

			let tx = TxEip1559 {
				chain_id,
				nonce,
				gas_limit,
				max_fee_per_gas,
				max_priority_fee_per_gas,
				to,
				value: U256::from(value),
				input: Bytes::from(data),
				access_list: Default::default(),
			};

			// Encode transaction to RLP format with EIP-2718 type prefix
			let mut typed_tx = Vec::new();
			typed_tx.push(TxType::Eip1559 as u8);
			tx.encode(&mut typed_tx);

			Ok(typed_tx)
		}
	}
}
