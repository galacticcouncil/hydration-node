// Originally created by Acala. Modified by GalacticCouncil.

// Copyright (C) 2020-2022 Acala Foundation, GalacticCouncil.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

//! Mocks for the transaction pause module.

#![cfg(test)]

use super::*;
use frame_support::{
	construct_runtime, ord_parameter_types, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Everything, Nothing},
};
use frame_system::EnsureSignedBy;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{traits::IdentityLookup, BuildStorage};

pub type AccountId = u128;
pub const ALICE: AccountId = 1;
pub const AUSD: CurrencyId = 2;

pub type Amount = i128;
pub type Balance = u128;
pub type CurrencyId = u32;

mod transaction_pause {
	pub use super::super::*;
}

impl frame_system::Config for Runtime {
	type RuntimeOrigin = RuntimeOrigin;
	type Nonce = u64;
	type Block = Block;
	type RuntimeCall = RuntimeCall;
	type Hash = H256;
	type Hashing = ::sp_runtime::traits::BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type BlockWeights = ();
	type BlockLength = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type DbWeight = ();
	type BaseCallFilter = Everything;
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Runtime {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<10>;
	type AccountStore = System;
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = ();
	type WeightInfo = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: CurrencyId| -> Balance {
		Default::default()
	};
}

parameter_types! {
	pub const MaxReserves: u32 = 50;
}

impl orml_tokens::Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = ();
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

ord_parameter_types! {
	pub const One: AccountId = 1;
}

impl Config for Runtime {
	type RuntimeEvent = RuntimeEvent;
	type UpdateOrigin = EnsureSignedBy<One, AccountId>;
	type WeightInfo = ();
}

type Block = frame_system::mocking::MockBlock<Runtime>;

construct_runtime!(
	pub enum Runtime
	{
		System: frame_system,
		TransactionPause: transaction_pause,
		Balances: pallet_balances,
		Tokens: orml_tokens,
	}
);

pub struct ExtBuilder;

impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let t = frame_system::GenesisConfig::<Runtime>::default()
			.build_storage()
			.unwrap();

		t.into()
	}
}
