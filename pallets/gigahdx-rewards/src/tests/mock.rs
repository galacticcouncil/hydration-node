// SPDX-License-Identifier: Apache-2.0

#![cfg(test)]

use crate as pallet_gigahdx_rewards;
use crate::traits::{ReferendaTrackInspect, TrackRewardTable};
use crate::types::ReferendumIndex;

use frame_support::sp_runtime::{
	traits::{AccountIdConversion, BlakeTwo256, IdentityLookup},
	BuildStorage, DispatchError, Permill,
};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64, Everything, LockIdentifier},
	PalletId,
};
use frame_system::EnsureRoot;
use hydradx_traits::gigahdx::MoneyMarketOperations;
use orml_traits::parameter_type_with_key;
use primitives::{AssetId, Balance};
use sp_core::H256;
use std::cell::RefCell;
use std::collections::HashMap;

// 16-byte AccountId so `PalletId::into_sub_account_truncating` produces a
// pot distinct from the parent (the first 8 bytes would otherwise collide).
pub type AccountId = u128;
type Block = frame_system::mocking::MockBlock<Test>;

#[allow(dead_code)]
pub const HDX: AssetId = 0;
pub const ST_HDX: AssetId = 670;
pub const ONE: Balance = 1_000_000_000_000;

pub const ALICE: AccountId = 1;
pub const BOB: AccountId = 2;
pub const CHARLIE: AccountId = 3;
#[allow(dead_code)]
pub const TREASURY: AccountId = 99;

pub const GIGAHDX_LOCK_ID: LockIdentifier = *b"ghdxlock";

construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Tokens: orml_tokens,
		GigaHdx: pallet_gigahdx,
		GigaHdxRewards: pallet_gigahdx_rewards,
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
	type AccountData = pallet_balances::AccountData<Balance>;
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
	type ExtensionsWeightInfo = ();
}

parameter_types! {
	pub const ExistentialDeposit: Balance = 1;
	pub const MaxLocks: u32 = 20;
}

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ExistentialDeposit;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
	type DoneSlashHandler = ();
}

parameter_type_with_key! {
	pub StHdxExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = StHdxExistentialDeposits;
	type CurrencyHooks = ();
	type MaxLocks = ConstU32<10>;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type DustRemovalWhitelist = Everything;
}

// ---------- TestMoneyMarket ----------

thread_local! {
	pub static MM_BALANCES: RefCell<HashMap<AccountId, Balance>> = RefCell::new(HashMap::new());
	pub static MM_SUPPLY_ROUND_NUM: RefCell<u128> = const { RefCell::new(1) };
	pub static MM_SUPPLY_ROUND_DEN: RefCell<u128> = const { RefCell::new(1) };
	pub static MM_SUPPLY_FAILS: RefCell<bool> = const { RefCell::new(false) };
	pub static MM_WITHDRAW_FAILS: RefCell<bool> = const { RefCell::new(false) };
}

pub struct TestMoneyMarket;

impl TestMoneyMarket {
	pub fn reset() {
		MM_BALANCES.with(|m| m.borrow_mut().clear());
		MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow_mut() = 1);
		MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow_mut() = 1);
		MM_SUPPLY_FAILS.with(|v| *v.borrow_mut() = false);
		MM_WITHDRAW_FAILS.with(|v| *v.borrow_mut() = false);
	}
	#[allow(dead_code)]
	pub fn set_supply_rounding(num: u128, den: u128) {
		MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow_mut() = num);
		MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow_mut() = den);
	}
	pub fn fail_supply() {
		MM_SUPPLY_FAILS.with(|v| *v.borrow_mut() = true);
	}
	#[allow(dead_code)]
	pub fn fail_withdraw() {
		MM_WITHDRAW_FAILS.with(|v| *v.borrow_mut() = true);
	}
	#[allow(dead_code)]
	pub fn balance_of(who: &AccountId) -> Balance {
		MM_BALANCES.with(|m| *m.borrow().get(who).unwrap_or(&0))
	}
}

impl MoneyMarketOperations<AccountId, AssetId, Balance> for TestMoneyMarket {
	fn supply(who: &AccountId, _asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		if MM_SUPPLY_FAILS.with(|v| *v.borrow()) {
			return Err(DispatchError::Other("MM supply failed"));
		}
		let num = MM_SUPPLY_ROUND_NUM.with(|v| *v.borrow());
		let den = MM_SUPPLY_ROUND_DEN.with(|v| *v.borrow());
		let actual = amount.saturating_mul(num) / den;
		MM_BALANCES.with(|m| *m.borrow_mut().entry(*who).or_default() += actual);
		Ok(actual)
	}

	fn withdraw(who: &AccountId, _asset: AssetId, amount: Balance) -> Result<Balance, DispatchError> {
		if MM_WITHDRAW_FAILS.with(|v| *v.borrow()) {
			return Err(DispatchError::Other("MM withdraw failed"));
		}
		MM_BALANCES.with(|m| {
			let mut map = m.borrow_mut();
			let bal = map.entry(*who).or_default();
			*bal = bal.saturating_sub(amount);
		});
		Ok(amount)
	}

	fn balance_of(who: &AccountId) -> Balance {
		MM_BALANCES.with(|m| *m.borrow().get(who).unwrap_or(&0))
	}
}

// ---------- TestExternalClaims ----------

thread_local! {
	pub static EXTERNAL_CLAIMS: RefCell<Balance> = const { RefCell::new(0) };
}

pub struct TestExternalClaims;

impl TestExternalClaims {
	#[allow(dead_code)]
	pub fn set(value: Balance) {
		EXTERNAL_CLAIMS.with(|v| *v.borrow_mut() = value);
	}

	#[allow(dead_code)]
	pub fn reset() {
		EXTERNAL_CLAIMS.with(|v| *v.borrow_mut() = 0);
	}
}

impl pallet_gigahdx::traits::ExternalClaims<AccountId> for TestExternalClaims {
	fn on(_who: &AccountId) -> Balance {
		EXTERNAL_CLAIMS.with(|v| *v.borrow())
	}
}

// ---------- pallet-gigahdx config ----------

parameter_types! {
	pub const StHdxAssetIdConst: AssetId = ST_HDX;
	pub const GigaHdxPalletId: PalletId = PalletId(*b"gigahdx!");
	pub const GigaHdxLockId: LockIdentifier = GIGAHDX_LOCK_ID;
	pub const GigaHdxMinStake: Balance = ONE; // 1 HDX
	pub const GigaHdxCooldownPeriod: u64 = 100;
	pub const GigaHdxMaxPendingUnstakes: u32 = 10;
}

impl pallet_gigahdx::Config for Test {
	type NativeCurrency = Balances;
	type MultiCurrency = Tokens;
	type StHdxAssetId = StHdxAssetIdConst;
	type MoneyMarket = TestMoneyMarket;
	type AuthorityOrigin = EnsureRoot<AccountId>;
	type PalletId = GigaHdxPalletId;
	type LockId = GigaHdxLockId;
	type MinStake = GigaHdxMinStake;
	type CooldownPeriod = GigaHdxCooldownPeriod;
	type MaxPendingUnstakes = GigaHdxMaxPendingUnstakes;
	type ExternalClaims = TestExternalClaims;
	type LegacyStaking = ();
	type WeightInfo = ();
	#[cfg(feature = "runtime-benchmarks")]
	type BenchmarkHelper = ();
}

// ---------- pallet-gigahdx-rewards config ----------

parameter_types! {
	// AccountId in this mock is u128 (16 bytes) so sub-account derivation
	// produces a pot distinct from the accumulator. Use a prefix that does
	// not collide with gigahdx (`b"giga..."`).
	pub const RewardPotPalletId: PalletId = PalletId(*b"rwd!ghdx");
}

pub struct TestReferendaTrackInspect;
impl ReferendaTrackInspect<ReferendumIndex, u16> for TestReferendaTrackInspect {
	fn track_of(_ref_index: ReferendumIndex) -> Option<u16> {
		Some(0u16)
	}
}

pub struct TestTrackRewardConfig;
impl TrackRewardTable<u16> for TestTrackRewardConfig {
	fn reward_percentage(_track_id: u16) -> Permill {
		Permill::from_percent(10)
	}
}

impl pallet_gigahdx_rewards::Config for Test {
	type TrackId = u16;
	type Referenda = TestReferendaTrackInspect;
	type TrackRewardConfig = TestTrackRewardConfig;
	type RewardPotPalletId = RewardPotPalletId;
	type WeightInfo = ();
}

// ---------- helpers ----------

pub fn accumulator_pot() -> AccountId {
	pallet_gigahdx_rewards::Pallet::<Test>::reward_accumulator_pot()
}

pub fn allocated_pot() -> AccountId {
	pallet_gigahdx_rewards::Pallet::<Test>::allocated_rewards_pot()
}

#[allow(dead_code)]
pub fn gigapot() -> AccountId {
	GigaHdxPalletId::get().into_account_truncating()
}

/// Mint HDX into the accumulator pot at runtime.
#[allow(dead_code)]
pub fn fund_accumulator(amount: Balance) {
	use frame_support::traits::Currency;
	let _ = <Balances as Currency<AccountId>>::deposit_creating(&accumulator_pot(), amount);
}

pub fn account_balance(who: &AccountId) -> Balance {
	use frame_support::traits::Currency;
	<Balances as Currency<AccountId>>::free_balance(who)
}

/// Convenience getter for a Stake record; returns a default record if absent.
pub fn stake_record(who: &AccountId) -> pallet_gigahdx::pallet::StakeRecord {
	pallet_gigahdx::Stakes::<Test>::get(who).unwrap_or_default()
}

/// Drain all `frame_system::events()` and return them.
pub fn last_events(n: usize) -> Vec<RuntimeEvent> {
	let evs: Vec<RuntimeEvent> = frame_system::Pallet::<Test>::events()
		.into_iter()
		.map(|e| e.event)
		.collect();
	let len = evs.len();
	evs.into_iter().skip(len.saturating_sub(n)).collect()
}

// ---------- Test ext builder ----------

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, Balance)>,
	pot_balance: Balance,
	pre_fund_accumulator: Option<Balance>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![
				(ALICE, 1_000 * ONE),
				(BOB, 1_000 * ONE),
				(CHARLIE, 1_000 * ONE),
				(TREASURY, 1_000 * ONE),
			],
			pot_balance: 0,
			pre_fund_accumulator: None,
		}
	}
}

impl ExtBuilder {
	#[allow(dead_code)]
	pub fn with_pot_balance(mut self, balance: Balance) -> Self {
		self.pot_balance = balance;
		self
	}

	pub fn with_accumulator(mut self, balance: Balance) -> Self {
		self.pre_fund_accumulator = Some(balance);
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		let mut balances = self.endowed_accounts.clone();
		if self.pot_balance > 0 {
			balances.push((gigapot(), self.pot_balance));
		}
		if let Some(amt) = self.pre_fund_accumulator {
			if amt > 0 {
				balances.push((accumulator_pot(), amt));
			}
		}
		pallet_balances::GenesisConfig::<Test> {
			balances,
			dev_accounts: None,
		}
		.assimilate_storage(&mut t)
		.unwrap();

		let mut ext: sp_io::TestExternalities = t.into();
		ext.execute_with(|| {
			TestMoneyMarket::reset();
			System::set_block_number(1);
		});
		ext
	}
}
