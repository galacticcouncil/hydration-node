// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2025  Intergalactic, Limited (GIB)
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

use crate::mock::*;
use crate::{AaveGasLimits, IsTestnet, Pallet as Parameters, RelayParentOffsetOverride};
use frame_support::{assert_noop, assert_ok};
use sp_runtime::DispatchError::BadOrigin;

const LIMITS: AaveGasLimits = AaveGasLimits {
	trade: 600_000,
	view: 200_000,
	reserves_list: 2_000_000,
};

#[test]
fn is_testnet_false_by_default() {
	ExtBuilder.build().execute_with(|| {
		assert!(!Parameters::<Test>::is_testnet());
	});
}

#[test]
fn is_testnet_true_when_set() {
	ExtBuilder.build().execute_with(|| {
		IsTestnet::<Test>::put(true);
		assert!(Parameters::<Test>::is_testnet());
	});
}

#[test]
fn relay_parent_offset_override_false_by_default() {
	ExtBuilder.build().execute_with(|| {
		assert!(!Parameters::<Test>::relay_parent_offset_override());
	});
}

#[test]
fn relay_parent_offset_override_true_when_set() {
	ExtBuilder.build().execute_with(|| {
		RelayParentOffsetOverride::<Test>::put(true);
		assert!(Parameters::<Test>::relay_parent_offset_override());
	});
}

#[test]
fn aave_gas_limits_should_be_none_by_default() {
	ExtBuilder.build().execute_with(|| {
		assert_eq!(Parameters::<Test>::aave_gas_limits(), None);
	});
}

#[test]
fn set_aave_gas_limits_should_store_limits_when_origin_is_authority() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Parameters::<Test>::set_aave_gas_limits(
			RuntimeOrigin::root(),
			Some(LIMITS)
		));

		assert_eq!(Parameters::<Test>::aave_gas_limits(), Some(LIMITS));
	});
}

#[test]
fn set_aave_gas_limits_should_clear_limits_when_none_is_given() {
	ExtBuilder.build().execute_with(|| {
		assert_ok!(Parameters::<Test>::set_aave_gas_limits(
			RuntimeOrigin::root(),
			Some(LIMITS)
		));

		assert_ok!(Parameters::<Test>::set_aave_gas_limits(RuntimeOrigin::root(), None));

		assert_eq!(Parameters::<Test>::aave_gas_limits(), None);
	});
}

#[test]
fn set_aave_gas_limits_should_fail_when_origin_is_not_authority() {
	ExtBuilder.build().execute_with(|| {
		assert_noop!(
			Parameters::<Test>::set_aave_gas_limits(RuntimeOrigin::signed(ALICE), Some(LIMITS)),
			BadOrigin
		);

		assert_eq!(Parameters::<Test>::aave_gas_limits(), None);
	});
}
