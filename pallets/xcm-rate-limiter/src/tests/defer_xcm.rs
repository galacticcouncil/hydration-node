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

pub use pretty_assertions::{assert_eq, assert_ne};

use xcm::VersionedXcm;

#[test]
fn deferred_by_should_not_track_or_limit_irrelevant_asset_xcms() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_withdraw_asset(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		assert_eq!(
			XcmRateLimiter::accumulated_amount(MultiLocation::here()),
			AccumulatedAmount::default()
		);
		assert_eq!(deferred, None);
	});
}

#[test]
fn deferred_by_should_track_incoming_teleported_asset_liquidity() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_receive_teleported_asset(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 2000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
		assert_eq!(deferred_block_number, Some(10));
	});
}

#[test]
fn deferred_by_should_defer_xcm_when_limit_exceeded() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 2000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
		assert_eq!(deferred_block_number, Some(10));
	});
}

#[test]
fn deferred_by_should_defer_xcm_when_v2_can_be_converted() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_xcm_v2(2000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 2000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
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
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 3000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
		assert_eq!(deferred_block_number, Some(20));
	});
}

#[test]
fn deferred_by_should_defer_by_max_of_all_assets_in_xcm() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let other_asset_loc = MultiLocation::new(1, GeneralIndex(42));
		let assets = vec![(MultiLocation::here(), 2000 * ONE), (other_asset_loc, 3000 * ONE)];
		let versioned_xcm = create_multi_reserve_asset_deposited(assets);
		let para_id = 999.into();

		//Act
		let deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_here = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_here.amount, 2000 * ONE);
		assert_eq!(accumulated_here.last_updated, 1);

		let accumulated_other = XcmRateLimiter::accumulated_amount(other_asset_loc);
		assert_eq!(accumulated_other.amount, 3000 * ONE);
		assert_eq!(accumulated_other.last_updated, 1);

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
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		// Transaction should be deferred by 10 blocks because it exceeds the limit by 1000 (1x the limit)
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 2000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
		assert_eq!(first_deferred_block_number, Some(10));

		// Second transaction should be put behind the first one by 20 blocks (2x the limit)
		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;
		let accumulated_amount = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_amount.amount, 4000 * ONE);
		assert_eq!(accumulated_amount.last_updated, 1);
		assert_eq!(second_deferred_block_number, Some(30));
	});
}

#[test]
fn deferred_by_should_defer_by_max_duration_when_it_is_reached() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 20_000 * ONE);
		let para_id = 999.into();

		//Act
		let deferred_by = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1.unwrap();
		let max_defer: u32 = <Test as Config>::MaxDeferDuration::get();
		//Assert
		assert_eq!(deferred_by, max_defer);
	});
}

#[test]
fn deferred_by_should_defer_successive_xcm_when_time_passes() {
	ExtBuilder::default().build().execute_with(|| {
		//Arrange
		let versioned_xcm = create_versioned_reserve_asset_deposited(MultiLocation::here(), 2000 * ONE);
		let para_id = 999.into();

		//Act
		let first_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;

		//Assert
		let accumulated_liquidity = XcmRateLimiter::accumulated_amount(MultiLocation::here());

		assert_eq!(accumulated_liquidity.amount, 2000 * ONE);
		assert_eq!(accumulated_liquidity.last_updated, 1);
		assert_eq!(first_deferred_block_number, Some(10));

		System::set_block_number(6);

		let second_deferred_block_number = XcmRateLimiter::deferred_by(para_id, 10, &versioned_xcm).1;
		let accumulated_liquidity = XcmRateLimiter::accumulated_amount(MultiLocation::here());
		assert_eq!(accumulated_liquidity.amount, 3500 * ONE);
		assert_eq!(accumulated_liquidity.last_updated, 6);
		assert_eq!(second_deferred_block_number, Some(25));
	});
}

pub fn create_versioned_reserve_asset_deposited(loc: MultiLocation, amount: u128) -> VersionedXcm<RuntimeCall> {
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(vec![(loc, amount).into()]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![
		Instruction::<RuntimeCall>::ReserveAssetDeposited(multi_assets),
	]))
}

pub fn create_multi_reserve_asset_deposited(locs_and_amounts: Vec<(MultiLocation, u128)>) -> VersionedXcm<RuntimeCall> {
	let locs_and_amounts = locs_and_amounts
		.into_iter()
		.map(|(loc, amount)| (loc, amount).into())
		.collect();
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(locs_and_amounts).unwrap();
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

pub fn create_versioned_withdraw_asset(loc: MultiLocation, amount: u128) -> VersionedXcm<RuntimeCall> {
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(vec![(loc, amount).into()]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![Instruction::<RuntimeCall>::WithdrawAsset(
		multi_assets,
	)]))
}

pub fn create_versioned_xcm_v2(amount: u128) -> VersionedXcm<RuntimeCall> {
	use xcm::v2::prelude::*;
	let multi_assets = MultiAssets::from_sorted_and_deduplicated(vec![(MultiLocation::here(), amount).into()]).unwrap();
	VersionedXcm::from(Xcm::<RuntimeCall>(vec![
		Instruction::<RuntimeCall>::ReserveAssetDeposited(multi_assets),
	]))
}
