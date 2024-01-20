// This file is part of galacticcouncil/warehouse.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

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

#![cfg(test)]
use super::*;

use crate::Config;
use crate::{self as liq_mining, types::DefaultPriceAdjustment};
use frame_support::{parameter_types, traits::Contains, traits::Everything, PalletId};
use frame_system as system;
use hydradx_traits::{pools::DustRemovalAccountWhitelist, registry::Registry, AssetKind, AMM};
use orml_traits::GetByKey;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage,
};

pub use frame_support::storage::with_transaction;
pub use sp_runtime::TransactionOutcome;

#[derive(Encode, Decode, Eq, PartialEq, Copy, Clone, PartialOrd, Ord, MaxEncodedLen, RuntimeDebug, TypeInfo)]
#[repr(u8)]
pub enum ReserveIdentifier {
	Nft,
	Marketplace,
	// always the last, indicate number of variants
	Count,
}

use std::{cell::RefCell, collections::HashMap};

pub type Balance = u128;
pub type AssetId = u32;
pub type Amount = i128;

pub type AccountId = u128;
pub type FarmId = crate::FarmId;
pub type BlockNumber = u64;
pub const ALICE: AccountId = 10;
pub const BOB: AccountId = 11;
pub const CHARLIE: AccountId = 12;
pub const DAVE: AccountId = 13;
pub const EVE: AccountId = 14;
pub const TREASURY: AccountId = 15;
pub const ACCOUNT_WITH_1M: AccountId = 16;
pub const GC: AccountId = 17;

pub const INITIAL_BALANCE: u128 = 1_000_000_000_000;

pub const BSX_ACA_SHARE_ID: AssetId = 100;
pub const BSX_KSM_SHARE_ID: AssetId = 101;
pub const BSX_DOT_SHARE_ID: AssetId = 102;
pub const BSX_ETH_SHARE_ID: AssetId = 103;
pub const BSX_HDX_SHARE_ID: AssetId = 104;
pub const BSX_TKN1_SHARE_ID: AssetId = 105;
pub const BSX_TKN2_SHARE_ID: AssetId = 106;
pub const KSM_DOT_SHARE_ID: AssetId = 107;
pub const ACA_KSM_SHARE_ID: AssetId = 108;

pub const BSX: AssetId = 1000;
pub const HDX: AssetId = 2000;
pub const ACA: AssetId = 3000;
pub const KSM: AssetId = 4000;
pub const DOT: AssetId = 5000;
pub const ETH: AssetId = 6000;
pub const TKN1: AssetId = 7_001;
pub const TKN2: AssetId = 7_002;
pub const UNKNOWN_ASSET: AssetId = 7_003;

pub const BSX_ACA_AMM: AccountId = 11_000;
pub const BSX_KSM_AMM: AccountId = 11_001;
pub const BSX_DOT_AMM: AccountId = 11_002;
pub const BSX_ETH_AMM: AccountId = 11_003;
pub const BSX_HDX_AMM: AccountId = 11_004;
pub const BSX_TKN1_AMM: AccountId = 11_005;
pub const BSX_TKN2_AMM: AccountId = 11_006;
pub const DEFAULT_AMM: AccountId = 11_007;
pub const KSM_DOT_AMM: AccountId = 11_008;
pub const ACA_KSM_AMM: AccountId = 11_009;

pub const BSX_ACA_YIELD_FARM_ID: FarmId = 12_000;
pub const BSX_KSM_YIELD_FARM_ID: FarmId = 12_001;
pub const BSX_DOT_YIELD_FARM_ID: FarmId = 12_002;

pub const BSX_FARM: FarmId = 1;
pub const KSM_FARM: FarmId = 2;
pub const GC_FARM: FarmId = 3;
pub const ACA_FARM: FarmId = 4;

pub const ONE: Balance = 1_000_000_000_000;

type Block = frame_system::mocking::MockBlock<Test>;

#[derive(Clone)]
pub struct AssetPair {
	pub asset_in: AssetId,
	pub asset_out: AssetId,
}

frame_support::construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		LiquidityMining: liq_mining::<Instance1>,
		LiquidityMining2: liq_mining::<Instance2>,
		//This LM instance is using dummy oracle for price_adjustment
		LiquidityMining3: liq_mining::<Instance3>,
		Tokens: orml_tokens,
		Balances: pallet_balances,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub static MockBlockNumberProvider: u64 = 0;
}

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = u64;

	fn current_block_number() -> Self::BlockNumber {
		Self::get()
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

pub struct Amm;

thread_local! {
	pub static AMM_POOLS: RefCell<HashMap<String, (AccountId, AssetId)>> = RefCell::new(HashMap::new());
	pub static DUSTER_WHITELIST: RefCell<Vec<AccountId>> = RefCell::new(Vec::new());
}

impl AMM<AccountId, AssetId, AssetPair, Balance> for Amm {
	fn get_max_out_ratio() -> u128 {
		0_u32.into()
	}

	fn get_fee(_pool_account_id: &AccountId) -> (u32, u32) {
		(0, 0)
	}

	fn get_max_in_ratio() -> u128 {
		0_u32.into()
	}

	fn get_pool_assets(_pool_account_id: &AccountId) -> Option<Vec<AssetId>> {
		None
	}

	fn get_spot_price_unchecked(_asset_a: AssetId, _asset_b: AssetId, _amount: Balance) -> Balance {
		Balance::from(0_u32)
	}

	fn validate_sell(
		_origin: &AccountId,
		_assets: AssetPair,
		_amount: Balance,
		_min_bought: Balance,
		_discount: bool,
	) -> Result<
		hydradx_traits::AMMTransfer<AccountId, AssetId, AssetPair, Balance>,
		frame_support::sp_runtime::DispatchError,
	> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn execute_buy(
		_transfer: &hydradx_traits::AMMTransfer<AccountId, AssetId, AssetPair, Balance>,
	) -> frame_support::dispatch::DispatchResult {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn execute_sell(
		_transfer: &hydradx_traits::AMMTransfer<AccountId, AssetId, AssetPair, Balance>,
	) -> frame_support::dispatch::DispatchResult {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn validate_buy(
		_origin: &AccountId,
		_assets: AssetPair,
		_amount: Balance,
		_max_limit: Balance,
		_discount: bool,
	) -> Result<
		hydradx_traits::AMMTransfer<AccountId, AssetId, AssetPair, Balance>,
		frame_support::sp_runtime::DispatchError,
	> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn get_min_pool_liquidity() -> Balance {
		Balance::from(0_u32)
	}

	fn get_min_trading_limit() -> Balance {
		Balance::from(0_u32)
	}

	// Fn bellow are used by liq. mining pallet
	fn exists(assets: AssetPair) -> bool {
		AMM_POOLS.with(|v| v.borrow().contains_key(&asset_pair_to_map_key(assets)))
	}

	fn get_pair_id(assets: AssetPair) -> AccountId {
		AMM_POOLS.with(|v| match v.borrow().get(&asset_pair_to_map_key(assets)) {
			Some(p) => p.0,
			None => DEFAULT_AMM,
		})
	}

	fn get_share_token(assets: AssetPair) -> AssetId {
		AMM_POOLS.with(|v| match v.borrow().get(&asset_pair_to_map_key(assets)) {
			Some(p) => p.1,
			None => BSX,
		})
	}
}

pub fn asset_pair_to_map_key(assets: AssetPair) -> String {
	format!("in:{}_out:{}", assets.asset_in, assets.asset_out)
}

parameter_types! {
	pub const LMPalletId: PalletId = PalletId(*b"TEST_lm_");
	pub const MinPlannedYieldingPeriods: BlockNumber = 100;
	pub const MinTotalFarmRewards: Balance = 1_000_000;
	#[derive(PartialEq, Eq)]
	pub const MaxEntriesPerDeposit: u8 = 5;
	pub const MaxYieldFarmsPerGlobalFarm: u8 = 4;
}

impl Config<Instance1> for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Tokens;
	type PalletId = LMPalletId;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type BlockNumberProvider = MockBlockNumberProvider;
	type AmmPoolId = AccountId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type NonDustableWhitelistHandler = Whitelist;
	type AssetRegistry = AssetRegistry;
	type PriceAdjustment = DefaultPriceAdjustment;
}

parameter_types! {
	pub const LMPalletId2: PalletId = PalletId(*b"TEST_lm2");
	pub const MinPlannedYieldingPeriods2: BlockNumber = 10;
	pub const MinTotalFarmRewards2: Balance = 100_000;
	pub const MininumDeposit2: Balance = 1;
	pub const MaxEntriesPerDeposit2: u8 = 1;
}

impl Config<Instance2> for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Tokens;
	type PalletId = LMPalletId2;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods2;
	type MinTotalFarmRewards = MinTotalFarmRewards2;
	type BlockNumberProvider = MockBlockNumberProvider;
	type AmmPoolId = AccountId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit2;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type NonDustableWhitelistHandler = Whitelist;
	type AssetRegistry = AssetRegistry;
	type PriceAdjustment = DefaultPriceAdjustment;
}

parameter_types! {
	pub const LMPalletId3: PalletId = PalletId(*b"TEST_lm3");
}

impl Config<Instance3> for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type MultiCurrency = Tokens;
	type PalletId = LMPalletId3;
	type MinPlannedYieldingPeriods = MinPlannedYieldingPeriods2;
	type MinTotalFarmRewards = MinTotalFarmRewards;
	type BlockNumberProvider = MockBlockNumberProvider;
	type AmmPoolId = AccountId;
	type MaxFarmEntriesPerDeposit = MaxEntriesPerDeposit;
	type MaxYieldFarmsPerGlobalFarm = MaxYieldFarmsPerGlobalFarm;
	type NonDustableWhitelistHandler = Whitelist;
	type AssetRegistry = AssetRegistry;
	type PriceAdjustment = DummyOraclePriceAdjustment;
}

parameter_types! {
	pub const MaxLocks: u32 = 1;
	pub const ExistentialDeposit: u128 = 500;
	pub const MaxReserves: u32 = 50;
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
	type ReserveIdentifier = ReserveIdentifier;
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = AssetRegistry;
	type MaxLocks = MaxLocks;
	type DustRemovalWhitelist = Whitelist;
	type MaxReserves = ConstU32<100_000>;
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

pub struct DummyOraclePriceAdjustment;

impl PriceAdjustment<GlobalFarmData<Test, Instance3>> for DummyOraclePriceAdjustment {
	type Error = DispatchError;

	type PriceAdjustment = FixedU128;

	fn get(global_farm: &GlobalFarmData<Test, Instance3>) -> Result<Self::PriceAdjustment, Self::Error> {
		//This is special case to test global-fram's fallback when oracle is not available.
		if global_farm.updated_at == 999_666_333 {
			Err(sp_runtime::DispatchError::Other(
				"Oracle is not available - updated_at == 999_666_333 is special case.",
			))
		} else {
			Ok(FixedU128::from_inner(500_000_000_000_000_000)) //0.5
		}
	}
}

pub struct Whitelist;

impl Contains<AccountId> for Whitelist {
	fn contains(account: &AccountId) -> bool {
		if *account == LiquidityMining::pot_account_id().unwrap() {
			return true;
		}

		DUSTER_WHITELIST.with(|v| v.borrow().contains(account))
	}
}

impl DustRemovalAccountWhitelist<AccountId> for Whitelist {
	type Error = DispatchError;

	fn add_account(account: &AccountId) -> Result<(), Self::Error> {
		if Whitelist::contains(account) {
			return Err(sp_runtime::DispatchError::Other("Account is already in the whitelist"));
		}

		DUSTER_WHITELIST.with(|v| v.borrow_mut().push(*account));

		Ok(())
	}

	fn remove_account(account: &AccountId) -> Result<(), Self::Error> {
		DUSTER_WHITELIST.with(|v| {
			let mut v = v.borrow_mut();

			let idx = v.iter().position(|x| *x == *account).unwrap();
			v.remove(idx);

			Ok(())
		})
	}
}

pub struct AssetRegistry;

impl Registry<AssetId, Vec<u8>, Balance, DispatchError> for AssetRegistry {
	fn exists(name: AssetId) -> bool {
		name != UNKNOWN_ASSET
	}

	fn retrieve_asset(_name: &Vec<u8>) -> Result<AssetId, DispatchError> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn retrieve_asset_type(_asset_id: AssetId) -> Result<AssetKind, DispatchError> {
		unimplemented!()
	}

	fn create_asset(_name: &Vec<u8>, _existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}

	fn get_or_create_asset(_name: Vec<u8>, _existential_deposit: Balance) -> Result<AssetId, DispatchError> {
		Err(sp_runtime::DispatchError::Other("NotImplemented"))
	}
}

impl GetByKey<AssetId, Balance> for AssetRegistry {
	fn get(_key: &AssetId) -> Balance {
		1_000_u128
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, BSX_ACA_SHARE_ID, INITIAL_BALANCE * ONE),
				(ALICE, BSX_DOT_SHARE_ID, INITIAL_BALANCE * ONE),
				(ALICE, BSX_KSM_SHARE_ID, INITIAL_BALANCE * ONE),
				(ALICE, BSX_TKN1_SHARE_ID, 3_000_000 * ONE),
				(ALICE, BSX_TKN2_SHARE_ID, 3_000_000 * ONE),
				(ALICE, ACA_KSM_SHARE_ID, 3_000_000 * ONE),
				(ALICE, BSX, INITIAL_BALANCE * ONE),
				(ACCOUNT_WITH_1M, BSX, 1_000_000 * ONE),
				(BOB, BSX_ACA_SHARE_ID, INITIAL_BALANCE * ONE),
				(BOB, BSX_DOT_SHARE_ID, INITIAL_BALANCE * ONE),
				(BOB, BSX_KSM_SHARE_ID, INITIAL_BALANCE * ONE),
				(BOB, BSX_TKN1_SHARE_ID, 2_000_000 * ONE),
				(BOB, BSX_TKN2_SHARE_ID, 2_000_000 * ONE),
				(BOB, ACA_KSM_SHARE_ID, 2_000_000 * ONE),
				(BOB, BSX, INITIAL_BALANCE * ONE),
				(BOB, KSM, INITIAL_BALANCE * ONE),
				(CHARLIE, BSX_ACA_SHARE_ID, INITIAL_BALANCE * ONE),
				(CHARLIE, BSX_DOT_SHARE_ID, INITIAL_BALANCE * ONE),
				(CHARLIE, BSX_KSM_SHARE_ID, INITIAL_BALANCE * ONE),
				(CHARLIE, BSX_TKN1_SHARE_ID, 5_000_000 * ONE),
				(CHARLIE, BSX_TKN2_SHARE_ID, 5_000_000 * ONE),
				(CHARLIE, BSX, INITIAL_BALANCE * ONE),
				(CHARLIE, KSM, INITIAL_BALANCE * ONE),
				(CHARLIE, ACA, INITIAL_BALANCE * ONE),
				(DAVE, BSX_ACA_SHARE_ID, INITIAL_BALANCE * ONE),
				(DAVE, BSX_DOT_SHARE_ID, INITIAL_BALANCE * ONE),
				(DAVE, BSX_KSM_SHARE_ID, INITIAL_BALANCE * ONE),
				(DAVE, BSX_TKN1_SHARE_ID, 10_000_000 * ONE),
				(DAVE, BSX_TKN2_SHARE_ID, 10_000_000 * ONE),
				(DAVE, BSX, INITIAL_BALANCE * ONE),
				(DAVE, KSM, INITIAL_BALANCE * ONE),
				(DAVE, ACA, INITIAL_BALANCE * ONE),
				(GC, BSX, INITIAL_BALANCE * ONE),
				(GC, TKN1, INITIAL_BALANCE * ONE),
				(GC, TKN2, INITIAL_BALANCE * ONE),
				(TREASURY, BSX, 1_000_000_000_000 * ONE),
				(TREASURY, ACA, 1_000_000_000_000 * ONE),
				(TREASURY, HDX, 1_000_000_000_000 * ONE),
				(TREASURY, KSM, 1_000_000_000_000 * ONE),
				(EVE, BSX_ACA_SHARE_ID, INITIAL_BALANCE * ONE),
				(EVE, BSX_DOT_SHARE_ID, INITIAL_BALANCE * ONE),
				(EVE, BSX_KSM_SHARE_ID, INITIAL_BALANCE * ONE),
				(EVE, BSX_TKN1_SHARE_ID, 10_000_000 * ONE),
				(EVE, BSX_TKN2_SHARE_ID, 10_000_000 * ONE),
				(EVE, BSX, INITIAL_BALANCE * ONE),
				(EVE, KSM, INITIAL_BALANCE * ONE),
				(EVE, ACA, INITIAL_BALANCE * ONE),
			],
		}
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		AMM_POOLS.with(|v| v.borrow_mut().clear());
		DUSTER_WHITELIST.with(|v| v.borrow_mut().clear());

		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		t.into()
	}
}

pub fn set_block_number(n: u64) {
	MockBlockNumberProvider::set(n);
	System::set_block_number(n);
}
