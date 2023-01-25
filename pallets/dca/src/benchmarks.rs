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

#![cfg(feature = "runtime-benchmarks")]

use super::*;

use frame_benchmarking::account;
use frame_benchmarking::benchmarks;
use frame_support::{assert_noop, assert_ok};
use frame_system::RawOrigin;
use hydradx_traits::Registry;
use orml_traits::MultiCurrencyExtended;
use sp_runtime::FixedU128;
use sp_runtime::Permill;

pub const ONE: Balance = 1_000_000_000_000;

fn schedule_fake<T: Config>(
	asset_in: T::Asset,
	asset_out: T::Asset,
	amount: Balance,
	recurrence: Recurrence,
) -> Schedule<T::Asset, T::BlockNumber>
where
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
	<T as pallet_omnipool::Config>::AssetId: Into<u32>,
	<T as crate::pallet::Config>::Asset: From<u32>,
	<T as crate::pallet::Config>::Asset: Into<u32>,
	<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
	<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>,
{
	let schedule1: Schedule<T::Asset, T::BlockNumber> = Schedule {
		period: 3u32.into(),
		recurrence,
		order: Order::Buy {
			asset_in: asset_in,
			asset_out: 3u32.into(),
			amount_out: ONE,
			max_limit: Balance::MAX,
			route: create_bounded_vec::<T>(vec![]),
		},
	};
	schedule1
}
pub fn create_bounded_vec<T: Config>(trades: Vec<Trade<T::Asset>>) -> BoundedVec<Trade<T::Asset>, ConstU32<5>> {
	let bounded_vec: BoundedVec<Trade<T::Asset>, sp_runtime::traits::ConstU32<5>> = trades.try_into().unwrap();
	bounded_vec
}

const TVL_CAP: Balance = 222_222_000_000_000_000_000_000;
type AssetIdOf<T> = <T as pallet_omnipool::Config>::AssetId;
type CurrencyOf<T> = <T as pallet_omnipool::Config>::Currency;
type OmnipoolPallet<T> = pallet_omnipool::Pallet<T>;

fn prepare_omnipool<T: pallet_omnipool::Config>() -> Result<(AssetIdOf<T>, AssetIdOf<T>, AssetIdOf<T>), DispatchError>
where
	CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
	T: crate::pallet::Config,
	<T as pallet_omnipool::Config>::AssetId: From<u32>,
{
	let stable_amount: Balance = 1_000_000_000_000_000u128;
	let native_amount: Balance = 1_000_000_000_000_000u128;
	let stable_price: FixedU128 = FixedU128::from((1, 2));
	let native_price: FixedU128 = FixedU128::from(1);
	let acc = OmnipoolPallet::<T>::protocol_account();

	//OmnipoolPallet::<T>::set_tvl_cap(RawOrigin::Root.into(), TVL_CAP).unwrap();

	CurrencyOf::<T>::update_balance(T::StableCoinAssetId::get(), &acc, stable_amount as i128).unwrap();
	CurrencyOf::<T>::update_balance(T::HdxAssetId::get(), &acc, native_amount as i128).unwrap();

	OmnipoolPallet::<T>::initialize_pool(
		RawOrigin::Root.into(),
		stable_price,
		native_price,
		Permill::from_percent(100),
		Permill::from_percent(100),
	)
	.unwrap();

	// Register new asset in asset registry
	let asset_a =
		<T as pallet_omnipool::Config>::AssetRegistry::create_asset(&b"FCK".to_vec(), Balance::from(1_000u32)).unwrap();
	let asset_b =
		<T as pallet_omnipool::Config>::AssetRegistry::create_asset(&b"FCK2".to_vec(), Balance::from(1_000u32))
			.unwrap();
	let share_asset =
		<T as pallet_omnipool::Config>::AssetRegistry::create_asset(&b"SHR".to_vec(), Balance::from(1_000u32)).unwrap();

	// Create account for token provider and set balance
	let owner: T::AccountId = account("owner", 0, 1);

	let token_price: FixedU128 = FixedU128::from((1, 5));
	let token_amount = 200_000_000_000_000u128;

	CurrencyOf::<T>::update_balance(asset_a, &acc, token_amount as i128).unwrap();
	CurrencyOf::<T>::update_balance(asset_b, &acc, token_amount as i128).unwrap();

	// Add the token to the pool
	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		asset_a,
		token_price,
		Permill::from_percent(100),
		owner.clone(),
	)
	.unwrap();
	OmnipoolPallet::<T>::add_token(
		RawOrigin::Root.into(),
		asset_b,
		token_price,
		Permill::from_percent(100),
		owner,
	)
	.unwrap();

	Ok((asset_a, asset_b, share_asset))
}

benchmarks! {
	 where_clause {  where
		CurrencyOf<T>: MultiCurrencyExtended<T::AccountId, Amount = i128>,
		T: crate::pallet::Config,
		<T as pallet_omnipool::Config>::AssetId: From<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<u32>,
		<T as crate::pallet::Config>::Asset: From<u32>,
		<T as crate::pallet::Config>::Asset: Into<u32>,
		<T as pallet_omnipool::Config>::AssetId: Into<<T as crate::pallet::Config>::Asset>,
		<T as pallet_omnipool::Config>::AssetId: From<<T as crate::pallet::Config>::Asset>
	}

	//TODO:
	//Ask lumir - do we want to pay
	//check why schedule benchmark fail.
	//then write benchmark for all
	//and then do the rest of the comments

	//WeifhTOFee::weight_to_fee(FeeForExecution)

	//use maxencodedlen and deposit function together

	/*execution_bond{
		let caller: T::AccountId = account("provider", 1, 1);
		let (asset_a, asset_b, share_asset) = prepare_omnipool::<T>()?;
		let token_amount = 200 * ONE;
		T::Currency::update_balance(0.into(), &caller, token_amount as i128)?;

		let schedule1 = schedule_fake::<T>(asset_a.into(), asset_b.into(), ONE, Recurrence::Fixed(5));
		let exeuction_block = 100u32;
		assert_ok!(crate::Pallet::<T>::schedule(RawOrigin::Signed(caller.clone()).into(), schedule1, Option::Some(exeuction_block.into())));

		let s = <ScheduleIdsPerBlock<T>>::get::<BlockNumberFor<T>>(exeuction_block.into());
		assert!(s.is_some());
		let mut execution_result = DcaExecutionResult::UnexpectedlyFailed;
	}: {
		let mut weight = 0u64;
		execution_result = crate::Pallet::<T>::execute_schedule(exeuction_block.into(), &mut weight, 1);
	}
	verify {
		assert_eq!(execution_result, DcaExecutionResult::Success)
	}*/

	schedule{
		let (asset_a, asset_b, share_asset) = prepare_omnipool::<T>()?;
		let caller: T::AccountId = account("provider", 1, 1);
		let token_amount = 200 * ONE;
		T::Currency::update_balance(0.into(), &caller, token_amount as i128)?;
		let schedule1 = schedule_fake::<T>(asset_a.into(), asset_b.into(), ONE, Recurrence::Fixed(5));

		T::Currency::update_balance(0.into(), &caller, ONE as i128)?;

	}: _(RawOrigin::Signed(caller.clone()), schedule1, Option::None)
	verify {

	}


}

#[cfg(test)]
mod tests {
	use super::Pallet;
	use crate::tests::mock::*;
	use frame_benchmarking::benchmarks;
	use frame_benchmarking::impl_benchmark_test_suite;
	use frame_support::assert_ok;

	impl_benchmark_test_suite!(
		Pallet,
		super::ExtBuilder::default()
			.with_registered_asset(0)
			.with_registered_asset(1)
			.with_registered_asset(2)
			.build(),
		super::Test
	);

	/*#[test]
	fn test_benchmarks() {
		ExtBuilder::default()
			.with_registered_asset(0)
			.with_registered_asset(1)
			.with_registered_asset(2)
			.build()
			.execute_with(|| {
				assert_ok!(Pallet::<Test>::test_benchmark_execution_bond());
			});
	}*/
}
