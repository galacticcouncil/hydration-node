// This file is part of Basilisk-node.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

#![cfg(test)]

use frame_support::{
	instances::Instance1,
	parameter_types,
	traits::{AsEnsureOriginWithArg, Everything, Nothing},
	PalletId,
};

use frame_system as system;
use frame_system::{EnsureRoot, EnsureSigned};
use hydradx_traits::{AssetPairAccountIdFor, Source};
use orml_traits::parameter_type_with_key;
use primitives::{
	constants::{
		chain::{DISCOUNTED_FEE, MAX_IN_RATIO, MAX_OUT_RATIO, MIN_POOL_LIQUIDITY, MIN_TRADING_LIMIT},
		currency::NATIVE_EXISTENTIAL_DEPOSIT,
	},
	Amount, AssetId, Balance,
};

use pallet_nft::{CollectionType, NftPermissions};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage,
};

pub const UNITS: Balance = 1_000_000_000_000;

pub type AccountId = u128;
pub type BlockNumber = u64;

pub const BSX: AssetId = 0;
pub const KSM: AssetId = 1;
pub const DOT: AssetId = 2;

pub const LIQ_MINING_NFT_COLLECTION: primitives::CollectionId = 1;

type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Duster: pallet_duster,
		XYK: pallet_xyk,
		LiquidityMining: pallet_xyk_liquidity_mining,
		NFT: pallet_nft,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		Currency: orml_tokens,
		AssetRegistry: pallet_asset_registry,
		WarehouseLM: pallet_liquidity_mining::<Instance1>,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub static MockBlockNumberProvider: u64 = 0;
	pub const BSXAssetId: AssetId = BSX;
	pub ExchangeFeeRate: (u32, u32) = (2, 1_000);
	pub RegistryStringLimit: u32 = 100;
	pub const SequentialIdOffset: u32 = 1_000_000;
}

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u64;

	fn current_block_number() -> Self::BlockNumber {
		System::block_number()
	}
}
impl system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type DbWeight = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Nonce = u64;
	type Block = Block;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = BlockHashCount;
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<u128>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl crate::Config for Test {}

parameter_types! {
	pub const WarehouseLMPalletId: PalletId = PalletId(*b"WhouseLm");
	pub const MaxEntriesPerDeposit: u8 = 5;
	pub const MaxYieldFarmsPerGlobalFarm: u8 = 5;
	pub const MinPlannedYieldingPeriods: BlockNumber = 100;
	pub const MinTotalFarmRewards: Balance = 1_000_000;
}

impl pallet_liquidity_mining::Config<Instance1> for Test {
	type AssetId = AssetId;
	type MultiCurrency = Currency;
	type PalletId = WarehouseLMPalletId;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods;
	type BlockNumberProvider = MockBlockNumberProvider;
	type AmmPoolId = AccountId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type AssetRegistry = AssetRegistry;
	type NonDustableWhitelistHandler = Duster;
	type RuntimeEvent = RuntimeEvent;
	type PriceAdjustment = pallet_liquidity_mining::DefaultPriceAdjustment;
}

parameter_types! {
	pub const LMPalletId: PalletId = PalletId(*b"LiqMinId");
	pub const NftCollection: primitives::CollectionId = LIQ_MINING_NFT_COLLECTION;
}

impl pallet_xyk_liquidity_mining::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type MultiCurrency = Currency;
	type CreateOrigin = EnsureRoot<AccountId>;
	type PalletId = LMPalletId;
	type NftCollectionId = NftCollection;
	type AMM = XYK;
	type WeightInfo = ();
	type NFTHandler = NFT;
	type LiquidityMiningHandler = WarehouseLM;
	type NonDustableWhitelistHandler = Duster;
}

impl pallet_duster::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type MultiCurrency = Currency;
	type MinCurrencyDeposits = AssetRegistry;
	type Reward = ();
	type NativeCurrencyId = BSXAssetId;
	type BlacklistUpdateOrigin = EnsureRoot<AccountId>;
	type WeightInfo = ();
}

parameter_types! {
	pub const ReserveCollectionIdUpTo: u128 = 9999;
}

impl pallet_nft::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type WeightInfo = pallet_nft::weights::BasiliskWeight<Test>;
	type NftCollectionId = primitives::CollectionId;
	type NftItemId = primitives::ItemId;
	type CollectionType = CollectionType;
	type Permissions = NftPermissions;
	type ReserveCollectionIdUpTo = ReserveCollectionIdUpTo;
}

parameter_types! {
	pub const ExistentialDeposit: u128 = NATIVE_EXISTENTIAL_DEPOSIT;
	pub const MaxReserves: u32 = 50;
	pub const MaxLocks: u32 = 1;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type RuntimeEvent = RuntimeEvent;
	type DustRemoval = ();
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = frame_system::Pallet<Test>;
	type MaxLocks = ();
	type WeightInfo = ();
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ();
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub const CollectionDeposit: Balance = 100 * UNITS; // 100 UNITS deposit to create asset class
	pub const ItemDeposit: Balance = 100 * UNITS; // 100 UNITS deposit to create asset instance
	pub const KeyLimit: u32 = 256;	// Max 256 bytes per key
	pub const ValueLimit: u32 = 1024;	// Max 1024 bytes per value
	pub const UniquesMetadataDepositBase: Balance = 100 * UNITS;
	pub const AttributeDepositBase: Balance = 10 * UNITS;
	pub const DepositPerByte: Balance = UNITS;
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = primitives::CollectionId;
	type ItemId = primitives::ItemId;
	type Currency = Balances;
	type ForceOrigin = frame_system::EnsureRoot<AccountId>;
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = primitives::UniquesStringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	type CreateOrigin = AsEnsureOriginWithArg<frame_system::EnsureSigned<AccountId>>;
	type Locker = ();
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		1u128
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = Nothing;
	type MaxReserves = MaxReserves;
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

#[derive(Default)]
pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

pub struct AssetPairAccountIdTest();

impl AssetPairAccountIdFor<AssetId, AccountId> for AssetPairAccountIdTest {
	fn from_assets(asset_a: AssetId, asset_b: AssetId, _: &str) -> AccountId {
		let mut a = asset_a as u128;
		let mut b = asset_b as u128;
		if a > b {
			std::mem::swap(&mut a, &mut b)
		}
		(a * 1000 + b) as AccountId
	}
}

impl pallet_asset_registry::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RegistryOrigin = EnsureSigned<AccountId>;
	type AssetId = AssetId;
	type Balance = Balance;
	type AssetNativeLocation = u8;
	type StringLimit = RegistryStringLimit;
	type SequentialIdStartAt = SequentialIdOffset;
	type NativeAssetId = BSXAssetId;
	type WeightInfo = ();
}

parameter_types! {
	pub const MinTradingLimit: Balance = MIN_TRADING_LIMIT;
	pub const MinPoolLiquidity: Balance = MIN_POOL_LIQUIDITY;
	pub const MaxInRatio: u128 = MAX_IN_RATIO;
	pub const MaxOutRatio: u128 = MAX_OUT_RATIO;
	pub const DiscountedFee: (u32, u32) = DISCOUNTED_FEE;
	pub const XYKOracleSourceIdentifier: Source = *b"snek/xyk";
}

impl pallet_xyk::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetRegistry = AssetRegistry;
	type AssetPairAccountId = AssetPairAccountIdTest;
	type Currency = Currency;
	type NativeAssetId = BSXAssetId;
	type WeightInfo = ();
	type GetExchangeFee = ExchangeFeeRate;
	type MinTradingLimit = MinTradingLimit;
	type MinPoolLiquidity = MinPoolLiquidity;
	type MaxInRatio = MaxInRatio;
	type MaxOutRatio = MaxOutRatio;
	type OracleSource = XYKOracleSourceIdentifier;
	type CanCreatePool = pallet_xyk::AllowAllPools;
	type AMMHandler = ();
	type DiscountedFee = DiscountedFee;
	type NonDustableWhitelistHandler = Duster;
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		pallet_asset_registry::GenesisConfig::<Test> {
			registered_assets: vec![(b"KSM".to_vec(), 1_000, Some(KSM)), (b"DOT".to_vec(), 1_000, Some(DOT))],
			native_asset_name: b"BSX".to_vec(),
			native_existential_deposit: 1_000_000_000_000,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		<pallet_xyk_liquidity_mining::GenesisConfig<Test> as BuildStorage>::assimilate_storage(
			&pallet_xyk_liquidity_mining::GenesisConfig::<Test>::default(),
			&mut t,
		)
		.unwrap();

		t.into()
	}
}

pub fn new_test_ext() -> sp_io::TestExternalities {
	let mut ext = ExtBuilder::default().build();
	ext.execute_with(|| {
		System::set_block_number(1);
	});

	ext
}
