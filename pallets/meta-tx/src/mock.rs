// This file is part of https://github.com/galacticcouncil/*
//
// Copyright (C) 2021-2024  Intergalactic, Limited (GIB)
// SPDX-License-Identifier: Apache-2.0

use crate as pallet_meta_tx;
use frame_support::{
	derive_impl, parameter_types,
	traits::{ConstU128, ConstU64},
};
use hydradx_traits::evm::{EvmFeePayerSupport, InspectEvmAccounts};
use sp_core::{ecdsa, sr25519, Pair, H160};
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

parameter_types! {
	pub storage FeePayer: Option<AccountId> = None;
}

/// Mirrors `pallet-evm-accounts`: an EVM address maps to `ETH\0` + address + zero padding.
pub struct MockEvmAccounts;

impl MockEvmAccounts {
	fn truncated(address: H160) -> AccountId {
		let mut data = [0u8; 32];
		data[0..4].copy_from_slice(b"ETH\0");
		data[4..24].copy_from_slice(address.as_bytes());
		AccountId32::from(data)
	}
}

impl InspectEvmAccounts<AccountId> for MockEvmAccounts {
	fn is_evm_account(account_id: AccountId) -> bool {
		AsRef::<[u8; 32]>::as_ref(&account_id).starts_with(b"ETH\0")
	}

	fn evm_address(account_id: &impl AsRef<[u8; 32]>) -> H160 {
		let account = account_id.as_ref();
		if account.starts_with(b"ETH\0") {
			H160::from_slice(&account[4..24])
		} else {
			H160::from_slice(&account[..20])
		}
	}

	fn truncated_account_id(evm_address: H160) -> AccountId {
		Self::truncated(evm_address)
	}

	fn bound_account_id(_evm_address: H160) -> Option<AccountId> {
		None
	}

	fn account_id(evm_address: H160) -> AccountId {
		Self::truncated(evm_address)
	}

	fn can_deploy_contracts(_evm_address: H160) -> bool {
		false
	}

	fn is_approved_contract(_address: H160) -> bool {
		false
	}
}

pub struct MockEvmFeePayer;

impl EvmFeePayerSupport for MockEvmFeePayer {
	type AccountId = AccountId;

	fn set_fee_payer(payer: Self::AccountId) -> Option<Self::AccountId> {
		let previous = FeePayer::get();
		FeePayer::set(&Some(payer));
		previous
	}

	fn clear_fee_payer() -> Option<Self::AccountId> {
		let previous = FeePayer::get();
		FeePayer::set(&None);
		previous
	}
}

impl pallet_meta_tx::Config for Test {
	type RuntimeCall = RuntimeCall;
	type Signature = MultiSignature;
	type Signer = MultiSigner;
	type EvmAccounts = MockEvmAccounts;
	type EvmFeePayer = MockEvmFeePayer;
	type ChainId = ConstU64<222_222>;
	type MaxDeadline = ConstU64<100>;
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

/// Deterministic secp256k1 keypair for `//name`, standing in for a Turnkey/MetaMask wallet.
pub fn evm_keypair(name: &str) -> ecdsa::Pair {
	ecdsa::Pair::from_string(&alloc::format!("//{name}"), None).expect("static seed is valid; qed")
}

/// The Ethereum address of `pair`, derived the same way the pallet recovers it.
pub fn evm_address_of(pair: &ecdsa::Pair) -> H160 {
	let probe = [0u8; 32];
	let signature = pair.sign_prehashed(&probe);
	let Ok(public) = sp_io::crypto::secp256k1_ecdsa_recover(signature.as_ref(), &probe) else {
		panic!("a freshly produced signature always recovers");
	};
	H160::from(sp_core::H256::from_slice(&sp_io::hashing::keccak_256(&public)))
}

/// Sign `payload` as `pair`, split into the `v, r, s` the extrinsic expects.
pub fn evm_sign(pair: &ecdsa::Pair, payload: &[u8; 32]) -> (u8, sp_core::H256, sp_core::H256) {
	let signature = pair.sign_prehashed(payload);
	let bytes: &[u8] = signature.as_ref();
	(
		bytes[64],
		sp_core::H256::from_slice(&bytes[..32]),
		sp_core::H256::from_slice(&bytes[32..64]),
	)
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
