// SPDX-License-Identifier: Apache-2.0

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_support::sp_runtime::traits::Saturating;
use frame_support::traits::{Currency, Get};
use frame_system::pallet_prelude::BlockNumberFor;
use frame_system::RawOrigin;
use primitives::{Balance, EvmAddress};

const ONE: Balance = 1_000_000_000_000;

#[benchmarks(where T: Config, T::NativeCurrency: Currency<T::AccountId, Balance = Balance>)]
mod benches {
	use super::*;

	fn fund<T: Config>(who: &T::AccountId, amount: Balance)
	where
		T::NativeCurrency: Currency<T::AccountId, Balance = Balance>,
	{
		let _ = T::NativeCurrency::deposit_creating(who, amount);
	}

	/// Set the AAVE pool address so the adapter's `pool not set` precondition
	/// passes. Address is unused by the stub `BenchmarkMoneyMarket`.
	fn set_dummy_pool<T: Config>() {
		GigaHdxPoolContract::<T>::put(EvmAddress::from([0xAAu8; 20]));
	}

	#[benchmark]
	fn giga_stake() {
		assert_ok!(T::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();
		let amount: Balance = 100 * ONE;
		fund::<T>(&caller, amount.saturating_mul(10));

		#[extrinsic_call]
		giga_stake(RawOrigin::Signed(caller.clone()), amount);

		let stake = Stakes::<T>::get(&caller).expect("stake recorded");
		assert_eq!(stake.hdx, amount);
		assert!(stake.gigahdx > 0);
	}

	#[benchmark]
	fn giga_unstake() {
		assert_ok!(T::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();
		let max: u32 = T::MaxPendingUnstakes::get();
		let per_unstake: Balance = ONE;
		let stake_amount: Balance = per_unstake.saturating_mul(max as Balance).saturating_mul(2);
		fund::<T>(&caller, stake_amount.saturating_mul(10));

		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));

		// Worst case: payout > active → yield transferred from gigapot.
		let gigapot = Pallet::<T>::gigapot_account_id();
		fund::<T>(&gigapot, stake_amount.saturating_mul(2));

		// Pre-populate MaxPendingUnstakes - 1 positions across distinct blocks so
		// the measured call hits the admission cap and the cached-counter update
		// at the per-account upper bound.
		for _ in 0..max.saturating_sub(1) {
			assert_ok!(Pallet::<T>::giga_unstake(
				RawOrigin::Signed(caller.clone()).into(),
				per_unstake,
			));
			let next = frame_system::Pallet::<T>::block_number().saturating_add(1u32.into());
			frame_system::Pallet::<T>::set_block_number(next);
		}
		let measured_amount = per_unstake;

		#[extrinsic_call]
		giga_unstake(RawOrigin::Signed(caller.clone()), measured_amount);

		let s = Stakes::<T>::get(&caller).expect("stake recorded");
		assert_eq!(s.unstaking_count as u32, max);
	}

	#[benchmark]
	fn unlock() {
		assert_ok!(T::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();
		let max: u32 = T::MaxPendingUnstakes::get();
		let per_unstake: Balance = ONE;
		let stake_amount: Balance = per_unstake.saturating_mul(max as Balance).saturating_mul(2);
		fund::<T>(&caller, stake_amount.saturating_mul(10));

		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));
		let mut target_id: BlockNumberFor<T> = frame_system::Pallet::<T>::block_number();
		for i in 0..max {
			let id = frame_system::Pallet::<T>::block_number();
			assert_ok!(Pallet::<T>::giga_unstake(
				RawOrigin::Signed(caller.clone()).into(),
				per_unstake,
			));
			if i == 0 {
				target_id = id;
			}
			let next = id.saturating_add(1u32.into());
			frame_system::Pallet::<T>::set_block_number(next);
		}

		let expires_at = target_id.saturating_add(T::CooldownPeriod::get());
		frame_system::Pallet::<T>::set_block_number(expires_at);

		#[extrinsic_call]
		unlock(RawOrigin::Signed(caller.clone()), target_id);

		let s = Stakes::<T>::get(&caller).expect("stake remains");
		assert_eq!(s.unstaking_count as u32, max - 1);
	}

	#[benchmark]
	fn set_pool_contract() {
		let new_pool = EvmAddress::from([0xBBu8; 20]);

		#[extrinsic_call]
		set_pool_contract(RawOrigin::Root, new_pool);

		assert_eq!(GigaHdxPoolContract::<T>::get(), Some(new_pool));
	}

	#[benchmark]
	fn cancel_unstake() {
		assert_ok!(T::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();
		let max: u32 = T::MaxPendingUnstakes::get();
		let per_unstake: Balance = ONE;
		let stake_amount: Balance = per_unstake.saturating_mul(max as Balance).saturating_mul(2);
		fund::<T>(&caller, stake_amount.saturating_mul(10));

		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));

		// Worst case: yield was paid → cancel folds principal + yield back.
		let gigapot = Pallet::<T>::gigapot_account_id();
		fund::<T>(&gigapot, stake_amount.saturating_mul(2));

		let mut target_id: BlockNumberFor<T> = frame_system::Pallet::<T>::block_number();
		for i in 0..max {
			let id = frame_system::Pallet::<T>::block_number();
			assert_ok!(Pallet::<T>::giga_unstake(
				RawOrigin::Signed(caller.clone()).into(),
				per_unstake,
			));
			if i + 1 == max {
				target_id = id;
			}
			let next = id.saturating_add(1u32.into());
			frame_system::Pallet::<T>::set_block_number(next);
		}

		#[extrinsic_call]
		cancel_unstake(RawOrigin::Signed(caller.clone()), target_id);

		let s = Stakes::<T>::get(&caller).expect("stake remains");
		assert_eq!(s.unstaking_count as u32, max - 1);
		assert!(s.hdx > 0);
	}
}
