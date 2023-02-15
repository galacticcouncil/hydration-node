// This file is part of HydraDX.

// Copyright (C) 2020-2021  Intergalactic, Limited (GIB).
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

use super::*;
use crate as xcm_rate_limit;
use crate::{Config, EthereumAddress};
use frame_support::parameter_types;
use hex_literal::hex;
use orml_traits::arithmetic::One;
use primitives::Balance;
use sp_core::H256;
use sp_runtime::{
	testing::Header,
	traits::{BlakeTwo256, IdentityLookup},
};

use sp_runtime::AccountId32 as AccountId;

use frame_support::traits::{Everything, GenesisBuild, Nothing};
use orml_traits::parameter_type_with_key;
use orml_xcm_support::IsNativeConcrete;
use orml_xcm_support::MultiCurrencyAdapter;
use pallet_currencies::BasicCurrencyAdapter;
use primitives::constants::chain::CORE_ASSET_ID;

pub const ALICE: [u8; 32] = [4u8; 32];
pub const BOB: [u8; 32] = [5u8; 32];

type UncheckedExtrinsic = frame_system::mocking::MockUncheckedExtrinsic<Test>;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test where
	 Block = Block,
	 NodeBlock = Block,
	 UncheckedExtrinsic = UncheckedExtrinsic,
	 {
		 System: frame_system,
		 XcmRateLimit: xcm_rate_limit,
		 Balances: pallet_balances,
		 Currencies: pallet_currencies,
		 Tokens: orml_tokens,
	 }
);

parameter_types! {
	pub const BlockHashCount: u64 = 250;
}

impl frame_system::Config for Test {
	type BaseCallFilter = Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type Origin = Origin;
	type Call = Call;
	type Index = u64;
	type BlockNumber = u64;
	type Hash = H256;
	type Hashing = BlakeTwo256;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type Header = Header;
	type Event = Event;
	type BlockHashCount = BlockHashCount;
	type DbWeight = ();
	type Version = ();
	type PalletInfo = PalletInfo;
	type AccountData = pallet_balances::AccountData<Balance>;
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = frame_support::traits::ConstU32<16>;
}

impl pallet_balances::Config for Test {
	type MaxLocks = ();
	type Balance = Balance;
	type Event = Event;
	type DustRemoval = ();
	type ExistentialDeposit = ();
	type AccountStore = frame_system::Pallet<Test>;
	type WeightInfo = ();
	type MaxReserves = ();
	type ReserveIdentifier = ();
}

parameter_types! {
	pub Prefix: &'static [u8] = b"I hereby claim all my xHDX tokens to wallet:";
	pub NativeCurrencyId: u32 = CORE_ASSET_ID;

}

pub struct CurrencyIdConverterMock;

impl Convert<MultiAsset, CurrencyId> for CurrencyIdConverterMock {
	fn convert(value: MultiAsset) -> Result<CurrencyId, MultiAsset> {
		let res = match value.id {
			Concrete(MultiLocation {
				interior: X1(Parachain(id)),
				..
			}) => id,
			_ => 0,
		};

		Ok(res)
	}
}

impl sp_runtime::traits::Convert<polkadot_xcm::v1::MultiAsset, Option<u32>> for CurrencyIdConverterMock {
	fn convert(value: MultiAsset) -> Option<CurrencyId> {
		let res = match value.id {
			Concrete(MultiLocation {
				interior: X1(Parachain(id)),
				..
			}) => id,
			_ => 0,
		};

		Some(res)
	}
}

impl sp_runtime::traits::Convert<polkadot_xcm::v1::MultiLocation, Option<u32>> for CurrencyIdConverterMock {
	fn convert(value: MultiLocation) -> Option<CurrencyId> {
		let res = match value {
			MultiLocation {
				interior: X1(Parachain(id)),
				..
			} => id,
			_ => 0,
		};

		Some(res)
	}
}

pub struct LocationToAccountIdConverterMock;

impl Convert<MultiLocation, AccountId> for LocationToAccountIdConverterMock {
	fn convert(value: MultiLocation) -> Result<AccountId, MultiLocation> {
		let res = match value {
			MultiLocation {
				interior: X1(AccountId32 { id, .. }),
				..
			} => AccountId::from(id),
			_ => {
				unimplemented!()
			}
		};

		Ok(res)
	}
}

impl Config for Test {
	type Event = Event;
	type Currency = Currencies;
	type Prefix = Prefix;
	type WeightInfo = ();
	type AssetTransactor = MultiCurrencyAdapter<
		Currencies,
		(), // UnknownTokens
		IsNativeConcrete<CurrencyId, CurrencyIdConverterMock>,
		AccountId,
		LocationToAccountIdConverterMock,
		CurrencyId,
		CurrencyIdConverterMock,
		(), // DepositToAlternative<Alternative, Currencies, CurrencyId, AccountId, Balance>,
	>;
	type LocationToAccountIdConverter = LocationToAccountIdConverterMock;
	type CurrencyIdConverter = CurrencyIdConverterMock;
}
pub type Amount = i128;

impl pallet_currencies::Config for Test {
	type Event = Event;
	type MultiCurrency = Tokens;
	type NativeCurrency = BasicCurrencyAdapter<Test, Balances, Amount, u32>;
	type GetNativeCurrencyId = NativeCurrencyId;
	type WeightInfo = ();
}

parameter_type_with_key! {
	pub ExistentialDeposits: |_currency_id: u32| -> Balance {
		One::one()
	};
}

parameter_types! {
	pub const MaxReserves: u32 = 50;

}

pub type CurrencyId = u32;

impl orml_tokens::Config for Test {
	type Event = Event;
	type Balance = Balance;
	type Amount = Amount;
	type CurrencyId = CurrencyId;
	type WeightInfo = ();
	type ExistentialDeposits = ExistentialDeposits;
	type OnDust = ();
	type MaxLocks = ConstU32<5>;
	type DustRemovalWhitelist = Nothing;
	type OnNewTokenAccount = ();
	type OnKilledTokenAccount = ();
	type ReserveIdentifier = ();
	type MaxReserves = MaxReserves;
}

pub const CLAIM_AMOUNT: Balance = 1_000_000_000_000;

#[derive(Default)]
pub struct ExtBuilder;

impl ExtBuilder {
	// builds genesis config
	pub fn build(self) -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default().build_storage::<Test>().unwrap();

		t.into()
	}
}
