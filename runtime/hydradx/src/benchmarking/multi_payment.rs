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

use crate::{AccountId, AssetId, Balance, Currencies, Runtime};
use primitives::Price;

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::BenchmarkError;
use frame_support::assert_ok;
use frame_system::RawOrigin;
use orml_benchmarking::runtime_benchmarks;
use sp_runtime::traits::SaturatedConversion;

use hydradx_traits::pools::SpotPriceProvider;
use orml_traits::MultiCurrencyExtended;

type MultiPaymentPallet<T> = pallet_transaction_multi_payment::Pallet<T>;

const SEED: u32 = 1;

pub fn update_balance(currency_id: AssetId, who: &AccountId, balance: Balance) {
	assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
		currency_id,
		who,
		balance.saturated_into()
	));
}

runtime_benchmarks! {
	{ Runtime, pallet_transaction_multi_payment}

	add_currency {
		let price = Price::from(10);
	}: { MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), 10, price)? }
	verify {
		assert_eq!(MultiPaymentPallet::<Runtime>::currencies(10), Some(price));
	}

	remove_currency {
		assert_ok!(MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), 10, Price::from(2)));
	}: { MultiPaymentPallet::<Runtime>::remove_currency(RawOrigin::Root.into(), 10)? }
	verify {
		assert_eq!(MultiPaymentPallet::<Runtime>::currencies(10), None)
	}

	set_currency {
		let maker: AccountId = account("maker", 0, SEED);
		let caller: AccountId = account("caller", 0, SEED);
		let fallback_account: AccountId = account("fallback_account", 1, SEED);
		pallet_transaction_multi_payment::FallbackAccount::<Runtime>::put(fallback_account);

		let asset_id = register_asset(b"TST".to_vec(), 100u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;

		MultiPaymentPallet::<Runtime>::add_currency(RawOrigin::Root.into(), asset_id, Price::from(1)).map_err(|_| BenchmarkError::Stop("Failed to add supported currency"))?;

		update_balance(asset_id, &caller,100_000_000_000_000);

	}: { MultiPaymentPallet::<Runtime>::set_currency(RawOrigin::Signed(caller.clone()).into(), asset_id)? }
	verify{
		assert_eq!(MultiPaymentPallet::<Runtime>::get_currency(caller), Some(asset_id));
	}

	get_spot_price {
		let maker: AccountId = account("maker", 0, SEED);

		let asset_out = 0u32;
		let asset_id = register_asset(b"TST".to_vec(), 1u128).map_err(|_| BenchmarkError::Stop("Failed to register asset"))?;
		update_balance(asset_out, &maker, 2_000_000_000_000_000);
		update_balance(asset_id, &maker, 2_000_000_000_000_000);

		create_pool(maker, asset_out, asset_id, 1_000_000_000_000_000, Price::from_inner(500_000_000_000_000_000));

	}: { <Runtime as pallet_transaction_multi_payment::Config>::SpotPriceProvider::spot_price(asset_id, asset_out) }
	verify{
		assert_eq!(<Runtime as pallet_transaction_multi_payment::Config>::SpotPriceProvider::spot_price(asset_id, asset_out),
			Some(Price::from((2,1))));

	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use orml_benchmarking::impl_benchmark_test_suite;

	fn new_test_ext() -> sp_io::TestExternalities {
		frame_system::GenesisConfig::default()
			.build_storage::<crate::Runtime>()
			.unwrap()
			.into()
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
