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

use crate as exchange;

use crate::Config;
use frame_support::parameter_types;
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use pallet_amm as amm;

use frame_support::traits::GenesisBuild;
use pallet_amm::AssetPairAccountIdFor;
use primitives::{fee, AssetId, Balance};

pub type Amount = i128;
pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const DAVE: AccountId = 4;
pub const FERDIE: AccountId = 5;
pub const GEORGE: AccountId = 6;

pub const HDX: AssetId = 1000;
pub const DOT: AssetId = 2000;
pub const ETH: AssetId = 3000;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		 Exchange: exchange::{Pallet, Call, Storage, Event<T>},
		 AMM: pallet_amm::{Pallet, Call, Storage, Event<T>},
		 Currency: orml_tokens::{Pallet, Event<T>},
		 AssetRegistry: pallet_asset_registry::{Pallet, Storage},
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;

	pub const HDXAssetId: AssetId = HDX;

	pub ExchangeFeeRate: fee::Fee = fee::Fee::default();
}
impl system::Config for Test {
	type BaseCallFilter = ();
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		Zero::zero()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
}

impl pallet_asset_registry::Config for Test {
	type AssetId = AssetId;
}

pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			let tmp = a;
			a = b;
			b = tmp;
		}
		return (a * 1000 + b) as u64;
	}
}

impl amm::Config for Test {
	type Event = Event;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type NativeAssetId = HDXAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
}

impl Config for Test {
	type Event = Event;
	type AMMPool = AMM;
	type Currency = Currency;
	type Resolver = exchange::Pallet<Test>;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1000_000_000_000_000u128),
				(BOB, HDX, 1000_000_000_000_000u128),
				(CHARLIE, HDX, 1000_000_000_000_000u128),
				(DAVE, HDX, 1000_000_000_000_000u128),
				(FERDIE, HDX, 1000_000_000_000_000u128),
				(GEORGE, HDX, 1000_000_000_000_000u128),
				(ALICE, ETH, 1000_000_000_000_000u128),
				(BOB, ETH, 1000_000_000_000_000u128),
				(CHARLIE, ETH, 1000_000_000_000_000u128),
				(DAVE, ETH, 1000_000_000_000_000u128),
				(FERDIE, ETH, 1000_000_000_000_000u128),
				(GEORGE, ETH, 1000_000_000_000_000u128),
				(ALICE, DOT, 1000_000_000_000_000u128),
				(BOB, DOT, 1000_000_000_000_000u128),
				(CHARLIE, DOT, 1000_000_000_000_000u128),
				(DAVE, DOT, 1000_000_000_000_000u128),
				(FERDIE, DOT, 1000_000_000_000_000u128),
				(GEORGE, DOT, 1000_000_000_000_000u128),
			],
		}
	}
}

impl ExtBuilder {
	// builds genesis config

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			endowed_accounts: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
