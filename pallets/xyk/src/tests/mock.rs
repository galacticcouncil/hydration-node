// This file is part of HydraDX-node.

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

use crate as xyk;
use crate::Config;
use crate::*;
use frame_support::parameter_types;
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, One},
	BuildStorage,
};

use crate::types::{AssetId, Balance};
use frame_support::traits::{Everything, Get, Nothing};
use hydradx_traits::{AssetPairAccountIdFor, CanCreatePool, Source};

use frame_system::EnsureSigned;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use std::cell::RefCell;

pub type Amount = i128;
pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;

pub const HDX: AssetId = 1000;
pub const DOT: AssetId = 2000;
pub const ACA: AssetId = 3000;

pub const HDX_DOT_POOL_ID: AccountId = 1_002_000;

pub const ONE: Balance = 1_000_000_000_000;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 XYK: xyk,
		 Currency: orml_tokens,
		 AssetRegistry: pallet_asset_registry,
	 }

);

thread_local! {
		static EXCHANGE_FEE: RefCell<(u32, u32)> = RefCell::new((2, 1_000));
		static DISCOUNTED_FEE: RefCell<(u32, u32)> = RefCell::new((7, 10_000));
		static MAX_OUT_RATIO: RefCell<u128> = RefCell::new(3);
}

struct ExchangeFee;
impl Get<(u32, u32)> for ExchangeFee {
	fn get() -> (u32, u32) {
		EXCHANGE_FEE.with(|v| *v.borrow())
	}
}

struct DiscountedFee;
impl Get<(u32, u32)> for DiscountedFee {
	fn get() -> (u32, u32) {
		DISCOUNTED_FEE.with(|v| *v.borrow())
	}
}

struct MaximumOutRatio;
impl Get<u128> for MaximumOutRatio {
	fn get() -> u128 {
		MAX_OUT_RATIO.with(|v| *v.borrow())
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const NativeAssetId: AssetId = HDX;
	pub RegistryStringLimit: u32 = 100;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureSigned<AccountId>;
	type AssetId = AssetId;
	type Balance = Balance;
	type AssetNativeLocation = u8;
	type StringLimit = RegistryStringLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type NativeAssetId = NativeAssetId;
	type WeightInfo = ();
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		One::one()
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = ();
	type MaxReserves = ();
	type CurrencyHooks = ();
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

parameter_types! {
	pub const MinTradingLimit: Balance = 1_000;
	pub const MinPoolLiquidity: Balance = 1_000;
	pub const MaxInRatio: u128 = 3;
	pub MaxOutRatio: u128 = MaximumOutRatio::get();
	pub ExchangeFeeRate: (u32, u32) = ExchangeFee::get();
	pub DiscountedFeeRate: (u32, u32) = DiscountedFee::get();
	pub const OracleSourceIdentifier: Source = *b"hydraxyk";
}

pub struct Disallow10_10Pool();

impl CanCreatePool<AssetId> for Disallow10_10Pool {
	fn can_create(asset_a: AssetId, asset_b: AssetId) -> bool {
		!matches!((asset_a, asset_b), (10u32, 10u32))
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetRegistry = AssetRegistry;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type NativeAssetId = NativeAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CanCreatePool = Disallow10_10Pool;
	type AMMHandler = ();
	type DiscountedFee = DiscountedFeeRate;
	type NonDustableWhitelistHandler = Whitelist;
	type OracleSource = OracleSourceIdentifier;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

// Returns default values for genesis config
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1_000_000_000_000_000u128),
				(BOB, HDX, 1_000_000_000_000_000u128),
				(ALICE, ACA, 1_000_000_000_000_000u128),
				(BOB, ACA, 1_000_000_000_000_000u128),
				(ALICE, DOT, 1_000_000_000_000_000u128),
				(BOB, DOT, 1_000_000_000_000_000u128),
				(CHARLIE, HDX, 1_000_000_000_000_000u128),
			],
		}
	}
}

impl ExtBuilder {
	// builds genesis config

	pub fn with_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_exchange_fee(self, f: (u32, u32)) -> Self {
		EXCHANGE_FEE.with(|v| *v.borrow_mut() = f);
		self
	}

	pub fn with_discounted_fee(self, f: (u32, u32)) -> Self {
		DISCOUNTED_FEE.with(|v| *v.borrow_mut() = f);
		self
	}

	pub fn with_max_out_ratio(self, f: u128) -> Self {
		MAX_OUT_RATIO.with(|v| *v.borrow_mut() = f);
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub struct Whitelist;

impl DustRemovalAccountWhitelist<AccountId> for Whitelist {
	type Error = DispatchError;

	fn add_account(_account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}

	fn remove_account(_account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}
}
