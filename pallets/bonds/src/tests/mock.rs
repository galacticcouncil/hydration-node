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
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	},
	traits::{ConstU32, ConstU64, Everything, GenesisBuild},
};
use frame_system::{EnsureRoot, EnsureSigned};
use sp_core::H256;
use std::{cell::RefCell, collections::HashMap};

use hydradx_traits::{BondRegistry, Registry};
use orml_traits::parameter_type_with_key;
pub use primitives::constants::{
	currency::NATIVE_EXISTENTIAL_DEPOSIT,
	time::{
		unix_time::{DAY, MONTH, WEEK},
		SLOT_DURATION,
	},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;
pub const SHARE: AssetId = 2;
pub const BOND: AssetId = 3;

pub const ONE: Balance = 1_000_000_000_000;
pub const INITIAL_BALANCE: Balance = 1_000 * ONE;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const TREASURY: AccountId = 400;

pub const NOW: Moment = 1689844300000; // unix time in milliseconds

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, Balance>> = RefCell::new(HashMap::default());
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
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
	pub const MinMaturity: Moment = WEEK;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |asset_id: AssetId| -> Balance {
		REGISTERED_ASSETS.with(|v| v.borrow().get(asset_id).cloned()).unwrap_or(Balance::MAX)
	};
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type Currency = Tokens;
	type AssetRegistry = DummyRegistry<Test>;
	type ExistentialDeposits = ExistentialDeposits;
	type TimestampProvider = Timestamp;
	type PalletId = BondsPalletId;
	type MinMaturity = MinMaturity;
	type IssueOrigin = EnsureSigned<AccountId>;
	type UnlockOrigin = EnsureRoot<AccountId>;
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
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
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
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
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

impl<T: Config> Registry<AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T> {
	fn exists(asset_id: AssetId) -> bool {
		REGISTERED_ASSETS.with(|v| v.borrow().contains_key(&asset_id))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<AssetId, DispatchError> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn create_asset(_name: &Vec<u8>, existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, existential_deposit);
			l as u32
		});
		Ok(assigned)
	}
}

impl<T: Config> BondRegistry<AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	fn create_bond_asset(existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, existential_deposit);
			l as u32
		});
		Ok(assigned)
	}
}

// pub struct DummyTimestampProvider<T>(sp_std::marker::PhantomData<T>);
//
// impl<T: Config> Time for DummyTimestampProvider<T>
// where
// 	<<T as frame_system::Config>::BlockNumber as TryInto<u64>>::Error: std::fmt::Debug,
// {
// 	type Moment = Moment;
//
// 	fn now() -> Self::Moment {
// 		TryInto::<Moment>::try_into(frame_system::Pallet::<T>::block_number())
// 			.unwrap()
// 			.checked_add(NOW)
// 			.unwrap()
// 	}
// }

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(AssetId, Balance)>,
	protocol_fee: Permill,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});

		Self {
			endowed_accounts: vec![(ALICE, HDX, 1_000 * ONE)],
			registered_assets: vec![(HDX, NATIVE_EXISTENTIAL_DEPOSIT)],
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
	pub fn with_registered_asset(mut self, asset: AssetId, ed: Balance) -> Self {
		self.registered_assets.push((asset, ed));
		self
	}
	pub fn with_protocol_fee(mut self, fee: Permill) -> Self {
		self.protocol_fee = fee;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		REGISTERED_ASSETS.with(|v| {
			self.registered_assets.iter().for_each(|(asset, existential_deposit)| {
				v.borrow_mut().insert(*asset, *existential_deposit);
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
