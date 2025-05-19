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

	impl_benchmark_test_suite!(Pallet, crate::mock::ExtBuilder::default().build(), crate::mock::Test);
}
