// This file is part of https://github.com/galacticcouncil/hydration-node

// Copyright (C) 2025  Intergalactic, Limited (GIB).
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

//! # pallet-gigahdx
//!
//! Liquid-staking primitive on top of an EVM money market.
//!
//! Users `giga_stake` HDX, which:
//!  1. Locks the HDX in the user's own account under [`Config::LockId`]
//!     (so it remains voteable via `pallet-conviction-voting`'s
//!     `LockableCurrency::max` lock semantics).
//!  2. Mints stHDX to the pallet's gigapot account.
//!  3. Calls [`MoneyMarketOperations::supply`] which deposits the stHDX
//!     into the money market and mints GIGAHDX (aToken) to the user.
//!
//! `giga_unstake` is the reverse path; see `specs/07-gigahdx-implementation-spec.md`
//! and `specs/09-gigahdx-money-market-adapter.md`.

#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[cfg(test)]
mod tests;

#[cfg(feature = "runtime-benchmarks")]
pub mod benchmarking;

pub mod weights;

/// Hook for benchmark setup — wires runtime-side helpers (asset registry
/// registration, etc.) that the pallet itself can't perform via its
/// extrinsics. Mirror of `pallet_dispenser::BenchmarkHelper`.
#[cfg(feature = "runtime-benchmarks")]
pub trait BenchmarkHelper<AccountId> {
	/// Register the stHDX asset so subsequent `mint_into` calls succeed.
	/// Must be idempotent — benchmarks may invoke this multiple times.
	fn register_assets() -> sp_runtime::DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
	fn register_assets() -> sp_runtime::DispatchResult {
		Ok(())
	}
}

#[frame_support::pallet]
pub mod pallet {
	pub use crate::weights::WeightInfo;
	use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use frame_support::sp_runtime::traits::{AccountIdConversion, CheckedAdd};
	use frame_support::sp_runtime::{ArithmeticError, Rounding};
	use frame_support::traits::fungibles::Mutate as FungiblesMutate;
	use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
	use frame_support::traits::{
		fungible, fungibles, Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, WithdrawReasons,
	};
	use frame_support::{transactional, PalletId};
	use frame_system::pallet_prelude::*;
	use hydra_dx_math::ratio::Ratio;
	pub use hydradx_traits::gigahdx::MoneyMarketOperations;
	use primitives::{AssetId, Balance, EvmAddress};
	use scale_info::TypeInfo;

	/// Per-account stake record.
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug, Default)]
	pub struct StakeRecord {
		/// HDX locked in this account under `Config::LockId` representing
		/// active stake principal. On unstake this is reduced by the **payout**
		/// (current HDX value of unstaked stHDX) up to its current value;
		/// any excess comes from the gigapot as yield.
		pub hdx: Balance,
		/// aToken (GIGAHDX) units this account's stake backs.
		///
		/// Stored as the value returned by [`MoneyMarketOperations::supply`],
		/// not the input — the MM may round at supply time and the stored
		/// value MUST match the account's GIGAHDX balance.
		pub gigahdx: Balance,
	}

	/// Pending-unstake record. At most one per account at a time.
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
	pub struct PendingUnstake<BlockNumber> {
		/// HDX value to release on `unlock`. Equals the unstake payout
		/// (principal share consumed + yield received from gigapot).
		pub amount: Balance,
		/// Block at which `unlock` becomes callable.
		pub expires_at: BlockNumber,
	}

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		/// Native (HDX) lockable currency. The `fungible::Inspect` bound is
		/// required so `giga_stake` can use `reducible_balance` (free balance
		/// minus transfer-blocking locks) instead of raw `free_balance`.
		type NativeCurrency: LockableCurrency<Self::AccountId, Balance = Balance, Moment = BlockNumberFor<Self>>
			+ fungible::Inspect<Self::AccountId, Balance = Balance>;

		/// Multi-asset register that holds stHDX (and any other registered
		/// fungible). Only this pallet mints / burns stHDX through it.
		type MultiCurrency: fungibles::Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>
			+ fungibles::Inspect<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// stHDX asset id.
		#[pallet::constant]
		type StHdxAssetId: Get<AssetId>;

		/// Money-market adapter.
		type MoneyMarket: MoneyMarketOperations<Self::AccountId, AssetId, Balance>;

		/// Origin allowed to set the pool contract address.
		type AuthorityOrigin: EnsureOrigin<Self::RuntimeOrigin>;

		/// Pallet account id used as the gigapot (yield) account. Derived
		/// via `PalletId::into_account_truncating`.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// LockIdentifier under which user HDX (active stake + pending
		/// unstake) is locked.
		#[pallet::constant]
		type LockId: Get<LockIdentifier>;

		/// Minimum HDX that can be staked in one call (anti-dust).
		#[pallet::constant]
		type MinStake: Get<Balance>;

		/// Cooldown period (in blocks) between `giga_unstake` and the
		/// matching `unlock` call.
		#[pallet::constant]
		type CooldownPeriod: Get<BlockNumberFor<Self>>;

		type WeightInfo: WeightInfo;

		/// Benchmark helper for setting up state that can't be created from
		/// the pallet's public API alone — primarily registering the stHDX
		/// asset in the asset registry so `MultiCurrency::mint_into` succeeds.
		#[cfg(feature = "runtime-benchmarks")]
		type BenchmarkHelper: crate::BenchmarkHelper<Self::AccountId>;
	}

	/// Per-account stake record. Absent if the account has never staked or
	/// has fully unstaked.
	#[pallet::storage]
	pub type Stakes<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, StakeRecord, OptionQuery>;

	/// Sum of all `Stakes[a].hdx`.
	#[pallet::storage]
	pub type TotalLocked<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Aave V3 Pool contract address. Must be set explicitly by
	/// `AuthorityOrigin` before any stake/unstake — the pallet refuses to
	/// silently default to the zero address.
	#[pallet::storage]
	pub type GigaHdxPoolContract<T: Config> = StorageValue<_, EvmAddress, OptionQuery>;

	/// At most one pending unstake per account. A second `giga_unstake`
	/// while this slot is full is rejected — caller must wait for the
	/// cooldown and `unlock` first.
	#[pallet::storage]
	pub type PendingUnstakes<T: Config> =
		StorageMap<_, Blake2_128Concat, T::AccountId, PendingUnstake<BlockNumberFor<T>>, OptionQuery>;

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		Staked {
			who: T::AccountId,
			amount: Balance,
			gigahdx: Balance,
		},
		Unstaked {
			who: T::AccountId,
			gigahdx_amount: Balance,
			payout: Balance,
			yield_share: Balance,
			expires_at: BlockNumberFor<T>,
		},
		Unlocked {
			who: T::AccountId,
			amount: Balance,
		},
		PoolContractUpdated {
			contract: EvmAddress,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stake amount is below `Config::MinStake`.
		BelowMinStake,
		/// Caller does not have enough unlocked HDX to back this stake (the
		/// admission check uses `reducible_balance`, which subtracts every
		/// transfer-blocking lock — including this pallet's own lock).
		InsufficientFreeBalance,
		/// Unstake amount exceeds the caller's `Stakes.gigahdx`.
		InsufficientStake,
		/// Caller has no active stake record.
		NoStake,
		/// Amount must be strictly greater than zero.
		ZeroAmount,
		/// stHDX mint failed (asset not registered, max issuance hit, or
		/// other `fungibles::Mutate::mint_into` precondition violated).
		/// Distinct from `MoneyMarketSupplyFailed` — this is a substrate-side
		/// asset-registry error, not an AAVE-side revert.
		StHdxMintFailed,
		/// AAVE `Pool.supply` reverted — typical causes: caller's EVM address
		/// is not bound, the asset reserve is misconfigured, or `Pool` is
		/// paused.
		MoneyMarketSupplyFailed,
		/// AAVE `Pool.withdraw` reverted.
		MoneyMarketWithdrawFailed,
		/// Arithmetic overflow during rate, lock, or storage update math.
		Overflow,
		/// The cooldown period has not yet elapsed for the pending unstake.
		CooldownNotElapsed,
		/// No pending unstake exists for the caller.
		NoPendingUnstake,
		/// Caller already has a pending unstake; must `unlock` it first.
		PendingUnstakeAlreadyExists,
		/// `set_pool_contract` was called while gigahdx (aToken / stHDX) is
		/// still in circulation. The pool is settable only when total stHDX
		/// supply is zero.
		OutstandingStake,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock HDX in the caller's account, mint stHDX, and supply it to the money market.
		///
		/// The pallet locks `amount` HDX in the caller's account under `Config::LockId`
		/// (so it remains voteable via `LockableCurrency` semantics), mints stHDX at the
		/// current exchange rate, and supplies the stHDX to the money market. The money
		/// market mints GIGAHDX (aToken) to the caller's EVM-mapped address.
		///
		/// `Stakes[caller].gigahdx` records the **actual** aToken amount returned by the
		/// money market (may differ from the requested mint amount by rounding).
		///
		/// Fails with `BelowMinStake` if `amount < Config::MinStake`, with
		/// `InsufficientFreeBalance` if the caller's `reducible_balance` does not cover
		/// `amount`, with `StHdxMintFailed` if stHDX minting fails (asset not registered,
		/// max issuance hit), or with `MoneyMarketSupplyFailed` if the AAVE `Pool.supply`
		/// call reverts.
		///
		/// Parameters:
		/// - `amount`: HDX amount to stake. Must be at least `Config::MinStake`.
		///
		/// Emits `Staked` event when successful.
		///
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::giga_stake().saturating_add(T::MoneyMarket::supply_weight()))]
		pub fn giga_stake(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(amount >= T::MinStake::get(), Error::<T>::BelowMinStake);

			// Use `reducible_balance` so the check respects every transfer-blocking
			// lock — including this pallet's own combined `LockId` lock (active
			// stake + pending unstake) and any unrelated conviction/vesting locks.
			let usable = <T::NativeCurrency as fungible::Inspect<T::AccountId>>::reducible_balance(
				&who,
				Preservation::Expendable,
				Fortitude::Polite,
			);
			ensure!(usable >= amount, Error::<T>::InsufficientFreeBalance);

			let gigahdx_to_mint = Self::calculate_gigahdx_given_hdx_amount(amount).map_err(|_| Error::<T>::Overflow)?;
			// Defense in depth: real AAVE V3 reverts on `Pool.supply(0)`, but a
			// fork that accepted it would leave the user with HDX locked and
			// `Stakes.gigahdx == 0`, with no exit path via `giga_unstake`.
			ensure!(gigahdx_to_mint > 0, Error::<T>::ZeroAmount);

			T::MultiCurrency::mint_into(T::StHdxAssetId::get(), &who, gigahdx_to_mint)
				.map_err(|_| Error::<T>::StHdxMintFailed)?;
			let actual_minted = T::MoneyMarket::supply(&who, T::StHdxAssetId::get(), gigahdx_to_mint)
				.map_err(|_| Error::<T>::MoneyMarketSupplyFailed)?;

			Stakes::<T>::try_mutate(&who, |maybe_stake| -> Result<(), Error<T>> {
				let stake = maybe_stake.get_or_insert_with(StakeRecord::default);
				stake.hdx = stake.hdx.checked_add(amount).ok_or(Error::<T>::Overflow)?;
				stake.gigahdx = stake.gigahdx.checked_add(actual_minted).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			TotalLocked::<T>::mutate(|x| *x = x.saturating_add(amount));
			Self::refresh_lock(&who)?;

			Self::deposit_event(Event::Staked {
				who,
				amount,
				gigahdx: actual_minted,
			});
			Ok(())
		}

		/// Unstake the caller's GIGAHDX and open a pending-unstake position.
		///
		/// Burns `gigahdx_amount` of the caller's GIGAHDX through the money market, which
		/// returns stHDX to the caller; the pallet then burns that stHDX. The HDX value
		/// (current rate × `gigahdx_amount`) is moved into a single pending-unstake
		/// position with a cooldown of `Config::CooldownPeriod`. Any portion of the payout
		/// that exceeds the user's active stake principal is paid as yield from the
		/// gigapot account.
		///
		/// At most one pending position per account — the caller must `unlock` an existing
		/// position before calling again, otherwise `PendingUnstakeAlreadyExists` is
		/// returned.
		///
		/// Implementation detail (must match `LockableAToken.sol`): the lock-manager
		/// precompile (`0x0806`) reads `Stakes[who].gigahdx` and treats it as the user's
		/// locked GIGAHDX. The aToken contract rejects burns where
		/// `amount > balance - locked`, so the pallet **pre-decrements `gigahdx` by
		/// `gigahdx_amount` before the money-market call**. The dispatchable runs in a
		/// storage layer so any failure rolls back the pre-decrement atomically.
		///
		/// Fails with `NoStake` if the caller has no active stake, `ZeroAmount` if
		/// `gigahdx_amount == 0`, `InsufficientStake` if `gigahdx_amount` exceeds the
		/// caller's `Stakes.gigahdx`, or `MoneyMarketWithdrawFailed` if the AAVE
		/// `Pool.withdraw` call reverts.
		///
		/// Parameters:
		/// - `gigahdx_amount`: GIGAHDX (aToken) amount to unstake. Must be greater than
		///   zero and not exceed the caller's `Stakes.gigahdx`.
		///
		/// Emits `Unstaked` event when successful.
		///
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::giga_unstake().saturating_add(T::MoneyMarket::withdraw_weight()))]
		pub fn giga_unstake(origin: OriginFor<T>, gigahdx_amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_giga_unstake(&who, gigahdx_amount)
		}

		/// Set the AAVE V3 Pool contract address used by the money-market adapter.
		///
		/// Refuses to swap the pool while gigahdx (aToken / stHDX) is still in
		/// circulation — a swap mid-flight would route subsequent `giga_unstake`
		/// calls to a pool that doesn't hold the user's atokens, reverting the
		/// burn and leaving HDX permanently locked. Returns `OutstandingStake`
		/// when total stHDX supply is non-zero.
		///
		/// Note: it is not enough to check `TotalLocked == 0` (the sum of
		/// `Stakes.hdx`). After a case-2 partial unstake the user's active
		/// stake can be drained while their `Stakes.gigahdx` (and the
		/// corresponding aToken balance) is still non-zero — those tokens
		/// remain bound to the current pool.
		///
		/// Parameters:
		/// - `origin`: Must be `T::AuthorityOrigin`.
		/// - `contract`: H160 address of the new AAVE V3 Pool contract.
		///
		/// Emits `PoolContractUpdated` event when successful.
		///
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::set_pool_contract())]
		pub fn set_pool_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResult {
			T::AuthorityOrigin::ensure_origin(origin)?;
			ensure!(Self::total_gigahdx_supply() == 0, Error::<T>::OutstandingStake);
			GigaHdxPoolContract::<T>::put(contract);
			Self::deposit_event(Event::PoolContractUpdated { contract });
			Ok(())
		}

		/// Release the caller's pending-unstake position after the cooldown elapses.
		///
		/// Removes the `PendingUnstakes` entry for the caller and refreshes the combined
		/// `Config::LockId` lock so the unstaked HDX becomes spendable. If the matching
		/// `Stakes` record has been fully drained (both `hdx` and `gigahdx` are zero), it
		/// is also removed.
		///
		/// Fails with `NoPendingUnstake` if the caller has no pending position, or with
		/// `CooldownNotElapsed` if `Config::CooldownPeriod` has not yet passed since the
		/// position was opened.
		///
		/// Emits `Unlocked` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::unlock())]
		pub fn unlock(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let entry = PendingUnstakes::<T>::get(&who).ok_or(Error::<T>::NoPendingUnstake)?;
			ensure!(
				frame_system::Pallet::<T>::block_number() >= entry.expires_at,
				Error::<T>::CooldownNotElapsed
			);

			PendingUnstakes::<T>::remove(&who);
			Self::refresh_lock(&who)?;
			if let Some(s) = Stakes::<T>::get(&who) {
				if s.hdx == 0 && s.gigahdx == 0 {
					Stakes::<T>::remove(&who);
				}
			}

			Self::deposit_event(Event::Unlocked {
				who,
				amount: entry.amount,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Internal helper for `giga_unstake`. Uses `?` freely; the
		/// `#[transactional]` attribute wraps the body in its own storage
		/// layer so any Err here rolls back partial mutations.
		#[transactional]
		fn do_giga_unstake(who: &T::AccountId, gigahdx_amount: Balance) -> DispatchResult {
			ensure!(
				PendingUnstakes::<T>::get(who).is_none(),
				Error::<T>::PendingUnstakeAlreadyExists
			);

			let stake = Stakes::<T>::get(who).ok_or(Error::<T>::NoStake)?;
			ensure!(gigahdx_amount > 0, Error::<T>::ZeroAmount);
			ensure!(gigahdx_amount <= stake.gigahdx, Error::<T>::InsufficientStake);

			// Payout reads live rate state — must run before any mint/burn below.
			let payout = Self::calculate_hdx_amount_given_gigahdx(gigahdx_amount).map_err(|_| Error::<T>::Overflow)?;

			// Pre-decrement `gigahdx` so `LockableAToken.burn`'s `freeBalance`
			// check (via the lock-manager precompile) lets the burn through.
			let new_gigahdx = stake.gigahdx.checked_sub(gigahdx_amount).ok_or(Error::<T>::Overflow)?;
			Stakes::<T>::insert(
				who,
				StakeRecord {
					hdx: stake.hdx,
					gigahdx: new_gigahdx,
				},
			);

			T::MoneyMarket::withdraw(who, T::StHdxAssetId::get(), gigahdx_amount)
				.map_err(|_| Error::<T>::MoneyMarketWithdrawFailed)?;

			T::MultiCurrency::burn_from(
				T::StHdxAssetId::get(),
				who,
				gigahdx_amount,
				Preservation::Expendable,
				Precision::Exact,
				Fortitude::Force,
			)?;

			// payout ≤ active → consume from active only;
			// payout > active → drain active, pull remainder from gigapot as yield.
			let (new_hdx, yield_share) = if payout <= stake.hdx {
				(stake.hdx - payout, 0)
			} else {
				let yield_amount = payout - stake.hdx;
				T::NativeCurrency::transfer(
					&Self::gigapot_account_id(),
					who,
					yield_amount,
					ExistenceRequirement::AllowDeath,
				)?;
				(0, yield_amount)
			};
			let principal_consumed = stake.hdx.saturating_sub(new_hdx);

			// Only `hdx` changes here; `gigahdx` was already pre-decremented
			// before the MM call and must stay at that value.
			Stakes::<T>::mutate(who, |maybe| {
				if let Some(s) = maybe {
					s.hdx = new_hdx;
				}
			});
			TotalLocked::<T>::mutate(|x| *x = x.saturating_sub(principal_consumed));

			let expires_at = frame_system::Pallet::<T>::block_number()
				.checked_add(&T::CooldownPeriod::get())
				.ok_or(Error::<T>::Overflow)?;
			PendingUnstakes::<T>::insert(
				who,
				PendingUnstake {
					amount: payout,
					expires_at,
				},
			);
			Self::refresh_lock(who)?;

			Self::deposit_event(Event::Unstaked {
				who: who.clone(),
				gigahdx_amount,
				payout,
				yield_share,
				expires_at,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Recompute the single combined balance lock for `who`:
		/// `lock_amount = Stakes[who].hdx + PendingUnstakes[who].amount`.
		/// Uses `set_lock` (not `extend_lock`) so the lock can shrink on unstake
		/// or unlock. Removes the lock entirely when both components are zero.
		#[transactional]
		fn refresh_lock(who: &T::AccountId) -> DispatchResult {
			let stake_amount = Stakes::<T>::get(who).map(|s| s.hdx).unwrap_or(0);
			let pending = PendingUnstakes::<T>::get(who).map(|p| p.amount).unwrap_or(0);
			let total = stake_amount.saturating_add(pending);
			if total == 0 {
				T::NativeCurrency::remove_lock(T::LockId::get(), who);
			} else {
				T::NativeCurrency::set_lock(T::LockId::get(), who, total, WithdrawReasons::all());
			}
			Ok(())
		}

		/// Account id of the gigapot (yield holder), derived from
		/// `Config::PalletId`.
		pub fn gigapot_account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// GIGAHDX (aToken) units backed by an active stake for `who`. Read by
		/// the lock-manager precompile to enforce `LockableAToken`'s
		/// `freeBalance = balance - locked` invariant on the EVM side.
		///
		/// Returns 0 when the account has no record. Equals the user's atoken
		/// balance while staked; pre-decremented by `giga_unstake` before the
		/// MM withdraw call so the burn passes that invariant.
		pub fn locked_gigahdx(who: &T::AccountId) -> Balance {
			Stakes::<T>::get(who).map(|s| s.gigahdx).unwrap_or(0)
		}

		/// Total HDX backing all stHDX:
		/// `TotalLocked + free_balance(gigapot_account_id)`.
		pub fn total_staked_hdx() -> Balance {
			TotalLocked::<T>::get().saturating_add(T::NativeCurrency::free_balance(&Self::gigapot_account_id()))
		}

		/// Total stHDX issued, read live from the asset registry — no pallet-side
		/// counter to keep in sync.
		pub fn total_gigahdx_supply() -> Balance {
			<T::MultiCurrency as fungibles::Inspect<T::AccountId>>::total_issuance(T::StHdxAssetId::get())
		}

		/// HDX/GIGAHDX exchange rate as `Ratio { n: total_staked_hdx, d: total_gigahdx_supply }`,
		/// floored at 1.0.
		///
		/// stHDX accrues HDX value monotonically under user flows, so a sub-1
		/// rate is only reachable via privileged operations (root drain from
		/// the gigapot) or migration bugs. The floor protects users and
		/// downstream consumers (e.g. AAVE oracle reads) from a transient
		/// sub-1 reading without leaking the artefact across pricing math.
		///
		/// Compare ratios with `cmp` / `partial_cmp` — those do proper
		/// cross-multiplication. Direct field-wise `==` only works when `n`
		/// and `d` happen to match exactly.
		pub fn exchange_rate() -> Ratio {
			let gigahdx_supply = Self::total_gigahdx_supply();
			if gigahdx_supply == 0 {
				return Ratio::one();
			}
			let raw = Ratio::new(Self::total_staked_hdx(), gigahdx_supply);
			core::cmp::max(raw, Ratio::one())
		}

		/// GIGAHDX (= stHDX) to mint for a given HDX `amount` at the current rate.
		///
		/// Bootstrap (no GIGAHDX in circulation) returns `amount` 1:1.
		pub fn calculate_gigahdx_given_hdx_amount(amount: Balance) -> Result<Balance, ArithmeticError> {
			let rate = Self::exchange_rate();
			multiply_by_rational_with_rounding(amount, rate.d, rate.n, Rounding::Down).ok_or(ArithmeticError::Overflow)
		}

		/// HDX paid out for unstaking `gigahdx_amount` of GIGAHDX/stHDX at the
		/// current rate.
		pub fn calculate_hdx_amount_given_gigahdx(gigahdx_amount: Balance) -> Result<Balance, ArithmeticError> {
			let rate = Self::exchange_rate();
			multiply_by_rational_with_rounding(gigahdx_amount, rate.n, rate.d, Rounding::Down)
				.ok_or(ArithmeticError::Overflow)
		}
	}
}
