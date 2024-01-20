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

use crate as pallet_bonds;
use crate::*;

use frame_support::{
	construct_runtime, parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage,
	},
	traits::{ConstU32, ConstU64, Everything, SortedMembers},
};
use frame_system::EnsureSignedBy;
use sp_core::H256;
use std::{cell::RefCell, collections::HashMap};

use hydradx_traits::CreateRegistry;
use orml_traits::parameter_type_with_key;
pub use primitives::constants::{
	currency::NATIVE_EXISTENTIAL_DEPOSIT,
	time::{
		unix_time::{DAY, MONTH, WEEK},
		SLOT_DURATION,
	},
};

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Balance = u128;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;

pub const ONE: Balance = 1_000_000_000_000;
pub const INITIAL_BALANCE: Balance = 1_000 * ONE;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const TREASURY: AccountId = 400;

pub const NOW: Moment = 1689844300000; // unix time in milliseconds

thread_local! {
	// maps AssetId -> existential deposit
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, (Balance, AssetKind)>> = RefCell::new(HashMap::default());
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Timestamp: pallet_timestamp,
		Tokens: orml_tokens,
		Bonds: pallet_bonds,
	}
);

parameter_types! {
	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub TreasuryAccount: AccountId = TREASURY;
	pub const BondsPalletId: PalletId = PalletId(*b"pltbonds");
}

parameter_type_with_key! {
	pub ExistentialDeposits: |asset_id: AssetId| -> Balance {
		REGISTERED_ASSETS.with(|v| v.borrow().get(asset_id).cloned()).unwrap_or((Balance::MAX, AssetKind::Token)).0
	};
}

pub struct AliceOrBob;
impl SortedMembers<AccountId> for AliceOrBob {
	fn sorted_members() -> Vec<AccountId> {
		vec![ALICE, BOB]
	}
}

pub struct AssetTypeWhitelist;
impl Contains<AssetKind> for AssetTypeWhitelist {
	fn contains(t: &AssetKind) -> bool {
		matches!(t, AssetKind::Token | AssetKind::XYK | AssetKind::StableSwap)
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Currency = Tokens;
	type AssetRegistry = DummyRegistry<Test>;
	type ExistentialDeposits = ExistentialDeposits;
	type TimestampProvider = Timestamp;
	type PalletId = BondsPalletId;
	type IssueOrigin = EnsureSignedBy<AliceOrBob, AccountId>;
	type AssetTypeWhitelist = AssetTypeWhitelist;
	type ProtocolFee = ProtocolFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = ();
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
	type AccountId = u64;
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
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type OnTimestampSet = ();
	type MinimumPeriod = MinimumPeriod;
	type WeightInfo = ();
}

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> CreateRegistry<AssetId, Balance> for DummyRegistry<T> {
	type Error = DispatchError;

	fn create_asset(_name: &[u8], _kind: AssetKind, existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, (existential_deposit, AssetKind::Bond));
			l as u32
		});
		Ok(assigned)
	}
}

impl<T: Config> Registry<AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T> {
	fn exists(_name: AssetId) -> bool {
		unimplemented!()
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<AssetId, DispatchError> {
		unimplemented!()
	}

	fn retrieve_asset_type(asset_id: AssetId) -> Result<AssetKind, DispatchError> {
		REGISTERED_ASSETS
			.with(|v| v.borrow().get(&asset_id).cloned())
			.map(|v| v.1)
			.ok_or(DispatchError::Other("AssetNotFound"))
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		unimplemented!()
	}

	fn get_or_create_asset(_name: Vec<u8>, _existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		unimplemented!()
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(AssetId, (Balance, AssetKind))>,
	protocol_fee: Permill,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});

		Self {
			endowed_accounts: vec![(ALICE, HDX, 1_000 * ONE)],
			registered_assets: vec![(HDX, (NATIVE_EXISTENTIAL_DEPOSIT, AssetKind::Token))],
			protocol_fee: Permill::from_percent(0),
		}
	}
}

impl ExtBuilder {
	pub fn add_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		for entry in accounts {
			self.endowed_accounts.push(entry);
		}
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId, ed: Balance, asset_kind: AssetKind) -> Self {
		self.registered_assets.push((asset, (ed, asset_kind)));
		self
	}
	pub fn with_protocol_fee(mut self, fee: Permill) -> Self {
		self.protocol_fee = fee;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		REGISTERED_ASSETS.with(|v| {
			self.registered_assets.iter().for_each(|(asset, existential_details)| {
				v.borrow_mut().insert(*asset, *existential_details);
			});
		});

		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = self.protocol_fee;
		});

		orml_tokens::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.flat_map(|(x, asset, amount)| vec![(*x, *asset, *amount)])
				.collect(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		r.execute_with(|| {
			System::set_block_number(1);
			Timestamp::set_timestamp(NOW);
		});

		r
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub fn next_asset_id() -> AssetId {
	REGISTERED_ASSETS.with(|v| v.borrow().len().try_into().unwrap())
}
