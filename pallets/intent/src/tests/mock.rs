// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

//! Test environment for Intent pallet.
#![allow(clippy::type_complexity)]

use crate as pallet_intent;
use crate::Config;
use frame_support::traits::{ConstU32, ConstU64, Everything};
use frame_support::{construct_runtime, parameter_types};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify};
use sp_runtime::{BuildStorage, MultiSignature};

type Block = frame_system::mocking::MockBlock<Test>;

pub type Signature = MultiSignature;
pub type Balance = u128;
pub type AssetId = u32;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Moment = u64;

pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const CHARLIE: AccountId = AccountId::new([3; 32]);

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;
pub const USDC: AssetId = 2;

pub const ONE: Balance = 1_000_000_000_000;

// Mock timestamp provider
use std::cell::RefCell;

thread_local! {
	pub static CURRENT_TIME: RefCell<Moment> = const { RefCell::new(0) };
}

pub struct MockTimestampProvider;

impl frame_support::traits::Time for MockTimestampProvider {
	type Moment = Moment;

	fn now() -> Self::Moment {
		CURRENT_TIME.with(|t| *t.borrow())
	}
}

pub fn set_timestamp(timestamp: Moment) {
	CURRENT_TIME.with(|t| *t.borrow_mut() = timestamp);
}

pub fn advance_time(duration: Moment) {
	CURRENT_TIME.with(|t| *t.borrow_mut() += duration);
}

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Intent: pallet_intent,
	}
);

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

parameter_types! {
	pub const HubAssetId: AssetId = HDX;
	pub const MaxAllowedIntentDuration: Moment = 86_400_000; // 24 hours in milliseconds
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type TimestampProvider = MockTimestampProvider;
	type HubAssetId = HubAssetId;
	type MaxAllowedIntentDuration = MaxAllowedIntentDuration;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	initial_timestamp: Moment,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// Clear thread-local storage for each test
		CURRENT_TIME.with(|t| {
			*t.borrow_mut() = 0;
		});

		Self {
			initial_timestamp: 1_000_000, // Default starting timestamp
		}
	}
}

impl ExtBuilder {
	pub fn with_timestamp(mut self, timestamp: Moment) -> Self {
		self.initial_timestamp = timestamp;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			System::set_block_number(1);
			set_timestamp(self.initial_timestamp);
		});
		ext
	}
}
