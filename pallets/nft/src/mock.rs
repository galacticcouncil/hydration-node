// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use crate as pallet_nft;

use frame_support::traits::{AsEnsureOriginWithArg, Everything};
use frame_support::{parameter_types, weights::Weight};
use frame_system::EnsureRoot;
use sp_core::{crypto::AccountId32, H256};
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, Perbill,
};

mod nfc {
	// Re-export needed for `impl_outer_event!`.
	pub use super::super::*;
}

type AccountId = AccountId32;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;

// Configure a mock runtime to test the pallet.
frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Uniques: pallet_uniques,
		NFT: pallet_nft,
		Balances: pallet_balances,
	}
);

parameter_types! {
	pub ReserveCollectionIdUpTo: u128 = 999;
}

#[derive(Eq, Copy, PartialEq, Clone)]
pub struct NftTestPermissions;

impl NftPermission<CollectionType> for NftTestPermissions {
	fn can_create(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}

	fn can_mint(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}

	fn can_transfer(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}

	fn can_burn(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}

	fn can_destroy(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}

	fn has_deposit(collection_type: &CollectionType) -> bool {
		matches!(*collection_type, CollectionType::Marketplace)
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_nft::weights::BasiliskWeight<Test>;
	type NftCollectionId = CollectionId;
	type NftItemId = ItemId;
	type CollectionType = CollectionType;
	type Permissions = NftTestPermissions;
	type ReserveCollectionIdUpTo = ReserveCollectionIdUpTo;
}

parameter_types! {
	pub const CollectionDeposit: Balance = 10_000 * BSX; // 1 UNIT deposit to create asset collection
	pub const ItemDeposit: Balance = 100 * BSX; // 1/100 UNIT deposit to create asset item
	pub const KeyLimit: u32 = 32;	// Max 32 bytes per key
	pub const ValueLimit: u32 = 64;	// Max 64 bytes per value
	pub const UniquesMetadataDepositBase: Balance = 1000 * BSX;
	pub const AttributeDepositBase: Balance = 100 * BSX;
	pub const DepositPerByte: Balance = 10 * BSX;
	pub const UniquesStringLimit: u32 = 32;
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = CollectionId;
	type ItemId = ItemId;
	type Currency = Balances;
	type ForceOrigin = EnsureRoot<AccountId>;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = UniquesStringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const MaximumBlockWeight: Weight = Weight::from_parts(1024, 0);
	pub const MaximumBlockLength: u32 = 2 * 1024;
	pub const AvailableBlockRatio: Perbill = Perbill::one();
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
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
	pub const ExistentialDeposit: u64 = 1;
	pub const MaxReserves: u32 = 50;
}
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

pub const ALICE: AccountId = AccountId::new([1u8; 32]);
pub const BOB: AccountId = AccountId::new([2u8; 32]);
pub const CHARLIE: AccountId = AccountId::new([3u8; 32]);
pub const ACCOUNT_WITH_NO_BALANCE: AccountId = AccountId::new([4u8; 32]);
pub const BSX: Balance = 100_000_000_000;
pub const COLLECTION_ID_0: <Test as pallet_uniques::Config>::CollectionId = 1000;
pub const COLLECTION_ID_1: <Test as pallet_uniques::Config>::CollectionId = 1001;
pub const COLLECTION_ID_2: <Test as pallet_uniques::Config>::CollectionId = 1002;
pub const COLLECTION_ID_RESERVED: <Test as pallet_uniques::Config>::CollectionId = 42;
pub const ITEM_ID_0: <Test as pallet_uniques::Config>::ItemId = 0;
pub const ITEM_ID_1: <Test as pallet_uniques::Config>::ItemId = 1;
pub const NON_EXISTING_COLLECTION_ID: <Test as pallet_uniques::Config>::CollectionId = 999;

pub struct ExtBuilder;
impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(ALICE, 200_000 * BSX), (BOB, 150_000 * BSX), (CHARLIE, 15_000 * BSX)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}
