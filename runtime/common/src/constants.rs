// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

pub mod currency {
	pub use crate::Balance;

	pub const HDX: Balance = 1_000_000_000_000;
	pub const DOLLARS: Balance = HDX * 10; // 10 HDX ~= 1 $
	pub const CENTS: Balance = DOLLARS / 100;
	pub const MILLICENTS: Balance = CENTS / 1_000;

	pub const FORTUNE: Balance = u128::MAX;
}

pub mod time {
	use crate::{BlockNumber, Moment};

	/// Since BABE is probabilistic this is the average expected block time that
	/// we are targeting. Blocks will be produced at a minimum duration defined
	/// by `SLOT_DURATION`, but some slots will not be allocated to any
	/// authority and hence no block will be produced. We expect to have this
	/// block time on average following the defined slot duration and the value
	/// of `c` configured for BABE (where `1 - c` represents the probability of
	/// a slot being empty).
	/// This value is only used indirectly to define the unit constants below
	/// that are expressed in blocks. The rest of the code should use
	/// `SLOT_DURATION` instead (like the Timestamp pallet for calculating the
	/// minimum period).
	///
	/// If using BABE with secondary slots (default) then all of the slots will
	/// always be assigned, in which case `MILLISECS_PER_BLOCK` and
	/// `SLOT_DURATION` should have the same value.
	///
	/// <https://research.web3.foundation/en/latest/polkadot/BABE/Babe/#6-practical-results>

	pub const MILLISECS_PER_BLOCK: Moment = 6_000;
	pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;

	pub const MINUTES: BlockNumber = 60 / (SECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	pub const SLOT_DURATION: Moment = MILLISECS_PER_BLOCK;

	// 1 in 4 blocks (on average, not counting collisions) will be primary BABE blocks.
	pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);

	pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 4 * HOURS;
	pub const EPOCH_DURATION_IN_SLOTS: u64 = {
		const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

		(EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
	};
	pub const INFINITY: u32 = u32::MAX;
}

pub mod chain {
	pub use crate::{AssetId, Balance};
	pub use frame_support::weights::{constants::WEIGHT_PER_SECOND, Weight};
	pub use sp_runtime::Perbill;

	/// Core asset id
	pub const CORE_ASSET_ID: AssetId = 0;

	/// Max fraction of pool to buy in single transaction
	pub const MAX_OUT_RATIO: u128 = 3;

	/// Max fraction of pool to sell in single transaction
	pub const MAX_IN_RATIO: u128 = 3;

	/// Trading limit
	pub const MIN_TRADING_LIMIT: Balance = 1000;

	pub const RUNTIME_AUTHORING_VERSION: u32 = 1;
	pub const RUNTIME_SPEC_VERSION: u32 = 12;
	pub const RUNTIME_IMPL_VERSION: u32 = 0;
	pub const RUNTIME_TRANSACTION_VERSION: u32 = 1;

	/// We assume that an on-initialize consumes 2.5% of the weight on average, hence a single extrinsic
	/// will not be allowed to consume more than `AvailableBlockRatio - 2.5%`.
	pub const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_perthousand(25);
	/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used
	/// by  Operational  extrinsics.
	pub const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
	/// We allow for 2 seconds of compute with a 6 second average block time.
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = 2 * WEIGHT_PER_SECOND;
}

#[cfg(test)]
mod tests {
	use super::time::{DAYS, EPOCH_DURATION_IN_BLOCKS, HOURS, MILLISECS_PER_BLOCK, MINUTES, SECS_PER_BLOCK};
	use crate::BlockNumber;

	#[test]
	// This function tests that time units are set up correctly
	fn time_units_work() {
		// 24 hours in a day
		assert_eq!(DAYS / 24, HOURS);
		// 60 minuts in an hour
		assert_eq!(HOURS / 60, MINUTES);
		// 1 minute = 60s = 10 blocks 6s each
		assert_eq!(MINUTES, 10 as BlockNumber);
		// 6s per block
		assert_eq!(SECS_PER_BLOCK, 6);
		// 1s = 1000ms
		pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
		assert_eq!(MILLISECS_PER_BLOCK / 1000, SECS_PER_BLOCK);
		// Extra check for epoch time because changing it bricks the block production and requires regenesis
		assert_eq!(EPOCH_DURATION_IN_BLOCKS, 4 * HOURS);
	}
}

