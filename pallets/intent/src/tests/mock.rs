// Copyright (C) 2020-2026  Intergalactic, Limited (GIB).
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

use crate as pallet_intent;
use crate::types;
use crate::types::Intent;
use crate::Config;
use frame_support::parameter_types;
use frame_support::storage::with_transaction;
use frame_support::traits::Everything;
use hydradx_traits::lazy_executor::Source;
use hydradx_traits::registry::Inspect;
use ice_support::AssetId;
use ice_support::Balance;
use orml_traits::parameter_type_with_key;
use primitives::constants::time::SLOT_DURATION;
use sp_core::ConstU32;
use sp_core::ConstU64;
use sp_core::H256;
use sp_runtime::traits::BlakeTwo256;
use sp_runtime::traits::IdentityLookup;
use sp_runtime::BuildStorage;
use sp_runtime::DispatchError;
use sp_runtime::DispatchResult;
use sp_runtime::TransactionOutcome;
use std::cell::RefCell;
use std::vec;

pub(crate) const ONE_DOT: u128 = 10_000_000_000;
pub(crate) const ONE_HDX: u128 = 1_000_000_000_000;
pub(crate) const ONE_QUINTIL: u128 = 1_000_000_000_000_000_000;

pub(crate) const HDX: AssetId = 0;
pub(crate) const HUB_ASSET_ID: AssetId = 1;
pub(crate) const DOT: AssetId = 2;
pub(crate) const ETH: AssetId = 3;
pub(crate) const BTC: AssetId = 4;

pub(crate) const ALICE: AccountId = 2;
pub(crate) const BOB: AccountId = 3;
pub(crate) const CHARLIE: AccountId = 4;

//5 SEC.
pub(crate) const MAX_INTENT_DEADLINE: pallet_intent::types::Moment = 5 * ONE_SECOND;
pub(crate) const ONE_SECOND: pallet_intent::types::Moment = 1_000;

type AccountId = u64;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Currencies: orml_tokens,
		Timestamp: pallet_timestamp,
		IntentPallet: pallet_intent,
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
	pub const SS58Prefix: u8 = 63;
}

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
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = ();
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
	type ExtensionsWeightInfo = ();
}

pub(crate) type Extrinsic = sp_runtime::testing::TestXt<RuntimeCall, ()>;
impl<LocalCall> frame_system::offchain::CreateTransactionBase<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	type RuntimeCall = RuntimeCall;
	type Extrinsic = Extrinsic;
}

impl<LocalCall> hydradx_traits::CreateBare<LocalCall> for Test
where
	RuntimeCall: From<LocalCall>,
{
	fn create_bare(call: Self::RuntimeCall) -> Extrinsic {
		Extrinsic::new_bare(call)
	}
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
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	pub const MinimumPeriod: u64 = SLOT_DURATION / 2;
}

impl pallet_timestamp::Config for Test {
	type Moment = u64;
	type MinimumPeriod = MinimumPeriod;
	type OnTimestampSet = ();
	type WeightInfo = ();
}

thread_local! {
	pub static QUEUD_TASKS: RefCell<Vec<(Source, AccountId)>> = RefCell::new(Vec::default());
}

pub struct DummyLazyExecutor<T>(sp_std::marker::PhantomData<T>);
impl<T: Config> hydradx_traits::lazy_executor::Mutate<AccountId> for DummyLazyExecutor<T> {
	type Error = DispatchError;
	type BoundedCall = types::CallData;

	fn queue(src: Source, origin: AccountId, _call: Self::BoundedCall) -> Result<(), Self::Error> {
		QUEUD_TASKS.with(|v| {
			if get_queued_task(src.clone()).is_some() {
				return Err(DispatchError::Other("Duplicate intent"));
			}

			v.borrow_mut().push((src, origin));

			Ok(())
		})
	}
}

pub fn get_queued_task(src: Source) -> Option<(Source, AccountId)> {
	QUEUD_TASKS.with(|v| {
		let m = v.borrow();

		if let Some((_, (_, acc))) = m.clone().into_iter().enumerate().find(|x| x.1 .0 == src) {
			Some((src, acc))
		} else {
			None
		}
	})
}

pub struct DummyRegistry;

impl Inspect for DummyRegistry {
	type AssetId = AssetId;
	type Location = u8;

	fn exists(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn decimals(_id: Self::AssetId) -> Option<u8> {
		todo!()
	}

	fn is_banned(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn asset_type(_id: Self::AssetId) -> Option<hydradx_traits::AssetKind> {
		todo!()
	}

	fn asset_name(_id: Self::AssetId) -> Option<Vec<u8>> {
		todo!()
	}

	fn asset_symbol(_id: Self::AssetId) -> Option<Vec<u8>> {
		todo!()
	}

	fn is_sufficient(_id: Self::AssetId) -> bool {
		todo!()
	}

	fn existential_deposit(_id: Self::AssetId) -> Option<u128> {
		Some(1_000)
	}
}

impl pallet_intent::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Currency = Currencies;
	type LazyExecutorHandler = DummyLazyExecutor<Test>;
	type RegistryHandler = DummyRegistry;
	type TimestampProvider = Timestamp;
	type HubAssetId = ConstU32<HUB_ASSET_ID>;
	type MaxAllowedIntentDuration = ConstU64<MAX_INTENT_DEADLINE>;
	type WeightInfo = ();
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(AccountId, AssetId, Balance)>,
	intents: Vec<(AccountId, Intent)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		QUEUD_TASKS.with(|v| {
			v.borrow_mut().clear();
		});

		Self {
			endowed_accounts: vec![],
			intents: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(AccountId, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
		self
	}

	pub fn with_intents(mut self, intents: Vec<(AccountId, Intent)>) -> Self {
		self.intents = intents;
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
			frame_system::Pallet::<Test>::set_block_number(1);

			let _ = with_transaction(|| {
				for (owner, intent) in self.intents {
					pallet_intent::Pallet::<Test>::add_intent(owner, intent).expect("add_intent should work");
				}

				TransactionOutcome::Commit(DispatchResult::Ok(()))
			});
		});

		r
	}
}
