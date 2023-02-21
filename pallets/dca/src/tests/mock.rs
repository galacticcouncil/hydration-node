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

use crate as dca;
use crate::{AMMTrader, Config, PriceProvider};
use frame_support::traits::{Everything, GenesisBuild, Nothing};
use frame_support::weights::constants::ExtrinsicBaseWeight;
use frame_support::weights::IdentityFee;
use frame_support::weights::WeightToFeeCoefficient;
use frame_support::PalletId;

use frame_support::parameter_types;
use frame_system as system;
use frame_system::EnsureRoot;
use orml_traits::parameter_type_with_key;
use pallet_currencies::BasicCurrencyAdapter;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider};
use sp_runtime::Perbill;
use sp_runtime::Permill;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup, One},
	DispatchError,
};

use sp_runtime::{DispatchResult, FixedU128};
use std::borrow::Borrow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Deref;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type BlockNumber = u64;
pub type AssetId = u32;
type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;
pub const ONE_HUNDRED_BLOCKS: BlockNumber = 100;

pub const ONE: Balance = 1_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system,
		 DCA: dca,
		 Tokens: orml_tokens,
		 MultiTransactionPayment: pallet_transaction_multi_payment,
		 TransasctionPayment: pallet_transaction_payment,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 RelaychainInfo: pallet_relaychain_info,
	 }
);

lazy_static::lazy_static! {
	pub static ref ORIGINAL_STORAGE_BOND_IN_NATIVE: Balance = 2_000_000;
}

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static FEE_ASSET: RefCell<Vec<(u64,AssetId)>> = RefCell::new(vec![(ALICE,HDX)]);
	pub static STORAGE_BOND: RefCell<Balance> = RefCell::new(*ORIGINAL_STORAGE_BOND_IN_NATIVE);
	pub static EXECUTION_BOND: RefCell<Balance> = RefCell::new(1_000_000);
	pub static SLIPPAGE: RefCell<Permill> = RefCell::new(Permill::from_percent(5));
	pub static BUY_EXECUTIONS: RefCell<Vec<BuyExecution>> = RefCell::new(vec![]);
	pub static SELL_EXECUTIONS: RefCell<Vec<SellExecution>> = RefCell::new(vec![]);
}

#[derive(Debug, PartialEq, Clone)]
pub struct BuyExecution {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_out: Balance,
	pub max_sell_amount: Balance,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SellExecution {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_in: Balance,
	pub min_buy_amount: Balance,
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
}

impl system::Config for Test {
	type BaseCallFilter = Everything;
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

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		One::one()
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
}

parameter_types! {
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;

	pub const TransactionByteFee: Balance = 10 * ONE / 100_000;

	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

pub struct WeightToFee;

impl WeightToFeePolynomial for WeightToFee {
	type Balance = Balance;

	/// Handles converting a weight scalar to a fee value, based on the scale and granularity of the
	/// node's balance type.
	///
	/// This should typically create a mapping between the following ranges:
	///   - [0, MAXIMUM_BLOCK_WEIGHT]
	///   - [Balance::min, Balance::max]
	///
	/// Yet, it can be used for any other sort of change to weight-fee. Some examples being:
	///   - Setting it to `0` will essentially disable the weight fee.
	///   - Setting it to `1` will cause the literal `#[weight = x]` values to be charged.
	fn polynomial() -> WeightToFeeCoefficients<Self::Balance> {
		// extrinsic base weight (smallest non-zero weight) is mapped to 1/10 CENT
		let p = ONE; // 1_000_000_000_000
		let q = 10 * Balance::from(ExtrinsicBaseWeight::get().ref_time()); // 7_919_840_000
		smallvec![WeightToFeeCoefficient {
			degree: 1,
			negative: false,
			coeff_frac: Perbill::from_rational(p % q, q),
			coeff_integer: p / q, // 124
		}]
	}
}

impl pallet_transaction_multi_payment::Config for Test {
	type Event = Event;
	type AcceptedCurrencyOrigin = EnsureRoot<AccountId>;
	type Currencies = Tokens;
	type SpotPriceProvider = SpotPriceProviderMock;
	type WeightInfo = ();
	type WithdrawFeeForSetCurrency = ();
	type WeightToFee = WeightToFee;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
}

impl pallet_transaction_payment::Config for Test {
	type Event = Event;
	type OnChargeTransaction = TransferFees<Currencies, MultiTransactionPayment, DepositAll<Test>>;
	type OperationalFeeMultiplier = ();
	type WeightToFee = WeightToFee;
	type FeeMultiplierUpdate = ();
	type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
}

impl pallet_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub ExecutionBondInNativeCurrency: Balance= EXECUTION_BOND.with(|v| *v.borrow());
	pub StorageBondInNativeCurrency: Balance= STORAGE_BOND.with(|v| *v.borrow());
	pub MaxSchedulePerBlock: u32 = 20;
	pub SlippageLimitPercentage: Permill = SLIPPAGE.with(|v| *v.borrow());
}

pub struct BlockNumberProviderMock {}

impl BlockNumberProvider for BlockNumberProviderMock {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		todo!()
	}
}

impl pallet_relaychain_info::Config for Test {
	type Event = Event;
	type RelaychainBlockNumberProvider = BlockNumberProviderMock;
}

pub struct AmmTraderMock {}

impl AMMTrader<Origin, AssetId, Balance> for AmmTraderMock {
	fn sell(
		_: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		min_buy_amount: Balance,
	) -> DispatchResult {
		if amount == 0 {
			return Err(DispatchError::Other("Min amount is not reached"));
		}

		SELL_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(SellExecution {
				asset_in,
				asset_out,
				amount_in: amount,
				min_buy_amount,
			});
		});

		Ok(())
	}

	fn buy(
		_: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		max_sell_amount: Balance,
	) -> DispatchResult {
		if amount == 0 {
			return Err(DispatchError::Other("Min amount is not reached"));
		}

		BUY_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(BuyExecution {
				asset_in,
				asset_out,
				amount_out: amount,
				max_sell_amount,
			});
		});

		Ok(())
	}
}

pub struct PriceProviderMock {}

impl PriceProvider<AssetId> for PriceProviderMock {
	type Price = FixedU128;

	fn spot_price(_: AssetId, _: AssetId) -> Option<Self::Price> {
		Some(FixedU128::from_float(0.8))
	}
}

pub struct SpotPriceProviderMock {}

impl SpotPriceProvider<AssetId> for SpotPriceProviderMock {
	type Price = FixedU128;

	fn pair_exists(_: AssetId, _: AssetId) -> bool {
		todo!()
	}

	fn spot_price(_: AssetId, _: AssetId) -> Option<Self::Price> {
		Some(FixedU128::from_float(0.8))
	}
}

impl Config for Test {
	type Event = Event;
	type Asset = AssetId;
	type AccountCurrencyAndPriceProvider = MultiTransactionPayment;
	type Currency = Currencies;
	type PriceProvider = PriceProviderMock;
	type RandomnessProvider = DCA;
	type StorageBondInNativeCurrency = StorageBondInNativeCurrency;
	type MaxSchedulePerBlock = MaxSchedulePerBlock;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
	type WeightToFee = IdentityFee<Balance>;
	type SlippageLimitPercentage = SlippageLimitPercentage;
	type WeightInfo = ();
	type AMMTrader = AmmTraderMock;
}
use frame_support::weights::{ConstantMultiplier, WeightToFeeCoefficients, WeightToFeePolynomial};
use hydradx_traits::pools::SpotPriceProvider;
use pallet_transaction_multi_payment::{DepositAll, TransferFees};
use smallvec::smallvec;

pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	init_pool: Option<(FixedU128, FixedU128)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
			registered_assets: vec![],
			init_pool: None,
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
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

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.filter(|a| a.1 == HDX)
				.flat_map(|(x, _, amount)| vec![(*x, *amount)])
				.collect(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

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
			FEE_ASSET.borrow().with(|v| {
				let user_and_fee_assets = v.borrow().deref().clone();
				for fee_asset in user_and_fee_assets {
					let _ = MultiTransactionPayment::add_currency(
						Origin::root(),
						fee_asset.1,
						FixedU128::from_inner(1000000),
					);
					let _ = MultiTransactionPayment::set_currency(Origin::signed(fee_asset.0), fee_asset.1);
				}
			});
		});

		r
	}
}

pub fn expect_events(e: Vec<Event>) {
	test_utils::expect_events::<Event, Test>(e);
}

#[macro_export]
macro_rules! assert_executed_sell_trades {
	($expected_trades:expr) => {{
		SELL_EXECUTIONS.borrow().with(|v| {
			let trades = v.borrow().clone();
			assert_eq!(trades, $expected_trades);
		});
	}};
}

#[macro_export]
macro_rules! assert_executed_buy_trades {
	($expected_trades:expr) => {{
		BUY_EXECUTIONS.borrow().with(|v| {
			let trades = v.borrow().clone();
			assert_eq!(trades, $expected_trades);
		});
	}};
}

#[macro_export]
macro_rules! assert_number_of_executed_buy_trades {
	($number_of_trades:expr) => {{
		BUY_EXECUTIONS.borrow().with(|v| {
			let trades = v.borrow().clone();
			assert_eq!(trades.len(), $number_of_trades);
		});
	}};
}

#[macro_export]
macro_rules! assert_number_of_executed_sell_trades {
	($number_of_trades:expr) => {{
		SELL_EXECUTIONS.borrow().with(|v| {
			let trades = v.borrow().clone();
			assert_eq!(trades.len(), $number_of_trades);
		});
	}};
}
