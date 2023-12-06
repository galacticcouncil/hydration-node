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
use crate::{Config, TransferFees};
use hydra_dx_math::types::Ratio;

use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	traits::{Everything, Get, Nothing},
	weights::{IdentityFee, Weight},
};
use frame_system as system;
use hydradx_traits::router::{RouteProvider, Trade};
use hydradx_traits::{AssetPairAccountIdFor, OraclePeriod, PriceOracle};
use orml_traits::currency::MutationHooks;
use orml_traits::parameter_type_with_key;
use pallet_currencies::BasicCurrencyAdapter;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, Perbill,
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
pub const SUPPORTED_CURRENCY: AssetId = 2000;
pub const SUPPORTED_CURRENCY_WITH_PRICE: AssetId = 3000;
pub const UNSUPPORTED_CURRENCY: AssetId = 4000;
pub const SUPPORTED_CURRENCY_NO_BALANCE: AssetId = 5000; // Used for insufficient balance testing
pub const HIGH_ED_CURRENCY: AssetId = 6000;
pub const HIGH_VALUE_CURRENCY: AssetId = 7000;

pub const HIGH_ED: Balance = 5;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAX_BLOCK_WEIGHT: Weight = Weight::from_parts(1024, 0);

thread_local! {
	static EXTRINSIC_BASE_WEIGHT: RefCell<Weight> = RefCell::new(Weight::zero());
}

pub struct ExtrinsicBaseWeight;
impl Get<Weight> for ExtrinsicBaseWeight {
	fn get() -> Weight {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow())
	}
}

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 PaymentPallet: multi_payment,
		 TransactionPayment: pallet_transaction_payment,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 Tokens: orml_tokens,
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;

	pub const HdxAssetId: u32 = HDX;
	pub const ExistentialDeposit: u128 = 2;
	pub const MaxLocks: u32 = 50;
	pub const RegistryStringLimit: u32 = 100;
	pub const FeeReceiver: AccountId = FEE_RECEIVER;

	pub RuntimeBlockWeights: system::limits::BlockWeights = system::limits::BlockWeights::builder()
		.base_block(Weight::zero())
		.for_class(DispatchClass::all(), |weights| {
			weights.base_extrinsic = ExtrinsicBaseWeight::get();
		})
		.for_class(DispatchClass::Normal, |weights| {
			weights.max_total = (NORMAL_DISPATCH_RATIO * MAX_BLOCK_WEIGHT).set_proof_size(u64::MAX).into();
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = (NORMAL_DISPATCH_RATIO * MAX_BLOCK_WEIGHT).set_proof_size(u64::MAX).into();
		})
		.for_class(DispatchClass::Operational, |weights| {
			weights.max_total = MAX_BLOCK_WEIGHT.set_proof_size(u64::MAX).into();
		})
		.avg_block_initialization(Perbill::from_percent(0))
		.build_or_panic();

	pub ExchangeFeeRate: (u32, u32) = (2, 1_000);
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = RuntimeBlockWeights;
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

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AcceptedCurrencyOrigin = frame_system::EnsureRoot<u64>;
	type Currencies = Currencies;
	type RouteProvider = DefaultRouteProvider;
	type OraclePriceProvider = PriceProviderMock;
	type WeightInfo = ();
	type WeightToFee = IdentityFee<Balance>;
	type NativeAssetId = HdxAssetId;
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

impl pallet_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransferFees<Currencies, DepositAll<Test>, FeeReceiver>;
	type LengthToFee = IdentityFee<Balance>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
}
pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b)
		}
		(a * 1000 + b) as u64
	}
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

parameter_types! {
	pub const MaxReserves: u32 = 50;
}

pub struct CurrencyHooks;
impl MutationHooks<AccountId, AssetId, Balance> for CurrencyHooks {
	type OnDust = ();
	type OnSlash = ();
	type PreDeposit = ();
	type PostDeposit = ();
	type PreTransfer = ();
	type PostTransfer = ();
	type OnNewTokenAccount = AddTxAssetOnAccount<Test>;
	type OnKilledTokenAccount = RemoveTxAssetOnKilled<Test>;
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
	type CurrencyHooks = CurrencyHooks;
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

		crate::GenesisConfig::<Test> {
			currencies: vec![
				(SUPPORTED_CURRENCY_NO_BALANCE, Price::from(1)),
				(SUPPORTED_CURRENCY, Price::from_float(1.5)),
				(SUPPORTED_CURRENCY_WITH_PRICE, Price::from_float(0.5)),
				(HIGH_ED_CURRENCY, Price::from(3)),
				(HIGH_VALUE_CURRENCY, Price::from_inner(100)),
			],
			account_currencies: self.account_currencies,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			System::set_block_number(1);
			// Make sure the prices are up-to-date.
			PaymentPallet::on_initialize(1);
		});
		ext
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}
