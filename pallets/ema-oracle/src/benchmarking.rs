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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

pub const HDX: AssetId = 1_000;
pub const DOT: AssetId = 2_000;

use frame_benchmarking::benchmarks;
use frame_support::{assert_ok, traits::Hooks};

#[cfg(test)]
use pretty_assertions::assert_eq;

use crate::Pallet as EmaOracle;

/// Default oracle source.
const SOURCE: Source = *b"dummysrc";

benchmarks! {
	on_finalize_no_entry {
		let block_num: u32 = 5;
	}: { EmaOracle::<T>::on_finalize(block_num.into()); }
	verify {
	}

	#[extra]
	on_finalize_insert_one_token {
		let block_num: BlockNumberFor<T> = 5u32.into();
		let prev_block = block_num.saturating_sub(One::one());

		frame_system::Pallet::<T>::set_block_number(prev_block);
		EmaOracle::<T>::on_initialize(prev_block);
		EmaOracle::<T>::on_finalize(prev_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);

		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		assert_ok!(OnActivityHandler::<T>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			Price::new(liquidity_asset_in, liquidity_asset_out)));
		let entry = OracleEntry {
			price: Price::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		assert_eq!(Accumulator::<T>::get().into_inner(), [((SOURCE, ordered_pair(HDX, DOT)), entry.clone())].into_iter().collect());

	}: { EmaOracle::<T>::on_finalize(block_num); }
	verify {
		assert!(Accumulator::<T>::get().is_empty());
		assert_eq!(Oracles::<T>::get((SOURCE, ordered_pair(HDX, DOT), LastBlock)).unwrap(), (entry, block_num));
	}

	#[extra]
	on_finalize_update_one_token {
		let initial_data_block: BlockNumberFor<T> = 5u32.into();
		// higher update time difference might make exponentiation more expensive
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		frame_system::Pallet::<T>::set_block_number(initial_data_block);
		EmaOracle::<T>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		assert_ok!(OnActivityHandler::<T>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			Price::new(liquidity_asset_in, liquidity_asset_out)));
		EmaOracle::<T>::on_finalize(initial_data_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);

		assert_ok!(OnActivityHandler::<T>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			Price::new(liquidity_asset_in, liquidity_asset_out)));
		let entry = OracleEntry {
			price: Price::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		assert_eq!(Accumulator::<T>::get().into_inner(), [((SOURCE, ordered_pair(HDX, DOT)), entry.clone())].into_iter().collect());

	}: { EmaOracle::<T>::on_finalize(block_num); }
	verify {
		assert!(Accumulator::<T>::get().is_empty());
		assert_eq!(Oracles::<T>::get((SOURCE, ordered_pair(HDX, DOT), LastBlock)).unwrap(), (entry, initial_data_block));
	}

	on_finalize_multiple_tokens {
		let b in 1 .. (T::MaxUniqueEntries::get() - 1);

		let initial_data_block: BlockNumberFor<T> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		frame_system::Pallet::<T>::set_block_number(initial_data_block);
		EmaOracle::<T>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				Price::new(liquidity_asset_in, liquidity_asset_out)));
		}
		EmaOracle::<T>::on_finalize(initial_data_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				Price::new(liquidity_asset_in, liquidity_asset_out)));
		}
	}: { EmaOracle::<T>::on_finalize(block_num); }
	verify {
		let entry = OracleEntry {
			price: Price::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_eq!(Oracles::<T>::get((SOURCE, ordered_pair(asset_a, asset_b), LastBlock)).unwrap(), (entry.clone(), initial_data_block));
		}
	}

	on_trade_multiple_tokens {
		let b in 1 .. (T::MaxUniqueEntries::get() - 1);

		let initial_data_block: BlockNumberFor<T> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		let mut entries = Vec::new();

		frame_system::Pallet::<T>::set_block_number(initial_data_block);
		EmaOracle::<T>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				Price::new(liquidity_asset_in, liquidity_asset_out)));
		}
		EmaOracle::<T>::on_finalize(initial_data_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);
		let entry = OracleEntry {
			price: Price::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				Price::new(liquidity_asset_in, liquidity_asset_out)));
			entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));
		}
		let asset_a = b * 1_000;
		let asset_b = asset_a + 500;

		let res = core::cell::RefCell::new(Err(DispatchError::Other("Not initialized")));
	}: {
		let _ = res.replace(
			OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				Price::new(liquidity_asset_in, liquidity_asset_out))
				.map_err(|(_w, e)| e)
		);
	}
	verify {
		assert_ok!(*res.borrow());
		entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));

		assert_eq!(Accumulator::<T>::get().into_inner(), entries.into_iter().collect());
	}

	on_liquidity_changed_multiple_tokens {
		let b in 1 .. (T::MaxUniqueEntries::get() - 1);

		let initial_data_block: BlockNumberFor<T> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		let mut entries = Vec::new();

		frame_system::Pallet::<T>::set_block_number(initial_data_block);
		EmaOracle::<T>::on_initialize(initial_data_block);
		let (amount_a, amount_b) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_a, liquidity_asset_b) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				Price::new(liquidity_asset_a, liquidity_asset_b)));
		}
		EmaOracle::<T>::on_finalize(initial_data_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);
		let entry = OracleEntry {
			price: Price::from((liquidity_asset_a, liquidity_asset_b)),
			volume: Volume::from_a_in_b_out(amount_a, amount_b),
			liquidity: Liquidity::new(liquidity_asset_a, liquidity_asset_b),
			updated_at: block_num,
		};
		for i in 0 .. b {
			let asset_a = i * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<T>::on_trade(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				Price::new(liquidity_asset_a, liquidity_asset_b)));
			entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));
		}
		let asset_a = b * 1_000;
		let asset_b = asset_a + 500;

		let res = core::cell::RefCell::new(Err(DispatchError::Other("Not initialized")));
	}: {
		let _ = res.replace(
			OnActivityHandler::<T>::on_liquidity_changed(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				Price::new(liquidity_asset_a, liquidity_asset_b))
				.map_err(|(_w, e)| e)
		);
	}
	verify {
		assert_ok!(*res.borrow());
		let liquidity_entry = OracleEntry {
			price: Price::from((liquidity_asset_a, liquidity_asset_b)),
			volume: Volume::default(),
			liquidity: Liquidity::new(liquidity_asset_a, liquidity_asset_b),
			updated_at: block_num,
		};
		entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), liquidity_entry));

		assert_eq!(Accumulator::<T>::get().into_inner(), entries.into_iter().collect());
	}

	get_entry {
		let initial_data_block: BlockNumberFor<T> = 5u32.into();
		let oracle_age: BlockNumberFor<T> = 999_999u32.into();
		let block_num = initial_data_block.saturating_add(oracle_age.saturating_add(One::one()));

		frame_system::Pallet::<T>::set_block_number(initial_data_block);
		EmaOracle::<T>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		let asset_a = 1_000;
		let asset_b = asset_a + 500;
		assert_ok!(OnActivityHandler::<T>::on_trade(
			SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			Price::new(liquidity_asset_in, liquidity_asset_out)));
		EmaOracle::<T>::on_finalize(initial_data_block);

		frame_system::Pallet::<T>::set_block_number(block_num);
		EmaOracle::<T>::on_initialize(block_num);

		let res = core::cell::RefCell::new(Err(OracleError::NotPresent));

		// aim to find a period that is not `LastBlock`, falling back to `LastBlock` if none is found.
		let period = T::SupportedPeriods::get().into_iter().find(|p| p != &LastBlock).unwrap_or(LastBlock);

	}: { let _ = res.replace(EmaOracle::<T>::get_entry(asset_a, asset_b, period, SOURCE)); }
	verify {
		assert_eq!(*res.borrow(), Ok(AggregatedEntry {
			price: Price::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::default(),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			oracle_age,
		}));
	}

	impl_benchmark_test_suite!(Pallet, crate::tests::new_test_ext(), crate::tests::Test);
}
