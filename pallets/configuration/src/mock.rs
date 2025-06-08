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

use crate as config;
use frame_support::pallet_prelude::Weight;
use frame_support::{
    parameter_types,
    traits::{Everything, Nothing},
    PalletId,
};
use frame_system as system;
use frame_system::EnsureRoot;
use hydradx_traits::{registry::Inspect, AssetKind};
use orml_tokens::AccountData;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
    traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
    BuildStorage, Permill,
};
use std::{cell::RefCell, collections::HashMap};

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 2;
pub const DOGE: AssetId = 333;
pub const REGISTERED_ASSET: AssetId = 1000;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub const TREASURY_INITIAL_BALANCE: Balance = 1_000_000 * ONE;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Configuration: config,
	 }
);

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static EXISTENTIAL_DEPOSIT: RefCell<HashMap<AssetId, u128>>= RefCell::new(HashMap::default());
	pub static PRECISIONS: RefCell<HashMap<AssetId, u32>>= RefCell::new(HashMap::default());
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub ExistentialDepositMultiplier: u8 = 5;
	pub OtcFee: Permill = Permill::from_percent(1u32);
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}


impl config::Config for Test {}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
}

impl system::Config for Test {
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
    type BlockHashCount = BlockHashCount;
    type DbWeight = ();
    type Version = ();
    type PalletInfo = PalletInfo;
    type AccountData = AccountData<u128>;
    type OnNewAccount = ();
    type OnKilledAccount = ();
    type SystemWeightInfo = ();
    type SS58Prefix = SS58Prefix;
    type OnSetCode = ();
    type MaxConsumers = frame_support::traits::ConstU32<16>;
    type SingleBlockMigrations = ();
    type MultiBlockMigrator = ();
    type PreInherents = ();
    type PostInherents = ();
    type PostTransactions = ();
}

pub struct ExtBuilder {
    endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
    registered_assets: Vec<AssetId>,
}

impl Default for ExtBuilder {
    fn default() -> Self {
        // If eg. tests running on one thread only, this thread local is shared.
        // let's make sure that it is empty for each  test case
        // or set to original default value
        REGISTERED_ASSETS.with(|v| {
            v.borrow_mut().clear();
        });
        EXISTENTIAL_DEPOSIT.with(|v| {
            v.borrow_mut().clear();
        });

        Self {
            endowed_accounts: vec![
                (ALICE, HDX, 10_000),
                (BOB, HDX, 10_000),
                (ALICE, DAI, 100),
                (BOB, DAI, 100),
                (TreasuryAccount::get(), HDX, 1_000_000),
            ],
            registered_assets: vec![HDX, DAI],
        }
    }
}

impl ExtBuilder {
    pub fn build(self) -> sp_io::TestExternalities {
        let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();
        let mut r: sp_io::TestExternalities = t.into();

        r.execute_with(|| {
            System::set_block_number(1);
        });
        r
    }
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
    test_utils::expect_events::<RuntimeEvent, Test>(e);
}
