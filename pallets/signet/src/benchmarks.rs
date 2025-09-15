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

#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::benchmarks;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use sp_std::vec;

benchmarks! {
    where_clause { 
        where 
            T: Config,
    }
    
    emit_custom_event {
        let caller: T::AccountId = frame_benchmarking::whitelisted_caller();
        let message = vec![1u8; 256];
        let value = 1234567890u128;
        
    }: _(RawOrigin::Signed(caller.clone()), message.clone(), value)
    verify {
        assert_last_event::<T>(Event::DataEmitted { 
            who: caller,
            message: BoundedVec::try_from(message).unwrap(),
            value,
        }.into());
    }
    
    impl_benchmark_test_suite!(Pallet, crate::mock::new_test_ext(), crate::mock::Test);
}

// Helper function to assert last event
fn assert_last_event<T: Config>(generic_event: <T as Config>::RuntimeEvent) {
    frame_system::Pallet::<T>::assert_last_event(generic_event.into());
}