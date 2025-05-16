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
use crate::traits::BenchmarkHelper;
use crate::types::Balance;
use frame_benchmarking::{account, benchmarks};
use frame_support::traits::fungibles::Mutate;
use frame_support::traits::EnsureOrigin;
use frame_support::BoundedVec;
use frame_system::RawOrigin;
use hydradx_traits::stableswap::AssetAmount;
use hydradx_traits::OraclePeriod;
use pallet_stableswap::types::{BoundedPegSources, PegSource};
use pallet_stableswap::{BenchmarkHelper as HSMBenchmarkHelper, MAX_ASSETS_IN_POOL};
use sp_runtime::{FixedU128, Perbill, Permill};
use sp_std::vec;
use sp_std::vec::Vec;

const DECIMALS: u8 = 18;
const ONE: Balance = 1_000_000_000_000_000_000;
const INITIAL_LIQUIDITY: Balance = 1_000;

const ASSET_ID_OFFSET: u32 = 5_000;

benchmarks! {
	where_clause { where
		T: Config,
		T: pallet_stableswap::Config,
		T::AssetId: From<u32>,
		<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
	}

	add_collateral_asset {
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		// main pool
		let (main_pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let main_collateral = assets[1];
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(101, 100);
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = Some(10_000 * ONE);

		// Setup: Add collateral asset - worst is cased is adding last possible asset
		// create and fill up to MAX_COLLATERALS - 1 assets
		for idx in 2..=MAX_COLLATERALS{
			let (pool_id, assets) = seed_pool::<T>((222_222u32 +idx).into(), hollar, ASSET_ID_OFFSET * idx)?;
			let collateral = assets[1];
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
		}

		let collateral_count = Collaterals::<T>::iter().count() as u32;
		assert_eq!(collateral_count, MAX_COLLATERALS - 1);
		let successful_origin = <T as crate::Config>::AuthorityOrigin::try_successful_origin().expect("Failed to get successful origin");
	}: _<T::RuntimeOrigin>(successful_origin, main_collateral, main_pool_id, purchase_fee, max_buy_price_coefficient, buy_back_fee, b, max_in_holding)
	verify {
		assert!(Collaterals::<T>::contains_key(main_collateral));
	}

	remove_collateral_asset {
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		let (pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(101, 100);
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
		let successful_origin = <T as crate::Config>::AuthorityOrigin::try_successful_origin().expect("Failed to get successful origin");
	}: _<T::RuntimeOrigin>(successful_origin, collateral)
	verify {
		assert!(!Collaterals::<T>::contains_key(collateral));
	}

	update_collateral_asset {
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		let (pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(101, 100);
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
		let new_max_buy_price_coefficient = Some(FixedU128::from_rational(102, 100));
		let new_buy_back_fee = Some(Permill::from_percent(2));
		let new_b = Some(Perbill::from_percent(60));
		let new_max_in_holding = Some(Some(20_000 * ONE));
		let successful_origin = <T as crate::Config>::AuthorityOrigin::try_successful_origin().expect("Failed to get successful origin");
	}: _<T::RuntimeOrigin>(successful_origin, collateral, new_purchase_fee, new_max_buy_price_coefficient, new_buy_back_fee, new_b, new_max_in_holding)
	verify {
		let info = Collaterals::<T>::get(collateral).unwrap();
		assert_eq!(info.purchase_fee, Permill::from_percent(2));
		assert_eq!(info.max_buy_price_coefficient, FixedU128::from_rational(102, 100));
		assert_eq!(info.buy_back_fee, Permill::from_percent(2));
		assert_eq!(info.buyback_rate, Perbill::from_percent(60));
		assert_eq!(info.max_in_holding, Some(20_000 * ONE));
	}

	sell {
		// Set up a scenario for selling collateral to get Hollar (worst case)
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		let (pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(4, 1);
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

		// Worst case is selling hollar back into hsm
		let caller: T::AccountId = account("seller", 0, 0);
		<T as Config>::BenchmarkHelper::bind_address(caller.clone()).unwrap();
		<T as Config>::Currency::set_balance(hollar, &caller, 1_000 * ONE);

		// Setup HSM account with enough balance
		<T as Config>::Currency::set_balance(collateral, &Pallet::<T>::account_id(), 10_000 * ONE);
		let cb = <T as Config>::Currency::balance(collateral, &caller);
		assert!(cb.is_zero());

		// Setup slippage limit (worst case)
		let amount_in = 10 * ONE;
		let slippage_limit = 1; // Minimum possible amount out

		<pallet_stableswap::Pallet<T> as frame_support::traits::OnFinalize<BlockNumberFor<T>>>::on_finalize(0u32.into()); // should not matter what block number it is

	}: _(RawOrigin::Signed(caller.clone()), hollar, collateral, amount_in, slippage_limit)
	verify {
		let caller_balance = <T as Config>::Currency::balance(hollar, &caller);
		let caller_collateral_balance = <T as Config>::Currency::balance(collateral, &caller);
		assert_eq!(caller_balance, 1000 * ONE - amount_in);
		assert!(caller_balance > 0);
	}

	buy {
		// Set up a scenario for buying collateral with Hollar (worst case)
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		let (pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(4, 1);
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

		// setup balances - worst case is buying collateral
		let caller: T::AccountId = account("buyer", 0, 0);
		<T as Config>::BenchmarkHelper::bind_address(caller.clone())?;
		<T as Config>::Currency::set_balance(hollar, &caller, 1_000 * ONE);
		<T as Config>::Currency::set_balance(collateral, &Pallet::<T>::account_id(), 10_000 * ONE);

		<pallet_stableswap::Pallet<T> as frame_support::traits::OnFinalize<BlockNumberFor<T>>>::on_finalize(0u32.into()); // should not matter what block number it is

		// Setup slippage limit (worst case) - maximum possible amount in
		let amount_out = 10 * ONE;
		let slippage_limit = 1_000 * ONE;
	}: _(RawOrigin::Signed(caller.clone()), hollar, collateral, amount_out, slippage_limit)
	verify {
		let caller_balance = <T as Config>::Currency::balance(collateral, &caller);
		let caller_hollar_balance = <T as Config>::Currency::balance(hollar, &caller);
		assert_eq!(caller_balance, amount_out);
		assert!(caller_hollar_balance < 1000 * ONE);
	}

	execute_arbitrage {
		// Set up a scenario for arbitrage (worst case)
		let hollar = T::HollarId::get();
		seed_asset::<T>(hollar, DECIMALS)?;
		let (pool_id, assets) = seed_pool::<T>(222_222u32.into(), hollar, ASSET_ID_OFFSET)?;
		let purchase_fee = Permill::from_percent(1);
		let max_buy_price_coefficient = FixedU128::from_rational(4, 1);
		let buy_back_fee = Permill::from_percent(1);
		let b = Perbill::from_percent(50);
		let max_in_holding: Option<Balance> = None; // No limit for arbitrage test

		let collateral = assets[1];

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

		// Create account with hollar
		let arb: T::AccountId = account("arber", 0, 0);
		<T as Config>::Currency::set_balance(collateral, &Pallet::<T>::account_id(), 10 * ONE);

		<pallet_stableswap::Pallet<T> as frame_support::traits::OnFinalize<BlockNumberFor<T>>>::on_finalize(0u32.into()); // should not matter what block number it is
	}: _(RawOrigin::None, collateral)
	verify {
		let acc_balance = <T as Config>::Currency::balance(collateral, &Pallet::<T>::account_id());
		assert!(acc_balance < 10 * ONE);
	}

	on_finalize {
		let block_num: BlockNumberFor<T> = 5u32.into();
		frame_system::Pallet::<T>::set_block_number(block_num);
		// Clear one asset
		HollarAmountReceived::<T>::insert::<T::AssetId, u128>(100u32.into(), 1000000u128);
	}: { Pallet::<T>::on_finalize(block_num); }
	verify {
		assert!(HollarAmountReceived::<T>::iter().count().is_zero());
	}

	set_flash_mint_facilitator{
		let successful_origin = <T as crate::Config>::AuthorityOrigin::try_successful_origin().expect("Failed to get successful origin");
		let flash_minter = EvmAddress::from_slice(&hex!(
			"0101010101010101010101010101010101010101"
		));
	}: _<T::RuntimeOrigin>(successful_origin, flash_minter)
	verify {
		assert_eq!(FlashMintFacilitator::<T>::get(), Some(flash_minter));
	}

	impl_benchmark_test_suite!(Pallet, tests::mock::ExtBuilder::default().build(), tests::mock::Test);
}

// Helper function to create a new asset for testing
fn seed_asset<T>(asset_id: T::AssetId, decimals: u8) -> DispatchResult
where
	T: Config + pallet_stableswap::Config,
	T::AssetId: From<u32>,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	<T as pallet_stableswap::Config>::BenchmarkHelper::register_asset(asset_id, decimals)
}

// Helper function to create a new stable pool for testing
fn seed_pool<T>(
	pool_id: T::AssetId,
	hollar_id: T::AssetId,
	offset: u32,
) -> Result<(T::AssetId, Vec<T::AssetId>), DispatchError>
where
	T: Config + pallet_stableswap::Config,
	T::AssetId: From<u32>,
	<T as frame_system::Config>::AccountId: AsRef<[u8; 32]> + IsType<AccountId32>,
{
	seed_asset::<T>(pool_id, DECIMALS)?;
	let mut assets = vec![hollar_id];

	let mut initial_liquidity = vec![INITIAL_LIQUIDITY * ONE];

	let mut pegs = vec![PegSource::Value((1, 1))];
	for idx in 0..MAX_ASSETS_IN_POOL - 1 {
		let asset_id: T::AssetId = (idx + offset).into();
		let _ = seed_asset::<T>(asset_id, DECIMALS);
		assets.push(asset_id);
		<T as pallet_stableswap::Config>::BenchmarkHelper::register_asset_peg(
			(hollar_id, asset_id),
			(1u128, 2u128),
			*b"benchmar",
		)?;
		let source = PegSource::Oracle((*b"benchmar", OraclePeriod::LastBlock, hollar_id));
		pegs.push(source);
		initial_liquidity.push(INITIAL_LIQUIDITY * ONE - 50 * ONE);
	}

	let amplification = 22;
	let fee = Permill::from_percent(1);

	let successful_origin = <T as pallet_stableswap::Config>::AuthorityOrigin::try_successful_origin()
		.expect("Failed to get successful origin");

	pallet_stableswap::Pallet::<T>::create_pool_with_pegs(
		successful_origin,
		pool_id,
		BoundedVec::try_from(assets.clone()).unwrap(),
		amplification,
		fee,
		BoundedPegSources::try_from(pegs).unwrap(),
		Permill::from_percent(100),
	)?;

	let provider: T::AccountId = account("provider", 1, 1);
	let r = <T as Config>::BenchmarkHelper::bind_address(provider.clone());
	if r.is_err() {
		log::warn!(target: "hsm", "Benchmarks - address already bounded.")
	}

	let mut liquidity_amounts = vec![];

	for (asset_id, liquidity) in assets.iter().zip(initial_liquidity) {
		<T as Config>::Currency::set_balance(*asset_id, &provider, liquidity);
		liquidity_amounts.push(AssetAmount::new(*asset_id, liquidity));
	}

	pallet_stableswap::Pallet::<T>::add_assets_liquidity(
		RawOrigin::Signed(provider.clone()).into(),
		pool_id,
		BoundedVec::truncate_from(liquidity_amounts),
		0,
	)
	.expect("To provide initial liquidity");

	Ok((pool_id, assets))
}
