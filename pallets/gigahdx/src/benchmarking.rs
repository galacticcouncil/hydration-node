// SPDX-License-Identifier: Apache-2.0

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_support::traits::Currency;
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
		let stake_amount: Balance = 100 * ONE;
		fund::<T>(&caller, stake_amount.saturating_mul(10));

		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));

		// Worst case: payout > active → yield transferred from gigapot.
		// Pre-fund the gigapot so the rate is > 1.
		let gigapot = Pallet::<T>::gigapot_account_id();
		fund::<T>(&gigapot, stake_amount.saturating_mul(2));

		let stake = Stakes::<T>::get(&caller).expect("setup invariant");
		let unstake_amount = stake.gigahdx;

		#[extrinsic_call]
		giga_unstake(RawOrigin::Signed(caller.clone()), unstake_amount);

		assert!(PendingUnstakes::<T>::get(&caller).is_some());
		assert_eq!(Stakes::<T>::get(&caller).map(|s| s.gigahdx).unwrap_or_default(), 0);
	}

	#[benchmark]
	fn unlock() {
		assert_ok!(T::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();
		let stake_amount: Balance = 100 * ONE;
		fund::<T>(&caller, stake_amount.saturating_mul(10));

		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));
		assert_ok!(Pallet::<T>::giga_unstake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));

		let position = PendingUnstakes::<T>::get(&caller).expect("position created");
		frame_system::Pallet::<T>::set_block_number(position.expires_at);

		#[extrinsic_call]
		unlock(RawOrigin::Signed(caller.clone()));

		assert!(PendingUnstakes::<T>::get(&caller).is_none());
	}

	#[benchmark]
	fn set_pool_contract() {
		let new_pool = EvmAddress::from([0xBBu8; 20]);

		#[extrinsic_call]
		set_pool_contract(RawOrigin::Root, new_pool);

		assert_eq!(GigaHdxPoolContract::<T>::get(), Some(new_pool));
	}
}
