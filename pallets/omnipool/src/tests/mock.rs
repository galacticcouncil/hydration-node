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

use crate::*;
use std::cell::RefCell;
use std::collections::HashMap;

use crate as pallet_omnipool;

use frame_support::traits::{ConstU128, Everything, GenesisBuild};
use frame_support::{
	assert_ok, construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
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

pub const DEFAULT_WEIGHT_CAP: u128 = 1_000_000_000_000_000_000;

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
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

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const DAIAssetId: AssetId = DAI;
	pub const PosiitionClassId: u32= 1000;

	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;
}

impl Config for Test {
	type Event = Event;
	type AssetId = AssetId;
	type PositionInstanceId = u32;
	type Currency = Tokens;
	type AddTokenOrigin = EnsureRoot<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
	type StableCoinAssetId = DAIAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTClassId = PosiitionClassId;
	type NFTHandler = DummyNFT;
	type TVLCap = TVLCap;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	asset_fee: Permill,
	protocol_fee: Permill,
	asset_weight_cap: Permill,
	min_liquidity: u128,
	min_trade_limit: u128,
	init_pool: Option<(FixedU128, FixedU128)>,
	pool_tokens: Vec<(AssetId, FixedU128, AccountId, Balance)>,
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
			*v.borrow_mut() = Permill::from_percent(100);
		});
		ASSET_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
		});
		PROTOCOL_FEE.with(|v| {
			*v.borrow_mut() = Permill::from_percent(0);
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
			],
			asset_fee: Permill::from_percent(0),
			protocol_fee: Permill::from_percent(0),
			asset_weight_cap: Permill::from_percent(100),
			min_liquidity: 0,
			registered_assets: vec![],
			min_trade_limit: 0,
			init_pool: None,
			pool_tokens: vec![],
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

	pub fn with_asset_weight_cap(mut self, cap: Permill) -> Self {
		self.asset_weight_cap = cap;
		self
	}

	pub fn with_asset_fee(mut self, fee: Permill) -> Self {
		self.asset_fee = fee;
		self
	}

	pub fn with_protocol_fee(mut self, fee: Permill) -> Self {
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

	pub fn with_initial_pool(mut self, stable_price: FixedU128, native_price: FixedU128) -> Self {
		self.init_pool = Some((stable_price, native_price));
		self
	}

	pub fn with_token(
		mut self,
		asset_id: AssetId,
		price: FixedU128,
		position_owner: AccountId,
		amount: Balance,
	) -> Self {
		self.pool_tokens.push((asset_id, price, position_owner, amount));
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

		if let Some((stable_price, native_price)) = self.init_pool {
			r.execute_with(|| {
				let stable_amount = Tokens::free_balance(DAI, &Omnipool::protocol_account());
				let native_amount = Tokens::free_balance(HDX, &Omnipool::protocol_account());

				assert_ok!(Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				));
				assert_pool_state_approx!(
					stable_price.checked_mul_int(stable_amount).unwrap() + native_amount,
					FixedU128::from((stable_amount, stable_price.checked_mul_int(stable_amount).unwrap()))
						.checked_mul_int(native_amount)
						.unwrap() + stable_amount,
					SimpleImbalance::default()
				);

				for (asset_id, price, owner, amount) in self.pool_tokens {
					assert_ok!(Tokens::transfer(
						Origin::signed(owner),
						Omnipool::protocol_account(),
						asset_id,
						amount
					));
					assert_ok!(Omnipool::add_token(
						Origin::root(),
						asset_id,
						price,
						self.asset_weight_cap,
						owner
					));
				}
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

impl<T: Config> Registry<T::AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	fn exists(asset_id: T::AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		Ok(T::AssetId::default())
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(T::AssetId::from(assigned))
	}
}

pub(crate) fn get_mock_minted_position(position_id: u32) -> Option<u64> {
	POSITIONS.with(|v| v.borrow().get(&position_id).copied())
}
