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
use crate::types::{CallResult, CoefficientRatio};
use crate::Config;
use crate::ERC20Function;
use core::ops::RangeInclusive;
use ethabi::ethereum_types::U256;
use evm::{ExitError, ExitReason, ExitSucceed};
use frame_support::pallet_prelude::{Hooks, Weight};
use frame_support::sp_runtime::{
	traits::{IdentifyAccount, Verify},
	MultiSignature,
};
use frame_support::traits::Contains;
use frame_support::traits::{ConstU128, ConstU32, ConstU64, Everything};
use frame_support::{construct_runtime, parameter_types};
use frame_system::EnsureRoot;
use hydradx_traits::pools::DustRemovalAccountWhitelist;
use hydradx_traits::{
	evm::{CallContext, EvmAddress, InspectEvmAccounts, EVM},
	stableswap::AssetAmount,
	AssetKind, BoundErc20, Inspect,
};
use hydradx_traits::{AccountIdFor, Liquidity, RawEntry, Source, Volume};
use orml_traits::parameter_type_with_key;
use orml_traits::MultiCurrencyExtended;
use pallet_stableswap::traits::PegRawOracle;
use pallet_stableswap::types::{BoundedPegSources, PegSource};
use precompile_utils::evm::writer::EvmDataReader;
use sp_core::{ByteArray, H256};
use sp_runtime::traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup};
use sp_runtime::{BoundedVec, Perbill};
use sp_runtime::{BuildStorage, DispatchError, Permill};
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
pub const PROVIDER: AccountId = AccountId::new([4; 32]);

pub const ONE: Balance = 1_000_000_000_000_000_000;

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
	pub static EVM_ADDRESS_MAP: RefCell<HashMap<EvmAddress, AccountId>> = RefCell::new(HashMap::default());
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
	type BenchmarkHelper = for_benchmark_tests::MockStableswapBenchmarkHelper;
}

parameter_types! {
	pub const HollarId: AssetId = HOLLAR;
	pub PalletId: frame_support::PalletId = frame_support::PalletId(*b"py/hsmdx");
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

impl PegRawOracle<AssetId, Balance, u64> for PegOracle {
	type Error = ();

	fn get_raw_entry(peg_asset: AssetId, source: PegSource<AssetId>) -> Result<RawEntry<Balance, u64>, Self::Error> {
		match source {
			PegSource::Value(v) => {
				let (n, d) = v;
				let u = System::block_number();
				return Ok(RawEntry {
					price: (n, d),
					volume: Volume::default(),
					liquidity: Liquidity::default(),
					updated_at: u,
				});
			}
			PegSource::Oracle((_, _, asset_id)) => {
				let (n, d, u) = PEG_ORACLE_VALUES
					.with(|v| v.borrow().get(&(asset_id, peg_asset)).copied())
					.ok_or(())?;

				Ok(RawEntry {
					price: (n, d),
					volume: Volume::default(),
					liquidity: Liquidity::default(),
					updated_at: u,
				})
			}
			PegSource::MMOracle(_) => {
				panic!("not supported");
			}
		}
	}
}

#[allow(unused)]
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
		// This must return length 32

		let mut buf: Vec<u8> = if let Some(ident) = identifier {
			ident.to_vec()
		} else {
			vec![]
		};
		buf.extend_from_slice(&(asset).to_le_bytes());

		while buf.len() < 32 {
			buf.push(0);
		}

		buf
	}
}

// Mock EVM implementation
pub struct MockEvm;

impl EVM<CallResult> for MockEvm {
	fn call(context: CallContext, data: Vec<u8>, _value: U256, _gas: u64) -> CallResult {
		EVM_CALLS.with(|v| v.borrow_mut().push((context.contract, data.clone())));

		// Check if the call has a pre-defined result in our mock
		let maybe_predefined = EVM_CALL_RESULTS.with(|v| v.borrow().get(&data).cloned());
		if let Some(result) = maybe_predefined {
			return (ExitReason::Succeed(ExitSucceed::Stopped), result);
		}

		// Handle the EVM functions
		if data.len() >= 4 {
			let function_bytes: [u8; 4] = data[0..4].try_into().unwrap_or([0; 4]);
			let function_u32 = u32::from_be_bytes(function_bytes);

			if let Ok(function) = ERC20Function::try_from(function_u32) {
				match function {
					ERC20Function::Mint => {
						// Should include recipient (32 bytes) and amount (32 bytes) parameters after the 4-byte selector
						if data.len() >= 4 + 32 + 32 {
							// Extract recipient address (padded to 32 bytes in ABI encoding)
							let recipient_bytes: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
							let recipient_evm = EvmAddress::from_slice(&recipient_bytes[12..32]);

							// Extract amount (32 bytes)
							let amount_bytes: [u8; 32] = data[4 + 32..4 + 64].try_into().unwrap_or([0; 32]);
							let amount = U256::from_big_endian(&amount_bytes);

							// Convert to Balance and account IDs for our operation
							if let Ok(amount) = Balance::try_from(amount) {
								let recipient = MockEvmAccounts::account_id(recipient_evm);
								let hollar_id = <Test as pallet_hsm::Config>::HollarId::get();

								// Increase the balance of the recipient
								let _ = Tokens::update_balance(hollar_id, &recipient, amount as i128);

								return (ExitReason::Succeed(ExitSucceed::Stopped), vec![]);
							}
						}
					}
					ERC20Function::Burn => {
						// Should include amount (32 bytes) parameter after the 4-byte selector
						if data.len() >= 4 + 32 {
							// Extract amount (32 bytes)
							let amount_bytes: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
							let amount = U256::from_big_endian(&amount_bytes);

							// Convert to Balance and account IDs for our operation
							if let Ok(amount) = Balance::try_from(amount) {
								let sender = context.sender;
								let account_id = MockEvmAccounts::account_id(sender);
								let hollar_id = <Test as pallet_hsm::Config>::HollarId::get();

								// Decrease the balance of the caller
								let _ = Tokens::update_balance(hollar_id, &account_id, -(amount as i128));

								return (ExitReason::Succeed(ExitSucceed::Stopped), vec![]);
							}
						}
					}
					ERC20Function::FlashLoan => {
						if data.len() >= 4 + 32 + 32 + 32 {
							// Extract recipient address (padded to 32 bytes in ABI encoding)
							let receiver: [u8; 32] = data[4..4 + 32].try_into().unwrap_or([0; 32]);
							let _receiver_evm = EvmAddress::from_slice(&receiver[12..32]);

							let hollar: [u8; 32] = data[4 + 32..4 + 32 + 32].try_into().unwrap_or([0; 32]);
							let _hollar_evm = EvmAddress::from_slice(&hollar[12..32]);

							let amount_bytes: [u8; 32] = data[4 + 32 + 32..4 + 32 + 32 + 32].try_into().unwrap();
							let amount = U256::from_big_endian(&amount_bytes);

							let arb_data = data[4 + 32 + 32 + 32 + 32 + 32..].to_vec();
							let mut reader = EvmDataReader::new(&arb_data);
							let _data_ident: u8 = reader.read().unwrap();
							let collateral_asset_id: u32 = reader.read().unwrap();
							let pool_id: u32 = reader.read().unwrap();
							let arb_account = ALICE.into();
							crate::Pallet::<Test>::mint_hollar(&arb_account, amount.as_u128()).unwrap();
							crate::Pallet::<Test>::execute_arbitrage_with_flash_loan(
								&arb_account,
								pool_id,
								collateral_asset_id,
								amount.as_u128(),
							)
							.unwrap();
							crate::Pallet::<Test>::burn_hollar(amount.as_u128()).unwrap();
							return (ExitReason::Succeed(ExitSucceed::Stopped), vec![]);
						} else {
							panic!("incorrect data len");
						}
					}
				}
			}
		}

		// Default failure for unrecognized calls
		(ExitReason::Error(ExitError::DesignatedInvalid), vec![])
	}

	fn view(_context: CallContext, _data: Vec<u8>, _gas: u64) -> CallResult {
		unimplemented!()
	}
}

// Mock EvmAccounts implementation
pub struct MockEvmAccounts;

fn map_to_acc(evm_addr: EvmAddress) -> AccountId {
	let alice_evm = EvmAddress::from_slice(&ALICE.as_slice()[0..20]);
	let provider_evm = EvmAddress::from_slice(&PROVIDER.as_slice()[0..20]);
	let bob_evm = EvmAddress::from_slice(&BOB.as_slice()[0..20]);
	let hsm_evm = EvmAddress::from_slice(&HSM::account_id().as_slice()[0..20]);

	if evm_addr == alice_evm {
		ALICE
	} else if evm_addr == provider_evm {
		PROVIDER
	} else if evm_addr == bob_evm {
		BOB
	} else if evm_addr == hsm_evm {
		HSM::account_id()
	} else {
		EVM_ADDRESS_MAP.with(|v| v.borrow().get(&evm_addr).cloned().expect("EVM address not found"))
	}
}

impl InspectEvmAccounts<AccountId> for MockEvmAccounts {
	fn is_evm_account(_account_id: AccountId) -> bool {
		unimplemented!()
	}

	fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> EvmAddress {
		let acc = account_id.as_ref();
		EvmAddress::from_slice(&acc[..20])
	}

	fn truncated_account_id(_evm_address: EvmAddress) -> AccountId {
		unimplemented!()
	}

	fn bound_account_id(_evm_address: EvmAddress) -> Option<AccountId> {
		unimplemented!()
	}

	fn account_id(evm_address: EvmAddress) -> AccountId {
		map_to_acc(evm_address)
	}

	fn can_deploy_contracts(_evm_address: EvmAddress) -> bool {
		unimplemented!()
	}

	fn is_approved_contract(_address: EvmAddress) -> bool {
		unimplemented!()
	}
}

pub struct GhoContractAddress;

impl Inspect for GhoContractAddress {
	type AssetId = AssetId;
	type Location = ();

	fn is_sufficient(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn exists(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		unimplemented!()
	}

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		unimplemented!()
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

impl BoundErc20 for GhoContractAddress {
	fn contract_address(id: Self::AssetId) -> Option<EvmAddress> {
		assert_eq!(id, HollarId::get());
		Some(GHO_ADDRESS.into())
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type HollarId = HollarId;
	type PalletId = PalletId;
	type GhoContractAddress = GhoContractAddress;
	type Currency = Tokens;
	type Evm = MockEvm;
	type EvmAccounts = MockEvmAccounts;
	type GasLimit = GasLimit;
	type GasWeightMapping = MockGasWeightMapping;
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = for_benchmark_tests::MockHSMBenchmarkHelper;
}

pub struct Whitelist;

impl Contains<AccountId> for Whitelist {
	fn contains(_account: &AccountId) -> bool {
		false
	}
}

impl DustRemovalAccountWhitelist<AccountId> for Whitelist {
	type Error = DispatchError;

	fn add_account(_account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}

	fn remove_account(_account: &AccountId) -> Result<(), Self::Error> {
		Ok(())
	}
}

pub struct MockGasWeightMapping;
impl pallet_evm::GasWeightMapping for MockGasWeightMapping {
	fn gas_to_weight(_gas: u64, _without_base_weight: bool) -> Weight {
		Weight::zero()
	}
	fn weight_to_gas(_weight: Weight) -> u64 {
		0
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	registered_assets: Vec<(AssetId, u8)>,
	collaterals: Vec<(AssetId, AssetId, Permill, CoefficientRatio, Permill, Option<Perbill>)>,
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
		EVM_ADDRESS_MAP.with(|v| {
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
		self.registered_assets = assets;
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
		max_buy_price_coefficient: CoefficientRatio,
		buy_back_fee: Permill,
	) -> Self {
		self.collaterals.push((
			asset_id,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			None,
		));
		self
	}

	pub fn with_collateral_buyback_limit(
		mut self,
		asset_id: AssetId,
		pool_id: AssetId,
		purchase_fee: Permill,
		max_buy_price_coefficient: CoefficientRatio,
		buy_back_fee: Permill,
		buyback_limit: Perbill,
	) -> Self {
		self.collaterals.push((
			asset_id,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			Some(buyback_limit),
		));
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
				// mint alice and bob with pool tokens
				for asset in liquidity.iter() {
					frame_support::assert_ok!(Tokens::update_balance(
						asset.asset_id,
						&AccountId::from(PROVIDER),
						asset.amount as i128
					));
				}

				Stableswap::add_assets_liquidity(
					RuntimeOrigin::signed(PROVIDER),
					pool_id,
					BoundedVec::try_from(liquidity).unwrap(),
					0,
				)
				.unwrap();
			}
			for (asset_id, pool_id, purchase_fee, max_buy_price_coefficient, buy_back_fee, bblimit) in self.collaterals
			{
				let limit = if let Some(l) = bblimit {
					l
				} else {
					Perbill::from_percent(50)
				};

				HSM::add_collateral_asset(
					RuntimeOrigin::root(),
					asset_id,
					pool_id,
					purchase_fee,
					max_buy_price_coefficient,
					buy_back_fee,
					limit,
					None,
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

pub fn default_peg() -> PegSource<AssetId> {
	PegSource::Value((1, 1))
}

pub fn move_block() {
	let current_block = System::current_block_number();
	HSM::on_finalize(current_block);
	Stableswap::on_finalize(current_block);
	System::set_block_number(current_block + 1);
}

#[cfg(feature = "runtime-benchmarks")]
mod for_benchmark_tests {
	use super::*;
	use crate::types::PegType;
	use frame_support::dispatch::DispatchResult;
	pub struct MockStableswapBenchmarkHelper;

	impl pallet_stableswap::BenchmarkHelper<AssetId> for MockStableswapBenchmarkHelper {
		fn register_asset(asset_id: AssetId, decimals: u8) -> DispatchResult {
			REGISTERED_ASSETS.with(|v| {
				v.borrow_mut().insert(asset_id, (asset_id as u32, decimals));
			});
			Ok(())
		}

		fn register_asset_peg(asset_pair: (AssetId, AssetId), peg: PegType, _source: Source) -> DispatchResult {
			set_peg_oracle_value(asset_pair.0, asset_pair.1, peg, 0);
			Ok(())
		}
	}

	pub struct MockHSMBenchmarkHelper;

	impl crate::traits::BenchmarkHelper<AccountId> for MockHSMBenchmarkHelper {
		fn bind_address(account: AccountId) -> DispatchResult {
			let evm_addr = EvmAddress::from_slice(&account.as_slice()[0..20]);
			EVM_ADDRESS_MAP.with(|v| {
				v.borrow_mut().insert(evm_addr, account);
			});
			Ok(())
		}
	}
}
