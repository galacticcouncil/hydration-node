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

pub mod math;
pub mod weights;

#[frame_support::pallet]
pub mod pallet {
	pub use crate::weights::WeightInfo;
	use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::traits::{AccountIdConversion, CheckedAdd};
	use frame_support::sp_runtime::{FixedPointNumber, FixedU128};
	use frame_support::storage::{with_transaction, TransactionOutcome};
	use frame_support::traits::fungibles::Mutate as FungiblesMutate;
	use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
	use frame_support::traits::{
		fungible, fungibles, Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, WithdrawReasons,
	};
	use frame_support::PalletId;
	use frame_system::pallet_prelude::*;
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
		pub hdx_locked: Balance,
		/// aToken (GIGAHDX) units this account's stake backs.
		///
		/// Stored as the value returned by [`MoneyMarketOperations::supply`],
		/// not the input — the MM may round at supply time and the stored
		/// value MUST match the account's GIGAHDX balance.
		pub st_minted: Balance,
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

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		/// HDX lockable currency. The `fungible::Inspect` bound is required so
		/// `giga_stake` can use `reducible_balance` (free balance minus
		/// transfer-blocking locks) instead of raw `free_balance`.
		type Currency: LockableCurrency<Self::AccountId, Balance = Balance, Moment = BlockNumberFor<Self>>
			+ fungible::Inspect<Self::AccountId, Balance = Balance>;

		/// stHDX is a multi-asset-registry fungible token. Only this pallet
		/// mints / burns it.
		type StHdx: fungibles::Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>
			+ fungibles::Inspect<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		/// stHDX asset id.
		#[pallet::constant]
		type StHdxAssetId: Get<AssetId>;

		/// Money-market adapter.
		type MoneyMarket: MoneyMarketOperations<Self::AccountId, AssetId, Balance>;

		/// Origin allowed to set the pool contract address.
		type GovernanceOrigin: EnsureOrigin<Self::RuntimeOrigin>;

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
	}

	/// Per-account stake record. Absent if the account has never staked or
	/// has fully unstaked.
	#[pallet::storage]
	pub type Stakes<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, StakeRecord, OptionQuery>;

	/// Sum of all `Stakes[a].hdx_locked`.
	#[pallet::storage]
	pub type TotalLocked<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Total stHDX issued.
	#[pallet::storage]
	pub type TotalStHdx<T: Config> = StorageValue<_, Balance, ValueQuery>;

	/// Aave V3 Pool contract address. Settable by `GovernanceOrigin`.
	#[pallet::storage]
	pub type GigaHdxPoolContract<T: Config> = StorageValue<_, EvmAddress, ValueQuery>;

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
			st_minted: Balance,
		},
		Unstaked {
			who: T::AccountId,
			st_amount: Balance,
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
		BelowMinStake,
		InsufficientFreeBalance,
		InsufficientStake,
		NoStake,
		ZeroAmount,
		MoneyMarketSupplyFailed,
		MoneyMarketWithdrawFailed,
		Overflow,
		/// The cooldown period has not yet elapsed for the pending unstake.
		CooldownNotElapsed,
		/// No pending unstake exists for the caller.
		NoPendingUnstake,
		/// Caller already has a pending unstake; must `unlock` it first.
		PendingUnstakeAlreadyExists,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Lock `amount` HDX in the caller's account, mint stHDX to the caller,
		/// and supply it to the money market. The MM mints GIGAHDX (aToken)
		/// to the caller's EVM-mapped address.
		///
		/// `Stakes[caller].st_minted` records the **actual** aToken amount
		/// returned by the MM (may differ from input by rounding).
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::giga_stake())]
		pub fn giga_stake(origin: OriginFor<T>, amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(amount >= T::MinStake::get(), Error::<T>::BelowMinStake);

			// Use `reducible_balance` so the check respects every transfer-blocking
			// lock — including this pallet's own combined `LockId` lock (active
			// stake + pending unstake) and any unrelated conviction/vesting locks.
			let usable = <T::Currency as fungible::Inspect<T::AccountId>>::reducible_balance(
				&who,
				Preservation::Expendable,
				Fortitude::Polite,
			);
			ensure!(usable >= amount, Error::<T>::InsufficientFreeBalance);

			// Compute stHDX to mint based on current rate.
			let s = TotalStHdx::<T>::get();
			let t = Self::total_hdx();
			let st_input = crate::math::st_input_for_stake(amount, s, t).map_err(|_| Error::<T>::Overflow)?;

			// Mint stHDX to caller, then supply to MM. Wrapped in `with_transaction`
			// so that if MM supply fails, the freshly-minted stHDX rolls back —
			// no orphaned stHDX on the user.
			let actual_minted = with_transaction(|| -> TransactionOutcome<Result<Balance, DispatchError>> {
				if let Err(e) = T::StHdx::mint_into(T::StHdxAssetId::get(), &who, st_input) {
					return TransactionOutcome::Rollback(Err(e));
				}
				match T::MoneyMarket::supply(&who, T::StHdxAssetId::get(), st_input) {
					Ok(actual) => TransactionOutcome::Commit(Ok(actual)),
					Err(e) => TransactionOutcome::Rollback(Err(e)),
				}
			})
			.map_err(|_| Error::<T>::MoneyMarketSupplyFailed)?;

			let prev = Stakes::<T>::get(&who).unwrap_or_default();
			let new_locked = prev.hdx_locked.checked_add(amount).ok_or(Error::<T>::Overflow)?;
			let new_minted = prev.st_minted.checked_add(actual_minted).ok_or(Error::<T>::Overflow)?;
			Stakes::<T>::insert(
				&who,
				StakeRecord {
					hdx_locked: new_locked,
					st_minted: new_minted,
				},
			);
			TotalLocked::<T>::mutate(|x| *x = x.saturating_add(amount));
			TotalStHdx::<T>::mutate(|x| *x = x.saturating_add(actual_minted));
			Self::refresh_lock(&who);

			Self::deposit_event(Event::Staked {
				who,
				amount,
				st_minted: actual_minted,
			});
			Ok(())
		}

		/// Set the AAVE V3 Pool contract H160 used by the money-market adapter.
		/// Gated by `GovernanceOrigin`.
		#[pallet::call_index(2)]
		#[pallet::weight(Weight::from_parts(10_000, 0))]
		pub fn set_pool_contract(origin: OriginFor<T>, contract: EvmAddress) -> DispatchResult {
			T::GovernanceOrigin::ensure_origin(origin)?;
			GigaHdxPoolContract::<T>::put(contract);
			Self::deposit_event(Event::PoolContractUpdated { contract });
			Ok(())
		}

		/// Unstake `st_amount` of the caller's GIGAHDX. The MM burns the
		/// aToken and returns stHDX to the caller, which the pallet then burns.
		/// The HDX value (current rate × st_amount) is moved into a single
		/// pending-unstake position; any portion that exceeds the user's
		/// active stake is paid as yield from the gigapot.
		///
		/// At most one pending position per account — caller must `unlock` an
		/// existing position before calling again.
		///
		/// Implementation detail (must match `LockableAToken.sol`):
		/// the lock-manager precompile (`0x0806`) reads `Stakes[who].st_minted`
		/// and treats it as the user's locked GIGAHDX. The aToken contract
		/// rejects burns where `amount > balance - locked`, so we
		/// **pre-decrement `st_minted` by `st_amount` before the MM call**.
		/// The whole body is wrapped in `with_transaction` for atomic rollback.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::giga_unstake())]
		pub fn giga_unstake(origin: OriginFor<T>, st_amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;
			with_transaction::<(), DispatchError, _>(|| {
				let outcome = Self::do_giga_unstake(&who, st_amount);
				match outcome {
					Ok(()) => TransactionOutcome::Commit(Ok(())),
					Err(e) => TransactionOutcome::Rollback(Err(e)),
				}
			})
		}

		/// Release the pending-unstake position once
		/// [`Config::CooldownPeriod`] has elapsed. Reduces `LockId` by the
		/// stored amount; the caller's HDX becomes spendable.
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
			Self::refresh_lock(&who);
			// `Stakes` may have been emptied by the unstake that opened this
			// position; once the position closes, drop the empty record too.
			if let Some(s) = Stakes::<T>::get(&who) {
				if s.hdx_locked == 0 && s.st_minted == 0 {
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
		/// Internal helper for `giga_unstake`. Uses `?` freely; the caller
		/// wraps it in `with_transaction` for atomic rollback.
		fn do_giga_unstake(who: &T::AccountId, st_amount: Balance) -> DispatchResult {
			ensure!(
				PendingUnstakes::<T>::get(who).is_none(),
				Error::<T>::PendingUnstakeAlreadyExists
			);

			let stake = Stakes::<T>::get(who).ok_or(Error::<T>::NoStake)?;
			ensure!(st_amount > 0, Error::<T>::ZeroAmount);
			ensure!(st_amount <= stake.st_minted, Error::<T>::InsufficientStake);

			// Compute payout from PRE-unstake totals.
			let s_pre = TotalStHdx::<T>::get();
			let t_pre = Self::total_hdx();
			let payout = crate::math::total_payout(st_amount, t_pre, s_pre).map_err(|_| Error::<T>::Overflow)?;

			// Pre-decrement `st_minted` so `LockableAToken.burn`'s `freeBalance`
			// check (via the lock-manager precompile) lets the burn through.
			let new_st_minted = stake.st_minted.checked_sub(st_amount).ok_or(Error::<T>::Overflow)?;
			Stakes::<T>::insert(
				who,
				StakeRecord {
					hdx_locked: stake.hdx_locked,
					st_minted: new_st_minted,
				},
			);

			// MM withdraw: returns stHDX to `who`, burns aToken from `who`.
			T::MoneyMarket::withdraw(who, T::StHdxAssetId::get(), st_amount)
				.map_err(|_| Error::<T>::MoneyMarketWithdrawFailed)?;

			// Burn the returned stHDX from the user.
			T::StHdx::burn_from(
				T::StHdxAssetId::get(),
				who,
				st_amount,
				Preservation::Expendable,
				Precision::Exact,
				Fortitude::Force,
			)?;

			// Split `payout` between the user's active stake and the gigapot.
			//   • payout ≤ active stake → consume from active only
			//   • payout > active stake → drain active, pull remainder from pot
			let (new_hdx_locked, yield_share) = if payout <= stake.hdx_locked {
				(stake.hdx_locked - payout, 0)
			} else {
				let yield_amount = payout - stake.hdx_locked;
				T::Currency::transfer(
					&Self::gigapot_account_id(),
					who,
					yield_amount,
					ExistenceRequirement::AllowDeath,
				)?;
				(0, yield_amount)
			};
			let principal_consumed = stake.hdx_locked.saturating_sub(new_hdx_locked);

			Stakes::<T>::insert(
				who,
				StakeRecord {
					hdx_locked: new_hdx_locked,
					st_minted: new_st_minted,
				},
			);
			TotalLocked::<T>::mutate(|x| *x = x.saturating_sub(principal_consumed));
			TotalStHdx::<T>::mutate(|x| *x = x.saturating_sub(st_amount));

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
			Self::refresh_lock(who);

			Self::deposit_event(Event::Unstaked {
				who: who.clone(),
				st_amount,
				payout,
				yield_share,
				expires_at,
			});
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Recompute the single combined balance lock for `who`:
		/// `lock_amount = Stakes[who].hdx_locked + PendingUnstakes[who].amount`.
		/// Uses `set_lock` (not `extend_lock`) so the lock can shrink on unstake
		/// or unlock. Removes the lock entirely when both components are zero.
		fn refresh_lock(who: &T::AccountId) {
			let stake_amount = Stakes::<T>::get(who).map(|s| s.hdx_locked).unwrap_or(0);
			let pending = PendingUnstakes::<T>::get(who).map(|p| p.amount).unwrap_or(0);
			let total = stake_amount.saturating_add(pending);
			if total == 0 {
				T::Currency::remove_lock(T::LockId::get(), who);
			} else {
				T::Currency::set_lock(T::LockId::get(), who, total, WithdrawReasons::all());
			}
		}

		/// Account id of the gigapot (yield holder), derived from
		/// `Config::PalletId`.
		pub fn gigapot_account_id() -> T::AccountId {
			T::PalletId::get().into_account_truncating()
		}

		/// Total HDX backing all stHDX:
		/// `TotalLocked + free_balance(gigapot_account_id)`.
		pub fn total_hdx() -> Balance {
			TotalLocked::<T>::get().saturating_add(T::Currency::free_balance(&Self::gigapot_account_id()))
		}

		/// Total stHDX issued.
		pub fn total_st_hdx_supply() -> Balance {
			TotalStHdx::<T>::get()
		}

		/// stHDX → HDX exchange rate as `FixedU128 = total_hdx / total_st_hdx_supply`.
		///
		/// Returns `1.0` when no stHDX has been issued yet (bootstrap).
		pub fn exchange_rate() -> FixedU128 {
			let s = Self::total_st_hdx_supply();
			if s == 0 {
				FixedU128::from(1)
			} else {
				FixedU128::checked_from_rational(Self::total_hdx(), s).unwrap_or(FixedU128::from(1))
			}
		}
	}
}
