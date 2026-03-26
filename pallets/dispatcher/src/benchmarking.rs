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
use frame_support::traits::OnIdle;
use frame_system::RawOrigin;
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

	dispatch_as_emergency_admin {
		let n in 1 .. 10_000;
		let remark = sp_std::vec![1u8; n as usize];

		let call: <T as pallet::Config>::RuntimeCall = frame_system::Call::remark { remark }.into();
	}: _(RawOrigin::Root, Box::new(call))

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

		let prefix = Stage::StateCommitments.storage_prefix();
		let tail = 10_000u32;
		for i in 0..(n + tail) {
			let mut key = prefix.to_vec();
			key.extend_from_slice(&i.to_le_bytes());
			sp_io::storage::set(&key, &i.to_le_bytes());
		}

		let per_key = T::DbWeight::get().reads_writes(2, 1);
		let remaining = per_key.saturating_mul(n as u64 * 2);
	}: {
		Pallet::<T>::on_idle(1u32.into(), remaining);
	}
	// No verify block — unit tests cover correctness.
	// In TestExternalities clear_prefix ignores the limit (removes all keys at once),
	// in production RocksDB it respects it. A verify that satisfies both is not possible.

	cleanup_on_idle_limit_zero {
		let mut key = Stage::StateCommitments.storage_prefix().to_vec();

		let one_le_bytes = 1u32.to_le_bytes();
		key.extend_from_slice(&one_le_bytes);
		sp_io::storage::set(&key, &one_le_bytes);

		let per_key = T::DbWeight::get().reads_writes(2, 1);
	}: {
		Pallet::<T>::on_idle(1u32.into(), per_key);
	}
	// No verify — same TestExternalities limitation as cleanup_on_idle.

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
