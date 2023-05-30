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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

pub const HDX: AssetId = 1_000;
pub const DOT: AssetId = 2_000;

use frame_benchmarking::benchmarks;
use frame_support::traits::{OnFinalize, OnInitialize};

use crate::Pallet as PriceOracle;

pub const ORACLE_ENTRY_1: PriceEntry = PriceEntry {
    price: Price::from_inner(2000000000000000000),
    trade_amount: 1_000,
    liquidity_amount: 2_000,
};

pub const NUM_OF_ITERS: u32 = 100;

benchmarks! {
    on_finalize_no_entry {
        let block_num: u32 = 5;
    }: { PriceOracle::<T>::on_finalize(block_num.into()); }
    verify {
    }

    on_finalize_one_token {
        let block_num: u32 = 5;
        let hdx_dot_pair_name = PriceOracle::<T>::get_name(HDX, DOT);

        frame_system::Pallet::<T>::set_block_number((block_num - 1).into());
        PriceOracle::<T>::on_initialize((block_num - 1).into());
        PriceOracle::<T>::on_create_pool(HDX, DOT)?;
        PriceOracle::<T>::on_finalize((block_num - 1).into());

        frame_system::Pallet::<T>::set_block_number(block_num.into());
        PriceOracle::<T>::on_initialize(block_num.into());
        PriceOracle::<T>::on_trade(HDX, DOT, ORACLE_ENTRY_1);

        assert_eq!(<PriceDataAccumulator<T>>::try_get(hdx_dot_pair_name.clone()), Ok(ORACLE_ENTRY_1));

        let price_data = PriceOracle::<T>::price_data_ten();
        let bucket_queue = price_data.iter().find(|&x| x.0 == hdx_dot_pair_name).unwrap().1;
        assert_eq!(bucket_queue.get_last(), PriceInfo{ avg_price: Price::zero(), volume: 0});

    }: { PriceOracle::<T>::on_finalize(block_num.into()); }
    verify {
        assert!(!PriceDataAccumulator::<T>::contains_key(hdx_dot_pair_name.clone()));
        let price_data = PriceOracle::<T>::price_data_ten();
        let bucket_queue = price_data.iter().find(|&x| x.0 == hdx_dot_pair_name).unwrap().1;
        assert_eq!(bucket_queue.get_last(), PriceInfo{ avg_price: Price::from(2), volume: 1_000});
    }

    on_finalize_multiple_tokens_all_bucket_levels {
        let block_num: u32 = BUCKET_SIZE.pow(2);
        let a in 1 .. NUM_OF_ITERS; // trade_num
        frame_system::Pallet::<T>::set_block_number(Zero::zero());
        PriceOracle::<T>::on_initialize(Zero::zero());

        let asset_a = AssetId::default();
        let asset_b = AssetId::default();

        for i in 0 .. a {
            let asset_a = i * 1_000;
            let asset_b = i * 2_000;
            PriceOracle::<T>::on_create_pool(asset_a, asset_b)?;
        }

        PriceOracle::<T>::on_finalize(Zero::zero());
        frame_system::Pallet::<T>::set_block_number((block_num - 1).into());
        PriceOracle::<T>::on_initialize((block_num - 1).into());

        for i in 0 .. a {
            let asset_a = i * 1_000;
            let asset_b = i * 2_000;
            PriceOracle::<T>::on_trade(asset_a, asset_b, ORACLE_ENTRY_1);
        }

        let asset_pair_name = PriceOracle::<T>::get_name(asset_a, asset_b);
        assert_eq!(PriceDataAccumulator::<T>::try_get(asset_pair_name), Ok(ORACLE_ENTRY_1));
        let price_data = PriceOracle::<T>::price_data_ten();
        for i in 0 .. a {
            let asset_a = i * 1_000;
            let asset_b = i * 2_000;
            let bucket_queue = price_data.iter().find(|&x| x.0 == PriceOracle::<T>::get_name(asset_a, asset_b)).unwrap().1;
            assert_eq!(bucket_queue.get_last(), PriceInfo{ avg_price: Price::zero(), volume: 0});
        }

        for round in block_num .. 2 * block_num - 1 {
            PriceOracle::<T>::on_finalize((round - 1).into());
            frame_system::Pallet::<T>::set_block_number(round.into());
            PriceOracle::<T>::on_initialize(round.into());

            for i in 0 .. a {
                let asset_a = i * 1_000;
                let asset_b = i * 2_000;
                PriceOracle::<T>::on_trade(asset_a, asset_b, ORACLE_ENTRY_1);
            }

            assert_eq!(PriceDataAccumulator::<T>::try_get(PriceOracle::<T>::get_name(asset_a, asset_b)), Ok(ORACLE_ENTRY_1));
        }

        frame_system::Pallet::<T>::set_block_number(block_num.into());

    }: { PriceOracle::<T>::on_finalize((2 * block_num - 1).into()); }
    verify {
        let asset_a = 1_000;
        let asset_b = 2_000;
        let asset_pair_name = PriceOracle::<T>::get_name(asset_a, asset_b);
        assert!(!PriceDataAccumulator::<T>::contains_key(PriceOracle::<T>::get_name(asset_a, asset_b)));
        let price_data = PriceOracle::<T>::price_data_ten();
        for i in 0 .. BucketQueue::BUCKET_SIZE {
            for j in 0 .. a {
                let asset_a = j * 1_000;
                let asset_b = j * 2_000;
                let bucket_queue = price_data.iter().find(|&x| x.0 == PriceOracle::<T>::get_name(asset_a, asset_b)).unwrap().1;
                assert_eq!(bucket_queue[i as usize], PriceInfo{ avg_price: Price::from(2), volume: 1_000});
            }
        }

        let bucket_queue = PriceOracle::<T>::price_data_hundred(asset_pair_name.clone());
        for i in 0 .. BucketQueue::BUCKET_SIZE {
            for j in 0 .. a {
                let asset_a = j * 1_000;
                let asset_b = j * 2_000;
                let bucket_queue = price_data.iter().find(|&x| x.0 == PriceOracle::<T>::get_name(asset_a, asset_b)).unwrap().1;
                assert_eq!(bucket_queue[i as usize], PriceInfo{ avg_price: Price::from(2), volume: 1_000});
            }
        }

        let bucket_queue = PriceOracle::<T>::price_data_thousand(asset_pair_name);
        for i in 0 .. BucketQueue::BUCKET_SIZE {
            for j in 0 .. a {
                let asset_a = j * 1_000;
                let asset_b = j * 2_000;
                let bucket_queue = price_data.iter().find(|&x| x.0 == PriceOracle::<T>::get_name(asset_a, asset_b)).unwrap().1;
                assert_eq!(bucket_queue[i as usize], PriceInfo{ avg_price: Price::from(2), volume: 1_000});
            }
        }
    }

    on_finalize_multiple_tokens {
        let block_num: u32 = 5;
        let b in 1 .. NUM_OF_ITERS; // token num
        let mut vec = Vec::new();
        let asset_a = AssetId::default();
        let asset_b = AssetId::default();

        frame_system::Pallet::<T>::set_block_number((block_num - 1).into());
        PriceOracle::<T>::on_initialize((block_num - 1).into());

        for i in 0 .. b {
            let asset_a = i * 1_000;
            let asset_a = i * 2_000;
            PriceOracle::<T>::on_create_pool(asset_a, asset_b)?;
        }

        PriceOracle::<T>::on_finalize((block_num - 1).into());
        frame_system::Pallet::<T>::set_block_number(block_num.into());
        PriceOracle::<T>::on_initialize((block_num).into());

        for i in 0 .. b {
            let asset_a = i * 1_000;
            let asset_a = i * 2_000;
            PriceOracle::<T>::on_trade(asset_a, asset_b, ORACLE_ENTRY_1);
            vec.push(ORACLE_ENTRY_1);
        }

        let price_data = PriceOracle::<T>::price_data_ten();
        let bucket_queue = price_data.iter().find(|&x| x.0 == PriceOracle::<T>::get_name(asset_a, asset_b)).unwrap().1;
        assert_eq!(bucket_queue.get_last(), PriceInfo{ avg_price: Price::zero(), volume: 0});

    }: { PriceOracle::<T>::on_finalize(block_num.into()); }
    verify {
        for i in 0 .. b {
            let asset_a = i * 1_000;
            let asset_a = i * 2_000;
            let asset_pair_name = PriceOracle::<T>::get_name(asset_a, asset_b);
            assert!(!PriceDataAccumulator::<T>::contains_key(asset_pair_name.clone()));
            let price_data = PriceOracle::<T>::price_data_ten();
            let bucket_queue = price_data.iter().find(|&x| x.0 == asset_pair_name).unwrap().1;
            assert_eq!(bucket_queue.get_last(), PriceInfo{ avg_price: Price::from(2), volume: 1_000});
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{new_test_ext, Test};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        new_test_ext().execute_with(|| {
            assert_ok!(Pallet::<Test>::test_benchmark_on_finalize_no_entry());
            assert_ok!(Pallet::<Test>::test_benchmark_on_finalize_one_token());
            assert_ok!(Pallet::<Test>::test_benchmark_on_finalize_multiple_tokens_all_bucket_levels());
            assert_ok!(Pallet::<Test>::test_benchmark_on_finalize_multiple_tokens());
        });
    }
}
