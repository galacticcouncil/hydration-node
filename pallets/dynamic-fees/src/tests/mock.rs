// This file is part of warehouse

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

//! Test environment for Assets pallet.

use sp_std::prelude::*;
use std::cell::RefCell;

use crate::tests::oracle::Oracle;
use crate::types::{FeeEntry, FeeParams};
use crate::{Config, UpdateAndRetrieveFees, Volume, VolumeProvider};

use frame_support::{
	construct_runtime, parameter_types,
	traits::{ConstU32, ConstU64},
};
use orml_traits::GetByKey;
pub use orml_traits::MultiCurrency;
use sp_core::H256;
use sp_runtime::{
	traits::{BlakeTwo256, IdentityLookup, One, Zero},
	BuildStorage, FixedU128, Perquintill,
};

type Block = frame_system::mocking::MockBlock<Test>;
pub type Balance = u128;
pub type AssetId = u32;
pub type AccountId = u64;

pub const HDX: AssetId = 0;

pub const ONE: Balance = 1_000_000_000_000;

pub(crate) type Fee = Perquintill;

thread_local! {
	pub static ORACLE: RefCell<Box<dyn CustomOracle>> = RefCell::new(Box::new(Oracle::new()));
	pub static BLOCK: RefCell<usize> = RefCell::new(0);
	pub static ASSET_FEE_PARAMS: RefCell<FeeParams<Fee>> = RefCell::new(fee_params_default());
	pub static PROTOCOL_FEE_PARAMS: RefCell<FeeParams<Fee>> = RefCell::new(fee_params_default());
}

fn fee_params_default() -> FeeParams<Fee> {
	FeeParams {
		min_fee: Fee::from_percent(1),
		max_fee: Fee::from_percent(40),
		decay: FixedU128::zero(),
		amplification: FixedU128::one(),
	}
}

construct_runtime!(
	pub enum Test
	{
		System: frame_system,
		DynamicFees: crate,
	}
);

impl frame_system::Config for Test {
	type BaseCallFilter = frame_support::traits::Everything;
	type BlockWeights = ();
	type BlockLength = ();
	type RuntimeOrigin = RuntimeOrigin;
	type RuntimeCall = RuntimeCall;
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
	type AccountData = ();
	type OnNewAccount = ();
	type OnKilledAccount = ();
	type SystemWeightInfo = ();
	type SS58Prefix = ();
	type OnSetCode = ();
	type MaxConsumers = ConstU32<16>;
}

parameter_types! {
	pub AssetFeeParams: FeeParams<Fee>= ASSET_FEE_PARAMS.with(|v| *v.borrow());
	pub ProtocolFeeParams: FeeParams<Fee>= PROTOCOL_FEE_PARAMS.with(|v| *v.borrow());
}

impl Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type Fee = Fee;
	type AssetId = AssetId;
	type BlockNumberProvider = System;
	type Oracle = OracleProvider;
	type AssetFeeParameters = AssetFeeParams;
	type ProtocolFeeParameters = ProtocolFeeParams;
}

pub struct ExtBuilder {
	initial_fee: Option<(Fee, Fee, u64)>,
}

impl Default for ExtBuilder {
	fn default() -> Self {
		ORACLE.with(|v| {
			*v.borrow_mut() = Box::new(Oracle::new());
		});

		Self { initial_fee: None }
	}
}

impl ExtBuilder {
	pub fn with_asset_fee_params(self, min_fee: Fee, max_fee: Fee, decay: FixedU128, amplification: FixedU128) -> Self {
		ASSET_FEE_PARAMS.with(|v| {
			*v.borrow_mut() = FeeParams {
				max_fee,
				min_fee,
				decay,
				amplification,
			}
		});

		self
	}

	pub fn with_protocol_fee_params(
		self,
		min_fee: Fee,
		max_fee: Fee,
		decay: FixedU128,
		amplification: FixedU128,
	) -> Self {
		PROTOCOL_FEE_PARAMS.with(|v| {
			*v.borrow_mut() = FeeParams {
				max_fee,
				min_fee,
				decay,
				amplification,
			}
		});

		self
	}

	pub fn with_oracle(self, oracle: impl CustomOracle + 'static) -> Self {
		ORACLE.with(|v| {
			*v.borrow_mut() = Box::new(oracle);
		});
		self
	}

	pub fn with_initial_fees(mut self, asset_fee: Fee, protocol_fee: Fee, block_number: u64) -> Self {
		self.initial_fee = Some((asset_fee, protocol_fee, block_number));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut r: sp_io::TestExternalities = frame_system::GenesisConfig::<Test>::default()
			.build_storage()
			.unwrap()
			.into();
		r.execute_with(|| {
			if let Some(initial_fee) = self.initial_fee {
				crate::AssetFee::<Test>::insert(
					HDX,
					FeeEntry {
						asset_fee: initial_fee.0,
						protocol_fee: initial_fee.1,
						timestamp: initial_fee.2,
					},
				);
			}
		});

		r
	}
}

pub struct OracleProvider;

impl VolumeProvider<AssetId, Balance> for OracleProvider {
	type Volume = AssetVolume;

	fn asset_volume(asset_id: AssetId) -> Option<Self::Volume> {
		let volume = ORACLE.with(|v| v.borrow().volume(asset_id, BLOCK.with(|v| *v.borrow())));
		Some(volume)
	}

	fn asset_liquidity(asset_id: AssetId) -> Option<Balance> {
		let liquidity = ORACLE.with(|v| v.borrow().liquidity(asset_id, BLOCK.with(|v| *v.borrow())));
		Some(liquidity)
	}
}

#[derive(Default, Clone, Debug)]
pub struct AssetVolume {
	pub(crate) amount_in: Balance,
	pub(crate) amount_out: Balance,
}

impl Volume<Balance> for AssetVolume {
	fn amount_in(&self) -> Balance {
		self.amount_in
	}

	fn amount_out(&self) -> Balance {
		self.amount_out
	}
}

impl From<(Balance, Balance, Balance)> for AssetVolume {
	fn from(value: (Balance, Balance, Balance)) -> Self {
		Self {
			amount_in: value.0,
			amount_out: value.1,
		}
	}
}

pub trait CustomOracle {
	fn volume(&self, _asset_id: AssetId, block: usize) -> AssetVolume;

	fn liquidity(&self, _asset_id: AssetId, block: usize) -> Balance;
}

pub(crate) fn retrieve_fee_entry(asset_id: AssetId) -> (Fee, Fee) {
	<UpdateAndRetrieveFees<Test> as GetByKey<AssetId, (Fee, Fee)>>::get(&asset_id)
}

pub(crate) fn get_oracle_entry(asset_id: AssetId, block_number: u64) -> AssetVolume {
	ORACLE.with(|v| v.borrow().volume(asset_id, block_number as usize))
}
