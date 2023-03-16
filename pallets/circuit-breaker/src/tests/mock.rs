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

pub use crate as pallet_circuit_breaker;
pub use frame_support::traits::{Everything, OnFinalize};
pub use frame_support::{assert_noop, assert_ok, parameter_types};
use frame_system::EnsureRoot;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use std::cell::RefCell;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type AssetId = u32;
pub type Balance = u128;

pub const ALICE: u64 = 1;

pub const HDX: AssetId = 100;
pub const DOT: AssetId = 200;
pub const LRNA: AssetId = 300;
pub const INITIAL_LIQUIDITY: Balance = 1_000_000;

thread_local! {
	pub static MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK: RefCell<(u32, u32)> = RefCell::new((2_000, 10_000)); // 20%
	pub static MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK: RefCell<Option<(u32, u32)>> = RefCell::new(Some((4_000, 10_000))); // 40%
	pub static MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK: RefCell<Option<(u32, u32)>> = RefCell::new(Some((2_000, 10_000))); // 20%
}

frame_support::construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		CircuitBreaker: pallet_circuit_breaker,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
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
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub DefaultMaxNetTradeVolumeLimitPerBlock: (u32, u32) = MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub DefaultMaxAddLiquidityLimitPerBlock: Option<(u32, u32)> = MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub DefaultMaxRemoveLiquidityLimitPerBlock: Option<(u32, u32)> = MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub const OmnipoolHubAsset: AssetId = LRNA;
}

impl pallet_circuit_breaker::Config for Test {
	type Event = Event;
	type AssetId = AssetId;
	type Balance = Balance;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	//type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type DefaultMaxNetTradeVolumeLimitPerBlock = DefaultMaxNetTradeVolumeLimitPerBlock;
	type DefaultMaxAddLiquidityLimitPerBlock = DefaultMaxAddLiquidityLimitPerBlock;
	type DefaultMaxRemoveLiquidityLimitPerBlock = DefaultMaxRemoveLiquidityLimitPerBlock;
	type OmnipoolHubAsset = OmnipoolHubAsset;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	max_net_trade_volume_limit_per_block: (u32, u32),
	max_add_liquidity_limit_per_block: Option<(u32, u32)>,
	max_remove_liquidity_limit_per_block: Option<(u32, u32)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			max_net_trade_volume_limit_per_block: (2_000, 10_000),
			max_add_liquidity_limit_per_block: Some((4_000, 10_000)),
			max_remove_liquidity_limit_per_block: Some((2_000, 10_000)),
		}
	}
}

impl ExtBuilder {
	pub fn with_max_trade_volume_limit_per_block(mut self, value: (u32, u32)) -> Self {
		self.max_net_trade_volume_limit_per_block = value;
		self
	}

	pub fn with_max_add_liquidity_limit_per_block(mut self, value: Option<(u32, u32)>) -> Self {
		self.max_add_liquidity_limit_per_block = value;
		self
	}

	pub fn with_max_remove_liquidity_limit_per_block(mut self, value: Option<(u32, u32)>) -> Self {
		self.max_remove_liquidity_limit_per_block = value;
		self
	}

	// builds genesis config
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_net_trade_volume_limit_per_block;
		});
		MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_add_liquidity_limit_per_block;
		});
		MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_remove_liquidity_limit_per_block;
		});

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

pub fn expect_events(e: Vec<Event>) {
	test_utils::expect_events::<Event, Test>(e);
}
