// This file is part of Basilisk-node.

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

use super::*;
pub use crate as multi_payment;
use crate::Config;
use hydra_dx_math::types::Ratio;

use crate as dynamic_evm_fee;
use crate::types::MultiplierProvider;
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	traits::{Everything, Get, Nothing},
	weights::{IdentityFee, Weight},
};
use frame_system as system;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::router::{RouteProvider, Trade};
use hydradx_traits::{AssetPairAccountIdFor, NativePriceOracle, OraclePeriod, PriceOracle};
use orml_traits::currency::MutationHooks;
use orml_traits::parameter_type_with_key;
use pallet_currencies::BasicCurrencyAdapter;
use pallet_transaction_payment::Multiplier;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, FixedPointNumber, FixedU128, Perbill,
};
use sp_std::cell::RefCell;
pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;
pub type Amount = i128;

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000_000u128;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const FEE_RECEIVER: AccountId = 300;

pub const HDX: AssetId = 0;
pub const WETH: AssetId = 1;
pub const SUPPORTED_CURRENCY: AssetId = 2000;
pub const SUPPORTED_CURRENCY_WITH_PRICE: AssetId = 3000;
pub const UNSUPPORTED_CURRENCY: AssetId = 4000;
pub const SUPPORTED_CURRENCY_NO_BALANCE: AssetId = 5000; // Used for insufficient balance testing
pub const HIGH_ED_CURRENCY: AssetId = 6000;
pub const HIGH_VALUE_CURRENCY: AssetId = 7000;

pub const HIGH_ED: Balance = 5;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAX_BLOCK_WEIGHT: Weight = Weight::from_parts(1024, 0);
pub const DEFAULT_ETH_HDX_ORACLE_PRICE: Ratio = Ratio::new(16420844565569051996, FixedU128::DIV);

thread_local! {
	static EXTRINSIC_BASE_WEIGHT: RefCell<Weight> = RefCell::new(Weight::zero());
	static MULTIPLIER: RefCell<Multiplier> = RefCell::new(Multiplier::from_rational(1,1000));
	static ETH_HDX_ORACLE_PRICE: RefCell<Ratio> = RefCell::new(DEFAULT_ETH_HDX_ORACLE_PRICE);
}

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 Tokens: orml_tokens,
		 DynamicEvmFee: dynamic_evm_fee,
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;

	pub const HdxAssetId: u32 = HDX;
	pub const WethAssetId: u32 = WETH;
	pub const ExistentialDeposit: u128 = 2;
	pub const MaxLocks: u32 = 50;
	pub const RegistryStringLimit: u32 = 100;
	pub const FeeReceiver: AccountId = FEE_RECEIVER;



	pub ExchangeFeeRate: (u32, u32) = (2, 1_000);
}

impl system::Config for Test {
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
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = SS58Prefix;
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

pub struct MultiplierProviderMock;

impl MultiplierProvider for MultiplierProviderMock {
	fn next() -> Multiplier {
		MULTIPLIER.with(|v| *v.borrow())
	}
}

pub struct NativePriceOracleMock;

impl NativePriceOracle<AssetId, EmaPrice> for NativePriceOracleMock {
	fn price(_: AssetId) -> Option<EmaPrice> {
		Some(ETH_HDX_ORACLE_PRICE.with(|v| *v.borrow()))
	}
}

pub struct DefaultBaseDFeePerGas;

impl Get<u128> for DefaultBaseDFeePerGas {
	fn get() -> u128 {
		80_000_000 / 3
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type DefaultBaseFeePerGas = DefaultBaseDFeePerGas;
	type Multiplier = MultiplierProviderMock;
	type NativePriceOracle = NativePriceOracleMock;

	type WethAssetId = HdxAssetId;
}

pub struct DefaultRouteProvider;

impl RouteProvider<AssetId> for DefaultRouteProvider {}

pub struct PriceProviderMock {}

impl PriceOracle<AssetId> for PriceProviderMock {
	type Price = Ratio;

	fn price(route: &[Trade<AssetId>], _period: OraclePeriod) -> Option<Ratio> {
		let asset_a = route.first().unwrap().asset_in;
		let asset_b = route.first().unwrap().asset_out;
		match (asset_a, asset_b) {
			(SUPPORTED_CURRENCY_WITH_PRICE, HDX) => Some(Ratio::new(1, 10)),
			_ => None,
		}
	}
}
impl pallet_balances::Config for Test {
	type MaxLocks = MaxLocks;
	/// The type for recording an account's balance.
	type Balance = Balance;
	/// The ubiquitous event type.
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub const MaxReserves: u32 = 50;
}
parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		match *currency_id {
			HIGH_ED_CURRENCY => HIGH_ED,
			HIGH_VALUE_CURRENCY => 1u128,
			_ => 2u128
		}
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
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = HdxAssetId;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	base_weight: Weight,
	native_balances: Vec<(AccountId, Balance)>,
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	account_currencies: Vec<(AccountId, AssetId)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			base_weight: Weight::zero(),
			native_balances: vec![(ALICE, INITIAL_BALANCE)],
			endowed_accounts: vec![
				(ALICE, HDX, INITIAL_BALANCE),
				(ALICE, SUPPORTED_CURRENCY, INITIAL_BALANCE), // used for fallback price test
				(ALICE, SUPPORTED_CURRENCY_WITH_PRICE, INITIAL_BALANCE),
			],

			account_currencies: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn base_weight(mut self, base_weight: u64) -> Self {
		self.base_weight = Weight::from_parts(base_weight, 0);
		self
	}
	pub fn account_native_balance(mut self, account: AccountId, balance: Balance) -> Self {
		self.native_balances.push((account, balance));
		self
	}
	pub fn account_tokens(mut self, account: AccountId, asset: AssetId, balance: Balance) -> Self {
		self.endowed_accounts.push((account, asset, balance));
		self
	}
	pub fn with_currencies(mut self, account_currencies: Vec<(AccountId, AssetId)>) -> Self {
		self.account_currencies = account_currencies;
		self
	}
	fn set_constants(&self) {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow_mut() = self.base_weight);
	}
	pub fn build(self) -> sp_io::TestExternalities {
		use frame_support::traits::OnInitialize;

		self.set_constants();
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self.native_balances,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let core_asset: u32 = 0;
		let mut buf: Vec<u8> = Vec::new();

		buf.extend_from_slice(&core_asset.to_le_bytes());
		buf.extend_from_slice(b"HDT");
		buf.extend_from_slice(&core_asset.to_le_bytes());

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
			// Make sure the prices are up-to-date.
		});
		ext
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

pub fn set_multiplier(multiplier: Multiplier) {
	MULTIPLIER.with(|v| {
		*v.borrow_mut() = multiplier;
	});
}
pub fn set_oracle_price(price: Ratio) {
	ETH_HDX_ORACLE_PRICE.with(|v| {
		*v.borrow_mut() = price;
	});
}
