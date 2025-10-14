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

pub use crate as pallet_circuit_breaker;
use frame_support::traits::{Contains, Get};
pub use frame_support::traits::{Everything, OnFinalize};
pub use frame_support::{assert_noop, assert_ok, parameter_types};

use frame_support::PalletId;
use frame_system::EnsureRoot;
use hydra_dx_math::omnipool::types::BalanceUpdate;
use orml_traits::{parameter_type_with_key, GetByKey, Handler, Happened, MultiCurrency, NamedMultiReservableCurrency};
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, ConstU128, ConstU32, Zero};
use sp_runtime::DispatchResult;
use sp_runtime::FixedU128;
use sp_runtime::Permill;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
};
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;

pub const ALICE: u64 = 1;
pub const BOB: u64 = 2;
pub const WHITELISTED_ACCCOUNT: u64 = 2;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const TRADER: u64 = 4;

pub const HDX: AssetId = 100;
pub const DOT: AssetId = 200;
pub const DAI: AssetId = 2;
pub const LRNA: AssetId = 300;
pub const ACA: AssetId = 4;

pub const ONE: Balance = 1_000_000_000_000;

pub const INITIAL_LIQUIDITY: Balance = 1_000_000;
pub const REGISTERED_ASSET: AssetId = 1000;
pub const NATIVE_AMOUNT: Balance = 10_000 * ONE;

pub const FIVE_PERCENT: (u32, u32) = (500, 10_000);
pub const TEN_PERCENT: (u32, u32) = (1_000, 10_000);

#[cfg(feature = "runtime-benchmarks")]
use frame_system::pallet_prelude::BlockNumberFor;
#[cfg(feature = "runtime-benchmarks")]
pub const DEFAULT_ASSET_DEPOSIT_PERIOD: BlockNumberFor<Test> = 10;

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = const { RefCell::new(Permill::from_percent(100)) };
	pub static ASSET_FEE: RefCell<Permill> = const { RefCell::new(Permill::from_percent(0)) };
	pub static PROTOCOL_FEE: RefCell<Permill> = const { RefCell::new(Permill::from_percent(0)) };
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = const { RefCell::new(1000u128) };
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = const { RefCell::new(1000u128) };
	pub static MAX_IN_RATIO: RefCell<Balance> = const { RefCell::new(1u128) };
	pub static MAX_OUT_RATIO: RefCell<Balance> = const { RefCell::new(1u128) };
	pub static MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK: RefCell<(u32, u32)> = const { RefCell::new((2_000, 10_000)) }; // 20%
	pub static MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK: RefCell<Option<(u32, u32)>> = const { RefCell::new(Some((4_000, 10_000))) }; // 40%
	pub static MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK: RefCell<Option<(u32, u32)>> = const { RefCell::new(Some((2_000, 10_000))) }; // 20%
	pub static ASSET_DEPOSIT_LIMIT: RefCell<HashMap<AssetId, Balance>> = RefCell::new(HashMap::default());
	pub static ASSET_DEPOSIT_PERIOD: RefCell<u128> = RefCell::new(u128::zero());
}

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Omnipool: pallet_omnipool,
		Tokens: orml_tokens,
		CircuitBreaker: pallet_circuit_breaker,
		Broadcast: pallet_broadcast,
		Currencies: pallet_currencies
	}
);

parameter_types! {
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
		pub NativeCurrencyId: AssetId = HDX;
		pub NamedReserveId: [u8;8] = *b"test_res";
		pub const MaxReserves: u32 = 50;
}
pub type Amount = i128;

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type ReserveAccount = TreasuryAccount;
	type GetNativeCurrencyId = NativeCurrencyId;
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
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
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
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

parameter_types! {
	pub DefaultMaxNetTradeVolumeLimitPerBlock: (u32, u32) = MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub DefaultMaxAddLiquidityLimitPerBlock: Option<(u32, u32)> = MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub DefaultMaxRemoveLiquidityLimitPerBlock: Option<(u32, u32)> = MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| *v.borrow());
	pub const OmnipoolHubAsset: AssetId = LRNA;

}

impl pallet_circuit_breaker::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type WhitelistedAccounts = CircuitBreakerWhitelist;
	type DefaultMaxNetTradeVolumeLimitPerBlock = DefaultMaxNetTradeVolumeLimitPerBlock;
	type DefaultMaxAddLiquidityLimitPerBlock = DefaultMaxAddLiquidityLimitPerBlock;
	type DefaultMaxRemoveLiquidityLimitPerBlock = DefaultMaxRemoveLiquidityLimitPerBlock;
	type OmnipoolHubAsset = OmnipoolHubAsset;
	type WeightInfo = ();
	type DepositLimiter = DepositLimiter;

	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = BenchmarkHelperMock;
}

#[cfg(feature = "runtime-benchmarks")]
pub struct BenchmarkHelperMock;
#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper<AccountId, AssetId, Balance> for BenchmarkHelperMock {
	fn deposit(who: AccountId, asset_id: AssetId, amount: Balance) -> DispatchResult {
		Tokens::deposit(asset_id, &who, amount)
	}

	fn register_asset(asset_id: AssetId, limit: Balance) -> DispatchResult {
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(asset_id, asset_id);
		});

		ASSET_DEPOSIT_PERIOD.with(|v| {
			*v.borrow_mut() = DEFAULT_ASSET_DEPOSIT_PERIOD.into();
		});

		ASSET_DEPOSIT_LIMIT.with(|v| {
			v.borrow_mut().insert(asset_id, limit);
		});

		Ok(())
	}
}
pub struct CircuitBreakerWhitelist;

impl Contains<AccountId> for CircuitBreakerWhitelist {
	fn contains(a: &AccountId) -> bool {
		WHITELISTED_ACCCOUNT == *a
	}
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
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
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
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = [u8; 8];
	type CurrencyHooks = Hooks;
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const PosiitionCollectionId: u32= 1000;

	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;
	pub MinWithdrawFee: Permill = Permill::from_percent(0);
	pub BurnFee: Permill = Permill::from_percent(0);
}

impl pallet_omnipool::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Tokens;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PosiitionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type UpdateTradabilityOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u32;
	type OmnipoolHooks = CircuitBreakerHooks<Test>;
	type PriceBarrier = ();
	type MinWithdrawalFee = MinWithdrawFee;
	type ExternalPriceOracle = WithdrawFeePriceOracle;
	type Fee = FeeProvider;
	type BurnProtocolFee = BurnFee;
}

impl pallet_broadcast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

pub struct CircuitBreakerHooks<T>(PhantomData<T>);

impl<T> OmnipoolHooks<RuntimeOrigin, AccountId, AssetId, Balance> for CircuitBreakerHooks<T>
where
	// Lrna: Get<AssetId>,
	T: Config + pallet_circuit_breaker::Config,
	<T as pallet_circuit_breaker::Config>::Balance: From<u128>,
	<T as pallet_circuit_breaker::Config>::AssetId: From<u32>, //TODO: get  rid of these if possible
	<T as frame_system::Config>::RuntimeOrigin: From<RuntimeOrigin>,
{
	type Error = DispatchError;

	fn on_liquidity_changed(origin: RuntimeOrigin, asset: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		/*CircuitBreaker::calculate_and_store_liquidity_limit(asset.asset_id, asset.before.reserve)?;
		CircuitBreaker::ensure_and_update_liquidity_limit(asset.asset_id, asset.after.reserve)?;*/

		match asset.delta_changes.delta_reserve {
			BalanceUpdate::Increase(amount) => {
				pallet_circuit_breaker::Pallet::<T>::ensure_add_liquidity_limit(
					origin.into(),
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?;
			}
			BalanceUpdate::Decrease(amount) => {
				pallet_circuit_breaker::Pallet::<T>::ensure_remove_liquidity_limit(
					origin.into(),
					asset.asset_id.into(),
					asset.before.reserve.into(),
					amount.into(),
				)?;
			}
		};

		Ok(Weight::zero())
	}

	fn on_trade(
		_: RuntimeOrigin,
		asset_in: AssetInfo<AssetId, Balance>,
		asset_out: AssetInfo<AssetId, Balance>,
	) -> Result<Weight, Self::Error> {
		let amount_in = match asset_in.delta_changes.delta_reserve {
			BalanceUpdate::Increase(am) => am,
			BalanceUpdate::Decrease(am) => am,
		};

		let amount_out = match asset_out.delta_changes.delta_reserve {
			BalanceUpdate::Increase(am) => am,
			BalanceUpdate::Decrease(am) => am,
		};

		pallet_circuit_breaker::Pallet::<T>::ensure_pool_state_change_limit(
			asset_in.asset_id.into(),
			asset_in.before.reserve.into(),
			amount_in.into(),
			asset_out.asset_id.into(),
			asset_out.before.reserve.into(),
			amount_out.into(),
		)?;

		Ok(Weight::zero())
	}

	fn on_hub_asset_trade(_: RuntimeOrigin, _: AssetInfo<AssetId, Balance>) -> Result<Weight, Self::Error> {
		Ok(Weight::zero())
	}

	fn on_liquidity_changed_weight() -> Weight {
		todo!()
	}

	fn on_trade_weight() -> Weight {
		todo!()
	}

	fn on_trade_fee(
		_fee_account: AccountId,
		_trader: AccountId,
		_asset: AssetId,
		_amount: Balance,
	) -> Result<Vec<Option<(Balance, AccountId)>>, Self::Error> {
		Ok(vec![])
	}

	fn consume_protocol_fee(
		_fee_account: AccountId,
		_amount: Balance,
	) -> Result<Option<(Balance, AccountId)>, Self::Error> {
		Ok(None)
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::weights::Weight;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::fee::GetDynamicFee;
use orml_traits::currency::MutationHooks;

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

use crate::traits::AssetDepositLimiter;
use crate::Config;
use hydradx_traits::registry::{AssetKind, Inspect as InspectRegistry};
use pallet_currencies::{BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use pallet_omnipool::traits::{AssetInfo, ExternalPriceProvider, OmnipoolHooks};

#[cfg(feature = "runtime-benchmarks")]
use crate::types::BenchmarkHelper;

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> InspectRegistry for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	type AssetId = T::AssetId;
	type Location = u8;

	fn is_sufficient(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		unimplemented!()
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		unimplemented!()
	}

	fn exists(asset_id: T::AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		asset.is_some()
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		Some(1u128)
	}
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
	max_net_trade_volume_limit_per_block: (u32, u32),
	max_add_liquidity_limit_per_block: Option<(u32, u32)>,
	max_remove_liquidity_limit_per_block: Option<(u32, u32)>,
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
		ASSET_DEPOSIT_LIMIT.with(|v| {
			v.borrow_mut().clear();
		});
		ASSET_DEPOSIT_PERIOD.with(|v| {
			*v.borrow_mut() = 0u128;
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
			max_net_trade_volume_limit_per_block: (2_000, 10_000),
			max_add_liquidity_limit_per_block: Some((4_000, 10_000)),
			max_remove_liquidity_limit_per_block: Some((2_000, 10_000)),
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

	pub fn with_initial_pool(mut self, stable_price: FixedU128, native_price: FixedU128) -> Self {
		self.init_pool = Some((stable_price, native_price));
		self
	}

	pub fn with_max_trade_volume_limit_per_block(mut self, value: (u32, u32)) -> Self {
		self.max_net_trade_volume_limit_per_block = value;
		self
	}

	pub fn with_max_add_liquidity_limit_per_block(mut self, value: Option<(u32, u32)>) -> Self {
		self.max_add_liquidity_limit_per_block = value;
		self
	}

	pub fn with_max_remove_liquidity_limit_per_block(mut self, value: Option<(u32, u32)>) -> Self {
		self.max_remove_liquidity_limit_per_block = value;
		self
	}
	pub fn with_asset_limit(self, asset_id: AssetId, limit: Balance) -> Self {
		ASSET_DEPOSIT_LIMIT.with(|v| {
			v.borrow_mut().insert(asset_id, limit);
		});
		self
	}

	pub fn with_deposit_period(self, period: u128) -> Self {
		ASSET_DEPOSIT_PERIOD.with(|v| {
			*v.borrow_mut() = period;
		});
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
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Add DAi and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			if self.register_stable_asset {
				v.borrow_mut().insert(DAI, DAI);
			}
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
		MAX_IN_RATIO.with(|v| {
			*v.borrow_mut() = self.max_in_ratio;
		});
		MAX_OUT_RATIO.with(|v| {
			*v.borrow_mut() = self.max_out_ratio;
		});
		MAX_NET_TRADE_VOLUME_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_net_trade_volume_limit_per_block;
		});
		MAX_ADD_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_add_liquidity_limit_per_block;
		});
		MAX_REMOVE_LIQUIDITY_LIMIT_PER_BLOCK.with(|v| {
			*v.borrow_mut() = self.max_remove_liquidity_limit_per_block;
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
			});
		}

		r.execute_with(|| System::set_block_number(1));

		r
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
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

impl GetDynamicFee<(AssetId, Balance)> for FeeProvider {
	type Fee = (Permill, Permill);
	fn get(_: (AssetId, Balance)) -> Self::Fee {
		(ASSET_FEE.with(|v| *v.borrow()), PROTOCOL_FEE.with(|v| *v.borrow()))
	}

	fn get_and_store(key: (AssetId, Balance)) -> Self::Fee {
		Self::get(key)
	}
}

pub struct DepositLimiter;

impl AssetDepositLimiter<AccountId, AssetId, Balance> for DepositLimiter {
	type DepositLimit = AssetLimit;
	type Period = LimitDepositPeriod;
	type Issuance = AssetIssuance;
	type OnLimitReached = LimitReachedHandler;
	type OnLockdownDeposit = OnLockdownDepositHandler;
	type OnDepositRelease = OnReleaseDepositHandler;
}

pub struct LimitDepositPeriod;

impl Get<u128> for LimitDepositPeriod {
	fn get() -> u128 {
		ASSET_DEPOSIT_PERIOD.with(|v| *v.borrow())
	}
}

pub struct AssetLimit;

impl GetByKey<AssetId, Option<Balance>> for AssetLimit {
	fn get(k: &AssetId) -> Option<Balance> {
		let asset = ASSET_DEPOSIT_LIMIT.with(|v| v.borrow().get(k).copied());

		Some(asset.unwrap_or(Balance::MAX))
	}
}

pub struct AssetIssuance;
impl GetByKey<AssetId, Balance> for AssetIssuance {
	fn get(k: &AssetId) -> Balance {
		Tokens::total_issuance(k)
	}
}

pub struct LimitReachedHandler;

impl Happened<AssetId> for LimitReachedHandler {
	fn happened(_t: &AssetId) {}
}

pub struct OnLockdownDepositHandler;
impl Handler<(AssetId, AccountId, Balance)> for OnLockdownDepositHandler {
	fn handle(t: &(AssetId, AccountId, Balance)) -> DispatchResult {
		Currencies::reserve_named(&NamedReserveId::get(), t.0, &t.1, t.2)?;
		Ok(())
	}
}

pub struct OnReleaseDepositHandler;
impl Handler<(AssetId, AccountId)> for OnReleaseDepositHandler {
	fn handle(t: &(AssetId, AccountId)) -> DispatchResult {
		let reserved_balance = Currencies::reserved_balance_named(&NamedReserveId::get(), t.0, &t.1);

		Currencies::unreserve_named(&NamedReserveId::get(), t.0, &t.1, reserved_balance);

		Ok(())
	}
}

pub struct Hooks;

impl MutationHooks<AccountId, AssetId, Balance> for Hooks {
	type OnDust = ();
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = crate::fuses::issuance::IssuanceIncreaseFuse<Test>;
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
}
