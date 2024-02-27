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

pub use crate as multi_payment;
use crate::Config;
use hydra_dx_math::types::Ratio;

use crate as dynamic_evm_fee;
use frame_support::{
	parameter_types,
	traits::{Everything, Get, Nothing},
	weights::Weight,
};
use frame_system as system;
use hydra_dx_math::ema::EmaPrice;
use hydradx_traits::router::RouteProvider;
use hydradx_traits::NativePriceOracle;
use orml_traits::parameter_type_with_key;
use pallet_currencies::BasicCurrencyAdapter;
use pallet_transaction_payment::Multiplier;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup},
	BuildStorage, FixedPointNumber, FixedU128,
};
use sp_std::cell::RefCell;
pub type AccountId = u64;
pub type Balance = u128;
pub type AssetId = u32;
pub type Amount = i128;

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000_000u128;

pub const ALICE: AccountId = 1;
pub const FEE_RECEIVER: AccountId = 300;

pub const HDX: AssetId = 0;
pub const WETH: AssetId = 1;
pub const SUPPORTED_CURRENCY: AssetId = 2000;
pub const SUPPORTED_CURRENCY_WITH_PRICE: AssetId = 3000;
pub const HIGH_ED_CURRENCY: AssetId = 6000;
pub const HIGH_VALUE_CURRENCY: AssetId = 7000;

pub const HIGH_ED: Balance = 5;

pub const DEFAULT_ETH_HDX_ORACLE_PRICE: Ratio = Ratio::new(8945857934143137845, FixedU128::DIV);

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

impl Get<FixedU128> for MultiplierProviderMock {
	fn get() -> Multiplier {
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
		15_000_000
	}
}

pub struct MinBaseFeePerGas;
impl Get<u128> for MinBaseFeePerGas {
	fn get() -> u128 {
		15_000_000 / 10
	}
}

pub struct MaxBaseFeePerGas;
impl Get<u128> for MaxBaseFeePerGas {
	fn get() -> u128 {
		14415000000
	}
}

impl Config for Test {
	type AssetId = AssetId;
	type MinBaseFeePerGas = MinBaseFeePerGas;
	type MaxBaseFeePerGas = MaxBaseFeePerGas;
	type DefaultBaseFeePerGas = DefaultBaseDFeePerGas;
	type FeeMultiplier = MultiplierProviderMock;
	type NativePriceOracle = NativePriceOracleMock;
	type WethAssetId = HdxAssetId;
	type WeightInfo = ();
}

pub struct DefaultRouteProvider;

impl RouteProvider<AssetId> for DefaultRouteProvider {}

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
		}
	}
}

impl ExtBuilder {
	fn set_constants(&self) {
		EXTRINSIC_BASE_WEIGHT.with(|v| *v.borrow_mut() = self.base_weight);
	}
	pub fn build(self) -> sp_io::TestExternalities {
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
