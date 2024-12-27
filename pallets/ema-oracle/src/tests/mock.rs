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

use crate as ema_oracle;
use crate::Config;
use ema_oracle::OracleEntry;
use frame_support::pallet_prelude::ConstU32;
use frame_support::parameter_types;
use frame_support::sp_runtime::{
	bounded_vec,
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};
use frame_support::traits::{Contains, Everything};
use frame_support::BoundedVec;
use frame_system::EnsureRoot;
use hydradx_traits::OraclePeriod::{self, *};
use hydradx_traits::Source;
use hydradx_traits::{Liquidity, Volume};
use sp_core::H256;

use crate::types::{AssetId, Balance, Price};
pub type BlockNumber = u64;
pub type AccountId = u64;

type Block = frame_system::mocking::MockBlock<Test>;

use crate::MAX_PERIODS;

pub const HDX: AssetId = 1_000;
pub const DOT: AssetId = 2_000;
pub const ACA: AssetId = 3_000;
// ensure this asset id is not used in the benchmarks, otherwise the benchmarking tests fail
pub const INSUFFICIENT_ASSET: AssetId = 123_456;

pub const ORACLE_ENTRY_1: OracleEntry<BlockNumber> = OracleEntry {
	price: Price::new(2_000, 1_000),
	volume: Volume {
		a_in: 1_000,
		b_out: 500,
		a_out: 0,
		b_in: 0,
	},
	liquidity: Liquidity::new(2_000, 1_000),
	updated_at: 5,
};
pub const ORACLE_ENTRY_2: OracleEntry<BlockNumber> = OracleEntry {
	price: Price::new(4_000, 4_000),
	volume: Volume {
		a_in: 0,
		b_out: 0,
		a_out: 2_000,
		b_in: 2_000,
	},
	liquidity: Liquidity::new(4_000, 4_000),
	updated_at: 5,
};

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 EmaOracle: ema_oracle,
	 }

);

parameter_types! {
	pub const BlockHashCount: BlockNumber = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_types! {
	pub SupportedPeriods: BoundedVec<OraclePeriod, ConstU32<MAX_PERIODS>> = bounded_vec![LastBlock, TenMinutes, Day, Week];
}

pub struct OracleWhitelist;
impl Contains<(Source, AssetId, AssetId)> for OracleWhitelist {
	fn contains(t: &(Source, AssetId, AssetId)) -> bool {
		(t.1 != INSUFFICIENT_ASSET && t.2 != INSUFFICIENT_ASSET) || ema_oracle::OracleWhitelist::<Test>::contains(t)
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type BlockNumberProvider = System;
	type SupportedPeriods = SupportedPeriods;
	type OracleWhitelist = OracleWhitelist;
	type MaxUniqueEntries = ConstU32<45>;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
	type WeightInfo = ();
}

pub type InitialDataEntry = (Source, (AssetId, AssetId), Price, Liquidity<Balance>);

#[derive(Default)]
pub struct ExtBuilder {
	pub initial_data: Vec<InitialDataEntry>,
}

impl ExtBuilder {
	pub fn with_initial_data(mut self, data: Vec<InitialDataEntry>) -> Self {
		self.initial_data = data;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
		crate::GenesisConfig::<Test> {
			initial_data: self.initial_data,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
		});
		ext
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}
