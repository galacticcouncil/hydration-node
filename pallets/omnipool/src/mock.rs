// This file is part of HydraDX.

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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

//! Test environment for Assets pallet.

use super::*;
use codec::Decode;

use crate as pallet_omnipool;

use frame_support::pallet_prelude::EnsureOrigin;
use frame_support::traits::{ConstU128, Everything, GenesisBuild};
use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use frame_system::RawOrigin;
use orml_traits::parameter_type_with_key;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

pub type Balance = u128;
pub type AssetId = u32;

pub const HDX: AssetId = 0;
pub const LRNA: AssetId = 1;
pub const DAI: AssetId = 2;

pub const LP1: u64 = 1;
pub const LP2: u64 = 2;
pub const LP3: u64 = 3;

construct_runtime!(
	pub enum Test where
		Block = Block,
		NodeBlock = Block,
		UncheckedExtrinsic = UncheckedExtrinsic,
	{
		System: frame_system::{Pallet, Call, Config, Storage, Event<T>},
		Balances: pallet_balances::{Pallet, Call, Storage, Config<T>, Event<T>},
		Omnipool: pallet_omnipool::{Pallet, Call, Storage, Event<T>},
		Tokens: orml_tokens::{Pallet, Event<T>},
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
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

impl pallet_balances::Config for Test {
	type Balance = Balance;
	type DustRemoval = ();
	type Event = Event;
	type ExistentialDeposit = ConstU128<1>;
	type AccountStore = System;
	type WeightInfo = ();
	type MaxLocks = ();
	type MaxReserves = ConstU32<50>;
	type ReserveIdentifier = [u8; 8];
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: AssetId| -> Balance {
		0
	};
}

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = i128;
	type CurrencyId = AssetId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ();
	type DustRemovalWhitelist = Everything;
}

parameter_types! {
	pub const HDXAssetId: AssetId = HDX;
	pub const LRNAAssetId: AssetId = LRNA;
	pub const DAIAssetId: AssetId = DAI;
	pub const PosiitionClassId: u32= 1000;

	pub const ProtocolFee: (u32,u32) = ( 0, 0);
	pub const AssetFee: (u32,u32) = ( 0, 0);
}

pub struct EnsureSignedOrRoot<AccountId>(sp_std::marker::PhantomData<AccountId>);

impl<O: Into<Result<RawOrigin<AccountId>, O>> + From<RawOrigin<AccountId>>, AccountId: Decode> EnsureOrigin<O>
	for EnsureSignedOrRoot<AccountId>
{
	type Success = Option<AccountId>;

	fn try_origin(o: O) -> Result<Self::Success, O> {
		o.into().and_then(|o| match o {
			RawOrigin::Signed(who) => Ok(Some(who)),
			RawOrigin::Root => Ok(None),
			r => Err(O::from(r)),
		})
	}

	#[cfg(feature = "runtime-benchmarks")]
	fn successful_origin() -> O {
		let zero_account_id = AccountId::decode(&mut sp_runtime::traits::TrailingZeroInput::zeroes())
			.expect("infinite length input; no invalid inputs for type; qed");
		O::from(RawOrigin::Signed(zero_account_id))
	}
}

impl Config for Test {
	type Event = Event;
	type Balance = Balance;
	type AssetId = AssetId;
	type PositionInstanceId = u32;
	type Currency = Tokens;
	type AddTokenOrigin = EnsureSignedOrRoot<Self::AccountId>;
	type HubAssetId = LRNAAssetId;
	type ProtocolFee = ProtocolFee;
	type AssetFee = AssetFee;
	type StableCoinAssetId = DAIAssetId;
	type WeightInfo = ();
	type NativeAssetId = HDXAssetId;
	type NFTClassId = PosiitionClassId;
	type NFTHandler = DummyNFT;
}

pub struct ExtBuilder {
	endowed_accounts: Vec<(u64, AssetId, Balance)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		Self {
			endowed_accounts: vec![],
		}
	}
}

impl ExtBuilder {
	pub fn with_endowed_accounts(mut self, accounts: Vec<(u64, AssetId, Balance)>) -> Self {
		self.endowed_accounts = accounts;
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

		t.into()
	}
}

use frame_support::traits::tokens::nonfungibles::{Create, Inspect, Mutate};
pub struct DummyNFT;

impl<AccountId: From<u64>> Inspect<AccountId> for DummyNFT {
	type InstanceId = u32;
	type ClassId = u32;

	fn owner(_class: &Self::ClassId, instance: &Self::InstanceId) -> Option<AccountId> {
		match instance {
			0..=5 => Some(AccountId::from(LP1)),
			6..=10 => Some(AccountId::from(LP2)),
			_ => None,
		}
	}
}

impl<AccountId: From<u64>> Create<AccountId> for DummyNFT {
	fn create_class(_class: &Self::ClassId, _who: &AccountId, _admin: &AccountId) -> DispatchResult {
		Ok(())
	}
}

impl<AccountId: From<u64>> Mutate<AccountId> for DummyNFT {
	fn mint_into(_class: &Self::ClassId, _instance: &Self::InstanceId, _who: &AccountId) -> DispatchResult {
		Ok(())
	}

	fn burn_from(_class: &Self::ClassId, _instance: &Self::InstanceId) -> DispatchResult {
		Ok(())
	}
}
