// This file is part of https://github.com/galacticcouncil/*
//
//                $$$$$$$      Licensed under the Apache License, Version 2.0 (the "License")
//             $$$$$$$$$$$$$        you may only use this file in compliance with the License
//          $$$$$$$$$$$$$$$$$$$
//                      $$$$$$$$$       Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
//         $$$$$$$$$$$   $$$$$$$$$$                       SPDX-License-Identifier: Apache-2.0
//      $$$$$$$$$$$$$$$$$$$$$$$$$$
//   $$$$$$$$$$$$$$$$$$$$$$$        $                      Built with <3 for decentralisation
//  $$$$$$$$$$$$$$$$$$$        $$$$$$$
//  $$$$$$$         $$$$$$$$$$$$$$$$$$      Unless required by applicable law or agreed to in
//   $       $$$$$$$$$$$$$$$$$$$$$$$       writing, software distributed under the License is
//      $$$$$$$$$$$$$$$$$$$$$$$$$$        distributed on an "AS IS" BASIS, WITHOUT WARRANTIES
//      $$$$$$$$$   $$$$$$$$$$$         OR CONDITIONS OF ANY KIND, either express or implied.
//        $$$$$$$$
//          $$$$$$$$$$$$$$$$$$            See the License for the specific language governing
//             $$$$$$$$$$$$$                   permissions and limitations under the License.
//                $$$$$$$
//                                                                 $$
//  $$$$$   $$$$$                    $$                       $
//   $$$     $$$  $$$     $$   $$$$$ $$  $$$ $$$$  $$$$$$$  $$$$  $$$    $$$$$$   $$ $$$$$$
//   $$$     $$$   $$$   $$  $$$    $$$   $$$  $  $$     $$  $$    $$  $$     $$   $$$   $$$
//   $$$$$$$$$$$    $$  $$   $$$     $$   $$        $$$$$$$  $$    $$  $$     $$$  $$     $$
//   $$$     $$$     $$$$    $$$     $$   $$     $$$     $$  $$    $$   $$     $$  $$     $$
//  $$$$$   $$$$$     $$      $$$$$$$$ $ $$$      $$$$$$$$   $$$  $$$$   $$$$$$$  $$$$   $$$$
//                  $$$

use crate as dispatcher;
use crate::Config;
use frame_support::pallet_prelude::Weight;
use frame_support::{
	parameter_types,
	traits::{Everything, Nothing},
	PalletId,
};
use frame_system as system;
use frame_system::EnsureRoot;
use hydradx_traits::{registry::Inspect, AssetKind};
use orml_tokens::AccountData;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	BuildStorage, Permill,
};
use std::{cell::RefCell, collections::HashMap};

type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type NamedReserveIdentifier = [u8; 8];

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 2;
pub const DOGE: AssetId = 333;
pub const REGISTERED_ASSET: AssetId = 1000;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

pub const TREASURY_INITIAL_BALANCE: Balance = 1_000_000 * ONE;

frame_support::construct_runtime!(
	pub enum Test
	 {
		 System: frame_system,
		 Dispatcher: dispatcher,
		 Tokens: orml_tokens,
	 }
);

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
	pub static EXISTENTIAL_DEPOSIT: RefCell<HashMap<AssetId, u128>>= RefCell::new(HashMap::default());
	pub static PRECISIONS: RefCell<HashMap<AssetId, u32>>= RefCell::new(HashMap::default());
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub ExistentialDepositMultiplier: u8 = 5;
	pub OtcFee: Permill = Permill::from_percent(1u32);
	pub const TreasuryPalletId: PalletId = PalletId(*b"aca/trsy");
	pub TreasuryAccount: AccountId = TreasuryPalletId::get().into_account_truncating();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |currency_id: AssetId| -> Balance {
		EXISTENTIAL_DEPOSIT.with(|v| *v.borrow().get(currency_id).unwrap_or(&(ONE / 10)))
	};
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

impl dispatcher::Config for Test {
	type RuntimeCall = RuntimeCall;
	type RuntimeEvent = RuntimeEvent;
	type TreasuryManagerOrigin = EnsureRoot<AccountId>;
	type AaveManagerOrigin = EnsureRoot<AccountId>;
	type TreasuryAccount = TreasuryAccount;
	type DefaultAaveManagerAccount = TreasuryAccount;
	type WeightInfo = ();
	type GasWeightMapping = MockGasWeightMapping;
}

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub const MaxReserves: u32 = 50;
}

impl system::Config for Test {
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
	type AccountData = AccountData<u128>;
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

pub struct DummyRegistry<T>(sp_std::marker::PhantomData<T>);

impl<T: Config> Inspect for DummyRegistry<T> {
	type AssetId = AssetId;
	type Location = u8;

	fn asset_type(_id: Self::AssetId) -> Option<AssetKind> {
		unimplemented!()
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		unimplemented!()
	}

	fn is_sufficient(_id: Self::AssetId) -> bool {
		unimplemented!()
	}

	fn exists(asset_id: AssetId) -> bool {
		let asset = REGISTERED_ASSETS.with(|v| v.borrow().get(&(asset_id)).copied());
		asset.is_some()
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

#[cfg(feature = "runtime-benchmarks")]
use hydradx_traits::Create as CreateRegistry;
#[cfg(feature = "runtime-benchmarks")]
use sp_runtime::DispatchError;
#[cfg(feature = "runtime-benchmarks")]
impl<T: Config> CreateRegistry<Balance> for DummyRegistry<T> {
	type Error = DispatchError;
	type Name = sp_runtime::BoundedVec<u8, sp_core::ConstU32<100>>;
	type Symbol = sp_runtime::BoundedVec<u8, sp_core::ConstU32<100>>;

	fn register_asset(
		_asset_id: Option<Self::AssetId>,
		_name: Option<Self::Name>,
		_kind: AssetKind,
		_existential_deposit: Option<Balance>,
		_symbol: Option<Self::Symbol>,
		_decimals: Option<u8>,
		_location: Option<Self::Location>,
		_xcm_rate_limit: Option<Balance>,
		_is_sufficient: bool,
	) -> Result<Self::AssetId, Self::Error> {
		let assigned = REGISTERED_ASSETS.with(|v| {
			//NOTE: This is to have same ids as real AssetRegistry which is used in the benchmarks.
			//1_000_000 - offset of the reals AssetRegistry
			// - 3 - remove assets reagistered by default for the vec.len()
			// +1 - first reg asset start with 1 not 0
			// => 1-th asset id == 1_000_001
			let l = 1_000_000 - 3 + 1 + v.borrow().len();
			v.borrow_mut().insert(l as u32, l as u32);
			l as u32
		});
		Ok(assigned)
	}

	fn get_or_register_asset(
		_name: Self::Name,
		_kind: AssetKind,
		_existential_deposit: Option<Balance>,
		_symbol: Option<Self::Symbol>,
		_decimals: Option<u8>,
		_location: Option<Self::Location>,
		_xcm_rate_limit: Option<Balance>,
		_is_sufficient: bool,
	) -> Result<Self::AssetId, Self::Error> {
		unimplemented!()
	}
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	registered_assets: Vec<AssetId>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		// If eg. tests running on one thread only, this thread local is shared.
		// let's make sure that it is empty for each  test case
		// or set to original default value
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().clear();
		});
		EXISTENTIAL_DEPOSIT.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 10_000),
				(BOB, HDX, 10_000),
				(ALICE, DAI, 100),
				(BOB, DAI, 100),
				(TreasuryAccount::get(), HDX, 1_000_000),
			],
			registered_assets: vec![HDX, DAI],
		}
	}
}

impl ExtBuilder {
	pub fn with_existential_deposit(self, asset_id: AssetId, precision: u32) -> Self {
		EXISTENTIAL_DEPOSIT.with(|v| {
			v.borrow_mut().insert(asset_id, 10u128.pow(precision));
		});
		PRECISIONS.with(|v| {
			v.borrow_mut().insert(asset_id, precision);
		});

		self
	}
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		// Add DAI and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(REGISTERED_ASSET, REGISTERED_ASSET);
			self.registered_assets.iter().for_each(|asset| {
				v.borrow_mut().insert(*asset, *asset);
			});
		});

		orml_tokens::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.flat_map(|(x, asset, amount)| vec![(*x, *asset, *amount * 10u128.pow(precision(*asset)))])
				.collect(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut r: sp_io::TestExternalities = t.into();

		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}

// thread_local! {
// 	pub static DUMMYTHREADLOCAL: RefCell<u128> = const { RefCell::new(100) };
// }

pub fn expect_events(e: Vec<RuntimeEvent>) {
	test_utils::expect_events::<RuntimeEvent, Test>(e);
}

pub fn precision(asset_id: AssetId) -> u32 {
	PRECISIONS.with(|v| *v.borrow().get(&asset_id).unwrap_or(&12))
}
