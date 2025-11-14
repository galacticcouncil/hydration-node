// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::manual_inspect)]

extern crate alloc;

// Re-export pallet items so that they can be accessed from the crate namespace.
use alloc::string::ToString;
use frame_support::pallet_prelude::Weight;
use frame_support::PalletId;
use ismp::router::{PostRequest, Request, Response, StorageValue as IsmpStorageValue, Timeout};
use ismp::{error::Error as IsmpError, module::IsmpModule};
pub use pallet::*;
use pallet_ismp::ModuleId;
use sp_core::{keccak_256, H256, U256};
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec::Vec;

pub(crate) const ISMP_ORACLE_PALLET_ID: PalletId = PalletId(*b"ismporcl");
pub const ISMP_ORACLE_ID: ModuleId = ModuleId::Pallet(ISMP_ORACLE_PALLET_ID);

/// stETH contract address. All unstructured storage positions below
/// are read from this contract address via EVM GET requests.
// Mainnet
// const STETH_PROXY: [u8; 20] = hex_literal::hex!("ae7ab96520de3a18e5e111b5eaab095312d7fe84");

// Sepolia
const STETH_PROXY: [u8; 20] = hex_literal::hex!("3e3FE7dBc6B4C189E7128855dD526361c49b40Af");

// stETH unstructured storage positions used to reconstruct _getTotalPooledEther() and _getTotalShares().
// These are read via GET requests to the stETH proxy contract.
const BUFFERED_ETHER_POSITION: H256 = H256(hex_literal::hex!(
	"ed310af23f61f96daefbcd140b306c0bdbf8c178398299741687b90e794772b0"
));
const DEPOSITED_VALIDATORS_POSITION: H256 = H256(hex_literal::hex!(
	"e6e35175eb53fc006520a2a9c3e9711a7c00de6ff2c32dd31df8c5a24cac1b5c"
));
const CL_BALANCE_POSITION: H256 = H256(hex_literal::hex!(
	"a66d35f054e68143c18f32c990ed5cb972bb68a68f500cd2dd3a16bbf3686483"
));
const CL_VALIDATORS_POSITION: H256 = H256(hex_literal::hex!(
	"9f70001d82b6ef54e9d3725b46581c3eb9ee3aa02b941b6aa54d678a9ca35b10"
));
const TOTAL_SHARES_POSITION: H256 = H256(hex_literal::hex!(
	"e3b4b636e601189b5f4c6742edf2538ac12bb61ed03e6da26949d69838fa447e"
));

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use ismp::dispatcher::{DispatchRequest, FeeMetadata, IsmpDispatcher};
	use sp_core::{H256, U256};

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_ismp::Config {
		/// The overarching event type.
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		type Balance: frame_support::traits::tokens::Balance;

		type IsmpHost: ismp::host::IsmpHost
			+ IsmpDispatcher<Account = Self::AccountId, Balance = <Self as Config>::Balance>
			+ Default;

		#[pallet::constant]
		type RequestsTimeout: Get<u64>;

		type RequestOrigin: EnsureOrigin<Self::RuntimeOrigin>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn last_commitment)]
	pub type LastSentCommitment<T: Config> = StorageValue<_, H256, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn responded_commitments)]
	pub type RespondedCommitments<T: Config> = StorageMap<_, Identity, H256, H256, ValueQuery>;

	#[pallet::error]
	pub enum Error<T> {
		GetRequestFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		GetRequestSent {
			commitment: H256,
		},
		PostRequestSent {
			commitment: H256,
		},
		GetRequestResponded {
			commitment: H256,
			storage_values: Vec<IsmpStorageValue>,
		},
		WstEthPriceReceived {
			commitment: H256,
			eth_per_wsteth: U256,
		},
		PostRequestResponded {
			commitment: H256,
		},
		GetRequestTimedOut {
			commitment: H256,
		},
		PostResponseTimedOut {
			commitment: H256,
		},
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(1_000_000, 0))]
		pub fn request_get(origin: OriginFor<T>, params: GetParams) -> DispatchResult {
			T::RequestOrigin::ensure_origin(origin)?;

			// For this oracle pallet we build the stETH keys internally,
			// so the caller only specifies dest and height.
			let get = ismp::dispatcher::DispatchGet {
				dest: params.dest,
				from: ISMP_ORACLE_ID.to_bytes(),
				keys: Pallet::<T>::steth_storage_keys(),
				height: params.height,
				context: Default::default(),
				timeout: T::RequestsTimeout::get(),
			};

			let dispatcher = T::IsmpHost::default();
			let hash = dispatcher
				.dispatch_request(
					DispatchRequest::Get(get),
					FeeMetadata {
						payer: Self::pallet_account_id(),
						fee: Default::default(),
					},
				)
				.map_err(|_| Error::<T>::GetRequestFailed)?;

			LastSentCommitment::<T>::put(hash);
			Self::deposit_event(Event::<T>::GetRequestSent { commitment: hash });

			Ok(())
		}

		#[pallet::call_index(1)]
		#[pallet::weight(Weight::from_parts(1_000_000, 0))]
		pub fn request_post(origin: OriginFor<T>, params: PostParams) -> DispatchResult {
			T::RequestOrigin::ensure_origin(origin)?;

			let post = ismp::dispatcher::DispatchPost {
				dest: params.dest,
				from: ISMP_ORACLE_ID.to_bytes(),
				to: params.to,
				timeout: T::RequestsTimeout::get(),
				body: params.body,
			};

			let dispatcher = T::IsmpHost::default();
			let hash = dispatcher
				.dispatch_request(
					DispatchRequest::Post(post),
					FeeMetadata {
						payer: Self::pallet_account_id(),
						fee: Default::default(),
					},
				)
				.map_err(|_| Error::<T>::GetRequestFailed)?;

			LastSentCommitment::<T>::put(hash);
			Self::deposit_event(Event::<T>::PostRequestSent { commitment: hash });

			Ok(())
		}
	}

	#[derive(Clone, codec::Encode, codec::Decode, scale_info::TypeInfo, PartialEq, Eq, RuntimeDebug)]
	pub struct GetParams {
		/// Destination state machine (e.g. EVM-1 for Ethereum mainnet)
		pub dest: ismp::host::StateMachine,
		/// Block height on the counterparty chain at which to read stETH state
		pub height: u64,
	}

	#[derive(Clone, codec::Encode, codec::Decode, scale_info::TypeInfo, PartialEq, Eq, RuntimeDebug)]
	pub struct PostParams {
		pub dest: ismp::host::StateMachine,
		pub to: Vec<u8>,
		pub body: Vec<u8>,
	}

	// Hack for implementing the [`Default`] bound needed for
	// [`IsmpModule`](ismp::module::IsmpModule)
	impl<T> Default for Pallet<T> {
		fn default() -> Self {
			Self(PhantomData)
		}
	}
}

impl<T: Config> IsmpModule for Pallet<T> {
	fn on_accept(&self, _request: PostRequest) -> Result<(), anyhow::Error> {
		Err(IsmpError::Custom("Module does not accept post requests".to_string()))?
	}

	fn on_response(&self, response: Response) -> Result<(), anyhow::Error> {
		match response {
			Response::Post(post_response) => {
				let hash = ismp::messaging::hash_request::<pallet_ismp::Pallet<T>>(&Request::Post(post_response.post));

				// RespondedCommitments::<T>::insert(hash, keccak_256(&post_response.response).into());
				Pallet::<T>::deposit_event(Event::<T>::PostRequestResponded { commitment: hash });
			}
			Response::Get(get_response) => {
				let hash = ismp::messaging::hash_request::<pallet_ismp::Pallet<T>>(&Request::Get(get_response.get));
				RespondedCommitments::<T>::insert(hash, sp_core::H256(keccak_256(&[0])));

				// Emitting the raw values for debugging/introspection
				Pallet::<T>::deposit_event(Event::<T>::GetRequestResponded {
					commitment: hash,
					storage_values: get_response.values.clone(),
				});

				// Try to decode them as stETH state to compute the wstETH price
				if let Some(price) = Pallet::<T>::compute_wsteth_price(&get_response.values) {
					Pallet::<T>::deposit_event(Event::<T>::WstEthPriceReceived {
						commitment: hash,
						eth_per_wsteth: price,
					});
				}
			}
		}

		Ok(())
	}

	fn on_timeout(&self, timeout: Timeout) -> Result<(), anyhow::Error> {
		match timeout {
			Timeout::Request(req) if req.get_request().is_ok() => {
				let commitment = ismp::messaging::hash_request::<pallet_ismp::Pallet<T>>(&req);
				Pallet::<T>::deposit_event(Event::<T>::GetRequestTimedOut { commitment });

				Ok(())
			}
			Timeout::Response(res) => {
				let commitment = ismp::messaging::hash_request::<pallet_ismp::Pallet<T>>(&res.request());
				Pallet::<T>::deposit_event(Event::<T>::PostResponseTimedOut { commitment });

				Ok(())
			}
			_ => Err(IsmpError::Custom(
				"Only Get requests and Post responses are allowed, found PostRequest".to_string(),
			))?,
		}
	}
}

impl<T: Config> Pallet<T> {
	// Build a single EVM key as `contract_address (20 bytes) || slot (32 bytes)`.
	fn make_evm_key(slot: H256) -> Vec<u8> {
		let mut out = Vec::with_capacity(20 + 32);
		out.extend_from_slice(&STETH_PROXY); // 20 bytes of stETH proxy address
		out.extend_from_slice(slot.as_bytes()); // 32-byte unstructured storage position
		out
	}

	// Build the list of storage keys we want to query on the stETH proxy.
	// These are raw unstructured storage positions used by the contract, wrapped
	// in the EVM key format expected by ISMP: `address || slot_hash`.
	fn steth_storage_keys() -> Vec<Vec<u8>> {
		sp_std::vec![
			Self::make_evm_key(BUFFERED_ETHER_POSITION),
			Self::make_evm_key(DEPOSITED_VALIDATORS_POSITION),
			Self::make_evm_key(CL_BALANCE_POSITION),
			Self::make_evm_key(CL_VALIDATORS_POSITION),
			Self::make_evm_key(TOTAL_SHARES_POSITION),
		]
	}

	// Try to reconstruct totalPooledEther and the implied ETH-per-wstETH price
	// from the GET response values.
	// Expects exactly 5 storage values in the same order as steth_storage_keys().
	// TODO: test order
	fn compute_wsteth_price(values: &[IsmpStorageValue]) -> Option<U256> {
		if values.len() != 5 {
			return None;
		}

		// NOTE: This assumes `IsmpStorageValue` has a `value: Vec<u8>` field containing
		// the raw 32-byte storage word. If the field name differs in your ismp crate,
		// adjust the accessor accordingly.
		let as_u256 = |v: &IsmpStorageValue| -> Option<U256> {
			let bytes: &[u8] = &v.value.clone()?;
			if bytes.len() != 32 {
				return None;
			}
			Some(U256::from_big_endian(bytes))
		};

		let buffered_ether = as_u256(&values[0])?;
		let deposited_validators = as_u256(&values[1])?;
		let cl_balance = as_u256(&values[2])?;
		let cl_validators = as_u256(&values[3])?;
		let total_shares = as_u256(&values[4])?;

		if deposited_validators < cl_validators || total_shares.is_zero() {
			return None;
		}

		// DEPOSIT_SIZE = 32 ether
		let deposit_size = U256::from(32u8) * U256::exp10(18);

		// transientBalance = (depositedValidators - clValidators) * 32 ether
		let transient = (deposited_validators - cl_validators) * deposit_size;

		// _getTotalPooledEther() as in stEth contract
		let total_pooled_ether = buffered_ether + cl_balance + transient;

		// ETH per 1 wstETH (per 1 share):
		// TODO: maybe need scale (10^18) here?
		let eth_per_wsteth = total_pooled_ether / total_shares;

		Some(eth_per_wsteth)
	}

	pub fn is_ismp_oracle(id: &[u8]) -> bool {
		id == &ISMP_ORACLE_ID.to_bytes()
	}

	pub fn pallet_account_id() -> T::AccountId {
		ISMP_ORACLE_PALLET_ID.into_account_truncating()
	}
}

// // PUBLIC API
// impl<T: Config> Pallet<T> {
// 	pub fn function_name() {}
// }
