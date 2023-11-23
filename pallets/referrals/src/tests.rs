// This file is part of HydraDX.

// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

mod claim;
mod convert;
mod link;
mod register;

use crate as pallet_referrals;
use crate::*;

use std::cell::RefCell;
use std::collections::HashMap;

use frame_support::{
	construct_runtime, parameter_types,
	sp_runtime::{
		testing::Header,
		traits::{BlakeTwo256, IdentityLookup},
	},
	traits::{ConstU32, ConstU64, Everything, GenesisBuild},
	PalletId,
};
use sp_core::H256;

use crate::traits::Convert;
use frame_support::{assert_noop, assert_ok};
use orml_traits::MultiCurrency;
use orml_traits::{parameter_type_with_key, MultiCurrencyExtended};
use sp_runtime::{DispatchError, FixedPointNumber, FixedU128};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub(crate) type AccountId = u64;
pub(crate) type AssetId = u32;

pub(crate) const ONE: Balance = 1_000_000_000_000;

pub const HDX: AssetId = 0;
pub const DAI: AssetId = 2;
pub const DOT: AssetId = 5;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
pub const TREASURY: AccountId = 400;

pub(crate) const INITIAL_ALICE_BALANCE: Balance = 1_000 * ONE;

thread_local! {
	pub static CONVERSION_RATE: RefCell<HashMap<(AssetId,AssetId), FixedU128>> = RefCell::new(HashMap::default());
	pub static TIER_VOLUME: RefCell<HashMap<Level, Option<Balance>>> = RefCell::new(HashMap::default());
}

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system,
		Referrals: pallet_referrals,
		Tokens: orml_tokens,
	}
);

parameter_types! {
	pub const RefarralPalletId: PalletId = PalletId(*b"test_ref");
	pub const CodeLength: u32 = 7;
	pub const RegistrationFee: (AssetId,Balance, AccountId) = (HDX, 222 * 1_000_000_000_000, TREASURY) ;
	pub const RewardAsset: AssetId = HDX;
}

pub struct Volume;

impl GetByKey<Level, Option<Balance>> for Volume {
	fn get(level: &Level) -> Option<Balance> {
		TIER_VOLUME.with(|v| v.borrow().get(level).copied()).unwrap_or_default()
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Tokens;
	type Convert = AssetConvert;
	type RewardAsset = RewardAsset;
	type PalletId = RefarralPalletId;
	type RegistrationFee = RegistrationFee;
	type CodeLength = CodeLength;
	type TierVolume = Volume;
	type WeightInfo = ();
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = u64;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
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
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_asset_id: AssetId| -> Balance {
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

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	trades: Vec<(AccountId, AssetId, Balance)>,
	rewards: Vec<(AccountId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		CONVERSION_RATE.with(|v| {
			v.borrow_mut().clear();
		});
		Self {
			endowed_accounts: vec![(ALICE, HDX, INITIAL_ALICE_BALANCE)],
			trades: vec![],
			rewards: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_trade_activity(mut self, trades: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.trades.extend(trades);
		self
	}

	pub fn with_rewards(mut self, rewards: Vec<(AccountId, Balance)>) -> Self {
		self.rewards.extend(rewards);
		self
	}

	pub fn with_conversion_price(self, pair: (AssetId, AssetId), price: FixedU128) -> Self {
		CONVERSION_RATE.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(pair, price);
		});
		self
	}

	pub fn with_tier_volumes(self, volumes: HashMap<Level, Option<Balance>>) -> Self {
		TIER_VOLUME.with(|v| {
			v.swap(&RefCell::new(volumes));
		});
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

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
			for (acc, asset, amount) in self.trades.iter() {
				Accrued::<Test>::insert(asset, acc, amount);
				Tokens::update_balance(*asset, &Pallet::<Test>::pot_account_id(), *amount as i128).unwrap();
			}
		});

		r.execute_with(|| {
			for (acc, amount) in self.rewards.iter() {
				Rewards::<Test>::insert(acc, amount);
				Tokens::update_balance(HDX, &Pallet::<Test>::pot_account_id(), *amount as i128).unwrap();
			}
		});

		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}

pub fn expect_events(e: Vec<RuntimeEvent>) {
	e.into_iter().for_each(frame_system::Pallet::<Test>::assert_has_event);
}

pub struct AssetConvert;

impl Convert<AccountId, AssetId, Balance> for AssetConvert {
	type Error = DispatchError;

	fn convert(
		who: AccountId,
		asset_from: AssetId,
		asset_to: AssetId,
		amount: Balance,
	) -> Result<Balance, Self::Error> {
		let price = CONVERSION_RATE
			.with(|v| v.borrow().get(&(asset_to, asset_from)).copied())
			.ok_or(Error::<Test>::InvalidCode)?;
		let result = price.saturating_mul_int(amount);
		Tokens::update_balance(asset_from, &who, -(amount as i128)).unwrap();
		Tokens::update_balance(asset_to, &who, result as i128).unwrap();
		Ok(result)
	}
}

#[macro_export]
macro_rules! assert_balance {
	( $x:expr, $y:expr, $z:expr) => {{
		assert_eq!(Tokens::free_balance($y, &$x), $z);
	}};
}
