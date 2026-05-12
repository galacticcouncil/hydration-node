// SPDX-License-Identifier: Apache-2.0

//! Benchmarks for `pallet-gigahdx-rewards`.
//!
//! Setup mirrors `pallet-gigahdx`'s benchmark style (v2 macro). For
//! `claim_rewards` the worst-case path is: non-zero `PendingRewards[who]`
//! plus an existing `Stakes[who]` entry so the compound path inside
//! `pallet_gigahdx::do_stake` walks the full storage update
//! (mint stHDX, money-market supply, mutate `Stakes`, bump `TotalLocked`,
//! refresh the balance lock).

use super::*;
use crate::pallet::PendingRewards;
use frame_benchmarking::v2::*;
use frame_support::assert_ok;
use frame_support::traits::Currency;
use frame_system::RawOrigin;
use pallet_gigahdx::BenchmarkHelper as _;
use primitives::{Balance, EvmAddress};

const ONE: Balance = 1_000_000_000_000;

#[benchmarks(where T: Config, <T as pallet_gigahdx::Config>::NativeCurrency: Currency<T::AccountId, Balance = Balance>)]
mod benches {
	use super::*;

	fn fund<T: Config>(who: &T::AccountId, amount: Balance)
	where
		<T as pallet_gigahdx::Config>::NativeCurrency: Currency<T::AccountId, Balance = Balance>,
	{
		let _ = <T as pallet_gigahdx::Config>::NativeCurrency::deposit_creating(who, amount);
	}

	/// Set a dummy AAVE pool address so `pallet-gigahdx`'s `pool not set`
	/// precondition passes. The runtime's `BenchmarkMoneyMarket` does not
	/// dispatch to it.
	fn set_dummy_pool<T: Config>() {
		pallet_gigahdx::GigaHdxPoolContract::<T>::put(EvmAddress::from([0xAAu8; 20]));
	}

	#[benchmark]
	fn claim_rewards() {
		// Register stHDX via the gigahdx-side benchmark helper. `Config` for
		// rewards extends `pallet_gigahdx::Config`, so the helper trait is
		// accessible as `<T as pallet_gigahdx::Config>::BenchmarkHelper`.
		assert_ok!(<T as pallet_gigahdx::Config>::BenchmarkHelper::register_assets());
		set_dummy_pool::<T>();

		let caller: T::AccountId = whitelisted_caller();

		// Endow the caller heavily so they can `giga_stake` (which moves HDX
		// into the lock) and still cover the post-claim free-balance state.
		let initial_stake: Balance = 1_000 * ONE;
		fund::<T>(&caller, initial_stake.saturating_mul(10));

		// Build an existing stake position so the compound path inside
		// `do_stake` hits the full mutate-existing-record code branch.
		assert_ok!(pallet_gigahdx::Pallet::<T>::giga_stake(
			RawOrigin::Signed(caller.clone()).into(),
			initial_stake,
		));

		// Fund the allocated-rewards pot so the payout transfer succeeds.
		let reward: Balance = 1_000 * ONE;
		fund::<T>(&Pallet::<T>::allocated_rewards_pot(), reward.saturating_mul(2));

		// Seed pending rewards: this is the value drained by `take(&who)`.
		PendingRewards::<T>::insert(&caller, reward);

		#[extrinsic_call]
		claim_rewards(RawOrigin::Signed(caller.clone()));

		// Pending was drained.
		assert_eq!(PendingRewards::<T>::get(&caller), 0);
		// Stake grew by the claimed amount.
		let stake = pallet_gigahdx::Stakes::<T>::get(&caller).expect("stake recorded");
		assert!(stake.hdx >= initial_stake.saturating_add(reward));
		assert!(stake.gigahdx > 0);
	}
}
