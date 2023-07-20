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

use frame_support::traits::{ConstU128, Everything, GenesisBuild};
use frame_support::{
	construct_runtime, parameter_types,
	traits::ConstU32,
};
use hydradx_traits::{Registry, BondRegistry};
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};
use std::{cell::RefCell, collections::HashMap};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;
pub type AssetDetailsT = AssetDetails<AssetId, Balance, BoundedVec<u8, ConstU32<32>>>;

pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;

pub const ONE: Balance = 1_000_000_000_000;
pub const INITIAL_BALANCE: Balance = 1_000 * ONE;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const TREASURY: AccountId = 400;

pub const NOW: Moment = 1689844300000;
pub const DAY: Moment = 86400000;
pub const WEEK: Moment = 7 * DAY;
pub const MONTH: Moment = 2629743000;

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, AssetDetailsT>> = RefCell::new(HashMap::default());
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
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

parameter_types! {
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type Currency = Tokens;
	type AssetRegistry = DummyRegistry<Test>;
	type TimestampProvider = DummyTimestampProvider<Test>;
	type PalletId = BondsPalletId;
	type MinMaturity = MinMaturity;
	type ProtocolFee = ProtocolFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = ();
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 42;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
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
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
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

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Registry<AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T> {
	fn exists(asset_id: AssetId) -> bool {
		REGISTERED_ASSETS.with(|v| v.borrow().contains_key(&asset_id))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<AssetId, DispatchError> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn create_asset(name: &Vec<u8>, existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let name_b = name.clone().try_into().map_err(|_| DispatchError::Other("AssetRegistryMockError"))?;
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, AssetDetailsT {
				name: name_b,
				asset_type: pallet_asset_registry::AssetType::Token,
				existential_deposit,
				xcm_rate_limit: None,
			});
			l as u32
		});
		Ok(assigned)
	}
}

impl<T: Config> BondRegistry<AssetId, Vec<u8>, Balance, AssetDetailsT, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	fn get_asset_details(asset_id: AssetId) -> Result<AssetDetailsT, DispatchError> {
		let maybe_asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).cloned());
			maybe_asset.ok_or(sp_runtime::DispatchError::Other("AssetRegistryMockError"))
	}

	fn create_bond_asset(name: &Vec<u8>, existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		let name_b = name.clone().try_into().map_err(|_| DispatchError::Other("AssetRegistryMockError"))?;
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, AssetDetailsT {
				name: name_b,
				asset_type: pallet_asset_registry::AssetType::Bond,
				existential_deposit,
				xcm_rate_limit: None,
			});
			l as u32
		});
		Ok(assigned)
	}
}

pub struct DummyTimestampProvider<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Time for DummyTimestampProvider<T>
	where <<T as frame_system::Config>::BlockNumber as TryInto<u64>>::Error: std::fmt::Debug
{
	type Moment = Moment;

	fn now() -> Self::Moment
	{
		TryInto::<Moment>::try_into(frame_system::Pallet::<T>::block_number()).unwrap().checked_add(NOW).unwrap()
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(AssetId, AssetDetailsT)>,
	protocol_fee: Permill,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});

		// Add HDX as pre-registered asset
		let hdx_asset_details = AssetDetailsT {
			name: "HDX".as_bytes().to_vec().try_into().unwrap(),
			asset_type: pallet_asset_registry::AssetType::Token,
			existential_deposit: 1_000,
			xcm_rate_limit: None,
		};

		Self {
			endowed_accounts: vec![(ALICE, HDX, 1_000 * ONE)],
			registered_assets: vec![(HDX, hdx_asset_details)],
			protocol_fee: Permill::from_percent(0),
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}
	pub fn add_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		for entry in accounts {
			self.endowed_accounts.push(entry);
		}
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId, asset_details: AssetDetailsT) -> Self {
		self.registered_assets.push((asset, asset_details));
		self
	}
	pub fn with_protocol_fee(mut self, fee: Permill) -> Self {
		self.protocol_fee = fee;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		REGISTERED_ASSETS.with(|v| {
			self.registered_assets.iter().for_each(|(asset, asset_details)| {
				v.borrow_mut().insert(*asset, asset_details.clone());
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

		r.execute_with(|| System::set_block_number(1));

		r
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub fn next_asset_id() -> AssetId {
		REGISTERED_ASSETS.with(|v| {
			v.borrow().len().try_into().unwrap()
	})
}