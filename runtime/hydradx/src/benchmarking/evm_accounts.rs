// Copyright (C) 2020-2024  Intergalactic, Limited (GIB).
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

use super::*;
use crate::{AccountId, Currencies, EVMAccounts, MultiTransactionPayment, Runtime, System};

use frame_benchmarking::BenchmarkError;
use frame_benchmarking::{account, whitelisted_caller};
use frame_support::assert_ok;
use frame_support::sp_runtime::traits::IdentifyAccount;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_core::Pair;
use sp_runtime::traits::StaticLookup;
use sp_std::prelude::*;

runtime_benchmarks! {
	{ Runtime, pallet_evm_accounts }

	bind_evm_address {
		let user: AccountId = account("user", 0, 1);
		let evm_address = EVMAccounts::evm_address(&user);
		assert!(EVMAccounts::account(evm_address).is_none());

	}: _(RawOrigin::Signed(user.clone()))
	verify {
		assert!(EVMAccounts::account(evm_address).is_some());
	}

	add_contract_deployer {
		let user: AccountId = account("user", 0, 1);
		let evm_address = EVMAccounts::evm_address(&user);
		assert!(EVMAccounts::contract_deployer(evm_address).is_none());

	}: _(RawOrigin::Root, evm_address)
	verify {
		assert!(EVMAccounts::contract_deployer(evm_address).is_some());
	}

	remove_contract_deployer {
		let user: AccountId = account("user", 0, 1);
		let evm_address = EVMAccounts::evm_address(&user);

		EVMAccounts::add_contract_deployer(RawOrigin::Root.into(), evm_address)?;

		assert!(EVMAccounts::contract_deployer(evm_address).is_some());

	}: _(RawOrigin::Root, evm_address)
	verify {
		assert!(EVMAccounts::contract_deployer(evm_address).is_none());
	}

	renounce_contract_deployer {
		let user: AccountId = account("user", 0, 1);
		let evm_address = EVMAccounts::evm_address(&user);

		EVMAccounts::add_contract_deployer(RawOrigin::Root.into(), evm_address)?;
		EVMAccounts::bind_evm_address(RawOrigin::Signed(user.clone()).into())?;

		assert!(EVMAccounts::contract_deployer(evm_address).is_some());

	}: _(RawOrigin::Signed(user))
	verify {
		assert!(EVMAccounts::contract_deployer(evm_address).is_none());
	}

	approve_contract {
		let contract: AccountId = account("contract", 0, 1);
		let evm_address = EVMAccounts::evm_address(&contract);
		assert!(EVMAccounts::approved_contract(evm_address).is_none());

	}: _(RawOrigin::Root, evm_address)
	verify {
		assert!(EVMAccounts::approved_contract(evm_address).is_some());
	}

	disapprove_contract {
		let contract: AccountId = account("contract", 0, 1);
		let evm_address = EVMAccounts::evm_address(&contract);

		EVMAccounts::approve_contract(RawOrigin::Root.into(), evm_address)?;

		assert!(EVMAccounts::approved_contract(evm_address).is_some());

	}: _(RawOrigin::Root, evm_address)
	verify {
		assert!(EVMAccounts::approved_contract(evm_address).is_none());
	}

	claim_account {
		let from: AccountId = whitelisted_caller();
		let contract_address = deploy_token_contract(from.clone());
		let asset_id = bind_erc20(contract_address);

		let pair = sp_core::sr25519::Pair::from_seed_slice([1; 64].as_slice()).unwrap();
		let user: AccountId = frame_support::sp_runtime::MultiSigner::from(pair.public()).into_account().into();
		let evm_address = EVMAccounts::evm_address(&user);
		// Use pregenerated signature from a test because `sign` requires `full_crypto` feature and should not be enabled in production.
		// Signature was generated with asset_id = 1000001
		let signature = primitives::Signature::Sr25519(sp_core::sr25519::Signature::from(hex_literal::hex!["7256148e7897a760e6f83783d3d3fe2bf321cf6d92dcf984b94510a8200f6267284ed02f8ed8d8b1b6e3d5fdee9fcf3f4abec5f03541a00109a5d4582d8ea683"]));

		MultiTransactionPayment::add_currency(RawOrigin::Root.into(), asset_id, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;

		assert_ok!(Currencies::transfer(RawOrigin::Signed(from).into(), lookup_of_account(user.clone()), asset_id, 1_000 * BSX));

		assert!(EVMAccounts::account(evm_address).is_none());
		assert!(!System::account_exists(&user));
	}: _(RawOrigin::None, user.clone(), asset_id, signature)
	verify {
		assert!(EVMAccounts::account(evm_address).is_some());
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap()
			.into()
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
pub fn lookup_of_account(who: AccountId) -> <<Runtime as frame_system::Config>::Lookup as StaticLookup>::Source {
	<Runtime as frame_system::Config>::Lookup::unlookup(who)
}
