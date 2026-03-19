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

use super::*;

use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;
use frame_support::traits::OnIdle;
use sp_std::boxed::Box;

benchmarks! {
	where_clause { where
		T: crate::Config,
	}

	dispatch_as_treasury {
		let n in 1 .. 10_000;
		let remark = sp_std::vec![1u8; n as usize];

		let call: <T as pallet::Config>::RuntimeCall = frame_system::Call::remark { remark }.into();
	}: _(RawOrigin::Root, Box::new(call))

	dispatch_as_aave_manager {
		let n in 1 .. 10_000;
		let remark = sp_std::vec![1u8; n as usize];

		let call: <T as pallet::Config>::RuntimeCall = frame_system::Call::remark { remark }.into();
	}: _(RawOrigin::Root, Box::new(call))

	note_aave_manager {
	}: _(RawOrigin::Root, Pallet::<T>::aave_manager_account())

	dispatch_with_extra_gas{
		let n in 1 .. 10_000;
		let remark = sp_std::vec![1u8; n as usize];

		let call: <T as pallet::Config>::RuntimeCall = frame_system::Call::remark { remark }.into();
		let caller: T::AccountId = account("caller", 0, 1);

	}: _(RawOrigin::Signed(caller), Box::new(call), 50_000)

	dispatch_evm_call {
		let n in 1 .. 10_000;
		let remark = sp_std::vec![1u8; n as usize];

		let call: <T as pallet::Config>::RuntimeCall = frame_system::Call::remark { remark }.into();
		let caller: T::AccountId = account("caller", 0, 1);
	}: _(RawOrigin::Signed(caller), Box::new(call))

	pause_hyperbridge_cleanup {
	}: pause_hyperbridge_cleanup(RawOrigin::Root, true)
	verify {
		assert!(!CleanupEnabled::<T>::get());
	}

	cleanup_on_idle {
		let n in 1..5_000;

		// Insert n+1 keys so clear_prefix returns SomeRemaining;
		// this is the hot path that runs every block during cleanup.
		let prefix = Stage::StateCommitments.storage_prefix();
		CleanupStage::<T>::put(Stage::StateCommitments);

		for i in 0..(n + 1) {
			let mut key = prefix.to_vec();
			key.extend_from_slice(&i.to_le_bytes());
			frame_support::storage::unhashed::put(&key, &i);
		}

		// Budget: enough to delete exactly n keys (50% headroom models on_idle split)
		let per_key = T::DbWeight::get().reads_writes(2, 1);
		let remaining = per_key.saturating_mul(n as u64 * 2);
	}: {
		Pallet::<T>::on_idle(1u32.into(), remaining);
	}
	verify {
		// Stage did not advance — there are still keys remaining
		assert_eq!(CleanupStage::<T>::get(), Some(Stage::StateCommitments));
	}

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
