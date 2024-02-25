// This file is part of Basilisk-node

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
use crate::{AccountId, AssetId, Balance, Currencies, EmaOracle, Runtime, System};
use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_support::assert_ok;
use frame_support::traits::{OnFinalize, OnInitialize};
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use orml_traits::MultiCurrencyExtended;
use primitives::BlockNumber;
use sp_runtime::traits::SaturatedConversion;
use sp_runtime::FixedU128;

type DynamicEvmFeePallet<T> = pallet_dynamic_evm_fee::Pallet<T>;
use crate::evm::WETH_ASSET_LOCATION;
use pallet_dynamic_evm_fee::BaseFeePerGas;

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, pallet_dynamic_evm_fee}


	on_initialize{
		//let maker: AccountId = account("maker", 0, SEED);

		crate::benchmarking::omnipool::init()?;

		let acc = Omnipool::protocol_account();
		// Register new asset in asset registry
		let token_id = register_asset(b"AS1".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		assert_eq!(token_id, 1000001, "Token ID should be 1000001");
		set_location(token_id, WETH_ASSET_LOCATION).map_err(|_| BenchmarkError::Stop("Failed to set location for weth"))?;
		add_as_accepted_currency(token_id, FixedU128::from_inner(16420844565569051996)).map_err(|_| BenchmarkError::Stop("Failed to add token as accepted currency"))?;
		// Create account for token provider and set balance
		let owner: AccountId = account("owner", 0, 1);

		let token_price = FixedU128::from((1,5));
		let token_amount = 200_000_000_000_000_u128;

		update_balance(token_id, &acc, token_amount);
		update_balance(0, &owner, 1_000_000_000_000_000_u128);

		// Add the token to the pool
		Omnipool::add_token(RawOrigin::Root.into(), token_id, token_price, Permill::from_percent(100), owner)?;
		let seller: AccountId = account("seller", 3, 1);
		update_balance(0, &seller, 500_000_000_000_000_u128);
		Omnipool::sell(RawOrigin::Signed(seller).into(), 0, token_id, 10000000000000, 0)?;

		set_period(10);
		let base_fee_per_gas = <BaseFeePerGas<Runtime>>::get();

	}: {
		DynamicEvmFeePallet::<Runtime>::on_initialize(1u32)
		}
	verify{
		assert!(<BaseFeePerGas<Runtime>>::get() != base_fee_per_gas);
	}
}
use crate::Omnipool;
use sp_runtime::Permill;

fn set_period(to: u32) {
	while System::block_number() < Into::<BlockNumber>::into(to) {
		let b = System::block_number();

		System::on_finalize(b);
		EmaOracle::on_finalize(b);

		System::on_initialize(b + 1_u32);
		EmaOracle::on_initialize(b + 1_u32);

		System::set_block_number(b + 1_u32);
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use orml_benchmarking::impl_benchmark_test_suite;
	use sp_runtime::BuildStorage;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::<crate::Runtime>::default()
			.build_storage()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<crate::Runtime> {
			registered_assets: vec![
				(
					Some(1),
					Some(b"LRNA".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
				(
					Some(2),
					Some(b"DAI".to_vec().try_into().unwrap()),
					1_000u128,
					None,
					None,
					None,
					true,
				),
			],
			native_asset_name: b"HDX".to_vec().try_into().unwrap(),
			native_existential_deposit: NativeExistentialDeposit::get(),
			native_decimals: 12,
			native_symbol: b"HDX".to_vec().try_into().unwrap(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
