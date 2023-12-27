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
use crate::{Config, Error, RandomnessProvider, RelayChainBlockHashProvider};
use cumulus_primitives_core::relay_chain::Hash;
use frame_support::traits::{Everything, Nothing};
use frame_support::weights::constants::ExtrinsicBaseWeight;
use frame_support::weights::WeightToFeeCoefficient;
use frame_support::weights::{IdentityFee, Weight};
use frame_support::PalletId;

use frame_support::BoundedVec;
use frame_support::{assert_ok, parameter_types};
use frame_system as system;
use frame_system::{ensure_signed, EnsureRoot};
use hydradx_traits::{AssetKind, NativePriceOracle, OraclePeriod, PriceOracle, Registry};
use orml_traits::{parameter_type_with_key, GetByKey};
use pallet_currencies::BasicCurrencyAdapter;
use primitive_types::U128;
use sp_core::H256;
use sp_runtime::traits::{AccountIdConversion, BlockNumberProvider, ConstU32};
use sp_runtime::Perbill;
use sp_runtime::Permill;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, One},
	BuildStorage, DispatchError,
};

use hydra_dx_math::support::rational::{round_to_rational, Rounding};
use sp_runtime::traits::Zero;
use sp_runtime::{DispatchResult, FixedU128};
use std::cell::RefCell;
use std::collections::HashMap;

type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type BlockNumber = u64;
pub type AssetId = u32;
type NamedReserveIdentifier = [u8; 8];

pub const BUY_DCA_FEE_IN_NATIVE: Balance = 1374044000;
pub const BUY_DCA_FEE_IN_DAI: Balance = 1209158720;
pub const SELL_DCA_FEE_IN_NATIVE: Balance = 1374576000;
pub const SELL_DCA_FEE_IN_DAI: Balance = 1209626880;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;
pub const FORBIDDEN_ASSET: AssetId = 4;
pub const REGISTERED_ASSET: AssetId = 1000;
pub const ONE_HUNDRED_BLOCKS: BlockNumber = 100;

//Since we always use the same parent hash in the tests, the generated radiuses are always the same
pub const GENERATED_SEARCH_RADIUSES: [u64; 10] = [1, 3, 6, 10, 28, 34, 114, 207, 504, 947];

pub const ONE: Balance = 1_000_000_000_000;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 DCA: dca,
		 Tokens: orml_tokens,
		 RouteExecutor: pallet_route_executor,
		 Omnipool: pallet_omnipool,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 EmaOracle: pallet_ema_oracle,
	 }
);

lazy_static::lazy_static! {
	pub static ref ORIGINAL_MIN_BUDGET_IN_NATIVE: Balance = 2_000_000;
	pub static ref ORIGINAL_MAX_PRICE_DIFFERENCE: Permill = Permill::from_percent(10);
	pub static ref AMOUNT_OUT_FOR_OMNIPOOL_SELL: Balance = 20 * ONE;
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
	pub static MIN_BUDGET: RefCell<Balance> = RefCell::new(*ORIGINAL_MIN_BUDGET_IN_NATIVE);
	pub static BUY_EXECUTIONS: RefCell<Vec<BuyExecution>> = RefCell::new(vec![]);
	pub static SELL_EXECUTIONS: RefCell<Vec<SellExecution>> = RefCell::new(vec![]);
	pub static SET_OMNIPOOL_ON: RefCell<bool> = RefCell::new(true);
	pub static MAX_PRICE_DIFFERENCE: RefCell<Permill> = RefCell::new(*ORIGINAL_MAX_PRICE_DIFFERENCE);
	pub static WITHDRAWAL_ADJUSTMENT: RefCell<(u32,u32, bool)> = RefCell::new((0u32,0u32, false));
	pub static CALCULATED_AMOUNT_OUT_FOR_SELL: RefCell<Balance> = RefCell::new(*AMOUNT_OUT_FOR_OMNIPOOL_SELL);
	pub static USE_PROD_RANDOMNESS: RefCell<bool> = RefCell::new(false);
	pub static PARENT_HASH: RefCell<Option<Hash>> = RefCell::new(Some([
			14, 87, 81, 192, 38, 229, 67, 178, 232, 171, 46, 176, 96, 153, 218, 161, 209, 229, 223, 71, 119, 143, 119,
			135, 250, 171, 69, 205, 241, 47, 227, 168,
		]
		.into()));

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

//NOTE: oracle is only used for benchmarking to have price from it
use pallet_ema_oracle::MAX_PERIODS;

parameter_types! {
	pub static MockBlockNumberProvider: u64 = 0;
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

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		System::block_number()
	}
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
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

pub type Amount = i128;

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		One::one()
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
	type ReserveIdentifier = NamedReserveIdentifier;
	type MaxReserves = MaxReserves;
	type CurrencyHooks = ();
}

parameter_types! {
		pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
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
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type HubAssetId = LRNAAssetId;
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
	type PriceBarrier = ();
	type MinWithdrawalFee = ();
	type ExternalPriceOracle = WithdrawFeePriceOracle;
	type Fee = FeeProvider;
}

pub struct WithdrawFeePriceOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for WithdrawFeePriceOracle {
	type Error = DispatchError;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		assert_eq!(asset_a, LRNA);
		let asset_state = Omnipool::load_asset_state(asset_b)?;
		let price = EmaPrice::new(asset_state.hub_reserve, asset_state.reserve);

		let adjusted_price = WITHDRAWAL_ADJUSTMENT.with(|v| {
			let (n, d, neg) = *v.borrow();
			let adjustment = EmaPrice::new(price.n * n as u128, price.d * d as u128);
			if neg {
				saturating_sub(price, adjustment)
			} else {
				saturating_add(price, adjustment)
			}
		});

		Ok(adjusted_price)
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
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
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = NamedReserveIdentifier;
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

pub const ASSET_PAIR_ACCOUNT: AccountId = 12;

parameter_types! {
	pub MaxNumberOfTrades: u8 = 3;
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
}

type Pools = (OmniPool, Xyk);

impl pallet_route_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = NativeCurrencyId;
	type Currency = FungibleCurrencies<Test>;
	type AMM = Pools;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type WeightInfo = ();
}

type OriginForRuntime = OriginFor<Test>;
pub const INVALID_CALCULATION_AMOUNT: Balance = 999;
pub const CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY: Balance = 10 * ONE;

pub struct OmniPool;
pub struct Xyk;

impl TradeExecution<OriginForRuntime, AccountId, AssetId, Balance> for OmniPool {
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		amount_in: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::Omnipool) {
			return Err(ExecutorError::NotSupported);
		}

		if amount_in == INVALID_CALCULATION_AMOUNT {
			return Err(ExecutorError::Error(DispatchError::Other("Some error happened")));
		}

		let amount_out = CALCULATED_AMOUNT_OUT_FOR_SELL.with(|v| *v.borrow());
		Ok(amount_out)
	}

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		amount_out: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::Omnipool) {
			return Err(ExecutorError::NotSupported);
		}

		if amount_out == INVALID_CALCULATION_AMOUNT {
			return Err(ExecutorError::Error(DispatchError::Other("Some error happened")));
		}

		Ok(CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY)
	}

	fn execute_sell(
		who: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::Omnipool) {
			return Err(ExecutorError::NotSupported);
		}

		if asset_in == FORBIDDEN_ASSET {
			return Err(ExecutorError::Error(pallet_omnipool::Error::<Test>::NotAllowed.into()));
		}

		SELL_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(SellExecution {
				asset_in,
				asset_out,
				amount_in,
				min_buy_amount: min_limit,
			});
		});

		let Ok(who) =  ensure_signed(who) else {
			return Err(ExecutorError::Error(Error::<Test>::InvalidState.into()));
		};
		let amount_out = CALCULATED_AMOUNT_OUT_FOR_SELL.with(|v| *v.borrow());

		Currencies::transfer(RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT), who, asset_out, amount_out)
			.map_err(ExecutorError::Error)?;
		Currencies::transfer(RuntimeOrigin::signed(who), ASSET_PAIR_ACCOUNT, asset_in, amount_in)
			.map_err(ExecutorError::Error)?;

		Ok(())
	}

	fn execute_buy(
		origin: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::Omnipool) {
			return Err(ExecutorError::NotSupported);
		}

		BUY_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(BuyExecution {
				asset_in,
				asset_out,
				amount_out,
				max_sell_amount: max_limit,
			});
		});

		let Ok(who) =  ensure_signed(origin) else {
			return Err(ExecutorError::Error(Error::<Test>::InvalidState.into()));
		};
		let amount_in = CALCULATED_AMOUNT_IN_FOR_OMNIPOOL_BUY;

		Currencies::transfer(RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT), who, asset_out, amount_out)
			.map_err(ExecutorError::Error)?;
		Currencies::transfer(RuntimeOrigin::signed(who), ASSET_PAIR_ACCOUNT, asset_in, amount_in)
			.map_err(ExecutorError::Error)?;

		Ok(())
	}

	fn get_liquidity_depth(
		_pool_type: PoolType<AssetId>,
		_asset_a: AssetId,
		_asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		todo!("Not implemented as not used directly within DCA context")
	}
}

pub const XYK_SELL_CALCULATION_RESULT: Balance = ONE * 5 / 4;
pub const XYK_BUY_CALCULATION_RESULT: Balance = ONE / 3;

impl TradeExecution<OriginForRuntime, AccountId, AssetId, Balance> for Xyk {
	type Error = DispatchError;

	fn calculate_sell(
		pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::XYK) {
			return Err(ExecutorError::NotSupported);
		}

		Ok(XYK_SELL_CALCULATION_RESULT)
	}

	fn calculate_buy(
		pool_type: PoolType<AssetId>,
		_asset_in: AssetId,
		_asset_out: AssetId,
		_: Balance,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::XYK) {
			return Err(ExecutorError::NotSupported);
		}

		Ok(XYK_BUY_CALCULATION_RESULT)
	}

	fn execute_sell(
		_who: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_in: Balance,
		min_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::XYK) {
			return Err(ExecutorError::NotSupported);
		}

		SELL_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(SellExecution {
				asset_in,
				asset_out,
				amount_in,
				min_buy_amount: min_limit,
			});
		});

		let amount_out = XYK_SELL_CALCULATION_RESULT;

		Currencies::transfer(RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT), ALICE, asset_out, amount_out)
			.map_err(ExecutorError::Error)?;
		Currencies::transfer(RuntimeOrigin::signed(ALICE), ASSET_PAIR_ACCOUNT, asset_in, amount_in)
			.map_err(ExecutorError::Error)?;

		Ok(())
	}

	fn execute_buy(
		_who: OriginForRuntime,
		pool_type: PoolType<AssetId>,
		asset_in: AssetId,
		asset_out: AssetId,
		amount_out: Balance,
		max_limit: Balance,
	) -> Result<(), ExecutorError<Self::Error>> {
		if !matches!(pool_type, PoolType::XYK) {
			return Err(ExecutorError::NotSupported);
		}

		BUY_EXECUTIONS.with(|v| {
			let mut m = v.borrow_mut();
			m.push(BuyExecution {
				asset_in,
				asset_out,
				amount_out,
				max_sell_amount: max_limit,
			});
		});

		let amount_in = XYK_BUY_CALCULATION_RESULT;

		Currencies::transfer(RuntimeOrigin::signed(ASSET_PAIR_ACCOUNT), ALICE, asset_out, amount_out)
			.map_err(ExecutorError::Error)?;
		Currencies::transfer(RuntimeOrigin::signed(ALICE), ASSET_PAIR_ACCOUNT, asset_in, amount_in)
			.map_err(ExecutorError::Error)?;

		Ok(())
	}

	fn get_liquidity_depth(
		_pool_type: PoolType<AssetId>,
		_asset_a: AssetId,
		_asset_b: AssetId,
	) -> Result<Balance, ExecutorError<Self::Error>> {
		todo!("No need to implement it as this is not used directly in DCA")
	}
}

pub struct PriceProviderMock {}

impl PriceOracle<AssetId> for PriceProviderMock {
	type Price = Ratio;

	fn price(_: &[Trade<AssetId>], period: OraclePeriod) -> Option<Ratio> {
		if period == OraclePeriod::Short {
			return Some(Ratio::new(80, 100));
		}
		Some(Ratio::new(88, 100))
	}
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub MinBudgetInNativeCurrency: Balance= MIN_BUDGET.with(|v| *v.borrow());
	pub MaxSchedulePerBlock: u32 = 20;
	pub OmnipoolMaxAllowedPriceDifference: Permill = MAX_PRICE_DIFFERENCE.with(|v| *v.borrow());
	pub NamedReserveId: NamedReserveIdentifier = *b"dcaorder";
	pub MaxNumberOfRetriesOnError: u8 = 3;
}

pub struct RandomnessProviderMock {}

impl RandomnessProvider for RandomnessProviderMock {
	fn generator(salt: Option<u32>) -> Result<StdRng, DispatchError> {
		let use_prod_randomness = USE_PROD_RANDOMNESS.with(|v| *v.borrow());

		if use_prod_randomness {
			DCA::generator(salt)
		} else {
			Ok(StdRng::seed_from_u64(0))
		}
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currencies = Currencies;
	type RandomnessProvider = RandomnessProviderMock;
	type MinBudgetInNativeCurrency = MinBudgetInNativeCurrency;
	type MaxSchedulePerBlock = MaxSchedulePerBlock;
	type NativeAssetId = NativeCurrencyId;
	type FeeReceiver = TreasuryAccount;
	type WeightToFee = IdentityFee<Balance>;
	type WeightInfo = ();
	type OraclePriceProvider = PriceProviderMock;
	type RouteExecutor = RouteExecutor;
	type RouteProvider = DefaultRouteProvider;
	type MaxPriceDifferenceBetweenBlocks = OmnipoolMaxAllowedPriceDifference;
	type NamedReserveId = NamedReserveId;
	type MaxNumberOfRetriesOnError = MaxNumberOfRetriesOnError;
	type TechnicalOrigin = EnsureRoot<Self::AccountId>;
	type RelayChainBlockHashProvider = ParentHashGetterMock;
	type AmmTradeWeights = ();
	type MinimumTradingLimit = MinTradeAmount;
	type NativePriceOracle = NativePriceOracleMock;
}

pub struct NativePriceOracleMock;

impl NativePriceOracle<AssetId, EmaPrice> for NativePriceOracleMock {
	fn price(_: AssetId) -> Option<EmaPrice> {
		Some(EmaPrice::from((88, 100)))
	}
}

pub struct DefaultRouteProvider;

impl RouteProvider<AssetId> for DefaultRouteProvider {}

pub struct ParentHashGetterMock {}

impl RelayChainBlockHashProvider for ParentHashGetterMock {
	fn parent_hash() -> Option<Hash> {
		PARENT_HASH.with(|v| *v.borrow())
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
use frame_support::weights::{WeightToFeeCoefficients, WeightToFeePolynomial};
use frame_system::pallet_prelude::OriginFor;
use hydra_dx_math::ema::EmaPrice;
use hydra_dx_math::to_u128_wrapper;
use hydra_dx_math::types::Ratio;
use hydradx_traits::router::{ExecutorError, PoolType, RouteProvider, Trade, TradeExecution};
use pallet_currencies::fungibles::FungibleCurrencies;
use pallet_omnipool::traits::ExternalPriceProvider;
use rand::prelude::StdRng;
use rand::SeedableRng;
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

impl<T: Config> Registry<T::AssetId, Vec<u8>, Balance, DispatchError> for DummyRegistry<T>
where
	T::AssetId: Into<AssetId> + From<u32>,
{
	fn exists(asset_id: T::AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id.into())).copied());
		matches!(asset, Some(_))
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<T::AssetId, DispatchError> {
		Ok(1.into())
	}

	fn retrieve_asset_type(_asset_id: T::AssetId) -> Result<AssetKind, DispatchError> {
		unimplemented!()
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<T::AssetId, DispatchError> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			let l = v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(T::AssetId::from(assigned))
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
	max_price_difference: Permill,
	min_trading_limit: Balance,
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
			max_price_difference: Permill::from_percent(10),
			min_trading_limit: 1000,
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

	pub fn with_min_trading_limit(mut self, limit: Balance) -> Self {
		self.min_trading_limit = limit;
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

		MAX_PRICE_DIFFERENCE.with(|v| {
			*v.borrow_mut() = self.max_price_difference;
		});

		MIN_TRADE_AMOUNT.with(|v| {
			*v.borrow_mut() = self.min_trading_limit;
		});

		let mut initial_native_accounts: Vec<(AccountId, Balance)> = vec![(ASSET_PAIR_ACCOUNT, 10000 * ONE)];
		let additional_accounts: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|a| a.1 == HDX)
			.flat_map(|(x, _, amount)| vec![(*x, *amount)])
			.collect::<_>();

		initial_native_accounts.extend(additional_accounts);

		pallet_balances::GenesisConfig::<Test> {
			balances: initial_native_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut initial_accounts = vec![
			(ASSET_PAIR_ACCOUNT, LRNA, 10000 * ONE),
			(ASSET_PAIR_ACCOUNT, DAI, 10000 * ONE),
			(ASSET_PAIR_ACCOUNT, BTC, 100000000 * ONE),
		];

		initial_accounts.extend(self.endowed_accounts);

		orml_tokens::GenesisConfig::<Test> {
			balances: initial_accounts,
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

		r
	}
}

pub fn set_max_price_diff(diff: Permill) {
	MAX_PRICE_DIFFERENCE.with(|v| {
		*v.borrow_mut() = diff;
	});
}

pub fn set_sell_amount_out(balance: Balance) {
	CALCULATED_AMOUNT_OUT_FOR_SELL.with(|v| {
		*v.borrow_mut() = balance;
	});
}

pub fn use_prod_randomness() {
	USE_PROD_RANDOMNESS.with(|v| {
		*v.borrow_mut() = true;
	});
}

pub fn set_parent_hash(hash: Option<Hash>) {
	PARENT_HASH.with(|v| {
		*v.borrow_mut() = hash;
	});
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

pub fn expect_dca_events(e: Vec<RuntimeEvent>) {
	let last_events = test_utils::last_events::<RuntimeEvent, Test>(e.len());

	let mut dca_events = vec![];

	for event in &last_events {
		let e = event.clone();
		if matches!(
			e,
			RuntimeEvent::DCA(crate::Event::<Test>::ExecutionStarted { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::Scheduled { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::ExecutionPlanned { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::TradeExecuted { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::TradeFailed { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::Terminated { .. })
				| RuntimeEvent::DCA(crate::Event::<Test>::Completed { .. })
		) {
			dca_events.push(e);
		}
	}

	pretty_assertions::assert_eq!(dca_events, e);
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

pub(super) fn saturating_add(l: EmaPrice, r: EmaPrice) -> EmaPrice {
	if l.n.is_zero() || r.n.is_zero() {
		return EmaPrice::new(l.n, l.d);
	}
	let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(l.n, l.d, r.n, r.d);
	// n = l.n * r.d - r.n * l.d
	let n = l_n.full_mul(r_d).saturating_add(r_n.full_mul(l_d));
	// d = l.d * r.d
	let d = l_d.full_mul(r_d);
	round_to_rational((n, d), Rounding::Nearest).into()
}

pub(super) fn saturating_sub(l: EmaPrice, r: EmaPrice) -> EmaPrice {
	if l.n.is_zero() || r.n.is_zero() {
		return EmaPrice::new(l.n, l.d);
	}
	let (l_n, l_d, r_n, r_d) = to_u128_wrapper!(l.n, l.d, r.n, r.d);
	// n = l.n * r.d - r.n * l.d
	let n = l_n.full_mul(r_d).saturating_sub(r_n.full_mul(l_d));
	// d = l.d * r.d
	let d = l_d.full_mul(r_d);
	round_to_rational((n, d), Rounding::Nearest).into()
}

pub struct FeeProvider;

impl GetByKey<AssetId, (Permill, Permill)> for FeeProvider {
	fn get(_: &AssetId) -> (Permill, Permill) {
		(ASSET_FEE.with(|v| *v.borrow()), PROTOCOL_FEE.with(|v| *v.borrow()))
	}
}
