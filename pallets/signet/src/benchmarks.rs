#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_system::RawOrigin;
use frame_support::traits::Currency;

benchmarks! {
    initialize {
        let admin: T::AccountId = whitelisted_caller();
        let deposit = T::Currency::minimum_balance();
    }: _(RawOrigin::Root, admin.clone(), deposit)
    verify {
        assert_eq!(Admin::<T>::get(), Some(admin));
        assert_eq!(SignatureDeposit::<T>::get(), deposit);
    }

    update_deposit {
        // Setup: Initialize first
        let admin: T::AccountId = whitelisted_caller();
        let initial_deposit = T::Currency::minimum_balance();
        let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin.clone(), initial_deposit);
        
        let new_deposit = initial_deposit * 2u32.into();
    }: _(RawOrigin::Signed(admin), new_deposit)
    verify {
        assert_eq!(SignatureDeposit::<T>::get(), new_deposit);
    }

    withdraw_funds {
        // Setup: Initialize and fund the pallet
        let admin: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin.clone(), T::Currency::minimum_balance());
        
        // Fund the pallet account
        let pallet_account = Pallet::<T>::account_id();
        let amount = T::Currency::minimum_balance() * 100u32.into();
        let _ = T::Currency::deposit_creating(&pallet_account, amount);
        
        let recipient: T::AccountId = whitelisted_caller();
        let withdraw_amount = T::Currency::minimum_balance() * 50u32.into();
    }: _(RawOrigin::Signed(admin), recipient.clone(), withdraw_amount)
    verify {
        // Verify funds were transferred
        assert!(T::Currency::free_balance(&recipient) >= withdraw_amount);
    }

    emit_custom_event {
        // Setup: Initialize first
        let admin: T::AccountId = whitelisted_caller();
        let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin, T::Currency::minimum_balance());
        
        let caller: T::AccountId = whitelisted_caller();
        let message = vec![1u8; 100];
        let value = 12345u128;
    }: _(RawOrigin::Signed(caller.clone()), message, value)
    verify {
        // Event was emitted
    }

    impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}