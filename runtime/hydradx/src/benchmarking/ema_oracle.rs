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

use sp_std::sync::Arc;
use codec::Decode;
use super::*;
use pallet_ema_oracle::OnActivityHandler;
pub const ALICE: u64 = 1;
use scale_info::prelude::string::ToString;
use pallet_ema_oracle::ordered_pair;
use hydradx_traits::oracle::OraclePeriod;
use hydradx_traits::AggregatedEntry;
pub const HDX: AssetId = 1_000;
pub const DOT: AssetId = 2_000;
use sp_runtime::{BoundedVec, DispatchError};
use hydradx_traits::OnLiquidityChangedHandler;
use frame_benchmarking::benchmarks;
use frame_support::{assert_ok, dispatch::RawOrigin, traits::Hooks};
use sp_std::boxed::Box;
use hydradx_traits::AggregatedOracle;
#[cfg(test)]
use pretty_assertions::assert_eq;
use sp_core::crypto::AccountId32;

use frame_benchmarking::{account, whitelisted_caller};
use frame_system::pallet_prelude::BlockNumberFor;
use sp_core::{ConstU32, Get};
use hydradx_traits::{Liquidity, OnTradeHandler, Source, Volume};
use orml_benchmarking::runtime_benchmarks;
use hydra_dx_math::ema::EmaPrice;
use pallet_ema_oracle::{Accumulator, OracleEntry};


/// Default oracle source.
const SOURCE: Source = *b"dummysrc";

pub const BITFROST_SOURCE: [u8; 8] = *b"bitfrost";


fn fill_whitelist_storage<T: pallet_ema_oracle::Config>(n: u32) {
    for i in 0..n {
        assert_ok!(EmaOracle::add_oracle(RawOrigin::Root.into(), SOURCE, (HDX, i)));
    }
}
runtime_benchmarks! {
	{ Runtime, pallet_ema_oracle }

	add_oracle {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries - 1);

		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::whitelisted_assets().len(), (max_entries - 1) as usize);

	}: _(RawOrigin::Root, SOURCE, (HDX, DOT))
	verify {
		assert!(pallet_ema_oracle::Pallet::<Runtime>::whitelisted_assets().contains(&(SOURCE, (HDX, DOT))));
	}

	remove_oracle {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries - 1);

		assert_ok!(EmaOracle::add_oracle(RawOrigin::Root.into(), SOURCE, (HDX, DOT)));

		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::whitelisted_assets().len(), max_entries as usize);


	}: _(RawOrigin::Root, SOURCE, (HDX, DOT))
	verify {
		assert!(!pallet_ema_oracle::Pallet::<Runtime>::whitelisted_assets().contains(&(SOURCE, (HDX, DOT))));
	}

	on_finalize_no_entry {
		let block_num: u32 = 5;
	}: { <pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block_num.into()); }
	verify {
	}

	#[extra]
	on_finalize_insert_one_token {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let block_num: BlockNumberFor<Runtime> = 5u32.into();
		let prev_block = block_num.saturating_sub(One::one());

		frame_system::Pallet::<Runtime>::set_block_number(prev_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(prev_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(prev_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);

		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);

		register_asset_with_id(b"AS1".to_vec(), HDX).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		register_asset_with_id(b"AS2".to_vec(), DOT).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		assert_ok!(OnActivityHandler::<Runtime>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		let entry = OracleEntry {
			price: EmaPrice::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		assert_eq!(Accumulator::<Runtime>::get().into_inner(), [((SOURCE, pallet_ema_oracle::ordered_pair(HDX, DOT)), entry.clone())].into_iter().collect());

	}: { <pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block_num); }
	verify {
		assert!(Accumulator::<Runtime>::get().is_empty());
		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::oracle((SOURCE, pallet_ema_oracle::ordered_pair(HDX, DOT), hydradx_traits::oracle::OraclePeriod::LastBlock)).unwrap(), (entry, block_num));
	}

	#[extra]
	on_finalize_update_one_token {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		// higher update time difference might make exponentiation more expensive
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);

		register_asset_with_id(b"AS1".to_vec(), HDX).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		register_asset_with_id(b"AS2".to_vec(), DOT).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		assert_ok!(OnActivityHandler::<Runtime>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);

		assert_ok!(OnActivityHandler::<Runtime>::on_trade(
			SOURCE, HDX, DOT, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		let entry = OracleEntry {
			price: EmaPrice::new(liquidity_asset_in, liquidity_asset_out),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		assert_eq!(Accumulator::<Runtime>::get().into_inner(), [((SOURCE, ordered_pair(HDX, DOT)), entry.clone())].into_iter().collect());

	}: { <pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block_num); }
	verify {
		assert!(Accumulator::<Runtime>::get().is_empty());
		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::oracle((SOURCE, ordered_pair(HDX, DOT), OraclePeriod::LastBlock)).unwrap(), (entry, initial_data_block));
	}

	on_finalize_multiple_tokens {
		let b in 1 .. (<<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get() - 1);

		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;
			register_asset_with_id([b"AS1", asset_a.to_string().as_bytes()].concat(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
			register_asset_with_id([b"AS2", asset_b.to_string().as_bytes()].concat(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		}
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		}
	}: { <pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(block_num); }
	verify {
		let entry = OracleEntry {
			price: EmaPrice::new(liquidity_asset_in, liquidity_asset_out),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};

		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;
			assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::oracle((SOURCE, ordered_pair(asset_a, asset_b), OraclePeriod::LastBlock)).unwrap(), (entry.clone(), initial_data_block));
		}
	}

	on_trade_multiple_tokens {
		let b in 1 .. (<<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get() - 1);

		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		let mut entries = Vec::new();

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;

			register_asset_with_id([b"AS1", asset_a.to_string().as_bytes()].concat(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
			register_asset_with_id([b"AS2", asset_b.to_string().as_bytes()].concat(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		}
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);
		let entry = OracleEntry {
			price: EmaPrice::new(liquidity_asset_in, liquidity_asset_out),
			volume: Volume::from_a_in_b_out(amount_in, amount_out),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			updated_at: block_num,
		};
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
			entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));
		}
		let asset_a = (b + 1) * 1_000;
		let asset_b = asset_a + 500;
		register_asset_with_id([b"AS1", asset_a.to_string().as_bytes()].concat(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		register_asset_with_id([b"AS2", asset_b.to_string().as_bytes()].concat(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let res = core::cell::RefCell::new(Err(DispatchError::Other("Not initialized")));
	}: {
		let _ = res.replace(
			OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
				EmaPrice::new(liquidity_asset_in, liquidity_asset_out))
				.map_err(|(_w, e)| e)
		);
	}
	verify {
		assert_ok!(*res.borrow());
		entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));

		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::accumulator().into_inner(), entries.into_iter().collect());
	}

	on_liquidity_changed_multiple_tokens {
		let b in 1 .. (<<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get() - 1);
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		let block_num = initial_data_block.saturating_add(1_000_000u32.into());

		let mut entries = Vec::new();

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let (amount_a, amount_b) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_a, liquidity_asset_b) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;

			register_asset_with_id([b"AS1", asset_a.to_string().as_bytes()].concat(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
			register_asset_with_id([b"AS2", asset_b.to_string().as_bytes()].concat(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				EmaPrice::new(liquidity_asset_a, liquidity_asset_b)));
		}
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);
		let entry = OracleEntry {
			price: EmaPrice::new(liquidity_asset_a, liquidity_asset_b),
			volume: Volume::from_a_in_b_out(amount_a, amount_b),
			liquidity: Liquidity::new(liquidity_asset_a, liquidity_asset_b),
			updated_at: block_num,
		};
		for i in 0 .. b {
			let asset_a = (i + 1) * 1_000;
			let asset_b = asset_a + 500;
			assert_ok!(OnActivityHandler::<Runtime>::on_trade(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				EmaPrice::new(liquidity_asset_a, liquidity_asset_b)));
			entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), entry.clone()));
		}
		let asset_a = (b + 1) * 1_000;
		let asset_b = asset_a + 500;
			register_asset_with_id([b"AS1", asset_a.to_string().as_bytes()].concat(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
			register_asset_with_id([b"AS2", asset_b.to_string().as_bytes()].concat(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let res = core::cell::RefCell::new(Err(DispatchError::Other("Not initialized")));
	}: {
		let _ = res.replace(
			OnActivityHandler::<Runtime>::on_liquidity_changed(
				SOURCE, asset_a, asset_b, amount_a, amount_b, liquidity_asset_a, liquidity_asset_b,
				EmaPrice::new(liquidity_asset_a, liquidity_asset_b))
				.map_err(|(_w, e)| e)
		);
	}
	verify {
		assert_ok!(*res.borrow());
		let liquidity_entry = OracleEntry {
			price: EmaPrice::new(liquidity_asset_a, liquidity_asset_b),
			volume: Volume::default(),
			liquidity: Liquidity::new(liquidity_asset_a, liquidity_asset_b),
			updated_at: block_num,
		};
		entries.push(((SOURCE, ordered_pair(asset_a, asset_b)), liquidity_entry));

		assert_eq!(pallet_ema_oracle::Pallet::<Runtime>::accumulator().into_inner(), entries.into_iter().collect());
	}

	get_entry {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		let oracle_age: BlockNumberFor<Runtime> = 999_999u32.into();
		let block_num = initial_data_block.saturating_add(oracle_age.saturating_add(One::one()));

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let (amount_in, amount_out) = (1_000_000_000_000, 2_000_000_000_000);
		let (liquidity_asset_in, liquidity_asset_out) = (1_000_000_000_000_000, 2_000_000_000_000_000);
		let asset_a = 1_000;
		let asset_b = asset_a + 500;

		register_asset_with_id(b"AS1".to_vec(), asset_a).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		register_asset_with_id(b"AS2".to_vec(), asset_b).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		assert_ok!(OnActivityHandler::<Runtime>::on_trade(
			SOURCE, asset_a, asset_b, amount_in, amount_out, liquidity_asset_in, liquidity_asset_out,
			EmaPrice::new(liquidity_asset_in, liquidity_asset_out)));
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);

		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);

		let res = core::cell::RefCell::new(Err(DispatchError::Other("Not initialized")));


		// aim to find a period that is not `LastBlock`, falling back to `LastBlock` if none is found.
		let period = <<Runtime as pallet_ema_oracle::Config>::SupportedPeriods as Get<BoundedVec<OraclePeriod, ConstU32<{pallet_ema_oracle::MAX_PERIODS}>>>>::get().into_iter().find(|p| p != &OraclePeriod::LastBlock).unwrap_or(OraclePeriod::LastBlock);

	}: {
		let entry = <pallet_ema_oracle::Pallet<Runtime> as AggregatedOracle<AssetId, Balance, BlockNumberFor<Runtime>, EmaPrice>>::get_entry(asset_a, asset_b, period, SOURCE).map_err(|e| DispatchError::Other("Oracle Error"));
		let _ = res.replace(entry);
	}
	verify {
		assert_eq!(*res.borrow(), Ok(AggregatedEntry {
			price: EmaPrice::from((liquidity_asset_in, liquidity_asset_out)),
			volume: Volume::default(),
			liquidity: Liquidity::new(liquidity_asset_in, liquidity_asset_out),
			oracle_age,
		}));
	}

	update_bifrost_oracle {
		let max_entries = <<Runtime as pallet_ema_oracle::Config>::MaxUniqueEntries as Get<u32>>::get();
		fill_whitelist_storage::<Runtime>(max_entries);

		let initial_data_block: BlockNumberFor<Runtime> = 5u32.into();
		let oracle_age: BlockNumberFor<Runtime> = 999_999u32.into();
		let block_num = initial_data_block.saturating_add(oracle_age.saturating_add(One::one()));

		frame_system::Pallet::<Runtime>::set_block_number(initial_data_block);
        <pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(initial_data_block);
		let asset_a = 0;
		let asset_b = 3;

		let hdx_loc = polkadot_xcm::v4::Location::new(0, polkadot_xcm::v4::Junctions::X1(Arc::new([polkadot_xcm::v4::Junction::GeneralIndex(0)])));
		let dot_loc = polkadot_xcm::v4::Location::new(1, polkadot_xcm::v4::Junctions::X2(Arc::new([polkadot_xcm::v4::Junction::Parachain(1000), polkadot_xcm::v4::Junction::GeneralIndex(0)])));

		let dot_asset_loc = AssetLocation::try_from(dot_loc.clone()).unwrap();

		register_asset_with_id_and_loc(b"AS2".to_vec(), asset_b, dot_asset_loc).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		let asset_a = Box::new(hdx_loc.into_versioned());
		let asset_b = Box::new(dot_loc.into_versioned());

		let account: [u8; 32] = [
			0x70, 0x61, 0x72, 0x61, 0xee, 0x07, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
			0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
		];//TODO: here we can just maybe import BifrostAcc

		let account = Decode::decode(&mut &account[..])
		.expect("infinite length input; no invalid inputs for type; qed");

	}: _(RawOrigin::Signed(account), asset_a, asset_b, (100,99))
	verify {
		//TODO: CONTINUE
		/*<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnFinalize<BlockNumberFor<Runtime>>>::on_finalize(initial_data_block);
		frame_system::Pallet::<Runtime>::set_block_number(block_num);
		<pallet_ema_oracle::Pallet<Runtime> as frame_support::traits::OnInitialize<BlockNumberFor<Runtime>>>::on_initialize(block_num);

		let entry = pallet_ema_oracle::Pallet::<Runtime>::oracle((BITFROST_SOURCE, pallet_ema_oracle::ordered_pair(0, 3), hydradx_traits::oracle::OraclePeriod::Day));
		assert!(entry.is_some());*/
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
