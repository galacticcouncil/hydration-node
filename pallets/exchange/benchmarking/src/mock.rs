// This file is part of Basilisk-node.

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

#![cfg(test)]

use super::*;
use frame_support::parameter_types;
use frame_support::traits::{Everything, GenesisBuild};
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, Zero},
};

use frame_system::EnsureSigned;
use hydradx_traits::AssetPairAccountIdFor;
use primitives::{
	constants::chain::{MAX_IN_RATIO, MAX_OUT_RATIO, MIN_POOL_LIQUIDITY, MIN_TRADING_LIMIT},
	AssetId, Balance,
};

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
				 Exchange: pallet_exchange::{Pallet, Call, Storage, Event<T>},
				 XYKPallet: pallet_xyk::{Pallet, Call, Storage, Event<T>},
				 Currency: orml_tokens::{Pallet, Event<T>},
				 AssetRegistry: pallet_asset_registry::{Pallet, Storage, Event<T>},
		 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const HDXAssetId: AssetId = HDX;
	pub ExchangeFeeRate: (u32, u32) = (2, 1_000);
	pub RegistryStringLimit: u32 = 100;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
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
	type SS58Prefix = ();
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
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
}

pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b)
		}
		(a * 1000 + b) as u64
	}
}

impl pallet_asset_registry::Config for Test {
	type Event = Event;
	type RegistryOrigin = EnsureSigned<AccountId>;
	type AssetId = AssetId;
	type Balance = Balance;
	type AssetNativeLocation = u8;
	type StringLimit = RegistryStringLimit;
	type NativeAssetId = HDXAssetId;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinTradingLimit: Balance = MIN_TRADING_LIMIT;
	pub const MinPoolLiquidity: Balance = MIN_POOL_LIQUIDITY;
	pub const MaxInRatio: u128 = MAX_IN_RATIO;
	pub const MaxOutRatio: u128 = MAX_OUT_RATIO;
}

impl pallet_xyk::Config for Test {
	type Event = Event;
	type AssetRegistry = AssetRegistry;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type NativeAssetId = HDXAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CanCreatePool = pallet_xyk::AllowAllPools;
	type AMMHandler = ();
}

impl pallet_exchange::Config for Test {
	type Event = Event;
	type AMMPool = XYKPallet;
	type Currency = Currency;
	type Resolver = pallet_exchange::Pallet<Test>;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl crate::Config for Test {}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1_000_000_000_000_000u128),
				(BOB, HDX, 1_000_000_000_000_000u128),
				(CHARLIE, HDX, 1_000_000_000_000_000u128),
				(DAVE, HDX, 1_000_000_000_000_000u128),
				(FERDIE, HDX, 1_000_000_000_000_000u128),
				(GEORGE, HDX, 1_000_000_000_000_000u128),
				(ALICE, ETH, 1_000_000_000_000_000u128),
				(BOB, ETH, 1_000_000_000_000_000u128),
				(CHARLIE, ETH, 1_000_000_000_000_000u128),
				(DAVE, ETH, 1_000_000_000_000_000u128),
				(FERDIE, ETH, 1_000_000_000_000_000u128),
				(GEORGE, ETH, 1_000_000_000_000_000u128),
				(ALICE, DOT, 1_000_000_000_000_000u128),
				(BOB, DOT, 1_000_000_000_000_000u128),
				(CHARLIE, DOT, 1_000_000_000_000_000u128),
				(DAVE, DOT, 1_000_000_000_000_000u128),
				(FERDIE, DOT, 1_000_000_000_000_000u128),
				(GEORGE, DOT, 1_000_000_000_000_000u128),
			],
		}
	}
}

impl ExtBuilder {
	// builds genesis config

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| System::set_block_number(1));
	ext
}
