// Copyright (C) 2020-2023  Intergalactic, Limited (GIB).
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

use crate::traits::{Freeze, VestingDetails};
use crate::types::{Vote, Voting};
use crate::*;

use frame_support::traits::Everything;
use frame_support::{assert_ok, PalletId};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{AsEnsureOriginWithArg, ConstU128, ConstU32, ConstU64, NeverEnsureOrigin},
	weights::RuntimeDbWeight,
};
use frame_system::{EnsureRoot, RawOrigin};
use orml_traits::{parameter_type_with_key, LockIdentifier, MultiCurrencyExtended};
use pallet_democracy::ReferendumIndex;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage,
};

use crate as pallet_staking;

type Block = frame_system::mocking::MockBlock<Test>;

type AccountId = u64;
type AssetId = u32;
type BlockNumber = u64;

pub const HDX: AssetId = 0;

pub const ALICE: AccountId = 1_000;
pub const BOB: AccountId = 1_001;
pub const CHARLIE: AccountId = 1_002;
pub const DAVE: AccountId = 1_003;
pub const VESTED_100K: AccountId = 1_004;

pub const ONE: u128 = 1_000_000_000_000;

pub const STAKING_LOCK: LockIdentifier = crate::STAKING_LOCK_ID;

pub const NON_DUSTABLE_BALANCE: Balance = 1_000 * ONE;

pub type PositionId = u128;

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		Uniques: pallet_uniques,
		Tokens: orml_tokens,
		Staking: pallet_staking,
	}
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
	pub static MockBlockNumberProvider: u64 = 0;
	pub const DbWeight: RuntimeDbWeight = RuntimeDbWeight{
		read: 1_u64, write: 1_u64
	};
}

impl BlockNumberProvider for MockBlockNumberProvider {
	type BlockNumber = BlockNumber;

	fn current_block_number() -> Self::BlockNumber {
		System::block_number()
	}
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
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub const MaxLocks: u32 = 20;
}
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type RuntimeEvent = RuntimeEvent;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = MaxLocks;
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type FreezeIdentifier = ();
	type MaxFreezes = ();
	type MaxHolds = ();
	type RuntimeHoldReason = ();
}

parameter_types! {
	pub const CollectionDeposit: Balance = 0;
	pub const ItemDeposit: Balance = 0;
	pub const KeyLimit: u32 = 256;
	pub const ValueLimit: u32 = 1024;
	pub const UniquesMetadataDepositBase: Balance = 1_000 * ONE;
	pub const AttributeDepositBase: Balance = ONE;
	pub const DepositPerByte: Balance = ONE;
	pub const UniquesStringLimit: u32 = 72;
}

impl pallet_uniques::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type CollectionId = u128;
	type ItemId = u128;
	type Currency = Balances;
	type ForceOrigin = EnsureRoot<AccountId>;
	// Standard collection creation is disallowed
	type CreateOrigin = AsEnsureOriginWithArg<NeverEnsureOrigin<AccountId>>;
	type Locker = ();
	type CollectionDeposit = CollectionDeposit;
	type ItemDeposit = ItemDeposit;
	type MetadataDepositBase = UniquesMetadataDepositBase;
	type AttributeDepositBase = AttributeDepositBase;
	type DepositPerByte = DepositPerByte;
	type StringLimit = UniquesStringLimit;
	type KeyLimit = KeyLimit;
	type ValueLimit = ValueLimit;
	type WeightInfo = ();

	#[cfg(feature = "runtime-benchmarks")]
	type Helper = ();
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
	type MaxLocks = ConstU32<10>;
	type DustRemovalWhitelist = Everything;
	type MaxReserves = ();
	type ReserveIdentifier = ();
	type CurrencyHooks = ();
}

parameter_types! {
	pub const StakingPalletId: PalletId = PalletId(*b"test_stk");
	pub const MinStake: Balance = 10 * ONE;
	pub const PeriodLength: BlockNumber = 10_000;
	pub const TimePointsW:Permill =  Permill::from_percent(80);
	pub const ActionPointsW: Perbill = Perbill::from_percent(20);
	pub const TimePointsPerPeriod: u8 = 2;
	pub const CurrentStakeWeight: u8 = 2;
	pub const UnclaimablePeriods: BlockNumber = 10;
	pub const PointPercentage: FixedU128 = FixedU128::from_rational(15,100);
	pub const MaxVotes: u32 = 10;
}

impl pallet_staking::Config for Test {
	type WeightInfo = ();
	type RuntimeEvent = RuntimeEvent;
	type AssetId = AssetId;
	type Currency = Tokens;
	type PeriodLength = PeriodLength;
	type PalletId = StakingPalletId;
	type NativeAssetId = ConstU32<HDX>;
	type MinStake = MinStake;
	type TimePointsWeight = TimePointsW;
	type ActionPointsWeight = ActionPointsW;
	type TimePointsPerPeriod = TimePointsPerPeriod;
	type UnclaimablePeriods = UnclaimablePeriods;
	type CurrentStakeWeight = CurrentStakeWeight;
	type BlockNumberProvider = MockBlockNumberProvider;
	type PositionItemId = PositionId;
	type CollectionId = u128;
	type NFTCollectionId = ConstU128<1>;
	type NFTHandler = Uniques;

	type PayablePercentage = SigmoidPercentage<PointPercentage, ConstU32<40_000>>;
	type MaxVotes = MaxVotes;
	type MaxPointsPerAction = DummyMaxPointsPerAction;
	type ReferendumInfo = DummyReferendumStatus;
	type Vesting = DummyVesting;
	type Collections = FreezableUniques;
	type AuthorityOrigin = EnsureRoot<AccountId>;

	#[cfg(feature = "runtime-benchmarks")]
	type MaxLocks = MaxLocks;
}

pub struct DummyMaxPointsPerAction;

impl GetByKey<Action, u32> for DummyMaxPointsPerAction {
	fn get(k: &Action) -> u32 {
		match k {
			Action::DemocracyVote => 100_u32,
		}
	}
}

pub struct DummyReferendumStatus;

impl DemocracyReferendum for DummyReferendumStatus {
	fn is_referendum_finished(index: pallet_democracy::ReferendumIndex) -> bool {
		index % 2 == 0
	}
}

pub struct DummyVesting;

impl VestingDetails<AccountId, Balance> for DummyVesting {
	fn locked(who: AccountId) -> Balance {
		if who == VESTED_100K {
			return 100_000 * ONE;
		}

		Zero::zero()
	}
}

pub struct FreezableUniques;

impl Freeze<AccountId, u128> for FreezableUniques {
	fn freeze_collection(owner: AccountId, collection: u128) -> DispatchResult {
		Uniques::freeze_collection(RuntimeOrigin::signed(owner), collection)
	}
}

pub fn set_block_number(n: u64) {
	System::set_block_number(n);
}

#[derive(Default)]
pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
	initial_block_number: BlockNumber,
	//(who, staked maount, created_at, pendig_rewards)
	stakes: Vec<(AccountId, Balance, BlockNumber, Balance)>,
	init_staking: bool,
	with_votings: Vec<(PositionId, Vec<(ReferendumIndex, Vote)>)>,
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_stakes(mut self, stakes: Vec<(AccountId, Balance, BlockNumber, Balance)>) -> Self {
		self.stakes = stakes;
		self
	}

	/// BlockNumber set before any action e.g. with_stakes()
	pub fn start_at_block(mut self, n: BlockNumber) -> Self {
		self.initial_block_number = n;
		self
	}

	pub fn with_initialized_staking(mut self) -> Self {
		self.init_staking = true;
		self
	}

	pub fn with_votings(mut self, votings: Vec<(u128, Vec<(ReferendumIndex, Vote)>)>) -> Self {
		self.with_votings = votings;
		self
	}
}

impl ExtBuilder {
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
			if self.initial_block_number.is_zero() {
				set_block_number(1);
			} else {
				set_block_number(self.initial_block_number);
			}

			if self.init_staking {
				let pot = Staking::pot_account_id();
				assert_ok!(Tokens::set_balance(
					RawOrigin::Root.into(),
					pot,
					HDX,
					NON_DUSTABLE_BALANCE,
					0
				));
				assert_ok!(Staking::initialize_staking(RawOrigin::Root.into()));
			}

			for (who, staked_amount, at, pending_rewards) in self.stakes {
				if !pending_rewards.is_zero() {
					set_pending_rewards(pending_rewards);
				}

				set_block_number(at);
				if let Some(position_id) = Staking::get_user_position_id(&who).unwrap() {
					assert_ok!(Staking::increase_stake(
						RuntimeOrigin::signed(who),
						position_id,
						staked_amount
					));
				} else {
					assert_ok!(Staking::stake(RuntimeOrigin::signed(who), staked_amount));
				}
			}

			for (position_id, votes) in self.with_votings {
				let v = Voting::<MaxVotes> {
					votes: votes.try_into().unwrap(),
				};

				pallet_staking::PositionVotes::<Test>::insert(position_id, v);
			}
		});

		r
	}
}

pub fn set_pending_rewards(amount: u128) {
	let pot = Staking::pot_account_id();
	assert_ok!(Tokens::update_balance(HDX, &pot, amount as i128));
}
