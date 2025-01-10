// This file is part of galacticcouncil/warehouse.
// Copyright (C) 2020-2023  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate as pallet_otc_settlements;
use crate::*;
use frame_support::{
	assert_ok, parameter_types,
	sp_runtime::{
		traits::{BlakeTwo256, IdentityLookup},
		BuildStorage, Permill,
	},
	traits::{
		tokens::nonfungibles::{Create, Inspect, Mutate},
		Everything, Nothing,
	},
};
use frame_system::{EnsureRoot, EnsureSigned};
use hydra_dx_math::{ema::EmaPrice, ratio::Ratio};
use hydradx_traits::{
	router::{PoolType, RefundEdCalculator},
	OraclePeriod, PriceOracle,
};
use orml_traits::{parameter_type_with_key, GetByKey};
use pallet_currencies::{fungibles::FungibleCurrencies, BasicCurrencyAdapter, MockBoundErc20, MockErc20Currency};
use pallet_omnipool::traits::ExternalPriceProvider;
use sp_core::offchain::{
	testing::PoolState, testing::TestOffchainExt, testing::TestTransactionPoolExt, OffchainDbExt, OffchainWorkerExt,
	TransactionPoolExt,
};
use sp_core::H256;
use sp_std::sync::Arc;

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 3;
pub const KSM: AssetId = 4;
pub const BTC: AssetId = 5;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Balances: pallet_balances,
		 Tokens: orml_tokens,
		 Currencies: pallet_currencies,
		 AssetRegistry: pallet_asset_registry,
		 OTC: pallet_otc,
		 Omnipool: pallet_omnipool,
		 Router: pallet_route_executor,
		 OtcSettlements: pallet_otc_settlements,
		 Broadcast:pallet_broadcast,
	 }
);

parameter_types! {
	pub ExistentialDepositMultiplier: u8 = 5;
	pub MinProfitLimit: Balance = 10_000_000_000_000;
	pub PricePrecision: FixedU128 = FixedU128::from_rational(1, 1_000_000);
	pub MinProfitPercentage: Perbill = Perbill::from_rational(1u32, 100_000_u32); // 0.001%
	pub OtcFee: Permill = Permill::from_percent(1u32);
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1
	};
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = FungibleCurrencies<Test>;
	type Router = Router;
	type ProfitReceiver = TreasuryAccount;
	type MinProfitPercentage = MinProfitPercentage;
	type PricePrecision = PricePrecision;
	type MinTradingLimit = MinTradingLimit;
	type MaxIterations = ConstU32<40>;
	type RouterWeightInfo = ();
	type WeightInfo = ();
}

impl pallet_otc::Config for Test {
	type AssetId = AssetId;
	type AssetRegistry = AssetRegistry;
	type Currency = Currencies;
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposits = ExistentialDeposits;
	type ExistentialDepositMultiplier = ExistentialDepositMultiplier;
	type Fee = OtcFee;
	type FeeReceiver = TreasuryAccount;
	type WeightInfo = ();
}

parameter_types! {
	pub DefaultRoutePoolType: PoolType<AssetId> = PoolType::Omnipool;
		pub const RouteValidationOraclePeriod: OraclePeriod = OraclePeriod::TenMinutes;
}

pub struct MockedEdCalculator;

impl RefundEdCalculator<Balance> for MockedEdCalculator {
	fn calculate() -> Balance {
		1_000_000_000_000
	}
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

impl pallet_route_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Balance = Balance;
	type NativeAssetId = HDXAssetId;
	type Currency = FungibleCurrencies<Test>;
	type InspectRegistry = AssetRegistry;
	type AMM = Omnipool;
	type EdToRefundCalculator = MockedEdCalculator;
	type OraclePriceProvider = PriceProviderMock;
	type OraclePeriod = RouteValidationOraclePeriod;
	type DefaultRoutePoolType = DefaultRoutePoolType;
	type ForceInsertOrigin = EnsureRoot<Self::AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
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
	pub const PositionCollectionId: u32= 1000;

	pub const ExistentialDeposit: u128 = 500;
	pub ProtocolFee: Permill = Permill::from_percent(0);
	pub AssetFee: Permill = Permill::from_percent(0);
	pub BurnFee: Permill = Permill::from_percent(0);
	pub AssetWeightCap: Permill = Permill::from_percent(100);
	pub MinAddedLiquidity: Balance = 1000u128;
	pub MinTradeAmount: Balance = 1000u128;
	pub MaxInRatio: Balance = 1u128;
	pub MaxOutRatio: Balance = 1u128;
	pub const TVLCap: Balance = Balance::MAX;

	pub const TransactionByteFee: Balance = 10 * ONE / 100_000;

	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
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
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

impl pallet_currencies::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type Erc20Currency = MockErc20Currency<Test>;
	type BoundErc20 = MockBoundErc20<Test>;
	type GetNativeCurrencyId = HDXAssetId;
	type WeightInfo = ();
}

impl pallet_broadcast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

parameter_types! {
	pub const MinTradingLimit: Balance = 1_000;
	pub const MinPoolLiquidity: Balance = 1_000;
	pub const DiscountedFee: (u32, u32) = (7, 10_000);
}

pub struct AllowPools;

impl hydradx_traits::CanCreatePool<AssetId> for AllowPools {
	fn can_create(_asset_a: AssetId, _asset_b: AssetId) -> bool {
		true
	}
}

pub struct AssetPairAccountIdTest;
impl hydradx_traits::AssetPairAccountIdFor<AssetId, u64> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> u64 {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b)
		}
		(a * 1000 + b) as u64
	}
}

parameter_types! {
	#[derive(PartialEq, Debug)]
	pub RegistryStringLimit: u32 = 100;
	#[derive(PartialEq, Debug)]
	pub MinRegistryStringLimit: u32 = 2;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

type AssetLocation = u8;

impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureRoot<AccountId>;
	type Currency = Tokens;
	type UpdateOrigin = EnsureSigned<u64>;
	type AssetId = AssetId;
	type AssetNativeLocation = AssetLocation;
	type StringLimit = RegistryStringLimit;
	type MinStringLimit = MinRegistryStringLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type RegExternalWeightMultiplier = frame_support::traits::ConstU64<1>;
	type RegisterAssetHook = ();
	type WeightInfo = ();
}

pub struct DummyDuster;

impl hydradx_traits::pools::DustRemovalAccountWhitelist<AccountId> for DummyDuster {
	type Error = DispatchError;

	fn add_account(_account: &AccountId) -> Result<(), Self::Error> {
		unimplemented!()
	}

	fn remove_account(_account: &AccountId) -> Result<(), Self::Error> {
		unimplemented!()
	}
}

impl pallet_omnipool::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type PositionItemId = u32;
	type Currency = Currencies;
	type HubAssetId = LRNAAssetId;
	type WeightInfo = ();
	type HdxAssetId = HDXAssetId;
	type NFTCollectionId = PositionCollectionId;
	type NFTHandler = DummyNFT;
	type AssetRegistry = AssetRegistry;
	type MinimumTradingLimit = MinTradeAmount;
	type MinimumPoolLiquidity = MinAddedLiquidity;
	type UpdateTradabilityOrigin = EnsureRoot<Self::AccountId>;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type CollectionId = u32;
	type AuthorityOrigin = EnsureRoot<Self::AccountId>;
	type OmnipoolHooks = ();
	type PriceBarrier = ();
	type MinWithdrawalFee = ();
	type ExternalPriceOracle = WithdrawFeePriceOracle;
	type Fee = FeeProvider;
	type BurnProtocolFee = BurnFee;
}

pub struct DummyNFT;

impl<AccountId: From<u64>> Inspect<AccountId> for DummyNFT {
	type ItemId = u32;
	type CollectionId = u32;

	fn owner(_class: &Self::CollectionId, _instance: &Self::ItemId) -> Option<AccountId> {
		todo!()
	}
}

impl<AccountId: From<u64>> Create<AccountId> for DummyNFT {
	fn create_collection(_class: &Self::CollectionId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u64> + Into<u64> + Copy> Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::CollectionId, _instance: &Self::ItemId, _who: &AccountId) -> DispatchResult {
		Ok(())
	}

	fn burn(
		_class: &Self::CollectionId,
		_instance: &Self::ItemId,
		_maybe_check_owner: Option<&AccountId>,
	) -> DispatchResult {
		Ok(())
	}
}

pub struct WithdrawFeePriceOracle;

impl ExternalPriceProvider<AssetId, EmaPrice> for WithdrawFeePriceOracle {
	type Error = DispatchError;

	fn get_price(_asset_a: AssetId, _asset_b: AssetId) -> Result<EmaPrice, Self::Error> {
		todo!()
	}

	fn get_price_weight() -> Weight {
		todo!()
	}
}

pub struct FeeProvider;

impl GetByKey<(AssetId, Balance), (Permill, Permill)> for FeeProvider {
	fn get(_: &(AssetId, Balance)) -> (Permill, Permill) {
		(Permill::from_percent(0), Permill::from_percent(0))
	}
}

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	init_pool: Option<(FixedU128, FixedU128)>,
	omnipool_liquidity: Vec<(AccountId, AssetId, Balance)>, //who, asset, amount/
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 1_000_000_000_000 * ONE),
				(ALICE, LRNA, 1_000_000_000_000 * ONE),
				(ALICE, DAI, 1_000_000_000_000_000_000 * ONE),
				(ALICE, DOT, 1_000_000_000_000 * ONE),
				(ALICE, KSM, 1_000_000_000_000 * ONE),
				(ALICE, BTC, 1_000_000_000_000 * ONE),
				(BOB, HDX, 1_000_000_000 * ONE),
				(BOB, DAI, 1_000_000_000 * ONE),
				(Omnipool::protocol_account(), HDX, 1_000_000 * ONE),
				(Omnipool::protocol_account(), LRNA, 1_000_000 * ONE),
				(Omnipool::protocol_account(), DAI, 1_000_000 * ONE),
				(Omnipool::protocol_account(), DOT, 1_000_000 * ONE),
				(Omnipool::protocol_account(), KSM, 1_000_000 * ONE),
				(Omnipool::protocol_account(), BTC, 1_000_000 * ONE),
			],
			init_pool: Some((FixedU128::from_float(0.5), FixedU128::from(1))),
			omnipool_liquidity: vec![(ALICE, KSM, 5_000 * ONE)],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> (sp_io::TestExternalities, Arc<parking_lot::RwLock<PoolState>>) {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let registered_assets = vec![
			(
				Some(LRNA),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"LRNA".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"LRNA".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(DAI),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DAI".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DAI".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(DOT),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"DOT".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(KSM),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"KSM".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"KSM".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				true,
			),
			(
				Some(BTC),
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"BTC".to_vec().try_into().unwrap()),
				10_000,
				Some::<BoundedVec<u8, RegistryStringLimit>>(b"BTC".to_vec().try_into().unwrap()),
				Some(12),
				None::<Balance>,
				false,
			),
		];

		let mut initial_native_accounts: Vec<(AccountId, Balance)> = vec![];
		let additional_accounts: Vec<(AccountId, Balance)> = self
			.endowed_accounts
			.iter()
			.filter(|a| a.1 == HDX)
			.flat_map(|(x, _, amount)| vec![(*x, *amount)])
			.collect::<_>();

		initial_native_accounts.extend(additional_accounts);

		pallet_asset_registry::GenesisConfig::<Test> {
			registered_assets,
			..Default::default()
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: initial_native_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();

		ext.execute_with(|| {
			System::set_block_number(1);
		});

		if let Some((stable_price, native_price)) = self.init_pool {
			ext.execute_with(|| {
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
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					DOT,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					KSM,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));
				assert_ok!(Omnipool::add_token(
					RuntimeOrigin::root(),
					BTC,
					stable_price,
					Permill::from_percent(100),
					Omnipool::protocol_account(),
				));

				for p in self.omnipool_liquidity {
					assert_ok!(Omnipool::add_liquidity(RuntimeOrigin::signed(p.0), p.1, p.2));
				}
			});
		}

		let (offchain, _offchain_state) = TestOffchainExt::with_offchain_db(ext.offchain_db());
		ext.register_extension(OffchainDbExt::new(offchain.clone()));
		ext.register_extension(OffchainWorkerExt::new(offchain));
		let (pool, pool_state) = TestTransactionPoolExt::new();
		ext.register_extension(TransactionPoolExt::new(pool));

		ext.persist_offchain_overlay();

		(ext, pool_state)
	}
}
