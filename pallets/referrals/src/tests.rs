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
mod flow;
mod link;
mod mock_amm;
mod register;
mod tiers;
mod trade_fee;

use crate as pallet_referrals;
use crate::*;

use std::cell::RefCell;
use std::collections::HashMap;

use frame_support::{
	assert_noop, assert_ok, construct_runtime, parameter_types,
	sp_runtime::traits::{BlakeTwo256, ConstU32, ConstU64, IdentityLookup, Zero},
	traits::Everything,
	PalletId,
};
use sp_core::H256;

use crate::tests::mock_amm::{Hooks, TradeResult};
use crate::traits::Convert;
use frame_system::EnsureRoot;
use hydra_dx_math::ema::EmaPrice;
use orml_traits::MultiCurrency;
use orml_traits::{parameter_type_with_key, MultiCurrencyExtended};
use sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{BuildStorage, DispatchError, Rounding};

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
	pub static CONVERSION_RATE: RefCell<HashMap<(AssetId,AssetId), EmaPrice>> = RefCell::new(HashMap::default());
	pub static TIER_VOLUME: RefCell<HashMap<Level, Option<Balance>>> = RefCell::new(HashMap::default());
	pub static TIER_REWARDS: RefCell<HashMap<Level, FeeDistribution>> = RefCell::new(HashMap::default());
	pub static SEED_AMOUNT: RefCell<Balance> = RefCell::new(Balance::zero());
	pub static EXTERNAL_ACCOUNT: RefCell<Option<AccountId>> = RefCell::new(None);
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Referrals: pallet_referrals,
		Tokens: orml_tokens,
		MockAmm: mock_amm,
	}
);

parameter_types! {
	pub const RefarralPalletId: PalletId = PalletId(*b"test_ref");
	pub const CodeLength: u32 = 10;
	pub const MinCodeLength: u32 = 4;
	pub const RegistrationFee: (AssetId,Balance, AccountId) = (HDX, 222 * 1_000_000_000_000, TREASURY) ;
	pub const RewardAsset: AssetId = HDX;
}

pub struct LevelVolumeAndRewards;

impl GetByKey<Level, (Balance, FeeDistribution)> for LevelVolumeAndRewards {
	fn get(level: &Level) -> (Balance, FeeDistribution) {
		let c = TIER_VOLUME.with(|v| v.borrow().get(level).copied());

		let volume = if let Some(l) = c {
			l.unwrap()
		} else {
			// if not explicitly set, we dont care about this in the test
			0
		};
		let rewards = TIER_REWARDS
			.with(|v| v.borrow().get(level).copied())
			.unwrap_or_default();

		(volume, rewards)
	}
}

pub struct SeedAmount;

impl Get<Balance> for SeedAmount {
	fn get() -> Balance {
		SEED_AMOUNT.with(|v| *v.borrow())
	}
}

pub struct ExtAccount;

impl Get<Option<AccountId>> for ExtAccount {
	fn get() -> Option<AccountId> {
		EXTERNAL_ACCOUNT.with(|v| *v.borrow())
	}
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type AssetId = AssetId;
	type Currency = Tokens;
	type Convert = AssetConvert;
	type PriceProvider = ConversionPrice;
	type RewardAsset = RewardAsset;
	type PalletId = RefarralPalletId;
	type RegistrationFee = RegistrationFee;
	type CodeLength = CodeLength;
	type MinCodeLength = MinCodeLength;
	type LevelVolumeAndRewardPercentages = LevelVolumeAndRewards;
	type ExternalAccount = ExtAccount;
	type SeedNativeAmount = SeedAmount;
	type WeightInfo = ();

	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = Benchmarking;
}

impl frame_system::Config for Test {
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

impl mock_amm::pallet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type TradeHooks = AmmTrader;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	referrer_shares: Vec<(AccountId, Balance)>,
	trader_shares: Vec<(AccountId, Balance)>,
	tiers: Vec<(AssetId, Level, FeeDistribution)>,
	assets: Vec<AssetId>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		CONVERSION_RATE.with(|v| {
			v.borrow_mut().clear();
		});
		SEED_AMOUNT.with(|v| {
			let mut c = v.borrow_mut();
			*c = 0u128;
		});
		TIER_VOLUME.with(|v| {
			v.borrow_mut().clear();
		});
		TIER_REWARDS.with(|v| {
			v.borrow_mut().clear();
		});
		EXTERNAL_ACCOUNT.with(|v| {
			let mut c = v.borrow_mut();
			*c = None;
		});

		Self {
			endowed_accounts: vec![(ALICE, HDX, INITIAL_ALICE_BALANCE)],
			referrer_shares: vec![],
			trader_shares: vec![],
			tiers: vec![],
			assets: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts.extend(accounts);
		self
	}

	pub fn with_referrer_shares(mut self, shares: Vec<(AccountId, Balance)>) -> Self {
		self.referrer_shares.extend(shares);
		self
	}

	pub fn with_trader_shares(mut self, shares: Vec<(AccountId, Balance)>) -> Self {
		self.trader_shares.extend(shares);
		self
	}

	pub fn with_assets(mut self, shares: Vec<AssetId>) -> Self {
		self.assets.extend(shares);
		self
	}
	pub fn with_tiers(mut self, shares: Vec<(AssetId, Level, FeeDistribution)>) -> Self {
		self.tiers.extend(shares);
		self
	}
	pub fn with_conversion_price(self, pair: (AssetId, AssetId), price: EmaPrice) -> Self {
		CONVERSION_RATE.with(|v| {
			let mut m = v.borrow_mut();
			m.insert(pair, price);
			m.insert((pair.1, pair.0), price.inverted());
		});
		self
	}
	pub fn with_seed_amount(self, amount: Balance) -> Self {
		SEED_AMOUNT.with(|v| {
			let mut m = v.borrow_mut();
			*m = amount;
		});
		self
	}

	pub fn with_tier_volumes(self, volumes: HashMap<Level, Option<Balance>>) -> Self {
		TIER_VOLUME.with(|v| {
			v.swap(&RefCell::new(volumes));
		});
		self
	}

	pub fn with_global_tier_rewards(self, rewards: HashMap<Level, FeeDistribution>) -> Self {
		TIER_REWARDS.with(|v| {
			v.swap(&RefCell::new(rewards));
		});
		self
	}

	pub fn with_external_account(self, acc: AccountId) -> Self {
		EXTERNAL_ACCOUNT.with(|v| {
			let mut m = v.borrow_mut();
			*m = Some(acc);
		});
		self
	}

	#[cfg(feature = "runtime-benchmarks")]
	pub fn with_default_volumes(self) -> Self {
		let mut volumes = HashMap::new();
		volumes.insert(Level::Tier0, Some(0));
		volumes.insert(Level::Tier1, Some(10_000_000_000_000));
		volumes.insert(Level::Tier2, Some(11_000_000_000_000));
		volumes.insert(Level::Tier3, Some(12_000_000_000_000));
		volumes.insert(Level::Tier4, Some(13_000_000_000_000));
		TIER_VOLUME.with(|v| {
			v.swap(&RefCell::new(volumes));
		});
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

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
			for (acc, amount) in self.referrer_shares.iter() {
				ReferrerShares::<Test>::insert(acc, amount);
				TotalShares::<Test>::mutate(|v| {
					*v = v.saturating_add(*amount);
				});
			}
			for (acc, amount) in self.trader_shares.iter() {
				TraderShares::<Test>::insert(acc, amount);
				TotalShares::<Test>::mutate(|v| {
					*v = v.saturating_add(*amount);
				});
			}
		});

		r.execute_with(|| {
			for (asset, level, tier) in self.tiers.iter() {
				AssetRewards::<Test>::insert(asset, level, tier);
			}
		});
		r.execute_with(|| {
			for asset in self.assets.iter() {
				PendingConversions::<Test>::insert(asset, ());
			}
		});
		r.execute_with(|| {
			let seed_amount = SEED_AMOUNT.with(|v| *v.borrow());
			Tokens::update_balance(HDX, &Referrals::pot_account_id(), seed_amount as i128).unwrap();
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
			.ok_or(Error::<Test>::ConversionMinTradingAmountNotReached)?;
		let result = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Down).unwrap();
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

pub struct AmmTrader;

const TRADE_PERCENTAGE: Permill = Permill::from_percent(1);

impl Hooks<AccountId, AssetId> for AmmTrader {
	fn simulate_trade(
		_who: &AccountId,
		asset_in: AssetId,
		asset_out: AssetId,
		amount: Balance,
	) -> Result<TradeResult<AssetId>, DispatchError> {
		let price = CONVERSION_RATE
			.with(|v| v.borrow().get(&(asset_out, asset_in)).copied())
			.expect("to have a price");
		let amount_out = multiply_by_rational_with_rounding(amount, price.n, price.d, Rounding::Down).unwrap();
		let fee_amount = TRADE_PERCENTAGE.mul_floor(amount_out);
		Ok(TradeResult {
			amount_in: amount,
			amount_out,
			fee: fee_amount,
			fee_asset: asset_out,
		})
	}

	fn on_trade_fee(
		source: &AccountId,
		trader: &AccountId,
		fee_asset: AssetId,
		fee: Balance,
	) -> Result<(), DispatchError> {
		Referrals::process_trade_fee(*source, *trader, fee_asset, fee)?;
		Ok(())
	}
}

pub struct ConversionPrice;

impl PriceProvider<AssetId> for ConversionPrice {
	type Price = EmaPrice;

	fn get_price(asset_a: AssetId, asset_b: AssetId) -> Option<Self::Price> {
		if asset_a == asset_b {
			return Some(EmaPrice::one());
		}
		CONVERSION_RATE.with(|v| v.borrow().get(&(asset_a, asset_b)).copied())
	}
}

#[cfg(feature = "runtime-benchmarks")]
use crate::traits::BenchmarkHelper;

#[cfg(feature = "runtime-benchmarks")]
pub struct Benchmarking;

#[cfg(feature = "runtime-benchmarks")]
impl BenchmarkHelper<AssetId, Balance> for Benchmarking {
	fn prepare_convertible_asset_and_amount() -> (AssetId, Balance) {
		let price = EmaPrice::new(1_000_000_000_000, 1_000_000_000_000);
		CONVERSION_RATE.with(|v| {
			let mut m = v.borrow_mut();
			m.insert((1234, HDX), price);
			m.insert((HDX, 1234), price.inverted());
		});

		(1234, 1_000_000_000_000)
	}
}
