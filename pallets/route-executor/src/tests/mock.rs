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

use crate as router;
use crate::{Config, Trade};
use frame_support::{
	parameter_types,
	traits::{Everything, Nothing},
};
use frame_system::{ensure_signed, pallet_prelude::OriginFor};
use hydradx_traits::router::{ExecutorError, PoolType, TradeExecution};
use orml_traits::parameter_type_with_key;
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter};
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError,
};

use std::cell::RefCell;
use std::ops::Deref;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AssetId = u32;
pub type Balance = u128;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Router: router,
		 Tokens: orml_tokens,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
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
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type ReserveIdentifier = ();
	type MaxReserves = ();
	type CurrencyHooks = ();
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 1;
	pub const MaxReserves: u32 = 50;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}

type Pools = (XYK, StableSwap, OmniPool, LBP);

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = NativeCurrencyId;
	type Currency = FungibleCurrencies<Test>;
	type AMM = Pools;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type WeightInfo = ();
}

pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const ASSET_PAIR_ACCOUNT: AccountId = 2;

pub const HDX: AssetId = 0;
pub const AUSD: AssetId = 1001;
pub const MOVR: AssetId = 1002;
pub const KSM: AssetId = 1003;
pub const RMRK: AssetId = 1004;
pub const SDN: AssetId = 1005;
pub const STABLE_SHARE_ASSET: AssetId = 1006;
pub const DOT: AssetId = 1007;

pub const ALICE_INITIAL_NATIVE_BALANCE: u128 = 1000;

pub const XYK_SELL_CALCULATION_RESULT: Balance = 13;
pub const LBP_SELL_CALCULATION_RESULT: Balance = 12;
pub const OMNIPOOL_SELL_CALCULATION_RESULT: Balance = 10;
pub const STABLESWAP_SELL_CALCULATION_RESULT: Balance = 4;

pub const LBP_BUY_CALCULATION_RESULT: Balance = 8;
pub const OMNIPOOL_BUY_CALCULATION_RESULT: Balance = 5;
pub const STABLESWAP_BUY_CALCULATION_RESULT: Balance = 3;
pub const XYK_BUY_CALCULATION_RESULT: Balance = 1;

pub const INVALID_CALCULATION_AMOUNT: Balance = 999;

pub fn default_omnipool_route() -> Vec<Trade<AssetId>> {
	vec![Trade {
		pool: PoolType::Omnipool,
		asset_in: HDX,
		asset_out: AUSD,
	}]
}

pub const HDX_AUSD_TRADE_IN_XYK: Trade<AssetId> = Trade {
	pool: PoolType::XYK,
	asset_in: HDX,
	asset_out: AUSD,
};

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

// Returns default values for genesis config
impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![(ALICE, HDX, 1000u128)],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![
				(ALICE, ALICE_INITIAL_NATIVE_BALANCE),
				(ASSET_PAIR_ACCOUNT, ALICE_INITIAL_NATIVE_BALANCE),
			],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut initial_accounts = vec![
			(ASSET_PAIR_ACCOUNT, STABLE_SHARE_ASSET, 1000u128),
			(ASSET_PAIR_ACCOUNT, AUSD, 1000u128),
			(ASSET_PAIR_ACCOUNT, MOVR, 1000u128),
			(ASSET_PAIR_ACCOUNT, KSM, 1000u128),
			(ASSET_PAIR_ACCOUNT, RMRK, 1000u128),
			(ASSET_PAIR_ACCOUNT, SDN, 1000u128),
			(ASSET_PAIR_ACCOUNT, DOT, 1000u128),
		];

		initial_accounts.extend(self.endowed_accounts);

		orml_tokens::GenesisConfig::<Test> {
			balances: initial_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}

type ExecutedTradeInfo = (PoolType<AssetId>, Balance, AssetId, AssetId);
thread_local! {
	pub static EXECUTED_SELLS: RefCell<Vec<ExecutedTradeInfo>> = RefCell::new(Vec::default());
	pub static EXECUTED_BUYS: RefCell<Vec<ExecutedTradeInfo>> = RefCell::new(Vec::default());
}

type OriginForRuntime = OriginFor<Test>;

macro_rules! impl_fake_executor {
	($pool_struct:ident, $pool_type: pat, $sell_calculation_result: expr, $buy_calculation_result: expr) => {
		impl TradeExecution<OriginForRuntime, AccountId, AssetId, Balance> for $pool_struct {
			type Error = DispatchError;

			fn calculate_sell(
				pool_type: PoolType<AssetId>,
				_asset_in: AssetId,
				_asset_out: AssetId,
				amount_in: Balance,
			) -> Result<Balance, ExecutorError<Self::Error>> {
				if !matches!(pool_type, $pool_type) {
					return Err(ExecutorError::NotSupported);
				}

				if amount_in == INVALID_CALCULATION_AMOUNT {
					return Err(ExecutorError::Error(DispatchError::Other("Some error happened")));
				}

				Ok($sell_calculation_result)
			}

			fn calculate_buy(
				pool_type: PoolType<AssetId>,
				_asset_in: AssetId,
				_asset_out: AssetId,
				amount_out: Balance,
			) -> Result<Balance, ExecutorError<Self::Error>> {
				if !matches!(pool_type, $pool_type) {
					return Err(ExecutorError::NotSupported);
				}

				if amount_out == INVALID_CALCULATION_AMOUNT {
					return Err(ExecutorError::Error(DispatchError::Other("Some error happened")));
				}

				Ok($buy_calculation_result)
			}

			fn execute_sell(
				who: OriginForRuntime,
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_in: Balance,
				_min_limit: Balance,
			) -> Result<(), ExecutorError<Self::Error>> {
				let who = ensure_signed(who).map_err(|_| ExecutorError::Error(DispatchError::Other("Wrong origin")))?;
				if !matches!(pool_type, $pool_type) {
					return Err(ExecutorError::NotSupported);
				}

				EXECUTED_SELLS.with(|v| {
					let mut m = v.borrow_mut();
					m.push((pool_type, amount_in, asset_in, asset_out));
				});

				let amount_out = $sell_calculation_result;

				Currencies::transfer(
					RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT),
					who,
					asset_out,
					amount_out,
				)
				.map_err(|e| ExecutorError::Error(e))?;
				Currencies::transfer(
					RuntimeOrigin::signed(who),
					ASSET_PAIR_ACCOUNT,
					asset_in,
					amount_in,
				)
				.map_err(|e| ExecutorError::Error(e))?;

				Ok(())
			}

			fn execute_buy(
				who: OriginForRuntime,
				pool_type: PoolType<AssetId>,
				asset_in: AssetId,
				asset_out: AssetId,
				amount_out: Balance,
				_max_limit: Balance,
			) -> Result<(), ExecutorError<Self::Error>> {
				let who = ensure_signed(who).map_err(|_| ExecutorError::Error(DispatchError::Other("Wrong origin")))?;

				if !matches!(pool_type, $pool_type) {
					return Err(ExecutorError::NotSupported);
				}
				EXECUTED_BUYS.with(|v| {
					let mut m = v.borrow_mut();
					m.push((pool_type, amount_out, asset_in, asset_out));
				});

				let amount_in = $buy_calculation_result;

				Currencies::transfer(
					RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT),
					who,
					asset_out,
					amount_out,
				)
				.map_err(|e| ExecutorError::Error(e))?;
				Currencies::transfer(
					RuntimeOrigin::signed(who),
					ASSET_PAIR_ACCOUNT,
					asset_in,
					amount_in,
				)
				.map_err(|e| ExecutorError::Error(e))?;

				Ok(())
			}

			fn get_liquidity_depth(
				_pool_type: PoolType<AssetId>,
				_asset_a: AssetId,
				_asset_b: AssetId,
			) -> Result<Balance, ExecutorError<Self::Error>> {
				Ok(100)
			}
		}
	};
}

#[allow(clippy::upper_case_acronyms)]
pub struct XYK;
pub struct StableSwap;
pub struct OmniPool;
#[allow(clippy::upper_case_acronyms)]
pub struct LBP;

impl_fake_executor!(
	XYK,
	PoolType::XYK,
	XYK_SELL_CALCULATION_RESULT,
	XYK_BUY_CALCULATION_RESULT
);
impl_fake_executor!(
	StableSwap,
	PoolType::Stableswap(_),
	STABLESWAP_SELL_CALCULATION_RESULT,
	STABLESWAP_BUY_CALCULATION_RESULT
);
impl_fake_executor!(
	OmniPool,
	PoolType::Omnipool,
	OMNIPOOL_SELL_CALCULATION_RESULT,
	OMNIPOOL_BUY_CALCULATION_RESULT
);

impl_fake_executor!(
	LBP,
	PoolType::LBP,
	LBP_SELL_CALCULATION_RESULT,
	LBP_BUY_CALCULATION_RESULT
);

pub fn assert_executed_sell_trades(expected_trades: Vec<(PoolType<AssetId>, Balance, AssetId, AssetId)>) {
	EXECUTED_SELLS.with(|v| {
		let trades = v.borrow().deref().clone();
		assert_eq!(trades, expected_trades);
	});
}

pub fn assert_last_executed_sell_trades(
	number_of_trades: usize,
	expected_trades: Vec<(PoolType<AssetId>, Balance, AssetId, AssetId)>,
) {
	EXECUTED_SELLS.with(|v| {
		let trades = v.borrow().deref().clone();
		let last_trades = trades.as_slice()[trades.len() - number_of_trades..].to_vec();
		assert_eq!(last_trades, expected_trades);
	});
}

pub fn assert_executed_buy_trades(expected_trades: Vec<(PoolType<AssetId>, Balance, AssetId, AssetId)>) {
	EXECUTED_BUYS.with(|v| {
		let trades = v.borrow().deref().clone();
		assert_eq!(trades, expected_trades);
	});
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

pub fn expect_no_route_executed_event() {
	let last_events = test_utils::last_events::<RuntimeEvent, Test>(20);

	let mut events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(e, RuntimeEvent::Router(crate::Event::<Test>::RouteExecuted { .. })) {
			events.push(e);
		}
	}

	pretty_assertions::assert_eq!(
		events.len(),
		0,
		"No RouteUpdated event expected, but there is such event emitted"
	);
}
