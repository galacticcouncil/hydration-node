// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for Assets pallet.

use super::*;
use std::cell::RefCell;
use std::collections::HashMap;

use crate as pallet_omnipool;

use frame_support::traits::{ConstU128, Everything, GenesisBuild};
use frame_support::{
	assert_ok, construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::EnsureSigned;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const REGISTERED_ASSET: AssetId = 1000;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const LP3: u64 = 3;

pub const ONE: Balance = 1_000_000_000_000;

pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<(u32,u32)> = RefCell::new((u32::MAX,1));
	pub static ASSET_FEE: RefCell<(u32,u32)> = RefCell::new((0,0));
	pub static PROTOCOL_FEE: RefCell<(u32,u32)> = RefCell::new((0,0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(Balance::default());
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(Balance::default());
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Omnipool: pallet_omnipool::{Pallet, Call, Storage, Event<T>},
		Tokens: orml_tokens::{Pallet, Event<T>},
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
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
	type Event = Event;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
}

struct WeightCap;
impl Get<(u32, u32)> for WeightCap {
	fn get() -> (u32, u32) {
		ASSET_WEIGHT_CAP.with(|v| *v.borrow())
	}
}
struct FeeAsset;
impl Get<(u32, u32)> for FeeAsset {
	fn get() -> (u32, u32) {
		ASSET_FEE.with(|v| *v.borrow())
	}
}
struct FeeProtocol;
impl Get<(u32, u32)> for FeeProtocol {
	fn get() -> (u32, u32) {
		PROTOCOL_FEE.with(|v| *v.borrow())
	}
}
struct MinLiquidity;
impl Get<Balance> for MinLiquidity {
	fn get() -> Balance {
		MIN_ADDED_LIQUDIITY.with(|v| *v.borrow())
	}
}
struct MinAmount;
impl Get<Balance> for MinAmount {
	fn get() -> Balance {
		MIN_TRADE_AMOUNT.with(|v| *v.borrow())
	}
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const DAIAssetId: AssetId = DAI;
	pub const PosiitionClassId: u32= 1000;

	pub ProtocolFee: (u32,u32) = FeeProtocol::get();
	pub AssetFee: (u32,u32) = FeeAsset::get();
	pub AssetWeightCap: (u32,u32) = WeightCap::get();
	pub MinAddedLiquidity: Balance = MinLiquidity::get();
	pub MinTradeAmount: Balance = MinAmount::get();
	pub const TVLCap: Balance = Balance::MAX;
}

impl Config for Test {
	type Event = Event;
	type Balance = Balance;
	type AssetId = AssetId;
	type PositionInstanceId = u32;
	type Currency = Tokens;
	type AddTokenOrigin = EnsureSigned<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
	type StableCoinAssetId = DAIAssetId;
	type WeightInfo = ();
	type NativeAssetId = HDXAssetId;
	type NFTClassId = PosiitionClassId;
	type NFTHandler = DummyNFT;
	type AssetWeightCap = AssetWeightCap;
	type TVLCap = TVLCap;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	asset_fee: (u32, u32),
	protocol_fee: (u32, u32),
	asset_weight_cap: (u32, u32),
	min_liquidity: u128,
	min_trade_limit: u128,
	init_pool: Option<(Balance, Balance, FixedU128, FixedU128)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		POSITIONS.with(|v| {
			v.borrow_mut().clear();
		});
		ASSET_WEIGHT_CAP.with(|v| {
			*v.borrow_mut() = (u32::MAX, 1);
		});
		ASSET_FEE.with(|v| {
			*v.borrow_mut() = (0, 0);
		});
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = (0, 0);
		});
		MIN_ADDED_LIQUDIITY.with(|v| {
			*v.borrow_mut() = 0;
		});
		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = 0;
		});

		Self {
			endowed_accounts: vec![
				(Omnipool::protocol_account(), DAI, 1000 * ONE),
				(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
				(Omnipool::protocol_account(), 1_000, 2000 * ONE),
			],
			asset_fee: (0, 0),
			protocol_fee: (0, 0),
			asset_weight_cap: (u32::MAX, 1),
			min_liquidity: 0,
			registered_assets: vec![],
			min_trade_limit: 0,
			init_pool: None,
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}
	pub fn add_endowed_accounts(mut self, account: (u64, AssetId, Balance)) -> Self {
		self.endowed_accounts.push(account);
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId) -> Self {
		self.registered_assets.push(asset);
		self
	}

	pub fn with_asset_weight_cap(mut self, cap: (u32, u32)) -> Self {
		self.asset_weight_cap = cap;
		self
	}

	pub fn with_asset_fee(mut self, fee: (u32, u32)) -> Self {
		self.asset_fee = fee;
		self
	}

	pub fn with_protocol_fee(mut self, fee: (u32, u32)) -> Self {
		self.protocol_fee = fee;
		self
	}
	pub fn with_min_added_liquidity(mut self, limit: Balance) -> Self {
		self.min_liquidity = limit;
		self
	}

	pub fn with_min_trade_amount(mut self, limit: Balance) -> Self {
		self.min_trade_limit = limit;
		self
	}

	pub fn with_initial_pool(
		mut self,
		stable_amount: Balance,
		native_amount: Balance,
		stable_price: FixedU128,
		native_price: FixedU128,
	) -> Self {
		self.init_pool = Some((stable_amount, native_amount, stable_price, native_price));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		// Add DAi and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(DAI, DAI);
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(REGISTERED_ASSET, REGISTERED_ASSET);
			self.registered_assets.iter().for_each(|asset| {
				v.borrow_mut().insert(*asset, *asset);
			});
		});

		ASSET_FEE.with(|v| {
			*v.borrow_mut() = self.asset_fee;
		});
		ASSET_WEIGHT_CAP.with(|v| {
			*v.borrow_mut() = self.asset_weight_cap;
		});

		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = self.protocol_fee;
		});

		MIN_ADDED_LIQUDIITY.with(|v| {
			*v.borrow_mut() = self.min_liquidity;
		});

		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = self.min_trade_limit;
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

		if let Some((stable_amount, native_amount, stable_price, native_price)) = self.init_pool {
			r.execute_with(|| {
				assert_ok!(Omnipool::initialize_pool(
					Origin::root(),
					stable_amount,
					native_amount,
					stable_price,
					native_price,
				));
				assert_pool_state!(
					stable_price.checked_mul_int(stable_amount).unwrap() + native_amount,
					native_amount * (stable_amount / stable_price.checked_mul_int(stable_amount).unwrap())
						+ stable_amount,
					SimpleImbalance::default()
				);
			});
		}

		r
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
pub struct DummyNFT;

impl<AccountId: From<u64>> Inspect<AccountId> for DummyNFT {
	type InstanceId = u32;
	type ClassId = u32;

	fn owner(_class: &Self::ClassId, instance: &Self::InstanceId) -> Option<AccountId> {
		let mut owner: Option<AccountId> = None;

		POSITIONS.with(|v| {
			if let Some(o) = v.borrow().get(instance) {
				owner = Some((*o).into());
			}
		});
		owner
	}
}

impl<AccountId: From<u64>> Create<AccountId> for DummyNFT {
	fn create_class(_class: &Self::ClassId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u64> + Into<u64> + Copy> Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::ClassId, _instance: &Self::InstanceId, _who: &AccountId) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(*_instance, (*_who).into());
		});
		Ok(())
	}

	fn burn_from(_class: &Self::ClassId, instance: &Self::InstanceId) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.remove(instance);
		});
		Ok(())
	}
}

use hydradx_traits::Registry;

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Registry<T::AssetId, Vec<u8>, T::Balance, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId>,
{
	fn exists(asset_id: T::AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		Ok(T::AssetId::default())
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: T::Balance) -> Result<T::AssetId, DispatchError> {
		Ok(T::AssetId::default())
	}
}

pub(crate) fn get_mock_minted_position(position_id: u32) -> Option<u64> {
	POSITIONS.with(|v| v.borrow().get(&position_id).copied())
}
