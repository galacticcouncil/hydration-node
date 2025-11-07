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
use codec::Encode;
use frame_support::pallet_prelude::Weight;
use frame_support::traits::SortedMembers;
use frame_support::PalletId;
use ismp::router::{PostRequest, Request, Response, StorageValue as IsmpStorageValue, Timeout};
use ismp::{error::Error as IsmpError, module::IsmpModule};
pub use pallet::*;
use pallet_ismp::ModuleId;
use sp_core::keccak_256;
use sp_runtime::traits::AccountIdConversion;
use sp_std::vec::Vec;

pub(crate) const ISMP_ORACLE_PALLET_ID: PalletId = PalletId(*b"ismporcl");
pub const ISMP_ORACLE_ID: ModuleId = ModuleId::Pallet(ISMP_ORACLE_PALLET_ID);

// pub const SEPOLIA_EURC_TOTAL_SYPPLY: [u8; 52] = hex!["808456652fdb597867f38412077A9182bf77359Fd9b04db6de40540f30c0cbd90608aadf720bcddf"];

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use frame_system::pallet_prelude::*;
	use ismp::dispatcher::{DispatchRequest, FeeMetadata, IsmpDispatcher};
	use sp_core::H256;

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
			let who = ensure_signed(origin)?;

			let get = ismp::dispatcher::DispatchGet {
				dest: params.dest,
				from: ISMP_ORACLE_ID.to_bytes(),
				keys: params.keys,
				height: params.height,
				context: Default::default(),
				timeout: T::RequestsTimeout::get(),
			};

			let dispatcher = T::IsmpHost::default();
			let hash = dispatcher
				.dispatch_request(
					DispatchRequest::Get(get),
					FeeMetadata {
						payer: who,
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
			let who = ensure_signed(origin)?;

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
						payer: who,
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
		pub dest: ismp::host::StateMachine,
		pub height: u64,
		pub keys: Vec<Vec<u8>>,
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
				Pallet::<T>::deposit_event(Event::<T>::GetRequestResponded {
					commitment: hash,
					storage_values: get_response.values.clone(),
				});
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
