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

use crate as pallet_amm_support;
pub use crate::*;

pub use frame_support::{
	assert_err, assert_ok, construct_runtime,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	},
	traits::{ConstU32, ConstU64, Everything},
};
use sp_core::H256;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const DOT: AssetId = 1;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		AmmSupport: pallet_amm_support,
	}
);

impl crate::Config for Test {
	type RuntimeEvent = RuntimeEvent;
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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
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

#[derive(Default)]
pub struct ExtBuilder {}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}
