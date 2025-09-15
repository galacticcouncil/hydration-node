// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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
use frame_system::pallet_prelude::*;
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(feature = "runtime-benchmarks")]
mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        
        /// Weight information for extrinsics in this pallet
        type WeightInfo: WeightInfo;
    }

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Custom data was emitted
        /// Fields: [who, message, value]
        DataEmitted {
            who: T::AccountId,
            message: BoundedVec<u8, ConstU32<256>>,
            value: u128,
        },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The provided message exceeds the maximum length of 256 bytes
        MessageTooLong,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// Emit a custom event with provided data
        ///
        /// Parameters:
        /// - `origin`: The transaction origin (must be signed)
        /// - `message`: UTF-8 encoded message (max 256 bytes)
        /// - `value`: Numeric value to include in the event
        ///
        /// Emits `DataEmitted` event when successful
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::emit_custom_event())]
        pub fn emit_custom_event(
            origin: OriginFor<T>,
            message: Vec<u8>,
            value: u128,
        ) -> DispatchResult {
            let who = ensure_signed(origin)?;
            
            let bounded_message = BoundedVec::<u8, ConstU32<256>>::try_from(message)
                .map_err(|_| Error::<T>::MessageTooLong)?;
            
            Self::deposit_event(Event::DataEmitted {
                who,
                message: bounded_message,
                value,
            });
            
            Ok(())
        }
    }
}