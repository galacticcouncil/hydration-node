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

// #[cfg(any(feature = "runtime-benchmarks", test))]
// mod benchmarks;

pub mod weights;
pub use weights::WeightInfo;

// Re-export pallet items so that they can be accessed from the crate namespace.
pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use codec::FullCodec;
    use frame_support::{dispatch::{GetDispatchInfo, PostDispatchInfo}, pallet_prelude::*};
    use frame_system::{pallet_prelude::*};
    use sp_runtime::traits::Dispatchable;
    use sp_std::boxed::Box;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        /// The overarching event type.
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// The overarching call type.
        type RuntimeCall: IsType<<Self as frame_system::Config>::RuntimeCall>
            + Dispatchable<RuntimeOrigin = Self::RuntimeOrigin, PostInfo = PostDispatchInfo>
            + GetDispatchInfo
            + FullCodec
            + TypeInfo
            + From<frame_system::Call<Self>>
            + Parameter;

        type TreasuryManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type AaveManagerOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// The weight information for this pallet.
        type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    pub struct Pallet<T>(_);

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        TreasuryManagerCallDispatched { call_hash: T::Hash, result: DispatchResultWithPostInfo },
        AaveManagerCallDispatched { call_hash: T::Hash, result: DispatchResultWithPostInfo },
    }

    #[pallet::error]
    pub enum Error<T> {
        /// The preimage of the call hash could not be loaded.
        UnavailablePreImage,
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(<T as Config>::WeightInfo::dispatch_as_treasury_manager())]
        pub fn dispatch_as_treasury_manager(origin: OriginFor<T>, call: Box<<T as Config>::RuntimeCall>,) -> DispatchResult {
            T::TreasuryManagerOrigin::ensure_origin(origin)?;

            Ok(())
        }
    }
}

// impl<T: Config> Pallet<T> {
//     /// Clean whitelisting/preimage and dispatch call.
//     ///
//     /// Return the call actual weight of the dispatched call if there is some.
//     fn do_dispatch(origin: T::Hash, call: <T as Config>::RuntimeCall) -> Option<Weight> {
//         WhitelistedCall::<T>::remove(call_hash);
//
//         T::Preimages::unrequest(&call_hash);
//
//         let result = call.dispatch(frame_system::Origin::<T>::Root.into());
//
//         let call_actual_weight = match result {
//             Ok(call_post_info) => call_post_info.actual_weight,
//             Err(call_err) => call_err.post_info.actual_weight,
//         };
//
//         Self::deposit_event(Event::<T>::WhitelistedCallDispatched { call_hash, result });
//
//         call_actual_weight
//     }
// }
