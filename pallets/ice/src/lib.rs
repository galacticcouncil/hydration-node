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

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
mod weights;

use frame_support::pallet_prelude::*;
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::tokens::Preservation;
use frame_support::PalletId;
use frame_support::{dispatch::DispatchResult, require_transactional, traits::Get};
use frame_system::offchain::SendTransactionTypes;
use frame_system::pallet_prelude::*;
use hydradx_traits::price::PriceProvider;
pub use pallet::*;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::AccountId32;
use types::*;
pub use weights::WeightInfo;

pub const UNSIGNED_TXS_PRIORITY: u64 = 1000;

type AssetId = pallet_intent::types::AssetId;
type Balance = pallet_intent::types::Balance;

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config + pallet_intent::Config + SendTransactionTypes<Call<Self>> {
		type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

		/// Pallet id - used to create a holding account
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Block number provider.
		type BlockNumberProvider: BlockNumberProvider<BlockNumber = BlockNumberFor<Self>>;

		/// Transfer support
		type Currency: Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// Weight information for extrinsics in this pallet.
		type WeightInfo: WeightInfo;
	}

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// Solution has been executed.
		Executed { who: T::AccountId },
	}

	#[pallet::error]
	pub enum Error<T> {}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::call_index(0)]
		#[pallet::weight(<T as Config>::WeightInfo::submit_solution())]
		pub fn submit_solution(
			origin: OriginFor<T>,
			solution: Solution,
			score: u64,
			valid_for_block: BlockNumberFor<T>,
		) -> DispatchResult {
			ensure_none(origin)?;
			Ok(())
		}
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
		fn on_finalize(_n: BlockNumberFor<T>) {}

		fn offchain_worker(block_number: BlockNumberFor<T>) {}
	}

	#[pallet::validate_unsigned]
	impl<T: Config> ValidateUnsigned for Pallet<T>
	where
		T::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	{
		type Call = Call<T>;

		/// Validates unsigned transactions for arbitrage execution
		///
		/// This function ensures that only valid arbitrage transactions originating from
		/// offchain workers are accepted, and prevents unauthorized external calls.
		fn validate_unsigned(source: TransactionSource, call: &Self::Call) -> TransactionValidity {
			match source {
				TransactionSource::External => {
					return InvalidTransaction::Call.into();
				}
				TransactionSource::Local => {}   // produced by our offchain worker
				TransactionSource::InBlock => {} // included in block
			};

			let valid_tx = |provide| {
				ValidTransaction::with_tag_prefix("ice-solution")
					.priority(UNSIGNED_TXS_PRIORITY)
					.and_provides([&provide])
					.longevity(3)
					.propagate(false)
					.build()
			};

			match call {
				Call::submit_solution { .. } => valid_tx(b"submit_solution".to_vec()),
				_ => InvalidTransaction::Call.into(),
			}
		}
	}
}

// PALLET PUBLIC API
impl<T: Config> Pallet<T> {
	pub fn get_pallet_account() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}
}

impl<T: Config> Pallet<T> {
	pub fn run<F>(block_no: BlockNumberFor<T>, solve: F) -> Option<Call<T>>
	where
		F: FnOnce(SolverData) -> Option<Solution>,
	{
		None
	}
}
