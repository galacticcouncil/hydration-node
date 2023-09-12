// This file is part of pallet-asset-registry.

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

#![cfg(test)]

use frame_support::parameter_types;
use frame_system as system;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use frame_support::traits::{Everything, GenesisBuild};

use polkadot_xcm::v3::MultiLocation;

use crate as pallet_asset_registry;

pub type AssetId = u32;
pub type Balance = u128;

pub const UNIT: Balance = 1_000_000_000_000;
pub const ALICE: u64 = 1_000;
pub const TREASURY: u64 = 2_222;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		 Tokens: orml_tokens::{Pallet, Call, Storage, Event<T>},
		 Registry: pallet_asset_registry::{Pallet, Call, Storage, Event<T>},
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const NativeAssetId: AssetId = 0;
	pub const RegistryStringLimit: u32 = 10;
	pub const SequentialIdStart: u32 = 1_000_000;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type RuntimeEvent = RuntimeEvent;
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
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

use codec::{Decode, Encode, MaxEncodedLen};
use scale_info::TypeInfo;

#[derive(Debug, Default, Encode, Decode, Clone, PartialEq, Eq, TypeInfo, MaxEncodedLen)]
pub struct AssetLocation(pub MultiLocation);

parameter_types! {
	pub const StoreFees: Balance = 10 * UNIT;
	pub const FeesBeneficiarry: u64 = TREASURY;
}

impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Tokens;
	type RegistryOrigin = frame_system::EnsureRoot<u64>;
	type UpdateOrigin = frame_system::EnsureSigned<u64>;
	type AssetId = u32;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStringLimit;
	type SequentialIdStartAt = SequentialIdStart;
	type NativeAssetId = NativeAssetId;
	type StorageFees = StoreFees;
	type StorageFeesBeneficiary = FeesBeneficiarry;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

#[derive(Default)]
#[allow(clippy::type_complexity)]
pub struct ExtBuilder {
	registered_assets: Vec<(
		Option<AssetId>,
		Option<Vec<u8>>,
		Balance,
		Option<Vec<u8>>,
		Option<u8>,
		Option<Balance>,
		bool,
	)>,
}

impl ExtBuilder {
	#[allow(clippy::type_complexity)]
	pub fn with_assets(
		mut self,
		assets: Vec<(
			Option<AssetId>,
			Option<Vec<u8>>,
			Balance,
			Option<Vec<u8>>,
			Option<u8>,
			Option<Balance>,
			bool,
		)>,
	) -> Self {
		self.registered_assets = assets;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		crate::GenesisConfig::<Test> {
			registered_assets: self.registered_assets,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();
		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}
