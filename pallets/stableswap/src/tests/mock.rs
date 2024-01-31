// This file is part of Basilisk-node.

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
#![allow(clippy::type_complexity)]

use core::ops::RangeInclusive;
use sp_runtime::DispatchResult;
use sp_std::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::num::NonZeroU16;

use crate as pallet_stableswap;

use crate::Config;

use frame_support::assert_ok;
use frame_support::traits::{Contains, Everything};
use frame_support::weights::Weight;
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
pub use orml_traits::MultiCurrency;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type AssetId = u32;
pub type AccountId = u64;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub const ONE: Balance = 1_000_000_000_000;

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, (u32,u8)>> = RefCell::new(HashMap::default());
	pub static ASSET_IDENTS: RefCell<HashMap<Vec<u8>, u32>> = RefCell::new(HashMap::default());
	pub static POOL_IDS: RefCell<Vec<AssetId>> = RefCell::new(Vec::new());
	pub static DUSTER_WHITELIST: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
	pub static LAST_LIQUDITY_CHANGE_HOOK: RefCell<Option<(AssetId, PoolState<AssetId>)>> = RefCell::new(None);
	pub static LAST_TRADE_HOOK: RefCell<Option<(AssetId, AssetId, AssetId, PoolState<AssetId>)>> = RefCell::new(None);
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Tokens: orml_tokens,
		Stableswap: pallet_stableswap,
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
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
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const DAIAssetId: AssetId = DAI;
	pub const MinimumLiquidity: Balance = 1000;
	pub const MinimumTradingLimit: Balance = 1000;
	pub AmplificationRange: RangeInclusive<NonZeroU16> = RangeInclusive::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(10_000).unwrap());
}

pub struct Whitelist;

impl Contains<AccountId> for Whitelist {
	fn contains(account: &AccountId) -> bool {
		DUSTER_WHITELIST.with(|v| v.borrow().contains(account))
	}
}

impl DustRemovalAccountWhitelist<AccountId> for Whitelist {
	type Error = DispatchError;

	fn add_account(account: &AccountId) -> Result<(), Self::Error> {
		DUSTER_WHITELIST.with(|v| v.borrow_mut().push(*account));
		Ok(())
	}

	fn remove_account(account: &AccountId) -> Result<(), Self::Error> {
		DUSTER_WHITELIST.with(|v| {
			let mut v = v.borrow_mut();

			let idx = v.iter().position(|x| *x == *account).unwrap();
			v.remove(idx);

			Ok(())
		})
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Tokens;
	type ShareAccountId = AccountIdConstructor;
	type AssetInspection = DummyRegistry;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type MinPoolLiquidity = MinimumLiquidity;
	type AmplificationRange = AmplificationRange;
	type MinTradingLimit = MinimumTradingLimit;
	type WeightInfo = ();
	type BlockNumberProvider = System;
	type DustAccountHandler = Whitelist;
	type Hooks = DummyHookAdapter;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = DummyRegistry;
}

pub struct InitialLiquidity {
	pub(crate) account: AccountId,
	pub(crate) assets: Vec<AssetAmount<AssetId>>,
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(Vec<u8>, AssetId, u8)>,
	created_pools: Vec<(AccountId, PoolInfo<AssetId, u64>, InitialLiquidity)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		ASSET_IDENTS.with(|v| {
			v.borrow_mut().clear();
		});
		POOL_IDS.with(|v| {
			v.borrow_mut().clear();
		});
		Self {
			endowed_accounts: vec![],
			registered_assets: vec![],
			created_pools: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_registered_asset(mut self, name: Vec<u8>, asset: AssetId, decimals: u8) -> Self {
		self.registered_assets.push((name, asset, decimals));
		self
	}

	pub fn with_registered_assets(mut self, assets: Vec<(Vec<u8>, AssetId, u8)>) -> Self {
		for (name, asset, decimals) in assets.into_iter() {
			self.registered_assets.push((name, asset, decimals));
		}
		self
	}

	pub fn with_pool(
		mut self,
		who: AccountId,
		pool: PoolInfo<AssetId, u64>,
		initial_liquidity: InitialLiquidity,
	) -> Self {
		self.created_pools.push((who, pool, initial_liquidity));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut all_assets: Vec<(Vec<u8>, AssetId, u8)> =
			vec![(b"DAI".to_vec(), DAI, 12u8), (b"HDX".to_vec(), HDX, 12u8)];
		all_assets.extend(self.registered_assets);

		for (name, asset, decimals) in all_assets.into_iter() {
			REGISTERED_ASSETS.with(|v| {
				v.borrow_mut().insert(asset, (asset, decimals));
			});

			ASSET_IDENTS.with(|v| {
				v.borrow_mut().insert(name, asset);
			})
		}

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
			for (_who, pool, initial_liquid) in self.created_pools {
				let pool_id = retrieve_current_asset_id();
				REGISTERED_ASSETS.with(|v| {
					v.borrow_mut().insert(pool_id, (pool_id, 12));
				});
				ASSET_IDENTS.with(|v| {
					v.borrow_mut().insert(b"main".to_vec(), pool_id);
				});

				assert_ok!(Stableswap::create_pool(
					RuntimeOrigin::root(),
					pool_id,
					pool.assets.clone().into(),
					pool.initial_amplification.get(),
					pool.fee,
				));
				POOL_IDS.with(|v| {
					v.borrow_mut().push(pool_id);
				});

				if initial_liquid.assets.len() as u128 > Balance::zero() {
					assert_ok!(Stableswap::add_liquidity(
						RuntimeOrigin::signed(initial_liquid.account),
						pool_id,
						initial_liquid.assets
					));
				}
			}
		});

		r
	}
}

#[cfg(feature = "runtime-benchmarks")]
use crate::types::BenchmarkHelper;
use crate::types::{AssetAmount, PoolInfo, PoolState, StableswapHooks};
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use hydradx_traits::{AccountIdFor, InspectRegistry};
use sp_runtime::traits::Zero;

pub struct DummyRegistry;

impl InspectRegistry<AssetId> for DummyRegistry {
	fn exists(asset_id: AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&asset_id).copied());
		matches!(asset, Some(_))
	}

	fn decimals(asset_id: AssetId) -> Option<u8> {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&asset_id).copied())?;
		Some(asset.1)
	}

	fn asset_name(_asset_id: AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}

	fn asset_symbol(_asset_id: AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}
}

#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper<AssetId> for DummyRegistry {
	fn register_asset(asset_id: AssetId, decimals: u8) -> DispatchResult {
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(asset_id, (asset_id, decimals));
		});

		Ok(())
	}
}

pub struct AccountIdConstructor;

impl AccountIdFor<u32> for AccountIdConstructor {
	type AccountId = AccountId;

	fn from_assets(asset: &u32, _identifier: Option<&[u8]>) -> Self::AccountId {
		(asset * 1000) as u64
	}

	fn name(asset: &u32, identifier: Option<&[u8]>) -> Vec<u8> {
		let mut buf: Vec<u8> = if let Some(ident) = identifier {
			ident.to_vec()
		} else {
			vec![]
		};
		buf.extend_from_slice(&(asset).to_le_bytes());

		buf
	}
}

pub(crate) fn pool_account(asset: u32) -> AccountId {
	AccountIdConstructor::from_assets(&asset, None)
}

pub(crate) fn retrieve_current_asset_id() -> AssetId {
	REGISTERED_ASSETS.with(|v| v.borrow().len() as AssetId)
}

pub(crate) fn get_pool_id_at(idx: usize) -> AssetId {
	POOL_IDS.with(|v| v.borrow()[idx])
}

pub struct DummyHookAdapter;

impl StableswapHooks<AssetId> for DummyHookAdapter {
	fn on_liquidity_changed(pool_id: AssetId, state: PoolState<AssetId>) -> DispatchResult {
		LAST_LIQUDITY_CHANGE_HOOK.with(|v| {
			*v.borrow_mut() = Some((pool_id, state));
		});

		Ok(())
	}

	fn on_trade(pool_id: AssetId, asset_in: AssetId, asset_out: AssetId, state: PoolState<AssetId>) -> DispatchResult {
		LAST_TRADE_HOOK.with(|v| {
			*v.borrow_mut() = Some((pool_id, asset_in, asset_out, state));
		});

		Ok(())
	}

	fn on_liquidity_changed_weight(_n: usize) -> Weight {
		Weight::zero()
	}

	fn on_trade_weight(_n: usize) -> Weight {
		Weight::zero()
	}
}

pub(crate) fn last_liquidity_changed_hook_state() -> Option<(AssetId, PoolState<AssetId>)> {
	LAST_LIQUDITY_CHANGE_HOOK.with(|v| v.borrow().clone())
}

pub(crate) fn last_trade_hook_state() -> Option<(AssetId, AssetId, AssetId, PoolState<AssetId>)> {
	LAST_TRADE_HOOK.with(|v| v.borrow().clone())
}
