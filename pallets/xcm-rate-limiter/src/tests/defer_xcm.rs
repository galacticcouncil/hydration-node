// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

use crate::tests::mock::RuntimeCall;
use crate::tests::mock::*;
use crate::*;
use cumulus_pallet_xcmp_queue::XcmDeferFilter;
use frame_support::assert_storage_noop;
pub use pretty_assertions::{assert_eq, assert_ne};
use sp_runtime::DispatchError::BadOrigin;
use sp_runtime::SaturatedConversion;
use xcm::lts::prelude::*;
use xcm::VersionedXcm;

/*
#[test]
#[ignore]
fn deferred_by_should_track_incoming_deposited_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 5);
		let para_id = 999.into();

		//Act
		XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(volume, 5);
	});
}
*/

/*
#[test]
#[ignore]
fn deferred_by_should_track_incoming_teleported_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_receive_teleported_asset(MultiLocation::here(), 5);
		let para_id = 999.into();

		//Act
		XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(volume, 5);
	});
}
*/
#[test]
fn deferred_by_should_defer_xcm_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(deferred_block_number, Some(10));
	});
}

#[test]
fn deferred_by_should_defer_xcm_when_limit_exceeded_double_limit() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 3000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(deferred_block_number, Some(20));
	});
}

#[test]
fn deferred_by_should_defer_successive_xcm_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		// Transaction should be deffered by 10 blocks because it exceeds the limit by 1000 (1x the limit)
		let volume = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(first_deferred_block_number, Some(10));

		// Second transaction should be put behind the first one by 20 blocks (2x the limit)
		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);
		assert_eq!(second_deferred_block_number, Some(30));
	});
}

#[test]
fn deferred_by_should_defer_successive_xcm_when_time_passes() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let accumulated_liquidity = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());

		assert_eq!(accumulated_liquidity.amount, 2000 * ONE);
		assert_eq!(accumulated_liquidity.last_updated, 1);
		assert_eq!(first_deferred_block_number, Some(10));

		System::set_block_number(6);

		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);
		let accumulated_liquidity = XcmRateLimiter::accumulated_liquidity_per_asset(MultiLocation::here());
		assert_eq!(accumulated_liquidity.amount, 3500 * ONE);
		assert_eq!(accumulated_liquidity.last_updated, 6);
		assert_eq!(second_deferred_block_number, Some(25));
	});
}

#[test]
fn deferred_duration_should_be_calculated_based_on_limit_and_incoming_amounts() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1500 * ONE;
	let accumulated_amount = 400 * ONE;
	let blocks_since_last_update = 0;
	let duration = calculate_deferred_duration(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(duration, 9);
}

#[test]
fn deferred_duration_should_return_zero_when_limit_not_reached() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 900 * ONE;
	let accumulated_amount = 0;
	let blocks_since_last_update = 0;
	let duration = calculate_deferred_duration(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(duration, 0);
}

#[test]
fn accumulated_amount_for_deferred_duration_should_decay() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1100 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 12;
	let duration = calculate_deferred_duration(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(duration, 1);
}

#[test]
fn defer_duration_should_incorporate_decay_amounts_and_incoming() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 1100 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 6;
	let duration = calculate_deferred_duration(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(duration, 7);
}

#[test]
fn long_time_since_update_should_reset_rate_limit() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 700 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 20;
	let duration = calculate_deferred_duration(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(duration, 0);
}

#[test]
fn calculate_new_accumulated_amount_should_decay_old_amounts_and_sum() {
	let global_duration = 10;
	let rate_limit = 1000 * ONE;
	let incoming_amount = 700 * ONE;
	let accumulated_amount = 1200 * ONE;
	let blocks_since_last_update = 6;
	let total_accumulated = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);

	assert_eq!(total_accumulated, 700 * ONE + 600 * ONE);
}

pub fn create_versioned_reserve_asset_deposited(loc: MultiLocation, amount: u128) -> VersionedXcm<RuntimeCall> {
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(vec![(loc, amount).into()]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![
		Instruction::<RuntimeCall>::ReserveAssetDeposited(multi_assets),
	]))
}

pub fn create_versioned_receive_teleported_asset(loc: MultiLocation, amount: u128) -> VersionedXcm<RuntimeCall> {
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(vec![(loc, amount).into()]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![
		Instruction::<RuntimeCall>::ReceiveTeleportedAsset(multi_assets),
	]))
}
