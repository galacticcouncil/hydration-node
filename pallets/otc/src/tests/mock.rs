//                    :                     $$\   $$\                 $$\                    $$$$$$$\  $$\   $$\ 
//                  !YJJ^                   $$ |  $$ |                $$ |                   $$  __$$\ $$ |  $$ |
//                7B5. ~B5^                 $$ |  $$ |$$\   $$\  $$$$$$$ | $$$$$$\  $$$$$$\  $$ |  $$ |\$$\ $$  |
//             .?B@G    ~@@P~               $$$$$$$$ |$$ |  $$ |$$  __$$ |$$  __$$\ \____$$\ $$ |  $$ | \$$$$  / 
//           :?#@@@Y    .&@@@P!.            $$  __$$ |$$ |  $$ |$$ /  $$ |$$ |  \__|$$$$$$$ |$$ |  $$ | $$  $$<  
//         ^?J^7P&@@!  .5@@#Y~!J!.          $$ |  $$ |$$ |  $$ |$$ |  $$ |$$ |     $$  __$$ |$$ |  $$ |$$  /\$$\ 
//       ^JJ!.   :!J5^ ?5?^    ^?Y7.        $$ |  $$ |\$$$$$$$ |\$$$$$$$ |$$ |     \$$$$$$$ |$$$$$$$  |$$ /  $$ |
//     ~PP: 7#B5!.         :?P#G: 7G?.      \__|  \__| \____$$ | \_______|\__|      \_______|\_______/ \__|  \__|
//  .!P@G    7@@@#Y^    .!P@@@#.   ~@&J:              $$\   $$ |                                                 
//  !&@@J    :&@@@@P.   !&@@@@5     #@@P.             \$$$$$$  |                                                 
//   :J##:   Y@@&P!      :JB@@&~   ?@G!                \______/                                                  
//     .?P!.?GY7:   .. .    ^?PP^:JP~     
//       .7Y7.  .!YGP^ ?BP?^   ^JJ^         This file is part of https://github.com/galacticcouncil/HydraDX-node
//         .!Y7Y#@@#:   ?@@@G?JJ^           Built with <3 for decentralisation.
//            !G@@@Y    .&@@&J:           
//              ^5@#.   7@#?.               Copyright (C) 2021-2023  Intergalactic, Limited (GIB).
//                :5P^.?G7.                 SPDX-License-Identifier: Apache-2.0
//                  :?Y!                    Licensed under the Apache License, Version 2.0 (the "License");
//                                          you may not use this file except in compliance with the License.
//                                          http://www.apache.org/licenses/LICENSE-2.0

use crate as otc;
use crate::{Config, Order};
use frame_support::{
	assert_ok, parameter_types,
	traits::{ConstU128, Everything, GenesisBuild, Nothing},
	weights::{constants::ExtrinsicBaseWeight, WeightToFeeCoefficient},
};
use frame_system as system;
use hydradx_traits::Registry;
use orml_traits::{parameter_type_with_key, MultiCurrency};
use pallet_currencies::BasicCurrencyAdapter;
use pretty_assertions::assert_eq;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{AccountIdConversion, BlakeTwo256, BlockNumberProvider, Get, IdentityLookup, One},
	DispatchError, Perbill, Permill,
};
use std::{cell::RefCell, collections::HashMap};
use test_utils::last_events;

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type AccountId = u64;
pub type Amount = i128;
pub type AssetId = u32;
pub type Balance = u128;
pub type BlockNumber = u64;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;
pub const BTC: AssetId = 3;
pub const DOGE: AssetId = 333;
pub const REGISTERED_ASSET: AssetId = 1000;
pub const ONE_HUNDRED_BLOCKS: BlockNumber = 100;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const LP3: u64 = 3;

pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system,
		 OTC: otc,
		 Tokens: orml_tokens,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
	 }
);

thread_local! {
	pub static REGISTERED_ASSETS: RefCell<HashMap<AssetId, u32>> = RefCell::new(HashMap::default());
}

parameter_types! {
	pub NativeCurrencyId: AssetId = HDX;
	pub ExistentialDepositMultiplier: u128 = 5 * ONE;
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		One::one()
	};
}

impl Config for Test {
	type AssetId = AssetId;
	type AssetRegistry = DummyRegistry<Test>;
	type Currency = Tokens;
	type Event = Event;
	type ExistentialDeposits = ExistentialDeposits;
	type ExistentialDepositMultiplier = ExistentialDepositMultiplier;
	type MultiReservableCurrency = Currencies;
	type NativeAssetId = NativeCurrencyId;
	type WeightInfo = ();
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
	type ReserveIdentifier = ();
	type MaxReserves = ();
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

impl pallet_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
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
		Ok(T::AssetId::default())
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

		Self {
			endowed_accounts: vec![
				(ALICE, HDX, 10_000 * ONE),
				(BOB, HDX, 10_000 * ONE),
				(ALICE, DAI, 100 * ONE),
				(BOB, DAI, 100 * ONE),
			],
			registered_assets: vec![HDX, DAI],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn add_endowed_accounts(mut self, account: (u64, AssetId, Balance)) -> Self {
		self.endowed_accounts.push(account);
		self
	}
	pub fn with_registered_asset(mut self, asset: AssetId) -> Self {
		self.registered_assets.push(asset);
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		// Add DAI and HDX as pre-registered assets
		REGISTERED_ASSETS.with(|v| {
			v.borrow_mut().insert(HDX, HDX);
			v.borrow_mut().insert(REGISTERED_ASSET, REGISTERED_ASSET);
			self.registered_assets.iter().for_each(|asset| {
				v.borrow_mut().insert(*asset, *asset);
			});
		});

		pallet_balances::GenesisConfig::<Test> {
			balances: self
				.endowed_accounts
				.iter()
				.filter(|a| a.1 == HDX)
				.flat_map(|(x, asset, amount)| vec![(*x, *amount)])
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

		r.execute_with(|| {});

		r
	}
}

thread_local! {
	pub static DummyThreadLocal: RefCell<u128> = RefCell::new(100);
}

pub fn expect_events(e: Vec<Event>) {
	test_utils::expect_events::<Event, Test>(e);
}
