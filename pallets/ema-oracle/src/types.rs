// This file is part of pallet-ema-oracle.

// Copyright (C) 2022-2023  Intergalactic, Limited (GIB).
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
use frame_support::sp_runtime::RuntimeDebug;
use hydra_dx_math::ema::{calculate_new_by_integrating_incoming, update_outdated_to_current, EmaPrice};
use hydra_dx_math::types::Fraction;
use hydradx_traits::{AggregatedEntry, Liquidity, Volume};
use scale_info::TypeInfo;
use sp_arithmetic::traits::{AtLeast32BitUnsigned, SaturatedConversion, UniqueSaturatedInto};

pub use hydradx_traits::{OraclePeriod, Source};

use sp_std::prelude::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type AssetId = u32;
pub type Balance = u128;
/// A price is a tuple of two `u128`s representing the numerator and denominator of a rational number.
pub type Price = EmaPrice;

/// A type representing data produced by a trade or liquidity event.
/// Includes the block number where it was created/updated.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Clone, PartialEq, Eq, Default, TypeInfo, MaxEncodedLen)]
pub struct OracleEntry<BlockNumber> {
	pub price: Price,
	pub volume: Volume<Balance>,
	pub liquidity: Liquidity<Balance>,
	pub updated_at: BlockNumber,
}

impl<BlockNumber> OracleEntry<BlockNumber> {
	/// Construct a new `OracleEntry`.
	pub const fn new(
		price: Price,
		volume: Volume<Balance>,
		liquidity: Liquidity<Balance>,
		updated_at: BlockNumber,
	) -> Self {
		Self {
			price,
			volume,
			liquidity,
			updated_at,
		}
	}
}

impl<BlockNumber> OracleEntry<BlockNumber>
where
	BlockNumber: AtLeast32BitUnsigned + Copy + UniqueSaturatedInto<u64>,
{
	/// Convert the `OracleEntry` into an `AggregatedEntry` for consumption. Determines the age by
	/// subtracting `initialized` from `self.updated_at`.
	pub fn into_aggregated(self, initialized: BlockNumber) -> AggregatedEntry<Balance, BlockNumber, Price> {
		AggregatedEntry {
			price: self.price,
			volume: self.volume,
			liquidity: self.liquidity,
			oracle_age: self.updated_at.saturating_sub(initialized),
		}
	}

	/// Return the raw data of the entry as a tuple of tuples, excluding the block number.
	pub fn raw_data(&self) -> (Price, (Balance, Balance, Balance, Balance), (Balance, Balance)) {
		(self.price, self.volume.clone().into(), self.liquidity.into())
	}

	/// Return an inverted version of the entry where the meaning of assets a and b are inverted.
	/// So the price of a/b become the price b/a and the volume switches correspondingly.
	pub fn inverted(self) -> Self {
		let price = self.price.inverted();
		let volume = self.volume.inverted();
		let liquidity = self.liquidity.inverted();
		Self {
			price,
			volume,
			liquidity,
			updated_at: self.updated_at,
		}
	}

	/// Update the volume in `self` by adding in the volume of `incoming` and taking over the other
	/// values from `incoming`.
	pub fn accumulate_volume_and_update_from(&mut self, incoming: &Self) {
		self.volume = incoming.volume.saturating_add(&self.volume);
		self.price = incoming.price;
		self.liquidity = incoming.liquidity;
		self.updated_at = incoming.updated_at;
	}

	/// Fast forward the oracle value to `new_updated_at`. Updates the block number and resets the volume.
	pub fn fast_forward_to(&mut self, new_updated_at: BlockNumber) {
		self.updated_at = new_updated_at;
		self.volume = Volume::default();
	}

	/// Determine a new entry based on `self` and a previous entry. Adds the volumes together and
	/// takes the values of `self` for the rest.
	pub fn with_added_volume_from(&self, previous_entry: &Self) -> Self {
		let volume = previous_entry.volume.saturating_add(&self.volume);
		Self {
			price: self.price,
			volume,
			liquidity: self.liquidity,
			updated_at: self.updated_at,
		}
	}

	/// Determine the next oracle entry based on a previous (`self`) and an `incoming` entry as well as
	/// a `period`.
	///
	/// Returns `None` if any of the calculations fail (including the `incoming` entry not being
	/// one iteration (block) more recent than `self`).
	///
	/// The period is used to determine the smoothing factor alpha for an exponential moving average.
	pub fn calculate_new_by_integrating_incoming(&self, period: OraclePeriod, incoming: &Self) -> Option<Self> {
		// incoming should be one step ahead of the previous value
		if !incoming.updated_at.checked_sub(&self.updated_at)?.is_one() {
			return None;
		}
		if period == OraclePeriod::LastBlock {
			return Some(incoming.clone());
		}
		// determine smoothing factor
		let smoothing = into_smoothing(period);
		let (price, volume, liquidity) =
			calculate_new_by_integrating_incoming(self.raw_data(), incoming.raw_data(), smoothing);

		Some(Self {
			price,
			volume: volume.into(),
			liquidity: liquidity.into(),
			updated_at: incoming.updated_at,
		})
	}

	/// Update `self` based on a previous (`self`) and an `incoming` oracle entry as well as  a `period`.
	pub fn update_to_new_by_integrating_incoming(
		&mut self,
		period: OraclePeriod,
		incoming: &Self,
	) -> Option<&mut Self> {
		*self = self.calculate_new_by_integrating_incoming(period, incoming)?;
		Some(self)
	}

	/// Determine the current intended oracle entry based on a previous (`self`) and an `update_with` entry as well as
	/// a `period`.
	///
	/// Returns `None` if any of the calculations fail (including the `update_with` entry not being
	/// more recent than `self`).
	///
	/// The period is used to determine the smoothing factor alpha for an exponential moving average.
	///
	/// Uses the difference between `updated_at` to determine the time (i.e. iterations) to cover.
	pub fn calculate_current_from_outdated(&self, period: OraclePeriod, update_with: &Self) -> Option<Self> {
		let iterations = update_with.updated_at.checked_sub(&self.updated_at)?;
		if iterations.is_zero() {
			return None;
		}
		if period == OraclePeriod::LastBlock {
			return Some(update_with.clone());
		}
		// determine smoothing factor
		let smoothing = into_smoothing(period);
		let (price, volume, liquidity) = update_outdated_to_current(
			iterations.saturated_into(),
			self.raw_data(),
			(update_with.price, update_with.liquidity.into()),
			smoothing,
		);

		Some(Self {
			price,
			volume: volume.into(),
			liquidity: liquidity.into(),
			updated_at: update_with.updated_at,
		})
	}

	/// Update `self` based on a previous (`self`) and an `update_with` entry as well as a `period`.
	/// See [`calculate_current_from_outdated`].
	pub fn update_outdated_to_current(&mut self, period: OraclePeriod, update_with: &Self) -> Option<&mut Self> {
		*self = self.calculate_current_from_outdated(period, update_with)?;
		Some(self)
	}
}

/// Convert a given `period` into the smoothing factor used in the weighted average.
/// See [`check_period_smoothing_factors`] for how the values are generated.
pub fn into_smoothing(period: OraclePeriod) -> Fraction {
	match period {
		OraclePeriod::LastBlock => Fraction::from_bits(170141183460469231731687303715884105728),
		OraclePeriod::Short => Fraction::from_bits(34028236692093846346337460743176821146),
		OraclePeriod::TenMinutes => Fraction::from_bits(3369132345751865974884897103284833777),
		OraclePeriod::Hour => Fraction::from_bits(566193622164623067326746434994622648),
		OraclePeriod::Day => Fraction::from_bits(23629079016800115510268356880200556),
		OraclePeriod::Week => Fraction::from_bits(3375783642235081630771268215908257),
	}
}

impl<BlockNumber> From<(Price, Volume<Balance>, Liquidity<Balance>, BlockNumber)> for OracleEntry<BlockNumber> {
	fn from((price, volume, liquidity, updated_at): (Price, Volume<Balance>, Liquidity<Balance>, BlockNumber)) -> Self {
		Self {
			price,
			volume,
			liquidity,
			updated_at,
		}
	}
}
