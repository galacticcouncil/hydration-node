// This file is part of HydraDX-node.

// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Test environment for HSM pallet.
#![allow(clippy::type_complexity)]

use crate as pallet_hsm;
use crate::types::CallResult;
use crate::Config;
use core::ops::RangeInclusive;
use ethabi::ethereum_types::U256;
use evm::{ExitReason, ExitSucceed};
use frame_support::sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiSignature,
};
use frame_support::traits::Contains;
use frame_support::traits::{ConstU128, ConstU16, ConstU32, ConstU64, Everything};
use frame_support::{construct_runtime, parameter_types};
use frame_system::EnsureRoot;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use hydradx_traits::{
	evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM},
	stableswap::AssetAmount,
};
use hydradx_traits::{AccountIdFor, Inspect, Liquidity, OraclePeriod, RawEntry, RawOracle, Source, Volume};
use orml_traits::parameter_type_with_key;
use pallet_stableswap::types::{BoundedPegSources, PegSource, PoolSnapshot};
use sp_core::H256;
use sp_runtime::traits::{BlakeTwo256, IdentityLookup};
use sp_runtime::BoundedVec;
use sp_runtime::{BuildStorage, DispatchError, DispatchResult, Permill};
use sp_std::num::NonZeroU16;
use std::cell::RefCell;
use std::collections::HashMap;

type Block = frame_system::mocking::MockBlock<Test>;

pub type Signature = MultiSignature;
pub type Balance = u128;
pub type AssetId = u32;
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 1;
pub const USDC: AssetId = 2;
pub const HOLLAR: AssetId = 3;

pub const ALICE: AccountId = AccountId::new([1; 32]);
pub const BOB: AccountId = AccountId::new([2; 32]);
pub const CHARLIE: AccountId = AccountId::new([3; 32]);

pub const ONE: Balance = 1_000_000_000_000;
pub const GHO_ADDRESS: [u8; 20] = [1u8; 20];

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, (u32,u8)>> = RefCell::new(HashMap::default());
	pub static EVM_CALLS: RefCell<Vec<(EvmAddress, Vec<u8>)>> = RefCell::new(Vec::new());
	pub static EVM_CALL_RESULTS: RefCell<HashMap<Vec<u8>, Vec<u8>>> = RefCell::new(HashMap::default());
	pub static PEG_ORACLE_VALUES: RefCell<HashMap<(AssetId,AssetId), (Balance,Balance,u64)>> = RefCell::new(HashMap::default());
}

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Tokens: orml_tokens,
		Broadcast: pallet_broadcast,
		Stableswap: pallet_stableswap,
		HSM: pallet_hsm,
	}
);

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
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type RuntimeEvent = RuntimeEvent;
	type BlockHashCount = ConstU64<250>;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
	type SingleBlockMigrations = ();
	type MultiBlockMigrator = ();
	type PreInherents = ();
	type PostInherents = ();
	type PostTransactions = ();
}

impl pallet_broadcast::Config for Test {
	type RuntimeEvent = RuntimeEvent;
}

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

// Mock Stableswap implementation
impl pallet_stableswap::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Tokens;
	type ShareAccountId = DummyAccountIdConstructor;
	type AssetInspection = DummyRegistry;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type UpdateTradabilityOrigin = EnsureRoot<AccountId>;
	type MinPoolLiquidity = ConstU128<1000>;
	type AmplificationRange = AmplificationRange;
	type MinTradingLimit = ConstU128<1000>;
	type WeightInfo = ();
	type BlockNumberProvider = System;
	type DustAccountHandler = Whitelist;
	type Hooks = ();
	type TargetPegOracle = PegOracle;
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

parameter_types! {
	pub const HollarId: AssetId = HOLLAR;
	pub PalletId: frame_support::PalletId = frame_support::PalletId(*b"py/hsmdx");
	pub GhoContractAddress: EvmAddress = EvmAddress::from(GHO_ADDRESS);
	pub const GasLimit: u64 = 1_000_000;
	pub AmplificationRange: RangeInclusive<NonZeroU16> = RangeInclusive::new(NonZeroU16::new(2).unwrap(), NonZeroU16::new(10_000).unwrap());
}

pub struct DummyRegistry;

impl hydradx_traits::registry::Inspect for DummyRegistry {
	type AssetId = AssetId;
	type Location = u8;

	fn exists(asset_id: AssetId) -> bool {
		REGISTERED_ASSETS.with(|v| v.borrow().contains_key(&asset_id))
	}

	fn decimals(asset_id: AssetId) -> Option<u8> {
		REGISTERED_ASSETS.with(|v| v.borrow().get(&asset_id).map(|(_, decimals)| *decimals))
	}

	fn is_sufficient(_id: Self::AssetId) -> bool {
		true
	}

	fn asset_type(_id: Self::AssetId) -> Option<hydradx_traits::AssetKind> {
		Some(hydradx_traits::AssetKind::Token)
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		false
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		None
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		Some(0)
	}
}

pub struct PegOracle;

impl RawOracle<AssetId, Balance, u64> for PegOracle {
	type Error = ();

	fn get_raw_entry(
		_source: Source,
		asset_a: AssetId,
		asset_b: AssetId,
		_period: OraclePeriod,
	) -> Result<RawEntry<Balance, u64>, Self::Error> {
		let (n, d, u) = PEG_ORACLE_VALUES
			.with(|v| v.borrow().get(&(asset_a, asset_b)).copied())
			.ok_or(())?;

		Ok(RawEntry {
			price: (n, d),
			volume: Volume::default(),
			liquidity: Liquidity::default(),
			updated_at: u,
		})
	}
}

pub(crate) fn set_peg_oracle_value(asset_a: AssetId, asset_b: AssetId, price: (Balance, Balance), updated_at: u64) {
	PEG_ORACLE_VALUES.with(|v| {
		v.borrow_mut()
			.insert((asset_a, asset_b), (price.0, price.1, updated_at));
	});
}

pub struct DummyAccountIdConstructor;

impl AccountIdFor<AssetId> for DummyAccountIdConstructor {
	type AccountId = AccountId;

	fn from_assets(asset: &u32, _identifier: Option<&[u8]>) -> Self::AccountId {
		AccountId::new(Self::name(asset, None).try_into().unwrap())
	}

	fn name(asset: &u32, identifier: Option<&[u8]>) -> Vec<u8> {
		let mut buf: Vec<u8> = if let Some(ident) = identifier {
			ident.to_vec()
		} else {
			vec![]
		};
		buf.extend_from_slice(&(asset).to_le_bytes());

		buf
	}
}

// Mock EVM implementation
pub struct MockEvm;

impl EVM<(evm::ExitReason, Vec<u8>)> for MockEvm {
	fn call(context: CallContext, data: Vec<u8>, _value: U256, _gas: u64) -> CallResult {
		//TODO: see liquidation pallet
		(ExitReason::Succeed(ExitSucceed::Returned), vec![])
	}

	fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> CallResult {
		unimplemented!()
	}
}

// Mock EvmAccounts implementation
pub struct MockEvmAccounts;

impl InspectEvmAccounts<AccountId> for MockEvmAccounts {
	fn is_evm_account(account_id: AccountId) -> bool {
		todo!()
	}

	fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress {
		todo!()
	}

	fn truncated_account_id(evm_address: EvmAddress) -> AccountId {
		todo!()
	}

	fn bound_account_id(evm_address: EvmAddress) -> Option<AccountId> {
		todo!()
	}

	fn account_id(evm_address: EvmAddress) -> AccountId {
		todo!()
	}

	fn can_deploy_contracts(evm_address: EvmAddress) -> bool {
		todo!()
	}

	fn is_approved_contract(address: EvmAddress) -> bool {
		todo!()
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type HollarId = HollarId;
	type PalletId = PalletId;
	type GhoContractAddress = GhoContractAddress;
	type Currency = Tokens;
	type Evm = MockEvm;
	type EvmAccounts = MockEvmAccounts;
	type GasLimit = GasLimit;
}

pub struct Whitelist;

impl Contains<AccountId> for Whitelist {
	fn contains(account: &AccountId) -> bool {
		false
	}
}

impl DustRemovalAccountWhitelist<AccountId> for Whitelist {
	type Error = DispatchError;

	fn add_account(account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}

	fn remove_account(account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(AssetId, u8)>,
	collaterals: Vec<(AssetId, AssetId, Permill, Permill, Permill)>,
	pools: Vec<(AssetId, Vec<AssetId>, u16, Permill, Vec<PegSource<AssetId>>)>,
	initial_pool_liquidity: Vec<(AssetId, Vec<AssetAmount<AssetId>>)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// Clear thread-local storage for each test
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		EVM_CALLS.with(|v| {
			v.borrow_mut().clear();
		});
		EVM_CALL_RESULTS.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![],
			registered_assets: vec![],
			collaterals: vec![],
			pools: vec![],
			initial_pool_liquidity: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_registered_asset(mut self, asset: AssetId, decimals: u8) -> Self {
		self.registered_assets.push((asset, decimals));
		self
	}

	pub fn with_registered_assets(mut self, assets: Vec<(AssetId, u8)>) -> Self {
		self.registered_assets.extend(assets);
		self
	}

	pub fn with_pool(
		mut self,
		pool_id: AssetId,
		assets: Vec<AssetId>,
		amplification: u16,
		fee: Permill,
		pegs: Vec<PegSource<AssetId>>,
	) -> Self {
		self.pools.push((pool_id, assets, amplification, fee, pegs));
		self
	}

	pub fn with_initial_pool_liquidity(mut self, pool_id: AssetId, liquidity: Vec<AssetAmount<AssetId>>) -> Self {
		self.initial_pool_liquidity.push((pool_id, liquidity));
		self
	}

	pub fn with_collateral(
		mut self,
		asset_id: AssetId,
		pool_id: AssetId,
		purchase_fee: Permill,
		max_buy_price_coefficient: Permill,
		buy_back_fee: Permill,
	) -> Self {
		self.collaterals
			.push((asset_id, pool_id, purchase_fee, max_buy_price_coefficient, buy_back_fee));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		orml_tokens::GenesisConfig::<Test> {
			balances: self.endowed_accounts,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			// Register assets
			for (asset_id, decimals) in self.registered_assets {
				REGISTERED_ASSETS.with(|v| {
					v.borrow_mut().insert(asset_id, (asset_id as u32, decimals));
				});
			}

			// Set up collaterals
			for (asset_id, pool_id, purchase_fee, max_buy_price_coefficient, buy_back_fee) in self.collaterals {
				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					asset_id,
					pool_id,
					purchase_fee,
					max_buy_price_coefficient,
					buy_back_fee,
					sp_runtime::Perbill::from_percent(50),
					None,
				)
				.unwrap();
			}

			for (pool_id, assets, amplification, fee, pegs) in self.pools {
				Stableswap::create_pool_with_pegs(
					RuntimeOrigin::root(),
					pool_id,
					BoundedVec::try_from(assets).unwrap(),
					amplification,
					fee,
					BoundedPegSources::try_from(pegs).unwrap(),
					Permill::from_percent(100),
				)
				.unwrap();
			}

			for (pool_id, liquidity) in self.initial_pool_liquidity {
				Stableswap::add_assets_liquidity(
					RuntimeOrigin::root(),
					pool_id,
					BoundedVec::try_from(liquidity).unwrap(),
					0,
				)
				.unwrap();
			}

			System::set_block_number(1);
		});
		ext
	}
}

// Helper function to set EVM call result for testing
pub fn set_evm_call_result(input: Vec<u8>, output: Vec<u8>) {
	EVM_CALL_RESULTS.with(|v| {
		v.borrow_mut().insert(input, output);
	});
}

// Helper function to get last EVM call
pub fn last_evm_call() -> Option<(EvmAddress, Vec<u8>)> {
	EVM_CALLS.with(|v| {
		let calls = v.borrow();
		calls.last().cloned()
	})
}

// Helper function to clear EVM calls
pub fn clear_evm_calls() {
	EVM_CALLS.with(|v| {
		v.borrow_mut().clear();
	});
}
