#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

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
	
	#[pallet::storage]
	#[pallet::getter(fn last_built_tx)]
	pub type LastBuiltTx<T: Config> = StorageValue<_, BoundedVec<u8, T::MaxDataLength>, ValueQuery>;
	
	#[pallet::storage]
	#[pallet::getter(fn tx_by_account)]
	pub type TxByAccount<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<u8, T::MaxDataLength>,
		ValueQuery
	>;
	
	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		TransactionBuilt {
			who: T::AccountId,
			tx_hash: [u8; 32],
			rlp_length: u32,
		},
	}
	
	#[pallet::error]
	pub enum Error<T> {
		InvalidAddress,
		DataTooLong,
		BuildFailed,
	}
	
	#[pallet::pallet]
	pub struct Pallet<T>(_);
	
	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn build_evm_transaction(
			origin: OriginFor<T>,
			to_address: Vec<u8>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u128,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			chain_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
			ensure!(to_address.len() == 20, Error::<T>::InvalidAddress);
			
			let mut to_array = [0u8; 20];
			to_array.copy_from_slice(&to_address[0..20]);
			
			let evm_tx = TransactionBuilder::new::<EVM>()
				.nonce(nonce)
				.to(to_array)
				.value(value)
				.input(data)
				.max_priority_fee_per_gas(max_priority_fee_per_gas)
				.max_fee_per_gas(max_fee_per_gas)
				.gas_limit(gas_limit)
				.chain_id(chain_id)
				.build();
			
			let rlp_encoded = evm_tx.build_for_signing();
			
			let tx_hash = sp_io::hashing::keccak_256(&rlp_encoded);
			
			let bounded_tx = BoundedVec::<u8, T::MaxDataLength>::try_from(rlp_encoded.clone())
				.map_err(|_| Error::<T>::DataTooLong)?;
			
			<LastBuiltTx<T>>::put(&bounded_tx);
			<TxByAccount<T>>::insert(&who, &bounded_tx);
			
			Self::deposit_event(Event::TransactionBuilt {
				who,
				tx_hash,
				rlp_length: rlp_encoded.len() as u32,
			});
			
			Ok(())
		}
		
		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn build_evm_contract_creation(
			origin: OriginFor<T>,
			value: u128,
			init_code: Vec<u8>,
			nonce: u64,
			gas_limit: u128,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			chain_id: u64,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			
			ensure!(init_code.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
			
			let evm_tx = TransactionBuilder::new::<EVM>()
				.nonce(nonce)
				.value(value)
				.input(init_code)
				.max_priority_fee_per_gas(max_priority_fee_per_gas)
				.max_fee_per_gas(max_fee_per_gas)
				.gas_limit(gas_limit)
				.chain_id(chain_id)
				.build();
			
			let rlp_encoded = evm_tx.build_for_signing();
			
			let tx_hash = sp_io::hashing::keccak_256(&rlp_encoded);
			
			let bounded_tx = BoundedVec::<u8, T::MaxDataLength>::try_from(rlp_encoded.clone())
				.map_err(|_| Error::<T>::DataTooLong)?;
			
			<LastBuiltTx<T>>::put(&bounded_tx);
			<TxByAccount<T>>::insert(&who, &bounded_tx);
			
			Self::deposit_event(Event::TransactionBuilt {
				who,
				tx_hash,
				rlp_length: rlp_encoded.len() as u32,
			});
			
			Ok(())
		}
	}
	
	impl<T: Config> Pallet<T> {
		pub fn get_rlp_encoded_tx(
			to_address: Option<Vec<u8>>,
			value: u128,
			data: Vec<u8>,
			nonce: u64,
			gas_limit: u128,
			max_fee_per_gas: u128,
			max_priority_fee_per_gas: u128,
			chain_id: u64,
		) -> Result<Vec<u8>, DispatchError> {
			ensure!(data.len() <= T::MaxDataLength::get() as usize, Error::<T>::DataTooLong);
			
			let mut builder = TransactionBuilder::new::<EVM>()
				.nonce(nonce)
				.value(value)
				.input(data)
				.max_priority_fee_per_gas(max_priority_fee_per_gas)
				.max_fee_per_gas(max_fee_per_gas)
				.gas_limit(gas_limit)
				.chain_id(chain_id);
			
			if let Some(addr) = to_address {
				ensure!(addr.len() == 20, Error::<T>::InvalidAddress);
				let mut to_array = [0u8; 20];
				to_array.copy_from_slice(&addr[0..20]);
				builder = builder.to(to_array);
			}
			
			let evm_tx = builder.build();
			Ok(evm_tx.build_for_signing())
		}
		
		pub fn parse_ethereum_address(address_hex: &[u8]) -> Result<[u8; 20], DispatchError> {
			ensure!(address_hex.len() == 40, Error::<T>::InvalidAddress);
			
			let mut result = [0u8; 20];
			hex::decode_to_slice(address_hex, &mut result)
				.map_err(|_| Error::<T>::InvalidAddress)?;
			Ok(result)
		}
	}
}