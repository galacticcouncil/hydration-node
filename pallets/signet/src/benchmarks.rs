#![cfg(feature = "runtime-benchmarks")]

use super::*;
use frame_benchmarking::{benchmarks, whitelisted_caller};
use frame_support::traits::Currency;
use frame_system::RawOrigin;

benchmarks! {
	initialize {
		let admin: T::AccountId = whitelisted_caller();
		let deposit = T::Currency::minimum_balance();
		let chain_id = b"test-chain".to_vec(); 
	}: _(RawOrigin::Root, admin.clone(), deposit, chain_id) 
	verify {
		assert_eq!(Admin::<T>::get(), Some(admin));
		assert_eq!(SignatureDeposit::<T>::get(), deposit);
	}

	update_deposit {
		let admin: T::AccountId = whitelisted_caller();
		let initial_deposit = T::Currency::minimum_balance();
		let chain_id = b"test-chain".to_vec(); 
		let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin.clone(), initial_deposit, chain_id); 
		let new_deposit = T::Currency::minimum_balance() * 2u32.into();
	}: _(RawOrigin::Signed(admin), new_deposit)
	verify {
		assert_eq!(SignatureDeposit::<T>::get(), new_deposit);
	}

	withdraw_funds {
		let admin: T::AccountId = whitelisted_caller();
		let chain_id = b"test-chain".to_vec(); 
		let _ = Pallet::<T>::initialize(RawOrigin::Root.into(), admin.clone(), T::Currency::minimum_balance(), chain_id); 

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

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
