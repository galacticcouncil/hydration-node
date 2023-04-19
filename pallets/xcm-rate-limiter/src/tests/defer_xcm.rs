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
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::parent(), 5);
		let para_id = 999.into();

		//Act
		XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
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
		let versioned_xcm = create_versioned_receive_teleported_asset(MultiLocation::parent(), 5);
		let para_id = 999.into();

		//Act
		XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
		assert_eq!(volume, 5);
	});
}
*/
#[test]
fn deferred_by_should_defer_xcm_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::parent(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
		assert_eq!(deferred_block_number, Some(10));
	});
}

#[test]
fn deferred_by_should_defer_xcm_when_limit_exceeded_double_limit() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::parent(), 3000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
		assert_eq!(deferred_block_number, Some(20));
	});
}

#[test]
#[ignore]
fn deferred_by_should_defer_successive_xcm_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::parent(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		// Transaction should be deffered by 10 blocks because it exceeds the limit by 1000 (1x the limit)
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
		assert_eq!(first_deferred_block_number, Some(10));

		// Second transaction should be put behind the first one by 20 blocks (2x the limit)
		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);
		assert_eq!(second_deferred_block_number, Some(30));
	});
}

#[test]
#[ignore]
fn deferred_by_should_defer_successive_xcm_when_time_passes() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::parent(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);

		//Assert
		let volume = XcmRateLimiter::liquidity_per_asset(MultiLocation::parent());
		assert_eq!(first_deferred_block_number, Some(10));

		System::set_block_number(5);

		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm);
		assert_eq!(second_deferred_block_number, Some(15));
	});
}

#[test]
fn set_limit_per_asset_should_work() {
	ExtBuilder::default().build().execute_with(|| {
		//Act
		assert_ok!(XcmRateLimiter::set_limit(
			RuntimeOrigin::root(),
			MultiLocation::parent(),
			1000 * ONE
		));

		//Assert
		let limit = XcmRateLimiter::rate_limit(MultiLocation::parent());
		assert_eq!(limit, Some(1000 * ONE));
	});
}

#[test]
fn set_limit_per_asset_should_fail_when_called_by_non_root() {
	ExtBuilder::default().build().execute_with(|| {
		//Act
		assert_noop!(
			XcmRateLimiter::set_limit(RuntimeOrigin::signed(ALICE), MultiLocation::parent(), 1000 * ONE),
			BadOrigin
		);
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

fn calculate_deferred_duration(
	global_duration: BlockNumber,
	rate_limit: u128,
	incoming_amount: u128,
	accumulated_amount: u128,
	blocks_since_last_update: BlockNumber,
) -> BlockNumber {
	let total_accumulated = calculate_new_accumulated_amount(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	);
	let global_duration: u128 = global_duration.max(1).saturated_into();
	// duration * (incoming + decayed - rate_limit)
	let deferred_duration =
		global_duration.saturating_mul(total_accumulated.saturating_sub(rate_limit)) / rate_limit.max(1);

	deferred_duration.saturated_into()
}

fn calculate_new_accumulated_amount(
	global_duration: BlockNumber,
	rate_limit: u128,
	incoming_amount: u128,
	accumulated_amount: u128,
	blocks_since_last_update: BlockNumber,
) -> u128 {
	incoming_amount.saturating_add(decay(
		global_duration,
		rate_limit,
		incoming_amount,
		accumulated_amount,
		blocks_since_last_update,
	))
}

fn decay(
	global_duration: BlockNumber,
	rate_limit: u128,
	incoming_amount: u128,
	accumulated_amount: u128,
	blocks_since_last_update: BlockNumber,
) -> u128 {
	let global_duration: u128 = global_duration.max(1).saturated_into();
	// acc - rate_limit * blocks / duration
	accumulated_amount
		.saturating_sub(rate_limit.saturating_mul(blocks_since_last_update.saturated_into()) / global_duration)
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
