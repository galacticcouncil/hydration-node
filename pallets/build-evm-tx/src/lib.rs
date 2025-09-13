#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use sp_std::vec::Vec;
	use signet_rs::{TransactionBuilder, TxBuilder, EVM};
	
	#[pallet::config]
	pub trait Config: frame_system::Config {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		
		#[pallet::constant]
		type MaxDataLength: Get<u32>;
	}
	
	
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		TransactionBuilt {
			who: T::AccountId,
			rlp_data: Vec<u8>,
		},
	}
	
	#[pallet::error]
	pub enum Error<T> {
		/// The provided address is not exactly 20 bytes
		InvalidAddressLength,
		/// Transaction data exceeds the maximum allowed length
		DataTooLong,
		/// Max priority fee per gas is greater than max fee per gas
		InvalidGasPrice,
	}
	
	#[pallet::pallet]
	pub struct Pallet<T>(_);
	
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Build an EVM transaction (regular or contract creation).
		/// 
		/// - `to_address`: Some(address) for regular transaction, None for contract creation
		/// - `data`: Transaction data (call data for regular tx, init code for contract creation)
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn build_evm_transaction(
			origin: OriginFor<T>,
			to_address: Option<Vec<u8>>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u128,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			chain_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			// Validate data length
			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
			
			// Validate to_address if provided
			if let Some(ref addr) = to_address {
				ensure!(addr.len() == 20, Error::<T>::InvalidAddressLength);
			}
			
			// Validate gas prices relationship
			ensure!(max_priority_fee_per_gas <= max_fee_per_gas, Error::<T>::InvalidGasPrice);
			
			let mut builder = TransactionBuilder::new::<EVM>()
				.nonce(nonce)
				.value(value)
				.input(data)
				.max_priority_fee_per_gas(max_priority_fee_per_gas)
				.max_fee_per_gas(max_fee_per_gas)
				.gas_limit(gas_limit)
				.chain_id(chain_id);
			
			// Add 'to' address if provided (regular transaction)
			// Omit for contract creation
			if let Some(addr) = to_address {
				let mut to_array = [0u8; 20];
				to_array.copy_from_slice(&addr[0..20]);
				builder = builder.to(to_array);
			}
			
			let evm_tx = builder.build();
			let rlp_encoded = evm_tx.build_for_signing();
			
			Self::deposit_event(Event::TransactionBuilt {
				who,
				rlp_data: rlp_encoded,
			});
			
			Ok(())
		}
	}
}