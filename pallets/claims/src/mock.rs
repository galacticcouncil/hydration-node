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

use super::*;
use crate as claims;
use crate::{Config, EthereumAddress};
use frame_support::parameter_types;
use hex_literal::hex;
use primitives::Balance;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage,
};

use frame_support::traits::Everything;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 ClaimsPallet: claims,
		 Balances: pallet_balances,
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub Prefix: &'static [u8] = b"I hereby claim all my xHDX tokens to wallet:";
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Balances;
	type Prefix = Prefix;
	type WeightInfo = ();
	type CurrencyBalance = Balance;
}

pub type AccountId = u64;
pub const ALICE: AccountId = 42;
pub const BOB: AccountId = 43;
pub const CHARLIE: AccountId = 44;

pub const CLAIM_AMOUNT: Balance = 1_000_000_000_000;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	// builds genesis config
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			//NOTE: total issuance must be less than Balance::MAX
			balances: vec![(ALICE, 1), (BOB, 1), (CHARLIE, Balance::MAX - 2)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		claims::GenesisConfig::<Test> {
			claims: vec![(
				// Test seed: "image stomach entry drink rice hen abstract moment nature broken gadget flash"
				// private key (m/44'/60'/0'/0/0) : 0xdd75dd5f4a9e964d1c4cc929768947859a98ae2c08100744878a4b6b6d853cc0
				EthereumAddress(hex!["8202c0af5962b750123ce1a9b12e1c30a4973557"]),
				CLAIM_AMOUNT,
			)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}
