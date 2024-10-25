// This file is part of HydraDX-node.

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

	pub const FORTUNE: Balance = u128::MAX;
	pub const UNITS: Balance = 1_000_000_000_000;
	pub const DOLLARS: Balance = UNITS * 100; // 100 UNITS ~= 1 $
	pub const CENTS: Balance = DOLLARS / 100; // 1 UNITS ~= 1 cent
	pub const MILLICENTS: Balance = CENTS / 1_000;
	pub const NATIVE_EXISTENTIAL_DEPOSIT: Balance = CENTS;

	pub fn deposit(items: u32, bytes: u32) -> Balance {
		items as Balance * 2 * DOLLARS + (bytes as Balance) * 30 * MILLICENTS
	}

	// Value on the right side of this condition represents balance deposited to liquidity mining's pot
	// account in native currency to prevent dusting of the pot. Pot exists for every instance of warehouse
	// liq. mining and this value has to be deposited to all pot's instances.
	// WARN: More tokens must be sent to pots when this value is changed.
	static_assertions::const_assert!(NATIVE_EXISTENTIAL_DEPOSIT < 100 * UNITS);
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

	pub const MILLISECS_PER_BLOCK: u64 = 12_000;

	// Time is measured by number of blocks.
	pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
	pub const HOURS: BlockNumber = MINUTES * 60;
	pub const DAYS: BlockNumber = HOURS * 24;

	pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;
	pub const SECS_PER_BLOCK: Moment = MILLISECS_PER_BLOCK / 1000;
	pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 4 * HOURS;

	pub mod unix_time {
		use crate::Moment;

		// in milliseconds
		pub const DAY: Moment = 86_400_000;
		pub const WEEK: Moment = 7 * DAY;
		pub const MONTH: Moment = 2_629_743_000;
	}
}

pub mod chain {
	pub use crate::{AssetId, Balance};
	pub use frame_support::weights::{constants::WEIGHT_REF_TIME_PER_SECOND, Weight};

	/// Core asset id
	pub const CORE_ASSET_ID: AssetId = 0;

	/// We allow for 0.5 seconds of compute
	pub const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
		WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
		polkadot_primitives::v7::MAX_POV_SIZE as u64,
	);

	/// The source of the data for the oracle.
	pub const OMNIPOOL_SOURCE: [u8; 8] = *b"omnipool";
	pub const STABLESWAP_SOURCE: [u8; 8] = *b"stablesw";
	pub const XYK_SOURCE: [u8; 8] = *b"hydraxyk";
}

#[cfg(test)]
mod tests {
	use super::time::{DAYS, EPOCH_DURATION_IN_BLOCKS, HOURS, MILLISECS_PER_BLOCK, MINUTES, SECS_PER_BLOCK};

	#[test]
	// This function tests that time units are set up correctly
	fn time_units_work() {
		// 24 hours in a day
		assert_eq!(DAYS / 24, HOURS);
		// 60 minuts in an hour
		assert_eq!(HOURS / 60, MINUTES);
		// 1 minute = 60s = 5 blocks 12s each
		assert_eq!(MINUTES, 5);
		// 6s per block
		assert_eq!(SECS_PER_BLOCK, 12);
		// 1s = 1000ms
		assert_eq!(MILLISECS_PER_BLOCK / 1000, SECS_PER_BLOCK);
		// Extra check for epoch time because changing it bricks the block production and requires regenesis
		assert_eq!(EPOCH_DURATION_IN_BLOCKS, 4 * HOURS);
	}
}
