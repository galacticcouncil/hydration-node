// Copyright (C) 2020-2024  Intergalactic, Limited (GIB). SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use crate::types::Balance;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::EnsureOrigin;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::{AssetKind, Create, OraclePeriod};
use pallet_stableswap::types::{BoundedPegSources, PegSource};
use pallet_stableswap::{BenchmarkHelper, MAX_ASSETS_IN_POOL};
use sp_runtime::{Perbill, Permill};

pub const ONE: Balance = 1_000_000_000_000;
const ASSET_ID_OFFSET: u32 = 2_000;

benchmarks! {
	where_clause { where
		T: Config,
		T: pallet_stableswap::Config,
		T::AssetId: From<u32>,
		<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	}

	add_collateral_asset {
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = ( 101, 100 );
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		let collateral = assets[1];

	}: _(RawOrigin::Root, collateral, pool_id, purchase_fee, max_buy_price_coefficient, buy_back_fee, b, max_in_holding)
	verify {
		assert!(Collaterals::<T>::contains_key(collateral));
	}

	remove_collateral_asset {
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = (101, 100 );
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		let collateral = assets[1];

		// Setup: Add collateral asset
		Pallet::<T>::add_collateral_asset(
			RawOrigin::Root.into(),
			collateral,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			b,
			max_in_holding
		)?;
	}: _(RawOrigin::Root, collateral)
	verify {
		assert!(!Collaterals::<T>::contains_key(collateral));
	}

	update_collateral_asset {
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = ( 101, 100 );
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		let collateral = assets[1];

		// Setup: Add collateral asset
		Pallet::<T>::add_collateral_asset(
			RawOrigin::Root.into(),
			collateral,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			b,
			max_in_holding
		)?;

		// New values
		let new_purchase_fee = Some(Permill::from_percent(2));
		let new_max_buy_price_coefficient = Some((102, 100 ));
		let new_buy_back_fee = Some(Permill::from_percent(2));
		let new_b = Some(Perbill::from_percent(60));
		let new_max_in_holding = Some(Some(20_000 * ONE));
	}: _(RawOrigin::Root, collateral, new_purchase_fee, new_max_buy_price_coefficient, new_buy_back_fee, new_b, new_max_in_holding)
	verify {
		let info = Collaterals::<T>::get(collateral).unwrap();
		assert_eq!(info.purchase_fee, Permill::from_percent(2));
		assert_eq!(info.max_buy_price_coefficient, (102,100));
		assert_eq!(info.buy_back_fee, Permill::from_percent(2));
		assert_eq!(info.b, Perbill::from_percent(60));
		assert_eq!(info.max_in_holding, Some(20_000 * ONE));
	}

	sell {
		// Set up a scenario for selling collateral to get Hollar (worst case)
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = (101, 100 );
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		let collateral = assets[1];

		// Add collateral asset
		Pallet::<T>::add_collateral_asset(
			RawOrigin::Root.into(),
			collateral,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			b,
			max_in_holding
		)?;

		// Create account with collateral
		let caller: T::AccountId = account("seller", 0, 0);
		<T as Config>::Currency::set_balance(collateral, &caller, 1_000 * ONE);

		// Setup HSM account with enough balance
		<T as Config>::Currency::set_balance(hollar, &Pallet::<T>::account_id(), 10_000 * ONE);

		// Setup slippage limit (worst case)
		let amount_in = 100 * ONE;
		let slippage_limit = 1; // Minimum possible amount out
	}: _(RawOrigin::Signed(caller.clone()), collateral, hollar, amount_in, slippage_limit)
	verify {
	}

	buy {
		// Set up a scenario for buying collateral with Hollar (worst case)
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = (101, 100 );
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		let collateral = assets[1];

		// Add collateral asset with some holdings
		Pallet::<T>::add_collateral_asset(
			RawOrigin::Root.into(),
			collateral,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			b,
			max_in_holding
		)?;

		// Setup HSM collateral holdings
		CollateralHoldings::<T>::insert(collateral, 1_000 * ONE);

		// Create account with hollar
		let caller: T::AccountId = account("buyer", 0, 0);
		<T as Config>::Currency::set_balance(hollar, &caller, 1_000 * ONE);

		// Setup slippage limit (worst case) - maximum possible amount in
		let amount_out = 100 * ONE;
		let slippage_limit = 1_000 * ONE;
	}: _(RawOrigin::Signed(caller.clone()), hollar, collateral, amount_out, slippage_limit)
	verify {
	}

	execute_arbitrage {
		// Set up a scenario for arbitrage (worst case)
		let hollar = T::HollarId::get();
		let (pool_id, assets) = seed_pool::<T>(hollar)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = (200, 100 ); // Large gap to ensure arbitrage opportunity
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = None; // No limit for arbitrage test

		let collateral = assets[1];

		// Add collateral asset
		Pallet::<T>::add_collateral_asset(
			RawOrigin::Root.into(),
			collateral,
			pool_id,
			purchase_fee,
			max_buy_price_coefficient,
			buy_back_fee,
			b,
			max_in_holding
		)?;

		// Setup HSM account with hollar minting permissions
		// Note: In a real environment, we would need to set up the EVM contract and permissions
		// For benchmarking, we'll assume minting works

		// Setup stable pool with imbalanced prices to create arbitrage opportunity
		// This would typically be done by manipulating the pool balances

		// For testing, we'll just mock the arbitrage calculation to return a positive opportunity
		// This might need adjustment in real testing
	}: _(RawOrigin::None, collateral)
	verify {
		// Verify arbitrage was attempted
		// Note: In benchmarking, the actual arbitrage may not succeed due to mock constraints
		// We're verifying the execution path was completed
	}

	impl_benchmark_test_suite!(Pallet, tests::mock::ExtBuilder::default().build(), tests::mock::Test);
}

// Helper function to create a new asset for testing
fn seed_asset<T: Config>(asset_id: T::AssetId, decimals: u8) -> DispatchResult
where
	T: pallet_stableswap::Config,
	T::AssetId: From<u32>,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	T::BenchmarkHelper::register_asset(asset_id, decimals)
}

// Helper function to create a new stable pool for testing
fn seed_pool<T: Config>(hollar_id: T::AssetId) -> Result<(T::AssetId, Vec<T::AssetId>), DispatchError>
where
	T: pallet_stableswap::Config,
	T::AssetId: From<u32>,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	let pool_id = 222_222u32.into(); // Use a fixed ID for testing
	seed_asset::<T>(pool_id, 18u8)?;
	seed_asset::<T>(hollar_id, 18u8)?;
	let mut assets = vec![hollar_id];

	//TODO: we should probably create a peg source in oracle for the worst case!
	let mut pegs = vec![PegSource::Value((1, 1))];
	for idx in 0..MAX_ASSETS_IN_POOL - 1 {
		let asset_id: T::AssetId = (idx + ASSET_ID_OFFSET).into();
		T::BenchmarkHelper::register_asset(asset_id, 18).expect("Failed to register asset");
		assets.push(asset_id);
		pegs.push(PegSource::Value((1, 1)));
	}

	let amplification = 22;
	let fee = Permill::from_percent(1);

	let successful_origin = T::AuthorityOrigin::try_successful_origin().expect("Failed to get successful origin");

	pallet_stableswap::Pallet::<T>::create_pool_with_pegs(
		successful_origin,
		pool_id,
		BoundedVec::try_from(assets.clone()).unwrap(),
		amplification,
		fee,
		BoundedPegSources::try_from(pegs).unwrap(),
		Permill::from_percent(100),
	)
	.unwrap();

	Ok((pool_id, assets))
}
