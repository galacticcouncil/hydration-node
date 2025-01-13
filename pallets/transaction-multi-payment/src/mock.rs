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

use frame_support::dispatch::{DispatchResultWithPostInfo, PostDispatchInfo};
use frame_support::{
	dispatch::DispatchClass,
	parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentifyAccount, IdentityLookup, Verify},
		BuildStorage, MultiSignature, Perbill,
	},
	traits::{Everything, Get, Nothing},
	weights::{IdentityFee, Weight},
};
use frame_system as system;
use hydradx_traits::{
	router::{RouteProvider, Trade},
	AssetKind, OraclePeriod, PriceOracle,
};
use orml_traits::{currency::MutationHooks, parameter_type_with_key};
use pallet_currencies::{BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use sp_core::{H160, H256, U256};
use sp_runtime::DispatchError;
use sp_std::cell::RefCell;

pub type AccountId = <<MultiSignature as Verify>::Signer as IdentifyAccount>::AccountId;
pub type Balance = u128;
pub type AssetId = u32;
pub type Amount = i128;

pub const INITIAL_BALANCE: Balance = 1_000_000_000_000_000u128;

pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const CHARLIE: AccountId = AccountId::new([3; 32]);
pub const DAVE: AccountId = AccountId::new([4; 32]);
pub const FEE_RECEIVER: AccountId = AccountId::new([5; 32]);

pub const HDX: AssetId = 0;
pub const WETH: AssetId = 20;
pub const DOT: AssetId = 5;
pub const SUPPORTED_CURRENCY: AssetId = 2000;
pub const SUPPORTED_CURRENCY_WITH_PRICE: AssetId = 3000;
pub const UNSUPPORTED_CURRENCY: AssetId = 4000;
pub const INSUFFICIENT_CURRENCY: AssetId = 10000;
pub const SUPPORTED_CURRENCY_NO_BALANCE: AssetId = 5000; // Used for insufficient balance testing
pub const HIGH_ED_CURRENCY: AssetId = 6000;
pub const HIGH_VALUE_CURRENCY: AssetId = 7000;

pub const HIGH_ED: Balance = 5;

const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);
const MAX_BLOCK_WEIGHT: Weight = Weight::from_parts(1024, 0);

thread_local! {
	static EXTRINSIC_BASE_WEIGHT: RefCell<Weight> = const { RefCell::new(Weight::zero()) };
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
		 EVMAccounts: pallet_evm_accounts,
		 Utility: pallet_utility,
	 }

);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;

	pub const HdxAssetId: u32 = HDX;
	pub const EvmAssetId: u32 = WETH;
	pub const DotAssetId: u32 = DOT;
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
	type RuntimeTask = RuntimeTask;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
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
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AcceptedCurrencyOrigin = frame_system::EnsureRoot<AccountId>;
	type Currencies = Currencies;
	type RouteProvider = DefaultRouteProvider;
	type OraclePriceProvider = PriceProviderMock;
	type WeightInfo = ();
	type WeightToFee = IdentityFee<Balance>;
	type NativeAssetId = HdxAssetId;
	type PolkadotNativeAssetId = DotAssetId;
	type EvmAssetId = EvmAssetId;
	type InspectEvmAccounts = EVMAccounts;
	type EvmPermit = PermitDispatchHandler;
	type TryCallCurrency<'a> = NoCallCurrency<Test>;
	type SwappablePaymentAssetSupport = MockedInsufficientAssetSupport;
}

pub struct MockedInsufficientAssetSupport;

impl InspectTransactionFeeCurrency<AssetId> for MockedInsufficientAssetSupport {
	fn is_transaction_fee_currency(_asset: AssetId) -> bool {
		true
	}
}

impl SwappablePaymentAssetTrader<AccountId, AssetId, Balance> for MockedInsufficientAssetSupport {
	fn is_trade_supported(_from: AssetId, _into: AssetId) -> bool {
		unimplemented!()
	}

	fn buy(
		_origin: &AccountId,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_amount: Balance,
		_max_limit: Balance,
		_dest: &AccountId,
	) -> DispatchResult {
		unimplemented!()
	}

	fn calculate_fee_amount(_swap_amount: Balance) -> Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn calculate_in_given_out(
		_insuff_asset_id: AssetId,
		_asset_out: AssetId,
		_asset_out_amount: Balance,
	) -> Result<Balance, DispatchError> {
		unimplemented!()
	}

	fn calculate_out_given_in(
		_asset_in: AssetId,
		_asset_out: AssetId,
		_asset_in_amount: Balance,
	) -> Result<Balance, DispatchError> {
		unimplemented!()
	}
}

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> hydradx_traits::registry::Inspect for DummyRegistry<T> {
	type AssetId = AssetId;
	type Location = u8;

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		unimplemented!()
	}

	fn is_sufficient(id: Self::AssetId) -> bool {
		id < INSUFFICIENT_CURRENCY
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		unimplemented!()
	}

	fn exists(_asset_id: AssetId) -> bool {
		true
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		unimplemented!()
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		unimplemented!()
	}
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
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

impl pallet_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = TransferFees<Currencies, DepositAll<Test>, FeeReceiver>;
	type LengthToFee = IdentityFee<Balance>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = IdentityFee<Balance>;
	type FeeMultiplierUpdate = ();
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
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type GetNativeCurrencyId = HdxAssetId;
	type WeightInfo = ();
}

pub struct EvmNonceProvider;
impl pallet_evm_accounts::EvmNonceProvider for EvmNonceProvider {
	fn get_nonce(_: sp_core::H160) -> sp_core::U256 {
		sp_core::U256::zero()
	}
}

impl pallet_evm_accounts::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type EvmNonceProvider = EvmNonceProvider;
	type FeeMultiplier = frame_support::traits::ConstU32<10>;
	type ControllerOrigin = frame_system::EnsureRoot<AccountId>;
	type WeightInfo = ();
}

impl pallet_utility::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type BatchHook = ();
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

#[derive(Clone, Debug, PartialEq)]
pub struct PermitDispatchData {
	pub source: H160,
	pub target: H160,
	pub input: Vec<u8>,
	pub value: U256,
	pub gas_limit: u64,
	pub max_fee_per_gas: U256,
	pub max_priority_fee_per_gas: Option<U256>,
	pub nonce: Option<U256>,
	pub access_list: Vec<(H160, Vec<H256>)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValidationData {
	pub source: H160,
	pub target: H160,
	pub input: Vec<u8>,
	pub value: U256,
	pub gas_limit: u64,
	pub deadline: U256,
	pub v: u8,
	pub r: H256,
	pub s: H256,
}

thread_local! {
	static PERMIT_VALIDATION: RefCell<Vec<ValidationData>> = const { RefCell::new(vec![]) };
	static PERMIT_DISPATCH: RefCell<Vec<PermitDispatchData>> = const { RefCell::new(vec![]) };
}

pub struct PermitDispatchHandler;

impl PermitDispatchHandler {
	pub fn last_validation_call_data() -> ValidationData {
		PERMIT_VALIDATION.with(|v| v.borrow().last().unwrap().clone())
	}

	pub fn last_dispatch_call_data() -> PermitDispatchData {
		PERMIT_DISPATCH.with(|v| v.borrow().last().unwrap().clone())
	}
}

impl EVMPermit for PermitDispatchHandler {
	fn validate_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		deadline: U256,
		v: u8,
		r: H256,
		s: H256,
	) -> sp_runtime::DispatchResult {
		let data = ValidationData {
			source,
			target,
			input,
			value,
			gas_limit,
			deadline,
			v,
			r,
			s,
		};
		PERMIT_VALIDATION.with(|v| v.borrow_mut().push(data));
		Ok(())
	}

	fn dispatch_permit(
		source: H160,
		target: H160,
		input: Vec<u8>,
		value: U256,
		gas_limit: u64,
		max_fee_per_gas: U256,
		max_priority_fee_per_gas: Option<U256>,
		nonce: Option<U256>,
		access_list: Vec<(H160, Vec<H256>)>,
	) -> DispatchResultWithPostInfo {
		let data = PermitDispatchData {
			source,
			target,
			input,
			value,
			gas_limit,
			max_fee_per_gas,
			max_priority_fee_per_gas,
			nonce,
			access_list,
		};
		PERMIT_DISPATCH.with(|v| v.borrow_mut().push(data));
		Ok(PostDispatchInfo::default())
	}

	fn gas_price() -> (U256, Weight) {
		(U256::from(222u128), Weight::zero())
	}

	fn dispatch_weight(_gas_limit: u64) -> Weight {
		todo!()
	}

	fn permit_nonce(_account: H160) -> U256 {
		U256::default()
	}

	fn on_dispatch_permit_error() {}
}
