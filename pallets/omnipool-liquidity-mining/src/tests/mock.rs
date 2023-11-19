// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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
#![allow(clippy::type_complexity)]
#![allow(clippy::too_many_arguments)]

use crate::*;
use std::cell::RefCell;
use std::collections::HashMap;

use crate as omnipool_liquidity_mining;

use frame_support::weights::Weight;
use frame_support::BoundedVec;
use hydradx_traits::liquidity_mining::PriceAdjustment;
use pallet_omnipool;

use frame_support::traits::{ConstU128, Contains, Everything};
use frame_support::{
	assert_ok, construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
	weights::RuntimeDbWeight,
};
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
use orml_traits::GetByKey;
use pallet_liquidity_mining as warehouse_liquidity_mining;
use sp_core::H256;
use sp_runtime::FixedU128;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage, Permill,
};

use warehouse_liquidity_mining::{GlobalFarmData, Instance1};

use hydradx_traits::{
	oracle::{OraclePeriod, Source},
	pools::DustRemovalAccountWhitelist,
	AssetKind,
};

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u128;
pub type BlockNumber = u64;
pub type Balance = u128;
pub type AssetId = u32;
//NTF types
pub type CollectionId = u128;
pub type ItemId = u128;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 1_000;
pub const KSM: AssetId = 1_001;
pub const ACA: AssetId = 1_002;

pub const LP1: AccountId = 1;
pub const LP2: AccountId = 2;

pub const ALICE: AccountId = 4;
pub const BOB: AccountId = 5;
pub const CHARLIE: AccountId = 6;
pub const GC: AccountId = 7;

pub const INITIAL_READ_WEIGHT: u64 = 1;
pub const INITIAL_WRITE_WEIGHT: u64 = 1;

pub const ONE: Balance = 1_000_000_000_000;

pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

pub const OMNIPOOL_COLLECTION_ID: u128 = 1_000;
pub const LM_COLLECTION_ID: u128 = 1;

thread_local! {
	pub static NFTS: RefCell<HashMap<(CollectionId, ItemId), AccountId>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);

	 pub static DUSTER_WHITELIST: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Omnipool: pallet_omnipool,
		Tokens: orml_tokens,
		WarehouseLM: warehouse_liquidity_mining::<Instance1>,
		OmnipoolMining: omnipool_liquidity_mining,
		EmaOracle: pallet_ema_oracle,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub static MockBlockNumberProvider: u64 = 0;
	pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight{
		read: INITIAL_READ_WEIGHT, write: INITIAL_WRITE_WEIGHT
	};
}

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		System::block_number()
	}
}

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
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const LMPalletId: PalletId = PalletId(*b"TEST_lm_");
	pub const LMCollectionId: CollectionId = LM_COLLECTION_ID;
	pub const PeriodOracle: OraclePeriod= OraclePeriod::Day;
	pub const OracleSource: Source = *b"omnipool";
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Tokens;
	type CreateOrigin = frame_system::EnsureRoot<AccountId>;
	type PalletId = LMPalletId;
	type NFTCollectionId = LMCollectionId;
	type NFTHandler = DummyNFT;
	type LiquidityMiningHandler = WarehouseLM;
	type OracleSource = OracleSource;
	type OraclePeriod = PeriodOracle;
	type PriceOracle = DummyOracle;
	type WeightInfo = ();
}

parameter_types! {
	pub const WarehouseLMPalletId: PalletId = PalletId(*b"TEST_lm_");
	pub const MinTotalFarmRewards: Balance = 1_000_000 * ONE;
	pub const MinPlannedYieldingPeriods: BlockNumber  = 100;
	#[derive(PartialEq, Eq)]
	pub const MaxEntriesPerDeposit: u32 = 5;
	pub const MaxYieldFarmsPerGlobalFarm: u32 = 10;
}

impl warehouse_liquidity_mining::Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Tokens;
	type PalletId = WarehouseLMPalletId;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods;
	type BlockNumberProvider = MockBlockNumberProvider;
	type AmmPoolId = AssetId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type AssetRegistry = DummyRegistry<Test>;
	type NonDustableWhitelistHandler = Whitelist;
	type PriceAdjustment = DummyOracle;
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
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
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

//NOTE: oracle is not used in the unit tests. It's here to satify benchmarks bounds.
use pallet_ema_oracle::MAX_PERIODS;
parameter_types! {
	pub SupportedPeriods: BoundedVec<OraclePeriod, ConstU32<MAX_PERIODS>> = BoundedVec::truncate_from(vec![
		OraclePeriod::LastBlock, OraclePeriod::Short, OraclePeriod::TenMinutes]);
}
impl pallet_ema_oracle::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = ();
	type BlockNumberProvider = MockBlockNumberProvider;
	type SupportedPeriods = SupportedPeriods;
	type MaxUniqueEntries = ConstU32<20>;
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const PositionCollectionId: CollectionId = OMNIPOOL_COLLECTION_ID;

	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub MinWithdrawFee: Permill = Permill::from_percent(0);
}

impl pallet_omnipool::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u128;
	type Currency = Tokens;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PositionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u128;
	type OmnipoolHooks = ();
	type PriceBarrier = ();
	type MinWithdrawalFee = MinWithdrawFee;
	type ExternalPriceOracle = WithdrawFeePriceOracle;
	type Fee = FeeProvider;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
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
	omnipool_liquidity: Vec<(AccountId, AssetId, Balance)>, //who, asset, amount/
	lm_global_farms: Vec<(
		Balance,
		PeriodOf<Test>,
		BlockNumber,
		AssetId,
		AccountId,
		Perquintill,
		Balance,
		FixedU128,
	)>,
	lm_yield_farms: Vec<(AccountId, GlobalFarmId, AssetId, FarmMultiplier, Option<LoyaltyCurve>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		NFTS.with(|v| {
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

		DUSTER_WHITELIST.with(|v| {
			v.borrow_mut().clear();
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
			register_stable_asset: true,
			pool_tokens: vec![],
			max_in_ratio: 1u128,
			max_out_ratio: 1u128,
			omnipool_liquidity: vec![],
			lm_global_farms: vec![],
			lm_yield_farms: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId) -> Self {
		self.registered_assets.push(asset);
		self
	}

	pub fn with_initial_pool(mut self, stable_price: FixedU128, native_price: FixedU128) -> Self {
		self.init_pool = Some((stable_price, native_price));
		self
	}

	pub fn with_liquidity(mut self, who: AccountId, asset: AssetId, amount: Balance) -> Self {
		self.omnipool_liquidity.push((who, asset, amount));
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

	pub fn with_global_farm(
		mut self,
		total_rewards: Balance,
		planned_yielding_periods: PeriodOf<Test>,
		blocks_per_period: BlockNumber,
		reward_currency: AssetId,
		owner: AccountId,
		yield_per_period: Perquintill,
		min_deposit: Balance,
		lrna_price_adjustment: FixedU128,
	) -> Self {
		self.lm_global_farms.push((
			total_rewards,
			planned_yielding_periods,
			blocks_per_period,
			reward_currency,
			owner,
			yield_per_period,
			min_deposit,
			lrna_price_adjustment,
		));
		self
	}

	pub fn with_yield_farm(
		mut self,
		owner: AccountId,
		id: GlobalFarmId,
		asset: AssetId,
		multiplier: FarmMultiplier,
		loyalty_curve: Option<LoyaltyCurve>,
	) -> Self {
		self.lm_yield_farms.push((owner, id, asset, multiplier, loyalty_curve));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Add DAI and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			if self.register_stable_asset {
				v.borrow_mut().insert(DAI, DAI);
			}
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(LRNA, LRNA);
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
				set_block_number(1);
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					HDXAssetId::get(),
					native_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					DAI,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));

				for (asset_id, price, owner, amount) in self.pool_tokens {
					assert_ok!(Tokens::transfer(
						RuntimeOrigin::signed(owner),
						Omnipool::protocol_account(),
						asset_id,
						amount
					));
					assert_ok!(Omnipool::add_token(
						RuntimeOrigin::root(),
						asset_id,
						price,
						self.asset_weight_cap,
						owner
					));
				}

				for p in self.omnipool_liquidity {
					assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(p.0), p.1, p.2));
				}

				for gf in self.lm_global_farms {
					assert_ok!(OmnipoolMining::create_global_farm(
						RuntimeOrigin::root(),
						gf.0,
						gf.1,
						gf.2,
						gf.3,
						gf.4,
						gf.5,
						gf.6,
						gf.7,
					));
				}

				for yf in self.lm_yield_farms {
					assert_ok!(OmnipoolMining::create_yield_farm(
						RuntimeOrigin::signed(yf.0),
						yf.1,
						yf.2,
						yf.3,
						yf.4
					));
				}
			});
		}

		r
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate, Transfer};
use hydra_dx_math::ema::EmaPrice;

pub struct DummyNFT;

impl<AccountId: From<u128>> Inspect<AccountId> for DummyNFT {
	type ItemId = ItemId;
	type CollectionId = CollectionId;

	fn owner(collection: &Self::CollectionId, item: &Self::ItemId) -> Option<AccountId> {
		let mut owner: Option<AccountId> = None;

		NFTS.with(|v| {
			if let Some(o) = v.borrow().get(&(*collection, *item)) {
				owner = Some((*o).into());
			}
		});
		owner
	}
}

impl<AccountId: From<u128>> Create<AccountId> for DummyNFT {
	fn create_collection(_collection: &Self::CollectionId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u128> + Into<u128> + Copy> Mutate<AccountId> for DummyNFT {
	fn mint_into(collection: &Self::CollectionId, item: &Self::ItemId, who: &AccountId) -> DispatchResult {
		NFTS.with(|v| {
			let mut m = v.borrow_mut();
			m.insert((*collection, *item), (*who).into());
		});
		Ok(())
	}

	fn burn(
		collection: &Self::CollectionId,
		item: &Self::ItemId,
		_maybe_check_owner: Option<&AccountId>,
	) -> DispatchResult {
		NFTS.with(|v| {
			let mut m = v.borrow_mut();
			m.remove(&(*collection, *item));
		});
		Ok(())
	}
}

impl Transfer<AccountId> for DummyNFT {
	fn transfer(collection: &Self::CollectionId, item: &Self::ItemId, destination: &AccountId) -> DispatchResult {
		NFTS.with(|v| {
			let mut m = v.borrow_mut();
			let key = (*collection, *item);

			if !m.contains_key(&key) {
				return Err(sp_runtime::DispatchError::Other("NFT not found"));
			}

			m.insert(key, *destination);

			Ok(())
		})
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

	fn retrieve_asset_type(_asset_id: T::AssetId) -> Result<AssetKind, DispatchError> {
		unimplemented!()
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			//NOTE: This is to have same ids as real AssetRegistry which is used in the benchmarks.
			//1_000_000 - offset of the reals AssetRegistry
			// - 3 - remove assets reagistered by default for the vec.len()
			// +1 - first reg asset start with 1 not 0
			// => 1-th asset id == 1_000_001
			let l = 1_000_000 - 3 + 1 + v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(T::AssetId::from(assigned))
	}
}

use hydradx_traits::oracle::AggregatedPriceOracle;
use pallet_omnipool::traits::ExternalPriceProvider;

pub struct DummyOracle;
pub type OraclePrice = hydra_dx_math::ema::EmaPrice;
impl AggregatedPriceOracle<AssetId, BlockNumber, OraclePrice> for DummyOracle {
	type Error = OracleError;

	fn get_price(
		_asset_a: AssetId,
		asset_b: AssetId,
		_period: OraclePeriod,
		_source: Source,
	) -> Result<(OraclePrice, BlockNumber), Self::Error> {
		match asset_b {
			KSM => Ok((
				OraclePrice {
					n: 650_000_000_000_000_000,
					d: 1_000_000_000_000_000_000,
				},
				0,
			)),
			//Tokens used in benchmarks
			1_000_001..=1_000_003 => Ok((
				OraclePrice {
					n: 1_000_000_000_000_000_000,
					d: 1_000_000_000_000_000_000,
				},
				0,
			)),
			_ => Err(OracleError::NotPresent),
		}
	}

	fn get_price_weight() -> Weight {
		Weight::zero()
	}
}

impl PriceAdjustment<GlobalFarmData<Test, Instance1>> for DummyOracle {
	type Error = DispatchError;

	type PriceAdjustment = FixedU128;

	fn get(_global_farm: &GlobalFarmData<Test, Instance1>) -> Result<Self::PriceAdjustment, Self::Error> {
		Ok(FixedU128::from_inner(500_000_000_000_000_000)) //0.5
	}
}

impl<T: Config> GetByKey<T::AssetId, Balance> for DummyRegistry<T> {
	fn get(_key: &T::AssetId) -> Balance {
		1_000_u128
	}
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
		if Whitelist::contains(account) {
			return Err(sp_runtime::DispatchError::Other("Account is already in the whitelist"));
		}

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

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}

pub struct WithdrawFeePriceOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for WithdrawFeePriceOracle {
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		assert_eq!(asset_a, LRNA);
		let asset_state = Omnipool::load_asset_state(asset_b)?;
		let price = EmaPrice::new(asset_state.hub_reserve, asset_state.reserve);
		Ok(price)
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
}

pub struct FeeProvider;

impl GetByKey<AssetId, (Permill, Permill)> for FeeProvider {
	fn get(_: &AssetId) -> (Permill, Permill) {
		(ASSET_FEE.with(|v| *v.borrow()), PROTOCOL_FEE.with(|v| *v.borrow()))
	}
}
