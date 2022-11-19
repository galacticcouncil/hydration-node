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

//! Test environment for Omnipool-subpools pallet.

use crate::*;
use std::cell::RefCell;
use std::collections::HashMap;

use crate as pallet_omnipool_subpools;
use crate::Config;

use core::ops::RangeInclusive;
use frame_support::traits::{ConstU128, Everything, GenesisBuild};
use frame_support::{
	assert_ok, construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::{EnsureRoot, EnsureSigned};
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrency;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
	FixedU128, Permill,
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

use pretty_assertions::*;

pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;

pub const ALICE: AccountId = 1000;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const ASSET_3: AssetId = 3;
pub const ASSET_4: AssetId = 4;
pub const ASSET_5: AssetId = 5;
pub const ASSET_6: AssetId = 6;
pub const ASSET_7: AssetId = 7;
pub const ASSET_8: AssetId = 8;

pub const SHARE_ASSET_AS_POOL_ID: AssetId = 500;
pub const SHARE_ASSET_AS_POOL_ID_2: AssetId = 501;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const LP3: u64 = 3;

pub const ONE: Balance = 1_000_000_000_000;

pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

pub const DEFAULT_WEIGHT_CAP: u128 = 1_000_000_000_000_000_000;

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Balances: pallet_balances,
		Omnipool: pallet_omnipool,
		Stableswap: pallet_stableswap,
		OmnipoolSubpools: pallet_omnipool_subpools,
		Tokens: orml_tokens,
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
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub const NativeAssetId: AssetId = HDX;
	pub RegistryStringLimit: u32 = 100;
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
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;
	pub const AmplificationRange: RangeInclusive<u16> = RangeInclusive::new(2, 10_000);
}

use hydradx_traits::{Registry, ShareTokenRegistry};
use sp_runtime::traits::Zero;

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<TAssetId> Registry<TAssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<TAssetId>
where
	TAssetId: Into<AssetId> + From<u32> + Default,
{
	fn exists(asset_id: TAssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(name: &Vec<u8>) -> Result<TAssetId, DispatchError> {
		Ok(TAssetId::default())
	}

	fn create_asset(name: &Vec<u8>, _existential_deposit: Balance) -> Result<TAssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});

		Ok(TAssetId::from(assigned))
	}
}

impl pallet_omnipool::Config for Test {
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
	type AssetRegistry = DummyRegistry<AssetId>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
}

impl pallet_stableswap::Config for Test {
	type Event = Event;
	type AssetId = AssetId;
	type Currency = Tokens;
	type ShareAccountId = AccountIdConstructor;
	type AssetRegistry = DummyRegistry<AssetId>;
	type PoolMasterOrigin = EnsureRoot<Self::AccountId>;
	type MinPoolLiquidity = MinTradeAmount;
	type MinTradingLimit = MinTradeAmount;
	type AmplificationRange = AmplificationRange;
	type WeightInfo = ();
}

impl Config for Test {
	type Event = Event;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	asset_fee: Permill,
	protocol_fee: Permill,
	asset_weight_cap: Permill,
	min_liquidity: u128,
	min_trade_limit: u128,
	register_stable_asset: bool,
	max_in_ratio: Balance,
	max_out_ratio: Balance,
	init_pool: Option<(FixedU128, FixedU128)>,
	pool_tokens: Vec<(AssetId, FixedU128, AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
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
			*v.borrow_mut() = 1000u128;
		});
		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = 1000u128;
		});
		MAX_IN_RATIO.with(|v| {
			*v.borrow_mut() = 1u128;
		});
		MAX_OUT_RATIO.with(|v| {
			*v.borrow_mut() = 1u128;
		});

		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![
				(Omnipool::protocol_account(), DAI, 1000 * ONE),
				(Omnipool::protocol_account(), HDX, NATIVE_AMOUNT),
			],
			registered_assets: vec![],
			asset_fee: Permill::from_percent(0),
			protocol_fee: Permill::from_percent(0),
			asset_weight_cap: Permill::from_percent(100),
			min_liquidity: 0,
			min_trade_limit: 0,
			init_pool: None,
			register_stable_asset: true,
			pool_tokens: vec![],
			max_in_ratio: 1u128,
			max_out_ratio: 1u128,
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

	pub fn without_stable_asset_in_registry(mut self) -> Self {
		self.register_stable_asset = false;
		self
	}
	pub fn with_max_in_ratio(mut self, value: Balance) -> Self {
		self.max_in_ratio = value;
		self
	}
	pub fn with_max_out_ratio(mut self, value: Balance) -> Self {
		self.max_out_ratio = value;
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
		MAX_IN_RATIO.with(|v| {
			*v.borrow_mut() = self.max_in_ratio;
		});
		MAX_OUT_RATIO.with(|v| {
			*v.borrow_mut() = self.max_out_ratio;
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
				let mut all_assets: Vec<AssetId> = vec![LRNA, DAI];
				all_assets.extend(self.registered_assets);

				for asset in all_assets.into_iter() {
					REGISTERED_ASSETS.with(|v| {
						v.borrow_mut().insert(asset, asset);
					});
				}

				let stable_amount = Tokens::free_balance(DAI, &Omnipool::protocol_account());
				let native_amount = Tokens::free_balance(HDX, &Omnipool::protocol_account());

				assert_ok!(Omnipool::initialize_pool(
					Origin::root(),
					stable_price,
					native_price,
					Permill::from_percent(100),
					Permill::from_percent(100)
				));

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
				System::set_block_number(1);
			});
		}

		r
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
pub struct DummyNFT;

impl<AccountId: From<u64>> Inspect<AccountId> for DummyNFT {
	type ItemId = u32;
	type CollectionId = u32;

	fn owner(_class: &Self::CollectionId, instance: &Self::ItemId) -> Option<AccountId> {
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
	fn create_collection(_class: &Self::CollectionId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u64> + Into<u64> + Copy> Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::CollectionId, _instance: &Self::ItemId, _who: &AccountId) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(*_instance, (*_who).into());
		});
		Ok(())
	}

	fn burn(
		_class: &Self::CollectionId,
		instance: &Self::ItemId,
		_maybe_check_owner: Option<&AccountId>,
	) -> DispatchResult {
		POSITIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.remove(instance);
		});
		Ok(())
	}
}

use hydradx_traits::AccountIdFor;

pub(crate) fn get_mock_minted_position(position_id: u32) -> Option<u64> {
	POSITIONS.with(|v| v.borrow().get(&position_id).copied())
}

pub struct AccountIdConstructor;

impl AccountIdFor<Vec<u32>> for AccountIdConstructor {
	type AccountId = AccountId;

	fn from_assets(assets: &Vec<u32>, _identifier: Option<&[u8]>) -> Self::AccountId {
		assets.into_iter().sum::<u32>() as u64
	}

	fn name(assets: &Vec<u32>, identifier: Option<&[u8]>) -> Vec<u8> {
		let mut buf: Vec<u8> = if let Some(ident) = identifier {
			ident.to_vec()
		} else {
			vec![]
		};
		buf.extend_from_slice(&(assets[0]).to_le_bytes());
		buf.extend_from_slice(&(assets[1]).to_le_bytes());

		buf
	}
}

//TODO: add this to test utils package once HydraDX is moved to 9.29
#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

//TODO: add this to test utils package once HydraDX is moved to 9.29
#[macro_export]
macro_rules! assert_balance_approx {
	( $who:expr, $asset:expr, $expected_balance:expr, $delta:expr) => {{
		let balance = Tokens::free_balance($asset, &$who);

		let diff = if balance >= $expected_balance {
			balance - $expected_balance
		} else {
			$expected_balance - balance
		};
		if diff > $delta {
			panic!(
				"\n{} not equal\nleft: {:?}\nright: {:?}\n",
				"The balances are not equal", balance, $expected_balance
			);
		}
	}};
}

//TODO: use this from test utils package once HydraDX is moved to 9.29
#[macro_export]
macro_rules! assert_eq_approx {
	( $x:tt, $y:tt, $z:tt, $r:tt) => {{
		let diff = if $x >= $y { $x - $y } else { $y - $x };
		if diff > $z {
			panic!("\n{} not equal\nleft: {:?}\nright: {:?}\n", $r, $x, $y);
		}
	}};
}

#[macro_export]
macro_rules! add_omnipool_token {
	($asset_id:expr) => {
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			$asset_id,
			FixedU128::from_float(0.65),
			Permill::from_percent(100),
			LP1
		));
	};
	($asset_id:expr, $price:expr) => {
		assert_ok!(Omnipool::add_token(
			Origin::root(),
			$asset_id,
			$price,
			Permill::from_percent(100),
			LP1
		));
	};
}

#[macro_export]
macro_rules! create_subpool {
	($pool_id:expr, $asset_a:expr, $asset_b:expr) => {
		assert_ok!(OmnipoolSubpools::create_subpool(
			Origin::root(),
			$pool_id,
			$asset_a,
			$asset_b,
			Permill::from_percent(50),
			100u16,
			Permill::from_float(0.003),
			Permill::from_float(0.003),
		));
	};
}

#[macro_export]
macro_rules! assert_that_asset_is_not_present_in_omnipool {
	($asset_id:expr) => {
		assert_err!(
			Omnipool::load_asset_state($asset_id),
			pallet_omnipool::Error::<Test>::AssetNotFound
		);
	};
}

#[macro_export]
macro_rules! assert_that_sharetoken_in_omnipool_as_another_asset {
	($share_asset_id:expr, $asset_reserve_state:expr) => {
		let pool_asset = Omnipool::load_asset_state($share_asset_id);
		assert!(
			pool_asset.is_ok(),
			"there was an unexpected problem when loading share asset '{}'",
			$share_asset_id
		);
		assert_eq!(
			pool_asset.unwrap(),
			$asset_reserve_state,
			"Omnipool asset with share id '{}' is other than expected",
			$share_asset_id
		);
	};
}

#[macro_export]
macro_rules! assert_that_asset_is_migrated_to_omnipool_subpool {
	($asset:expr, $pool_id:expr, $asset_details:expr) => {
		let migrate_asset = OmnipoolSubpools::migrated_assets($asset);

		assert!(
			migrate_asset.is_some(),
			"Asset '{}' can not be found in omnipool subpools migrated asset storage",
			$asset
		);
		assert_eq!(
			migrate_asset.unwrap(),
			($pool_id, $asset_details),
			"asset details for asset `{}` is not as expected",
			$asset
		);
	};
}

#[macro_export]
macro_rules! assert_that_stableswap_subpool_is_created_with_poolinfo {
	($pool_id:expr, $pool_info:expr) => {
		let stableswap_pool = Stableswap::pools($pool_id);
		assert!(
			stableswap_pool.is_some(),
			"subpool with id {} is not found in stableswap pools",
			$pool_id
		);
		assert_eq!(
			stableswap_pool.unwrap(),
			$pool_info,
			"subpool with id {} has different PoolInfo than expected",
			$pool_id
		);
	};
}

#[macro_export]
macro_rules! assert_stableswap_pool_assets {
	($pool_id:expr, $assets:expr) => {
		let subpool = Stableswap::get_pool($pool_id).unwrap();
		assert_eq!(subpool.assets.to_vec(), $assets);
	};
}

#[macro_export]
macro_rules! assert_that_nft_position_is_present {
	( $position_id:expr) => {{
		assert!(
			get_mock_minted_position($position_id).is_some(),
			"Position instance was not minted with id {}",
			$position_id
		);
	}};
}
#[macro_export]
macro_rules! assert_that_nft_position_is_not_present {
	( $position_id:expr) => {{
		assert!(
			get_mock_minted_position($position_id).is_none(),
			"Position instance is present with id {}",
			$position_id
		);
	}};
}

#[macro_export]
macro_rules! assert_that_position_is_present_in_omnipool {
	( $owner:expr, $position_id:expr, $position:expr) => {{
		let position = Omnipool::load_position($position_id, $owner);
		assert_eq!(position.unwrap(), $position, "The position is as expected")
	}};
}

#[macro_export]
macro_rules! assert_that_position_is_not_present_in_omnipool {
	( $owner:expr, $position_id:expr) => {{
		assert!(
			Omnipool::load_position($position_id, $owner).is_err(),
			"Position in omnipool is (unexpectedly) present"
		);
	}};
}

fn last_events(n: usize) -> Vec<Event> {
	frame_system::Pallet::<Test>::events()
		.into_iter()
		.rev()
		.take(n)
		.rev()
		.map(|e| e.event)
		.collect()
}

//TODO: use test utils for this once upgraded to polkadot 0.9.29
pub fn expect_events(e: Vec<Event>) {
	let last_events = last_events(e.len());
	pretty_assertions::assert_eq!(last_events, e);
}

/*pub fn expect_events(e: Vec<Event>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}*/
