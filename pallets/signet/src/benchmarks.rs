#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;

benchmarks! {
    initialize {
        let admin: T::AccountId = whitelisted_caller();
        let deposit = 1000u128;
    }: _(RawOrigin::Root, admin.clone(), deposit)
    verify {
        assert_eq!(Admin::<T>::get(), Some(admin));
        assert_eq!(SignatureDeposit::<T>::get(), deposit);
    }

    update_deposit {
        // Setup: Initialize first
        let admin: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin.clone(), 1000u128);
        
        let new_deposit = 2000u128;
    }: _(RawOrigin::Signed(admin), new_deposit)
    verify {
        assert_eq!(SignatureDeposit::<T>::get(), new_deposit);
    }

    emit_custom_event {
        // Setup: Initialize first
        let admin: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin, 1000u128);
        
        let caller: T::AccountId = whitelisted_caller();
        let message = vec![1u8; 100];
        let value = 12345u128;
    }: _(RawOrigin::Signed(caller.clone()), message, value)
    verify {
        // Event was emitted
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}