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
use crate::{AMMTrader, Config};
use frame_support::traits::{Everything, GenesisBuild, Nothing};
use frame_support::weights::constants::ExtrinsicBaseWeight;
use frame_support::weights::IdentityFee;
use frame_support::weights::WeightToFeeCoefficient;
use frame_support::PalletId;

use frame_support::{assert_ok, parameter_types};
use frame_system as system;
use frame_system::EnsureRoot;
use hydradx_traits::{OraclePeriod, PriceOracle, Registry};
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
use std::cell::RefCell;
use std::collections::HashMap;
type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type BlockNumber = u64;
pub type AssetId = u32;
type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;
pub const REGISTERED_ASSET: AssetId = 1000;
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
		 Omnipool: pallet_omnipool,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 RelaychainInfo: pallet_relaychain_info,
	 }
);

lazy_static::lazy_static! {
	pub static ref ORIGINAL_STORAGE_BOND_IN_NATIVE: Balance = 2_000_000;
	pub static ref ORIGINAL_MAX_PRICE_DIFFERENCE: Permill = Permill::from_percent(10);
}

thread_local! {
	pub static POSITIONS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::default());
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static ASSET_WEIGHT_CAP: RefCell<Permill> = RefCell::new(Permill::from_percent(100));
	pub static ASSET_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static PROTOCOL_FEE: RefCell<Permill> = RefCell::new(Permill::from_percent(0));
	pub static MIN_ADDED_LIQUDIITY: RefCell<Balance> = RefCell::new(1000u128);
	pub static MIN_TRADE_AMOUNT: RefCell<Balance> = RefCell::new(1000u128);
	pub static MAX_IN_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static MAX_OUT_RATIO: RefCell<Balance> = RefCell::new(1u128);
	pub static FEE_ASSET: RefCell<Vec<(u64,AssetId)>> = RefCell::new(vec![(ALICE,HDX)]);
	pub static STORAGE_BOND: RefCell<Balance> = RefCell::new(*ORIGINAL_STORAGE_BOND_IN_NATIVE);
	pub static SLIPPAGE: RefCell<Permill> = RefCell::new(Permill::from_percent(5));
	pub static BUY_EXECUTIONS: RefCell<Vec<BuyExecution>> = RefCell::new(vec![]);
	pub static SELL_EXECUTIONS: RefCell<Vec<SellExecution>> = RefCell::new(vec![]);
	pub static SET_OMNIPOOL_ON: RefCell<bool> = RefCell::new(true);
	pub static MAX_PRICE_DIFFERENCE: RefCell<Permill> = RefCell::new(*ORIGINAL_MAX_PRICE_DIFFERENCE);
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct BuyExecution {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
	pub amount_out: Balance,
	pub max_sell_amount: Balance,
}

#[derive(Debug, PartialEq, Eq, Clone)]
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
		pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const DAIAssetId: AssetId = DAI;
	pub const PosiitionCollectionId: u32= 1000;

	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;
	pub ProtocolFee: Permill = PROTOCOL_FEE.with(|v| *v.borrow());
	pub AssetFee: Permill = ASSET_FEE.with(|v| *v.borrow());
	pub AssetWeightCap: Permill =ASSET_WEIGHT_CAP.with(|v| *v.borrow());
	pub MinAddedLiquidity: Balance = MIN_ADDED_LIQUDIITY.with(|v| *v.borrow());
	pub MinTradeAmount: Balance = MIN_TRADE_AMOUNT.with(|v| *v.borrow());
	pub MaxInRatio: Balance = MAX_IN_RATIO.with(|v| *v.borrow());
	pub MaxOutRatio: Balance = MAX_OUT_RATIO.with(|v| *v.borrow());
	pub const TVLCap: Balance = Balance::MAX;

	pub const TransactionByteFee: Balance = 10 * ONE / 100_000;

	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

impl pallet_omnipool::Config for Test {
	type Event = Event;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type HubAssetId = LRNAAssetId;
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
	type StableCoinAssetId = DAIAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PosiitionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = DummyRegistry<Test>;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u32;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type OmnipoolHooks = ();
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
	pub StorageBondInNativeCurrency: Balance= STORAGE_BOND.with(|v| *v.borrow());
	pub MaxSchedulePerBlock: u32 = 20;
	pub SlippageLimitPercentage: Permill = SLIPPAGE.with(|v| *v.borrow());
	pub OmnipoolMaxAllowedPriceDifference: Permill = MAX_PRICE_DIFFERENCE.with(|v| *v.borrow());
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
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		min_buy_amount: Balance,
	) -> DispatchResult {
		if amount == 0 {
			return Err(DispatchError::Other("Min amount is not reached"));
		}

		//We only want to excecute omnipool trade in case of benchmarking
		Self::execute_trade_in_omnipool(origin, asset_in, asset_out, amount, min_buy_amount)?;

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
		_origin: Origin,
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

impl AmmTraderMock {
	fn execute_trade_in_omnipool(
		origin: Origin,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
		min_buy_amount: Balance,
	) -> DispatchResult {
		let mut set_omnipool_on = true;
		SET_OMNIPOOL_ON.with(|v| {
			let omnipool_on = v.borrow_mut();
			set_omnipool_on = *omnipool_on;
		});
		if set_omnipool_on {
			Omnipool::sell(origin, asset_in, asset_out, amount, min_buy_amount)?;
		}

		Ok(())
	}
}

pub struct PriceProviderMock {}

impl PriceOracle<AssetId, Ratio> for PriceProviderMock {
	fn price(_: AssetId, _: AssetId, _: OraclePeriod) -> Option<Ratio> {
		Some(Ratio::new(88, 100))
	}
}

pub struct SpotPriceProviderMock {}

impl SpotPriceProvider<AssetId> for SpotPriceProviderMock {
	type Price = FixedU128;

	fn pair_exists(_: AssetId, _: AssetId) -> bool {
		todo!()
	}

	fn spot_price(_: AssetId, _: AssetId) -> Option<Self::Price> {
		Some(FixedU128::from_rational(80, 100))
	}
}

impl Config for Test {
	type Event = Event;
	type Asset = AssetId;
	type Currency = Currencies;
	type RandomnessProvider = DCA;
	type StorageBondInNativeCurrency = StorageBondInNativeCurrency;
	type MaxSchedulePerBlock = MaxSchedulePerBlock;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
	type WeightToFee = IdentityFee<Balance>;
	type SlippageLimitPercentage = SlippageLimitPercentage;
	type WeightInfo = ();
	type AMMTrader = AmmTraderMock;
	type OraclePriceProvider = PriceProviderMock;
	type SpotPriceProvider = SpotPriceProviderMock;
	type MaxPriceDifference = OmnipoolMaxAllowedPriceDifference;
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::weights::{WeightToFeeCoefficients, WeightToFeePolynomial};
use hydra_dx_math::types::Ratio;
use hydradx_traits::pools::SpotPriceProvider;
use smallvec::smallvec;

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

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Registry<T::Asset, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	T::Asset: Into<AssetId> + From<u32>,
{
	fn exists(asset_id: T::Asset) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<T::Asset, DispatchError> {
		Ok(T::Asset::default())
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::Asset, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(T::Asset::from(assigned))
	}
}

pub type AccountId = u64;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
	asset_weight_cap: Permill,
	register_stable_asset: bool,
	init_pool: Option<(FixedU128, FixedU128)>,
	pool_tokens: Vec<(AssetId, FixedU128, AccountId, Balance)>,
	omnipool_trade: bool,
	max_price_difference: Permill,
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

		Self {
			endowed_accounts: vec![(Omnipool::protocol_account(), DAI, 1000 * ONE)],
			asset_weight_cap: Permill::from_percent(100),
			registered_assets: vec![],
			init_pool: None,
			register_stable_asset: true,
			pool_tokens: vec![],
			omnipool_trade: false,
			max_price_difference: Permill::from_percent(10),
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_max_price_difference(mut self, price_diff: Permill) -> Self {
		self.max_price_difference = price_diff;
		self
	}

	#[allow(dead_code)] //This is used only in benchmark but it complains with warning
	pub fn with_omnipool_trade(mut self, omnipool_is_on: bool) -> Self {
		self.omnipool_trade = omnipool_is_on;
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();
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

		SET_OMNIPOOL_ON.with(|v| {
			*v.borrow_mut() = self.omnipool_trade;
		});

		MAX_PRICE_DIFFERENCE.with(|v| {
			*v.borrow_mut() = self.max_price_difference;
		});

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

		if let Some((stable_price, native_price)) = self.init_pool {
			r.execute_with(|| {
				assert_ok!(Omnipool::set_tvl_cap(Origin::root(), u128::MAX));

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
			});
		}

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
