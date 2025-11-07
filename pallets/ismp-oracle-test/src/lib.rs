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
use ismp::router::{PostRequest, Request, Response, Timeout};
use ismp::{error::Error as IsmpError, module::IsmpModule};
pub use pallet::*;
use pallet_ismp::ModuleId;
use sp_std::vec::Vec;

pub const PALLET_ID: ModuleId = ModuleId::Pallet(frame_support::PalletId(*b"ismporcl"));

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
	pub type LastSentCommitment<T: Config> = StorageValue<_, T::AccountId>;

	#[pallet::storage]
	#[pallet::getter(fn responded_commitments)]
	pub type RespondedCommitments<T: Config> = StorageMap<_, Identity, H256, H256, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn eurc_supply)]
	pub type LastEURCTotalSupply<T: Config> = StorageValue<_, sp_core::U256>;

	#[pallet::error]
	pub enum Error<T> {
		GetRequestFailed,
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		GetRequestSent { commitment: H256 },
		GetRequestResponded { commitment: H256 },
		GetRequestTimedOut { commitment: H256 },
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(Weight::from_parts(1_000_000, 0))]
		pub fn request_get(origin: OriginFor<T>, params: GetParams) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let get = ismp::dispatcher::DispatchGet {
				dest: params.dest,
				from: PALLET_ID.to_bytes(),
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

			Self::deposit_event(Event::<T>::GetRequestSent { commitment: hash });

			Ok(())
		}
	}

	#[derive(Clone, codec::Encode, codec::Decode, scale_info::TypeInfo, PartialEq, Eq, RuntimeDebug)]
	pub struct GetParams {
		pub dest: ismp::host::StateMachine,
		pub height: u64,
		pub keys: Vec<Vec<u8>>,
	}
}

pub struct IsmpModuleCallback<T: Config>(sp_std::marker::PhantomData<T>);
impl<T: Config> Default for IsmpModuleCallback<T> {
	fn default() -> Self {
		Self(core::marker::PhantomData)
	}
}

impl<T: Config> IsmpModule for IsmpModuleCallback<T> {
	fn on_accept(&self, _request: PostRequest) -> Result<(), anyhow::Error> {
		Err(IsmpError::Custom("Module does not accept post requests".to_string()))?
	}

	fn on_response(&self, response: Response) -> Result<(), anyhow::Error> {
		match response {
			Response::Post(_) => Err(IsmpError::Custom("Module does not accept post responses".to_string()))?,
			Response::Get(get_response) => {
				Pallet::<T>::deposit_event(Event::<T>::GetRequestResponded {
					commitment: ismp::messaging::hash_request::<pallet_ismp::Pallet<T>>(&Request::Get(get_response.get)),
				});
				for value in get_response.values {
					match value.key { Vec { .. } => {} }
				}
			},
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
			_ => Err(IsmpError::Custom("Only Get requests allowed, found Post".to_string()))?,
		}
	}
}

// impl<T: Config> Pallet<T> {}
//
// // PUBLIC API
// impl<T: Config> Pallet<T> {
// 	pub fn function_name() {}
// }
