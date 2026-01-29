// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
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

use crate as pallet_ice;
use crate::*;
use frame_support::parameter_types;
use frame_support::storage::with_transaction;
use frame_support::traits::Everything;
use frame_support::PalletId;
use frame_system::ensure_signed;
use frame_system::pallet_prelude::OriginFor;
use frame_system::EnsureRoot;
use hydra_dx_math::types::Ratio;
use hydradx_traits::amm::{SimulatorConfig, SimulatorError, SimulatorSet, TradeResult};
use hydradx_traits::router::{AssetPair, PoolType, Route, RouteProvider};
use hydradx_traits::OraclePeriod;
use hydradx_traits::PriceOracle;
use ice_support::SwapType;
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrency;
use pallet_intent::types::CallData;
use pallet_intent::types::Intent;
use pallet_route_executor::ExecutorError;
use pallet_route_executor::Trade;
use pallet_route_executor::TradeExecution;
pub use primitives::constants::time::SLOT_DURATION;
use sp_core::ConstU32;
use sp_core::ConstU64;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::traits::IdentityLookup;
use sp_runtime::BuildStorage;
use sp_runtime::DispatchError;
use sp_runtime::DispatchResult;
use sp_runtime::FixedU128;
use sp_runtime::TransactionOutcome;

use std::cell::RefCell;
use std::vec;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type AssetId = u32;
pub type Balance = u128;

pub(crate) const ONE_DOT: u128 = 10_000_000_000;
pub(crate) const ONE_HDX: u128 = 1_000_000_000_000;
pub(crate) const ONE_QUINTIL: u128 = 1_000_000_000_000_000_000;

//Assets
pub(crate) const HDX: AssetId = 0;
pub(crate) const HUB_ASSET_ID: AssetId = 1;
pub(crate) const DOT: AssetId = 2;
pub(crate) const GETH: AssetId = 3;
pub(crate) const ETH: AssetId = 4;

//5 SEC.
pub(crate) const MAX_INTENT_DEADLINE: pallet_intent::types::Moment = 5 * ONE_SECOND;
pub(crate) const ONE_SECOND: pallet_intent::types::Moment = 1_000;

//Accounts
//acccounts holding amount in for all router dummy pools
const ROUTER_POOLS_POT: AccountId = 1;
pub(crate) const ALICE: AccountId = 2;
pub(crate) const BOB: AccountId = 3;
pub(crate) const DAVE: AccountId = 4;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Currencies: orml_tokens,
		Timestamp: pallet_timestamp,
		Intents: pallet_intent,
		Router: pallet_route_executor,
		Broadcast: pallet_broadcast,
		ICE: pallet_ice,
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
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
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
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
	type ExtensionsWeightInfo = ();
}

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> hydradx_traits::CreateBare<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: Self::RuntimeCall) -> Extrinsic {
		Extrinsic::new_bare(call)
	}
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
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
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

pub struct DummyLazyExecutor<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> hydradx_traits::lazy_executor::Mutate<AccountId> for DummyLazyExecutor<T> {
	type Error = DispatchError;
	type BoundedCall = CallData;

	fn queue(
		_src: hydradx_traits::lazy_executor::Source,
		_origin: AccountId,
		_call: Self::BoundedCall,
	) -> Result<(), Self::Error> {
		Ok(())
	}
}

impl pallet_intent::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LazyExecutorHandler = DummyLazyExecutor<Test>;
	type TimestampProvider = Timestamp;
	type HubAssetId = ConstU32<HUB_ASSET_ID>;
	type MaxAllowedIntentDuration = ConstU64<MAX_INTENT_DEADLINE>;
	type WeightInfo = ();
}

impl pallet_broadcast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const IceId: PalletId = PalletId(*b"iceTest#");
}

impl pallet_ice::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type PalletId = IceId;
	type BlockNumberProvider = System;
	type Simulator = TestSimulatorConfig;
	type WeightInfo = ();
}

// Mock SimulatorConfig
pub struct TestSimulatorConfig;

impl SimulatorConfig for TestSimulatorConfig {
	type Simulators = MockSimulatorSet;
	type RouteProvider = MockRouteProvider;
	type PriceDenominator = NativeCurrencyId;
}

// Mock SimulatorSet
pub struct MockSimulatorSet;

impl SimulatorSet for MockSimulatorSet {
	type State = ();

	fn initial_state() -> Self::State {}

	fn simulate_sell(
		_pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_amount_in: Balance,
		_min_amount_out: Balance,
		_state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError> {
		Err(SimulatorError::Other)
	}

	fn simulate_buy(
		_pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_amount_out: Balance,
		_max_amount_in: Balance,
		_state: &Self::State,
	) -> Result<(Self::State, TradeResult), SimulatorError> {
		Err(SimulatorError::Other)
	}

	fn get_spot_price(
		_pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_state: &Self::State,
	) -> Result<Ratio, SimulatorError> {
		Ok(Ratio::new(1, 1))
	}
}

// Mock RouteProvider
pub struct MockRouteProvider;

impl RouteProvider<AssetId> for MockRouteProvider {
	fn get_route(_pair: AssetPair<AssetId>) -> Route<AssetId> {
		Route::default()
	}
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
	pub const RouteValidationOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;

	pub const RouterPalletId: PalletId = PalletId(*b"routerac");
}

impl pallet_route_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = NativeCurrencyId;
	type Currency = Currencies;
	type AMM = RouterPools;
	type OraclePriceProvider = PriceProviderMock;
	type OraclePeriod = RouteValidationOraclePeriod;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type ForceInsertOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

pub struct PriceProviderMock {}

impl PriceOracle<AssetId> for PriceProviderMock {
	type Price = Ratio;

	fn price(route: &[Trade<AssetId>], _: OraclePeriod) -> Option<Ratio> {
		let has_insufficient_asset = route.iter().any(|t| t.asset_in > 2000 || t.asset_out > 2000);
		if has_insufficient_asset {
			return None;
		}
		Some(Ratio::new(88, 100))
	}
}

#[derive(Debug)]
struct RouterSettlement {
	trade_type: SwapType,
	pool_type: pallet_route_executor::PoolType<AssetId>,
	asset_in: AssetId,
	asset_out: AssetId,
	amount: Balance,
	amount_in: Balance,
	amount_out: Balance,
}
thread_local! {
	pub static ROUTER_SETTLEMENTS: RefCell<Vec<RouterSettlement>> = RefCell::new(Vec::default());
}

type OriginForRuntime = OriginFor<Test>;
pub struct RouterPools;
impl TradeExecution<OriginForRuntime, AccountId, AssetId, Balance> for RouterPools {
	type Error = DispatchError;

	fn execute_buy(
		who: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		_max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		ROUTER_SETTLEMENTS.with(|v| {
			let mut m = v.borrow_mut();

			let idx = m
				.iter()
				.position(|x| {
					//NOTE: who is router account at this point we can't match on it
					x.trade_type == SwapType::ExactOut
						&& x.pool_type == pool_type
						&& x.asset_in == asset_in
						&& x.asset_out == asset_out
						&& x.amount == amount_out
				})
				.expect("router result to exist");

			let p = m.get(idx).expect("item to exits in router pools results");

			let dest = ensure_signed(who.clone()).expect("origin should works");
			Currencies::transfer(who, ROUTER_POOLS_POT, asset_in, p.amount_in).expect("currencies transfer to works");
			Currencies::deposit(p.asset_out, &dest, p.amount_out).expect("currencies deposit to works");

			m.remove(idx);

			Ok(())
		})
	}

	fn execute_sell(
		who: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		_min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		ROUTER_SETTLEMENTS.with(|v| {
			let mut m = v.borrow_mut();

			let idx = m
				.iter()
				.position(|x| {
					//NOTE: who is router account at this point we can't match on it
					x.trade_type == SwapType::ExactIn
						&& x.pool_type == pool_type
						&& x.asset_in == asset_in
						&& x.asset_out == asset_out
						&& x.amount == amount_in
				})
				.expect("router result to exist");

			let p = m.get(idx).expect("item to exits in router pools results");

			let dest = ensure_signed(who.clone()).expect("origin should works");
			Currencies::transfer(who, ROUTER_POOLS_POT, asset_in, p.amount_in).expect("currencies transfer to works");
			Currencies::deposit(p.asset_out, &dest, p.amount_out).expect("currencies deposit to works");

			m.remove(idx);

			Ok(())
		})
	}

	fn get_liquidity_depth(
		_pool_type: PoolType<AssetId>,
		_asset_a: AssetId,
		_asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		Err(ExecutorError::Error(DispatchError::Other("Not Implemented 1")))
	}

	fn calculate_out_given_in(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		ROUTER_SETTLEMENTS.with(|v| {
			let m = v.borrow();

			let idx = m
				.iter()
				.position(|x| {
					x.trade_type == SwapType::ExactIn
						&& x.pool_type == pool_type
						&& x.asset_in == asset_in
						&& x.asset_out == asset_out
						&& x.amount == amount_in
				})
				.expect("router result to exist");

			let p = m.get(idx).expect("item to exits in router pools results");

			Ok(p.amount_out)
		})
	}

	fn calculate_in_given_out(
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		ROUTER_SETTLEMENTS.with(|v| {
			let m = v.borrow();

			let idx = m
				.iter()
				.position(|x| {
					x.trade_type == SwapType::ExactOut
						&& x.pool_type == pool_type
						&& x.asset_in == asset_in
						&& x.asset_out == asset_out
						&& x.amount == amount_out
				})
				.expect("router result to exist");

			let p = m.get(idx).expect("item to exits in router pools results");

			Ok(p.amount_in)
		})
	}

	fn calculate_spot_price_with_fee(
		_pool_type: PoolType<AssetId>,
		_asset_a: AssetId,
		_asset_b: AssetId,
	) -> Result<FixedU128, ExecutorError<Self::Error>> {
		Err(ExecutorError::Error(DispatchError::Other("Not Implemented 4")))
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	intents: Vec<(AccountId, Intent)>,
	router_settlements: Vec<RouterSettlement>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		ROUTER_SETTLEMENTS.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![],
			intents: vec![],
			router_settlements: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_intents(mut self, intents: Vec<(AccountId, Intent)>) -> Self {
		self.intents = intents;
		self
	}

	pub fn with_router_settlement(
		mut self,
		trade_type: SwapType,
		pool_type: pallet_route_executor::PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		amount_in: Balance,
		amount_out: Balance,
	) -> Self {
		self.router_settlements.push(RouterSettlement {
			trade_type,
			pool_type,
			asset_in,
			asset_out,
			amount,
			amount_in,
			amount_out,
		});
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		for rr in self.router_settlements {
			ROUTER_SETTLEMENTS.with(|v| {
				v.borrow_mut().push(rr);
			});
		}

		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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
			frame_system::Pallet::<Test>::set_block_number(1);

			let _ = with_transaction(|| {
				for (owner, intent) in self.intents {
					pallet_intent::Pallet::<Test>::add_intent(owner, intent).expect("add_intent should work");
				}

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		r
	}
}
