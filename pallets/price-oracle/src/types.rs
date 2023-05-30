// This file is part of pallet-price-oracle.

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

use codec::{Decode, Encode};
use frame_support::sp_runtime::traits::{CheckedAdd, CheckedDiv, CheckedMul, Zero};
use frame_support::sp_runtime::{FixedU128, RuntimeDebug};
use scale_info::TypeInfo;
use sp_std::iter::Sum;
use sp_std::ops::{Add, Index, IndexMut};
use sp_std::prelude::*;

#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

pub type AssetId = u32;
pub type Balance = u128;
pub type Price = FixedU128;

/// A type representing data produced by a trade.
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, Default, TypeInfo)]
pub struct PriceEntry {
    pub price: Price,
    pub trade_amount: Balance,
    pub liquidity_amount: Balance,
}

impl Add for PriceEntry {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            price: self.price.add(other.price),
            trade_amount: self.trade_amount.add(other.trade_amount),
            liquidity_amount: self.liquidity_amount.add(other.liquidity_amount),
        }
    }
}

impl Zero for PriceEntry {
    fn zero() -> Self {
        Self {
            price: Price::zero(),
            trade_amount: Balance::zero(),
            liquidity_amount: Balance::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        self == &PriceEntry::zero()
    }
}

impl Add for &PriceEntry {
    type Output = PriceEntry;
    fn add(self, other: Self) -> Self::Output {
        PriceEntry {
            price: self.price.add(other.price),
            trade_amount: self.trade_amount.add(other.trade_amount),
            liquidity_amount: self.liquidity_amount.add(other.liquidity_amount),
        }
    }
}

impl<'a> Sum<&'a Self> for PriceEntry {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Self>,
    {
        iter.fold(PriceEntry::default(), |a, b| &a + b)
    }
}

impl PriceEntry {
    /// Updates the previous average value with a new entry.
    pub fn calculate_new_price_entry(&self, previous_price_entry: &Self) -> Option<Self> {
        let total_liquidity = previous_price_entry
            .liquidity_amount
            .checked_add(self.liquidity_amount)?;
        let product_of_old_values = previous_price_entry
            .price
            .checked_mul(&Price::from_inner(previous_price_entry.liquidity_amount))?;
        let product_of_new_values = self.price.checked_mul(&Price::from_inner(self.liquidity_amount))?;
        Some(Self {
            price: product_of_old_values
                .checked_add(&product_of_new_values)?
                .checked_div(&Price::from_inner(total_liquidity))?,
            trade_amount: previous_price_entry.trade_amount.checked_add(self.trade_amount)?,
            liquidity_amount: total_liquidity,
        })
    }
}

pub const BUCKET_SIZE: u32 = 10;

pub type Bucket = [PriceInfo; BUCKET_SIZE as usize];

#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, Default, TypeInfo)]
pub struct PriceInfo {
    pub avg_price: Price,
    pub volume: Balance,
}

impl Add for &PriceInfo {
    type Output = PriceInfo;
    fn add(self, other: Self) -> Self::Output {
        PriceInfo {
            avg_price: self.avg_price.add(other.avg_price),
            volume: self.volume.add(other.volume),
        }
    }
}

impl<'a> Sum<&'a Self> for PriceInfo {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = &'a Self>,
    {
        iter.fold(PriceInfo::default(), |a, b| &a + b)
    }
}

/// A circular buffer storing average prices and volumes
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[derive(RuntimeDebug, Encode, Decode, Copy, Clone, PartialEq, Eq, TypeInfo)]
pub struct BucketQueue {
    bucket: Bucket,
    last: u32,
}

impl BucketQueue {
    // make sure that BUCKET_SIZE != 0
    pub const BUCKET_SIZE: u32 = BUCKET_SIZE;
}

impl Default for BucketQueue {
    fn default() -> Self {
        Self {
            bucket: Bucket::default(),
            last: Self::BUCKET_SIZE - 1,
        }
    }
}

pub trait BucketQueueT {
    fn update_last(&mut self, price_info: PriceInfo);
    fn get_last(&self) -> PriceInfo;
    fn calculate_average(&self) -> PriceInfo;
}

impl BucketQueueT for BucketQueue {
    /// Add new entry to the front and remove the oldest entry.
    fn update_last(&mut self, price_info: PriceInfo) {
        self.last = (self.last + 1) % Self::BUCKET_SIZE;
        self.bucket[self.last as usize] = price_info;
    }

    /// Get the last entry added
    fn get_last(&self) -> PriceInfo {
        self.bucket[self.last as usize]
    }

    /// Calculate average price and volume from all the entries.
    fn calculate_average(&self) -> PriceInfo {
        let sum = self.bucket.iter().sum::<PriceInfo>();
        PriceInfo {
            avg_price: sum
                .avg_price
                .checked_div(&Price::from(Self::BUCKET_SIZE as u128))
                .expect("avg_price is valid value; BUCKET_SIZE is non-zero integer; qed"),
            volume: sum
                .volume
                .checked_div(Self::BUCKET_SIZE as u128)
                .expect("avg_price is valid value; BUCKET_SIZE is non-zero integer; qed"),
        }
    }
}

impl Index<usize> for BucketQueue {
    type Output = PriceInfo;
    fn index(&self, index: usize) -> &Self::Output {
        &self.bucket[index]
    }
}

impl IndexMut<usize> for BucketQueue {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.bucket[index]
    }
}
