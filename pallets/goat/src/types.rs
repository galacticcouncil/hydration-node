// This file is part of hydration-node.

// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum CliffBehavior {
	/// Vesting simply starts after the cliff (no catch-up at cliff block).
	Linear,
	/// At cliff, release all accrued periods from start to cliff immediately (catch-up),
	/// then continue linearly per period.
	CatchUp,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct VestingSchedule<BlockNumber> {
	pub start: BlockNumber,
	pub period: BlockNumber,
	pub count: u32,
	pub cliff: Option<BlockNumber>,
	pub cliff_behavior: CliffBehavior,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Reward<AssetId, Balance, BlockNumber> {
	pub asset: AssetId,
	pub amount: Balance,
	pub vesting: Option<VestingSchedule<BlockNumber>>, // default vesting for this asset
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub enum BountyStatus<AccountId> {
	Open,
	OpenToAll,
	Assigned { worker: AccountId },
	Delivered(AccountId),
	PartiallyPaid(AccountId),
	Approved(AccountId),
	Cancelled,
	Expired,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Payout<AccountId, AssetId, Balance, BlockNumber> {
	pub to: AccountId,
	pub asset: AssetId,
	pub amount: Balance,
	pub at: BlockNumber,
	pub vesting_applied: bool,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct Application<AccountId, BlockNumber, Metadata> {
	pub applicant: AccountId,
	pub metadata: Metadata,
	pub submitted_at: BlockNumber,
}

#[derive(Clone, Encode, Decode, Eq, PartialEq, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct VestingScheduleInfo<AssetId, Balance, BlockNumber> {
	pub asset: AssetId,
	pub total_amount: Balance,
	pub claimed: Balance,
	pub start: BlockNumber,
	pub period: BlockNumber,
	pub count: u32,
	pub cliff: Option<BlockNumber>,
	pub cliff_behavior: CliffBehavior,
}