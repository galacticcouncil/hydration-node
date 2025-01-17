// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
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

use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU128, ConstU32, ConstU64, Contains},
	weights::{RuntimeDbWeight, Weight},
};
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage,
};

type BlockNumber = u64;
type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;
pub type MockPalletCall = mock_pallet::Call<Test>;

use crate as pallet_lazy_executor;

const UNIT: Balance = 1_000_000_000_000;
pub const ALICE: AccountId = 1_000;
pub const BOB: AccountId = 1_001;
pub const CHARLIE: AccountId = 1_002;
pub const MOCK_PALLET_VALID_ORIGIN: AccountId = 1_003;

pub const MAX_ALLOWED_WEIGHT: Weight = Weight::from_parts(5_000, 20_000);

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		LazyExecutor: pallet_lazy_executor,
		MockPallet: mock_pallet,
	}
);

pub mod mock_pallet {
	pub use pallet::*;
	#[frame_support::pallet(dev_mode)]
	pub mod pallet {
		use crate::tests::mock::{AccountId, MOCK_PALLET_VALID_ORIGIN};
		use crate::{ensure_signed, OriginFor};
		use frame_support::{ensure, pallet_prelude::*};

		#[pallet::pallet]
		pub struct Pallet<T>(_);

		#[pallet::config]
		pub trait Config: frame_system::Config<AccountId = AccountId> + Sized {
			type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
		}

		#[pallet::event]
		#[pallet::generate_deposit(pub(super) fn deposit_event)]
		pub enum Event<T: Config> {
			CallExecuted { who: T::AccountId, weight: Weight },
		}

		#[pallet::error]
		pub enum Error<T> {
			// Account is not allowed to perform action
			Forbidden,
		}

		#[pallet::call]
		impl<T: Config> Pallet<T> {
			#[pallet::call_index(1)]
			#[pallet::weight(*weight)]
			pub fn dummy_call(origin: OriginFor<T>, weight: Weight) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(who == MOCK_PALLET_VALID_ORIGIN, Error::<T>::Forbidden);

				Self::deposit_event(Event::CallExecuted { who, weight });

				Ok(())
			}

			pub fn filtered_call(origin: OriginFor<T>, weight: Weight) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(who == MOCK_PALLET_VALID_ORIGIN, Error::<T>::Forbidden);

				Self::deposit_event(Event::CallExecuted { who, weight });

				Ok(())
			}
		}
	}
}

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

pub struct MockBaseFilter;
impl Contains<RuntimeCall> for MockBaseFilter {
	fn contains(call: &RuntimeCall) -> bool {
		!matches!(call, RuntimeCall::MockPallet(MockPalletCall::filtered_call { .. }))
	}
}

impl frame_system::Config for Test {
	type BaseCallFilter = MockBaseFilter;
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
}

impl mock_pallet::Config for Test {
	type RuntimeEvent = RuntimeEvent;
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
	type RuntimeHoldReason = ();
	type RuntimeFreezeReason = ();
}

impl pallet_lazy_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type BlockNumberProvider = MockBlockNumberProvider;
}

pub struct ExtBuilder;
impl Default for ExtBuilder {
	fn default() -> Self {
		ExtBuilder
	}
}

impl ExtBuilder {
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: vec![(ALICE, 200_000 * UNIT), (BOB, 150_000 * UNIT), (CHARLIE, 15_000 * UNIT)],
		}
		.assimilate_storage(&mut t)
		.unwrap();

		//TODO: set maxAllowedWeight

		let mut ext = sp_io::TestExternalities::new(t);
		ext.execute_with(|| {
			pallet_lazy_executor::MaxAllowedWeight::<Test>::put(MAX_ALLOWED_WEIGHT);

			System::set_block_number(1)
		});
		ext
	}
}
