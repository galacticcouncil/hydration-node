// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

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

use crate as otc;
use crate::Config;
use frame_support::{
	parameter_types,
	traits::{Everything, Nothing},
};
use frame_system as system;
use hydradx_traits::{AssetKind, Registry};
use orml_tokens::AccountData;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
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

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 OTC: otc,
		 Tokens: orml_tokens,
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
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		EXISTENTIAL_DEPOSIT.with(|v| *v.borrow().get(currency_id).unwrap_or(&ONE))
	};
}

impl Config for Test {
	type AssetId = AssetId;
	type AssetRegistry = DummyRegistry<Test>;
	type Currency = Tokens;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposits = ExistentialDeposits;
	type ExistentialDepositMultiplier = ExistentialDepositMultiplier;
	type WeightInfo = ();
}

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
	type AccountData = AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
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
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Registry<AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T> {
	fn exists(asset_id: AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id)).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<AssetId, DispatchError> {
		Ok(0)
	}

	fn retrieve_asset_type(_asset_id: AssetId) -> Result<AssetKind, DispatchError> {
		unimplemented!()
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(assigned)
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
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
			],
			registered_assets: vec![HDX, DAI],
		}
	}
}

impl ExtBuilder {
	pub fn with_existential_deposit(self, asset_id: AssetId, precision: u32) -> Self {
		EXISTENTIAL_DEPOSIT.with(|v| {
			v.borrow_mut().insert(asset_id, 10u128.pow(precision));
		});
		PRECISIONS.with(|v| {
			v.borrow_mut().insert(asset_id, precision);
		});

		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Add DAI and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(REGISTERED_ASSET, REGISTERED_ASSET);
			self.registered_assets.iter().for_each(|asset| {
				v.borrow_mut().insert(*asset, *asset);
			});
		});

		orml_tokens::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.flat_map(|(x, asset, amount)| vec![(*x, *asset, *amount * 10u128.pow(precision(*asset)))])
				.collect(),
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

thread_local! {
	pub static DUMMYTHREADLOCAL: RefCell<u128> = RefCell::new(100);
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

pub fn precision(asset_id: AssetId) -> u32 {
	PRECISIONS.with(|v| *v.borrow().get(&asset_id).unwrap_or(&12))
}
