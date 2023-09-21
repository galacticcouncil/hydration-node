// This file is part of HydraDX-node

// Copyright (C) 2020-2022  Intergalactic, Limited (GIB).
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
#![allow(clippy::result_large_err)]

use crate::{AccountId, AssetId, Balance, Currencies, Router, Runtime, System, LBP};

use frame_benchmarking::account;
use frame_support::dispatch::DispatchResult;
use frame_support::{assert_ok, ensure};
use frame_system::RawOrigin;
use hydradx_traits::router::{PoolType, RouterT};
use orml_benchmarking::runtime_benchmarks;
use orml_traits::{MultiCurrency, MultiCurrencyExtended};
use pallet_route_executor::Trade;
use primitives::constants::currency::UNITS;
use sp_std::vec;

pub const INITIAL_BALANCE: Balance = 10_000_000 * UNITS;

fn funded_account(name: &'static str, index: u32, assets: &[AssetId]) -> AccountId {
	let account: AccountId = account(name, index, 0);
	for asset in assets {
		assert_ok!(<Currencies as MultiCurrencyExtended<_>>::update_balance(
			*asset,
			&account,
			INITIAL_BALANCE.try_into().unwrap(),
		));
	}
	account
}

fn setup_lbp(caller: AccountId, asset_in: AssetId, asset_out: AssetId) -> DispatchResult {
	let asset_in_amount = 1_000_000 * UNITS;
	let asset_out_amount = 2_000_000 * UNITS;
	let initial_weight = 20_000_000;
	let final_weight = 80_000_000;
	let fee = (2, 1_000);
	let fee_collector = caller.clone();
	let repay_target = 0;

	let pool_id = LBP::pair_account_from_assets(asset_in, asset_out);

	LBP::create_pool(
		RawOrigin::Root.into(),
		caller.clone(),
		asset_in,
		asset_in_amount,
		asset_out,
		asset_out_amount,
		initial_weight,
		final_weight,
		pallet_lbp::WeightCurveType::Linear,
		fee,
		fee_collector,
		repay_target,
	)?;
	ensure!(
		pallet_lbp::PoolData::<Runtime>::contains_key(&pool_id),
		"Pool does not exist."
	);

	let start = 1u32;
	let end = 11u32;

	LBP::update_pool_data(
		RawOrigin::Signed(caller).into(),
		pool_id,
		None,
		Some(start),
		Some(end),
		None,
		None,
		None,
		None,
		None,
	)?;

	System::set_block_number(2u32);
	Ok(())
}

runtime_benchmarks! {
	{Runtime, pallet_route_executor}

	// Calculates the weight of LBP trade. Used in the calculation to determine the weight of the overhead.
	multi_trade_execution_sell_in_lbp {
		let c in 0..1;	// if c == 1, calculate_sell_trade_amounts is executed
		let s in 0..1;	// if e == 1, sell is executed
		let asset_in = 0u32;
		let asset_out = 1u32;
		let caller: AccountId = funded_account("caller", 7, &[asset_in, asset_out]);
		let seller: AccountId = funded_account("seller", 8, &[asset_in, asset_out]);

		setup_lbp(caller, asset_in, asset_out)?;

		let trades = vec![Trade {
			pool: PoolType::LBP,
			asset_in,
			asset_out
		}];

		let amount_to_sell: Balance = UNITS;

	}: {
		if c != 0 {
			Router::calculate_sell_trade_amounts(trades.as_slice(), amount_to_sell)?;
		}
		if s != 0 {
			Router::sell(RawOrigin::Signed(seller.clone()).into(), asset_in, asset_out, amount_to_sell, 0u128, trades.clone())?;
		}
	}
	verify {
		if s != 0 {
			assert_eq!(<Currencies as MultiCurrency<_>>::free_balance(
			asset_in,
			&seller,
			), INITIAL_BALANCE - amount_to_sell);
		}
	}

	// Calculates the weight of LBP trade. Used in the calculation to determine the weight of the overhead.
	multi_trade_execution_buy_in_lbp {
		let c in 1..2;	// number of times `calculate_buy_trade_amounts` is executed
		let b in 0..1;	// if e == 1, buy is executed
		let asset_in = 0u32;
		let asset_out = 1u32;
		let caller: AccountId = funded_account("caller", 0, &[asset_in, asset_out]);
		let buyer: AccountId = funded_account("buyer", 1, &[asset_in, asset_out]);

		setup_lbp(caller, asset_in, asset_out)?;

		let trades = vec![Trade {
			pool: PoolType::LBP,
			asset_in,
			asset_out
		}];

		let amount_to_buy = UNITS;

	}: {
		for _ in 1..c {
			Router::calculate_buy_trade_amounts(trades.as_slice(), amount_to_buy)?;
		}
		if b != 0 {
			Router::buy(RawOrigin::Signed(buyer.clone()).into(), asset_in, asset_out, amount_to_buy, u128::MAX, trades)?
		}
	}
	verify {
		if b != 0 {
			assert!(<Currencies as MultiCurrency<_>>::free_balance(
			asset_in,
			&buyer,
			) < INITIAL_BALANCE);
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::NativeExistentialDeposit;
	use frame_support::traits::GenesisBuild;
	use orml_benchmarking::impl_benchmark_test_suite;

	fn new_test_ext() -> sp_io::TestExternalities {
		let mut t = frame_system::GenesisConfig::default()
			.build_storage::<Runtime>()
			.unwrap();

		pallet_asset_registry::GenesisConfig::<Runtime> {
			registered_assets: vec![(b"LRNA".to_vec(), 1_000u128, Some(1))],
			native_asset_name: b"HDX".to_vec(),
			native_existential_deposit: NativeExistentialDeposit::get(),
		}
		.assimilate_storage(&mut t)
		.unwrap();

		sp_io::TestExternalities::new(t)
	}

	impl_benchmark_test_suite!(new_test_ext(),);
}
