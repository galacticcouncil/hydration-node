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

use crate::polkadot_test_net::*;
use codec::Encode;
use ismp::host::StateMachine;
use pallet_referenda::TracksInfo;
use primitives::constants::time::{HOURS, MINUTES};
use sp_core::storage::StorageKey;
use xcm_emulator::TestExt;

fn set_parameters_storage_to_testnet() {
	let key = StorageKey(frame_support::storage::storage_prefix(b"Parameters", b"IsTestnet").to_vec());
	let value = true.encode();
	sp_io::storage::set(&key.0, &value);
}

#[test]
fn is_testnet_sets_correct_referenda_params_when_default() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Assert
		let tracks: Vec<_> = <hydradx_runtime::Runtime as pallet_referenda::Config>::Tracks::tracks().collect();

		let root_track = tracks
			.iter()
			.find(|track| track.id == 0)
			.expect("Root track should exist");

		assert_eq!(root_track.info.prepare_period, HOURS);
		assert_eq!(root_track.info.confirm_period, 12 * HOURS);
		assert_eq!(root_track.info.min_enactment_period, 10 * MINUTES);
	});
}

#[test]
fn is_testnet_sets_correct_referenda_params_when_testnet() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Prepare
		set_parameters_storage_to_testnet();

		// Assert
		let tracks: Vec<_> = <hydradx_runtime::Runtime as pallet_referenda::Config>::Tracks::tracks().collect();
		let root_track = tracks
			.iter()
			.find(|track| track.id == 0)
			.expect("Root track should exist");

		assert_eq!(root_track.info.prepare_period, 1);
		assert_eq!(root_track.info.confirm_period, 1);
		assert_eq!(root_track.info.min_enactment_period, 1);
	});
}

#[test]
fn is_testnet_sets_correct_ismp_params_when_default() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Assert
		let ismp_coprocessor = <hydradx_runtime::Runtime as pallet_ismp::Config>::Coprocessor::get();
		let host_state_machine = <hydradx_runtime::Runtime as pallet_ismp::Config>::HostStateMachine::get();

		assert_eq!(ismp_coprocessor, Some(StateMachine::Polkadot(3367)));
		assert_eq!(host_state_machine, StateMachine::Polkadot(2034));
	});
}

#[test]
fn is_testnet_sets_correct_ismp_params_when_testnet() {
	TestNet::reset();
	Hydra::execute_with(|| {
		// Prepare
		set_parameters_storage_to_testnet();

		// Assert
		let ismp_coprocessor = <hydradx_runtime::Runtime as pallet_ismp::Config>::Coprocessor::get();
		let host_state_machine = <hydradx_runtime::Runtime as pallet_ismp::Config>::HostStateMachine::get();

		assert_eq!(ismp_coprocessor, Some(StateMachine::Kusama(4009)));
		assert_eq!(host_state_machine, StateMachine::Kusama(2034));
	});
}
