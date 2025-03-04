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
	traits::{fungible, ConstU128, ConstU32, ConstU64, Contains, Imbalance, OnUnbalanced},
	weights::{RuntimeDbWeight, Weight, WeightToFee as WeightToFeeT},
};
use pallet_transaction_payment::FungibleAdapter;
use sp_core::H256;
use sp_runtime::SaturatedConversion;
use sp_runtime::{
	traits::{BlakeTwo256, BlockNumberProvider, IdentityLookup},
	BuildStorage,
};

type BlockNumber = u64;
pub type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;
type Balance = u128;
pub type MockPalletCall = mock_pallet::Call<Test>;
pub type LazyExecutorCall = pallet::Call<Test>;

use crate::{self as pallet_lazy_executor, pallet};

const UNIT: Balance = 1_000_000_000_000;
pub const ALICE: AccountId = 1_000;
pub const BOB: AccountId = 1_001;
pub const CHARLIE: AccountId = 1_002;
pub const ACC_ZERO_BALANCE: AccountId = 1_003;

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		Balances: pallet_balances,
		LazyExecutor: pallet_lazy_executor,
		MockPallet: mock_pallet,
		TransactionPayment: pallet_transaction_payment,
	}
);

pub mod mock_pallet {
	pub use pallet::*;
	#[frame_support::pallet(dev_mode)]
	pub mod pallet {
		use crate::tests::mock::AccountId;
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
			pub fn dummy_call(origin: OriginFor<T>, allowed_origin: Vec<AccountId>, weight: Weight) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(allowed_origin.contains(&who), Error::<T>::Forbidden);

				Self::deposit_event(Event::CallExecuted { who, weight });

				Ok(())
			}

			pub fn filtered_call(
				origin: OriginFor<T>,
				allowed_origin: Vec<AccountId>,
				weight: Weight,
			) -> DispatchResult {
				let who = ensure_signed(origin)?;

				ensure!(allowed_origin.contains(&who), Error::<T>::Forbidden);

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

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<C> frame_system::offchain::SendTransactionTypes<C> for Test
where
	RuntimeCall: From<C>,
{
	type OverarchingCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl pallet_lazy_executor::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type UnsignedPriority = ConstU64<100>;
	type UnsignedLongevity = ConstU64<3>;

	type WeightInfo = ();
}

parameter_types! {
	pub static WeightToFee: u128 = 1;
	pub static TransactionByteFee: u128 = 1;
	pub static OperationalFeeMultiplier: u8 = 5;
}

impl WeightToFeeT for WeightToFee {
	type Balance = u128;

	fn weight_to_fee(weight: &Weight) -> Self::Balance {
		Self::Balance::saturated_from(weight.ref_time()).saturating_mul(WEIGHT_TO_FEE.with(|v| *v.borrow()))
	}
}

impl WeightToFeeT for TransactionByteFee {
	type Balance = u128;

	fn weight_to_fee(weight: &Weight) -> Self::Balance {
		Self::Balance::saturated_from(weight.ref_time()).saturating_mul(TRANSACTION_BYTE_FEE.with(|v| *v.borrow()))
	}
}

parameter_types! {
	pub(crate) static TipUnbalancedAmount: u128 = 0;
	pub(crate) static FeeUnbalancedAmount: u128 = 0;
}

pub struct DealWithFees;
impl OnUnbalanced<fungible::Credit<<Test as frame_system::Config>::AccountId, Balances>> for DealWithFees {
	fn on_unbalanceds<B>(
		mut fees_then_tips: impl Iterator<Item = fungible::Credit<<Test as frame_system::Config>::AccountId, Balances>>,
	) {
		if let Some(fees) = fees_then_tips.next() {
			FeeUnbalancedAmount::mutate(|a| *a += fees.peek());
			if let Some(tips) = fees_then_tips.next() {
				TipUnbalancedAmount::mutate(|a| *a += tips.peek());
			}
		}
	}
}

impl pallet_transaction_payment::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type OnChargeTransaction = FungibleAdapter<Balances, DealWithFees>;
	type OperationalFeeMultiplier = OperationalFeeMultiplier;
	type WeightToFee = WeightToFee;
	type LengthToFee = TransactionByteFee;
	type FeeMultiplierUpdate = ();
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

		let mut r: sp_io::TestExternalities = t.into();
		r.execute_with(|| {
			System::set_block_number(1);
		});

		r
	}
}
