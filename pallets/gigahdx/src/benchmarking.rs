// SPDX-License-Identifier: Apache-2.0
//
// Benchmarks for `pallet-gigahdx`. The runtime swaps `Config::MoneyMarket`
// to a no-op `BenchmarkMoneyMarket` under `runtime-benchmarks` so the
// measurements capture only the substrate-side bookkeeping cost, not the
// EVM round-trip into AAVE.

use super::*;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use primitives::{Balance, EvmAddress};

// 1 HDX in raw units (assumes 12 decimals as in primitives/src/constants.rs).
const ONE: Balance = 1_000_000_000_000;

#[benchmarks(where T: Config, T::NativeCurrency: Currency<T::AccountId, Balance = Balance>)]
mod benches {
	use super::*;

	/// Fund `who` with `amount` HDX via the lockable currency directly.
	fn fund<T: Config>(who: &T::AccountId, amount: Balance)
	where
		T::NativeCurrency: Currency<T::AccountId, Balance = Balance>,
	{
		let _ = T::NativeCurrency::deposit_creating(who, amount);
	}

	/// Set the AAVE pool address so `MoneyMarket::supply` doesn't bail out
	/// on the `pool not set` precondition. The benchmark MoneyMarket is a
	/// stub, so the address itself is not used — but the pallet's adapter
	/// reads it before delegating, so it must be `Some`.
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

		// Stakes record populated; lock active.
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

		// Existing stake to unstake from.
		assert_ok!(Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			stake_amount,
		));

		// Worst-case path is case 2 (payout > active → yield transferred from
		// gigapot). Pre-fund the gigapot so the rate is > 1 and the case-2
		// branch fires when the caller exits.
		let gigapot = Pallet::<T>::gigapot_account_id();
		fund::<T>(&gigapot, stake_amount.saturating_mul(2));

		// Unstake the full position.
		let stake = Stakes::<T>::get(&caller).expect("setup invariant");
		let unstake_amount = stake.gigahdx;

		#[extrinsic_call]
		giga_unstake(RawOrigin::Signed(caller.clone()), unstake_amount);

		// Pending position created; gigahdx zeroed.
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

		// Fast-forward to the position's expiry.
		let position = PendingUnstakes::<T>::get(&caller).expect("position created");
		frame_system::Pallet::<T>::set_block_number(position.expires_at);

		#[extrinsic_call]
		unlock(RawOrigin::Signed(caller.clone()));

		assert!(PendingUnstakes::<T>::get(&caller).is_none());
	}

	#[benchmark]
	fn set_pool_contract() {
		// Precondition: TotalLocked == 0. Clean state — no setup needed.
		let new_pool = EvmAddress::from([0xBBu8; 20]);

		#[extrinsic_call]
		set_pool_contract(RawOrigin::Root, new_pool);

		assert_eq!(GigaHdxPoolContract::<T>::get(), Some(new_pool));
	}
}
