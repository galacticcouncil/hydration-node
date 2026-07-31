// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

use crate as pallet_meta_tx;
use frame_support::{derive_impl, traits::ConstU128};
use sp_core::{sr25519, Pair};
use sp_runtime::{
	traits::{IdentifyAccount, IdentityLookup},
	AccountId32, BuildStorage, MultiSignature, MultiSigner,
};

pub type AccountId = AccountId32;
pub type Balance = u128;
type Block = frame_system::mocking::MockBlock<Test>;

frame_support::construct_runtime!(
	pub enum Test {
		System: frame_system,
		Balances: pallet_balances,
		Utility: pallet_utility,
		MetaTx: pallet_meta_tx,
	}
);

#[derive_impl(frame_system::config_preludes::TestDefaultConfig)]
impl frame_system::Config for Test {
	type Block = Block;
	type AccountId = AccountId;
	type Lookup = IdentityLookup<Self::AccountId>;
	type AccountData = pallet_balances::AccountData<Balance>;
}

#[derive_impl(pallet_balances::config_preludes::TestDefaultConfig)]
impl pallet_balances::Config for Test {
	type Balance = Balance;
	type AccountStore = System;
	type ExistentialDeposit = ConstU128<1>;
}

impl pallet_utility::Config for Test {
	type RuntimeEvent = RuntimeEvent;
	type RuntimeCall = RuntimeCall;
	type PalletsOrigin = OriginCaller;
	type BatchHook = ();
	type WeightInfo = ();
}

impl pallet_meta_tx::Config for Test {
	type RuntimeCall = RuntimeCall;
	type Signature = MultiSignature;
	type Signer = MultiSigner;
	type WeightInfo = ();
}

/// Deterministic sr25519 keypair for `//name`.
pub fn keypair(name: &str) -> sr25519::Pair {
	sr25519::Pair::from_string(&alloc::format!("//{name}"), None).expect("static seed is valid; qed")
}

/// Account id derived from a keypair the same way the runtime derives it.
pub fn account_of(pair: &sr25519::Pair) -> AccountId {
	MultiSigner::from(pair.public()).into_account()
}

/// Sign `payload` as `pair`, producing the runtime's signature type.
pub fn sign(pair: &sr25519::Pair, payload: &[u8]) -> MultiSignature {
	MultiSignature::from(pair.sign(payload))
}

extern crate alloc;

#[derive(Default)]
pub struct ExtBuilder {
	balances: Vec<(AccountId, Balance)>,
}

impl ExtBuilder {
	pub fn with_balance(mut self, who: AccountId, amount: Balance) -> Self {
		self.balances.push((who, amount));
		self
	}

	pub fn build(self) -> sp_io::TestExternalities {
		let mut storage = frame_system::GenesisConfig::<Test>::default().build_storage().unwrap();

		pallet_balances::GenesisConfig::<Test> {
			balances: self.balances,
			..Default::default()
		}
		.assimilate_storage(&mut storage)
		.unwrap();

		let mut ext: sp_io::TestExternalities = storage.into();
		ext.execute_with(|| System::set_block_number(1));
		ext
	}
}
