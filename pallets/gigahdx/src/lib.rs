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
//! `giga_unstake` is the reverse path.

#![cfg_attr(not(feature = "std"), no_std)]
// `giga_unstake` returns `DispatchResultWithPostInfo`; the call macro's
// `.map(Into::into).map_err(Into::into)` wrapper is then an identity conversion.
#![allow(clippy::useless_conversion)]

pub use pallet::*;

pub mod traits;

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

	/// Used by the `migrate` benchmark. Must leave `who` with no external
	/// claim that would survive `force_unstake` (otherwise migrate's
	/// admission refuses).
	fn setup_legacy_staking_position(who: &AccountId, amount: primitives::Balance) -> sp_runtime::DispatchResult;
}

#[cfg(feature = "runtime-benchmarks")]
impl<AccountId> BenchmarkHelper<AccountId> for () {
	fn register_assets() -> sp_runtime::DispatchResult {
		Ok(())
	}
	fn setup_legacy_staking_position(_who: &AccountId, _amount: primitives::Balance) -> sp_runtime::DispatchResult {
		Err(sp_runtime::DispatchError::Other(
			"BenchmarkHelper: no legacy staking source configured",
		))
	}
}

#[frame_support::pallet]
pub mod pallet {
	pub use crate::traits::{ExternalClaims, LegacyStakeMigrator};
	pub use crate::weights::WeightInfo;
	use codec::{Decode, DecodeWithMemTracking, Encode, MaxEncodedLen};
	use frame_support::pallet_prelude::*;
	use frame_support::sp_runtime::helpers_128bit::multiply_by_rational_with_rounding;
	use frame_support::sp_runtime::traits::{AccountIdConversion, CheckedAdd};
	use frame_support::sp_runtime::{ArithmeticError, Rounding};
	use frame_support::traits::fungibles::Mutate as FungiblesMutate;
	use frame_support::traits::tokens::{Fortitude, Precision, Preservation};
	use frame_support::traits::{
		fungibles, Currency, ExistenceRequirement, LockIdentifier, LockableCurrency, WithdrawReasons,
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
		/// Total unstaking amount.
		pub unstaking: Balance,
		/// Total number of unstaking positions.
		pub unstaking_count: u16,
	}

	impl StakeRecord {
		/// True when the record carries no state and can be reaped — dropping a
		/// record while any field is non-zero (e.g. residual `unstaking`) would
		/// orphan `PendingUnstakes` entries or its lock.
		fn is_empty(&self) -> bool {
			self.hdx == 0 && self.gigahdx == 0 && self.unstaking == 0 && self.unstaking_count == 0
		}
	}

	/// One pending-unstake position. Keyed by the originating block; cooldown
	/// expiry is `block + Config::CooldownPeriod`.
	#[derive(Encode, Decode, DecodeWithMemTracking, TypeInfo, MaxEncodedLen, Clone, PartialEq, Eq, Debug)]
	pub struct PendingUnstake {
		pub amount: Balance,
	}

	pub const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);

	/// Defensive tripwire bound for `realize_yield`. Aggregate solvency
	/// guarantees the gigapot covers all accrued yield; a *per-account*
	/// `realize_yield` can fall a few atomic units short purely from
	/// cross-user floor-rounding (one staker's clamped negative residual
	/// nudging another's rate up). Anything beyond this many atomic units is
	/// an accounting bug, not rounding — `debug_assert` panics so tests and
	/// fuzzing surface it; release still returns `GigapotInsufficient`.
	/// 1 µHDX ≫ any realistic rounding accumulation, ≪ any real shortfall.
	const MAX_GIGAPOT_ROUNDING_SHORTFALL: Balance = 1_000_000;

	#[pallet::pallet]
	#[pallet::storage_version(STORAGE_VERSION)]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config<RuntimeEvent: From<Event<Self>>> {
		type NativeCurrency: LockableCurrency<Self::AccountId, Balance = Balance, Moment = BlockNumberFor<Self>>;

		/// Multi-asset register that holds stHDX (and any other registered
		/// fungible). Only this pallet mints / burns stHDX through it.
		type MultiCurrency: fungibles::Mutate<Self::AccountId, AssetId = AssetId, Balance = Balance>
			+ fungibles::Inspect<Self::AccountId, AssetId = AssetId, Balance = Balance>;

		#[pallet::constant]
		type StHdxAssetId: Get<AssetId>;

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

		/// Maximum number of concurrent pending-unstake positions per account.
		#[pallet::constant]
		type MaxPendingUnstakes: Get<u32>;

		/// Inspector returning the sum of non-overlapping HDX claims on the
		/// caller. Any non-zero value blocks `giga_stake` admission — the
		/// strict policy rejects stakes whenever the account carries a lock
		/// the runtime has not whitelisted for overlap (e.g. `pyconvot`).
		type ExternalClaims: crate::traits::ExternalClaims<Self::AccountId>;

		/// Bridge into the legacy NFT staking pallet. `migrate` calls
		/// `force_unstake` here to destroy the caller's legacy position
		/// before re-staking the freed HDX into gigahdx.
		type LegacyStaking: crate::traits::LegacyStakeMigrator<Self::AccountId>;

		/// HDX backing the caller's active votes — the floor `giga_unstake`
		/// keeps `hdx` above. Pulled lazily here (not maintained on the voting
		/// path); wired to `pallet-gigahdx-rewards` in the runtime.
		type VotingCommitment: crate::traits::VotingCommitmentInspect<Self::AccountId>;

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

	/// Pending unstake positions, keyed by `(account, originating_block)`.
	/// Same-block unstakes by the same account compound into one entry.
	/// Per-account count bounded by `Config::MaxPendingUnstakes`.
	#[pallet::storage]
	pub type PendingUnstakes<T: Config> = StorageDoubleMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		Twox64Concat,
		BlockNumberFor<T>,
		PendingUnstake,
		OptionQuery,
	>;

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
			position_id: BlockNumberFor<T>,
			gigahdx_amount: Balance,
			payout: Balance,
			yield_share: Balance,
			expires_at: BlockNumberFor<T>,
		},
		Unlocked {
			who: T::AccountId,
			position_id: BlockNumberFor<T>,
			amount: Balance,
		},
		UnstakeCancelled {
			who: T::AccountId,
			position_id: BlockNumberFor<T>,
			amount: Balance,
			gigahdx: Balance,
		},
		PoolContractUpdated {
			contract: EvmAddress,
		},
		/// Caller migrated their legacy NFT staking position into gigahdx.
		/// `hdx_unlocked` is the sum of legacy stake + previously locked
		/// rewards + freshly paid rewards; `gigahdx_received` is the aToken
		/// amount actually credited by the money market.
		MigratedFromLegacy {
			who: T::AccountId,
			hdx_unlocked: Balance,
			gigahdx_received: Balance,
		},
		/// Accrued yield was moved from the gigapot into the caller's locked
		/// stake principal. `amount` is the HDX transferred and added to
		/// `Stakes[who].hdx`; `gigahdx` and the exchange rate are unchanged.
		YieldRealized {
			who: T::AccountId,
			amount: Balance,
		},
	}

	#[pallet::error]
	pub enum Error<T> {
		/// Stake amount is below `Config::MinStake`.
		BelowMinStake,
		/// Caller doesn't have enough unencumbered HDX to back the stake
		/// after subtracting their existing gigahdx commitment.
		InsufficientFreeBalance,
		/// Caller holds a non-overlapping lock (legacy staking, vesting, …)
		/// reported by `Config::ExternalClaims`. Strict policy: gigahdx
		/// admission requires the caller to have no claims on their HDX
		/// other than those the runtime explicitly allows to coexist
		/// (e.g. `pyconvot`). Release the conflicting lock before staking.
		BlockedByExternalLock,
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
		/// The cooldown period has not yet elapsed for the targeted position.
		CooldownNotElapsed,
		/// No pending unstake position with the supplied id exists for the caller.
		PendingUnstakeNotFound,
		/// Caller has reached `Config::MaxPendingUnstakes` concurrent positions.
		TooManyPendingUnstakes,
		/// `set_pool_contract` was called while gigahdx (aToken / stHDX) is
		/// still in circulation. The pool is settable only when total stHDX
		/// supply is zero.
		OutstandingStake,
		/// Unstake would drop `Stakes[who].hdx` below the HDX backing the
		/// caller's active votes (`T::VotingCommitment`); remove the relevant
		/// conviction votes first.
		StakeFrozen,
		/// `slash` fallback could not extract the full `seize_hdx` from the
		/// borrower's account (e.g. existential-deposit constraint).
		SeizeFailed,
		/// The gigapot lacks the HDX to cover the caller's accrued yield.
		/// Only reachable in a drained/floored state, not normal operation.
		GigapotInsufficient,
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
		/// `BlockedByExternalLock` if `Config::ExternalClaims::on(caller) > 0`
		/// (the caller holds a non-allowed lock — strict policy rejects
		/// stake admission entirely), with `InsufficientFreeBalance` if
		/// `free_balance − own_gigahdx_commitment < amount`, with
		/// `StHdxMintFailed` if stHDX minting fails, or with
		/// `MoneyMarketSupplyFailed` if the AAVE `Pool.supply` call reverts.
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
			Self::ensure_stakeable(&who, amount)?;
			Self::do_stake(&who, amount)?;
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
		#[pallet::weight(T::WeightInfo::giga_unstake()
			.saturating_add(T::MoneyMarket::withdraw_weight())
			.saturating_add(<T::VotingCommitment as crate::traits::VotingCommitmentInspect<T::AccountId>>::committed_weight()))]
		pub fn giga_unstake(origin: OriginFor<T>, gigahdx_amount: Balance) -> DispatchResultWithPostInfo {
			let who = ensure_signed(origin)?;
			// The weight annotation can't see the caller, so it declared the
			// worst-case vote scan (`committed_weight()`); refund down to the
			// reservations actually read.
			let votes_scanned = Self::do_unstake(&who, gigahdx_amount)?;
			let actual = T::WeightInfo::giga_unstake()
				.saturating_add(T::MoneyMarket::withdraw_weight())
				.saturating_add(<T as frame_system::Config>::DbWeight::get().reads(votes_scanned.into()));
			Ok(Some(actual).into())
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
		/// `Stakes.hdx`). When an unstake payout exceeds the user's active
		/// stake, the active stake is drained but `Stakes.gigahdx` (and the
		/// corresponding aToken balance) can stay non-zero — those tokens
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

		/// Release a single pending-unstake position whose cooldown has elapsed.
		///
		/// Fails with `PendingUnstakeNotFound` if no position with `position_id`
		/// exists, or `CooldownNotElapsed` if the targeted position is still cooling.
		///
		/// Parameters:
		/// - `position_id`: id of the position to release (as recorded in the
		///   `Unstaked` event for the originating unstake).
		///
		/// Emits `Unlocked` event when successful.
		///
		#[pallet::call_index(3)]
		#[pallet::weight(T::WeightInfo::unlock())]
		pub fn unlock(origin: OriginFor<T>, position_id: BlockNumberFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let entry = PendingUnstakes::<T>::get(&who, position_id).ok_or(Error::<T>::PendingUnstakeNotFound)?;
			let expires_at = position_id
				.checked_add(&T::CooldownPeriod::get())
				.ok_or(Error::<T>::Overflow)?;
			ensure!(
				frame_system::Pallet::<T>::block_number() >= expires_at,
				Error::<T>::CooldownNotElapsed,
			);
			PendingUnstakes::<T>::remove(&who, position_id);

			Stakes::<T>::mutate_exists(&who, |maybe| {
				if let Some(s) = maybe.as_mut() {
					s.unstaking = s.unstaking.saturating_sub(entry.amount);
					s.unstaking_count = s.unstaking_count.saturating_sub(1);
					if s.is_empty() {
						*maybe = None;
					}
				}
			});
			Self::refresh_lock(&who)?;

			Self::deposit_event(Event::Unlocked {
				who,
				position_id,
				amount: entry.amount,
			});
			Ok(())
		}

		/// Cancel a single pending-unstake position, folding its `amount` HDX
		/// back into the active stake at the current exchange rate.
		///
		/// The pending HDX is already locked in the caller's account; this
		/// extrinsic relabels it as active stake and mints fresh aTokens at
		/// today's rate. The number of aTokens minted may differ from the
		/// amount burned at unstake time if the exchange rate moved.
		///
		/// Cooldown is not a gate — cancellation is valid throughout the
		/// pending window, until the caller invokes `unlock`.
		///
		/// Fails with `PendingUnstakeNotFound` if `position_id` is not present.
		///
		/// Parameters:
		/// - `position_id`: id of the position to cancel.
		///
		/// Emits `UnstakeCancelled` event when successful.
		///
		#[pallet::call_index(4)]
		#[pallet::weight(T::WeightInfo::cancel_unstake().saturating_add(T::MoneyMarket::supply_weight()))]
		#[transactional]
		pub fn cancel_unstake(origin: OriginFor<T>, position_id: BlockNumberFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let entry = PendingUnstakes::<T>::take(&who, position_id).ok_or(Error::<T>::PendingUnstakeNotFound)?;

			Stakes::<T>::mutate(&who, |maybe| {
				if let Some(s) = maybe {
					s.unstaking = s.unstaking.saturating_sub(entry.amount);
					s.unstaking_count = s.unstaking_count.saturating_sub(1);
				}
			});

			let gigahdx = Self::do_stake(&who, entry.amount)?;
			Self::deposit_event(Event::UnstakeCancelled {
				who,
				position_id,
				amount: entry.amount,
				gigahdx,
			});
			Ok(())
		}

		/// Migrate the caller's legacy NFT staking position into gigahdx.
		///
		/// Atomically: destroys the legacy position via `LegacyStaking::
		/// force_unstake` (paying out 100% of rewards — no sigmoid slash, no
		/// unclaimable-period penalty), then re-stakes the freed HDX under
		/// the same admission gate as `giga_stake`. Refuses partial migration
		/// — the legacy position is consumed whole or not at all.
		///
		/// Admission runs *after* `force_unstake` so the legacy lock is no
		/// longer counted against `ExternalClaims`.
		///
		/// Fails with `BelowMinStake` if the legacy position unlocks less
		/// than `Config::MinStake`, with `BlockedByExternalLock` if the
		/// caller still carries another non-overlap-whitelisted lock, with
		/// `InsufficientFreeBalance` if the post-unstake balance can't cover
		/// the new gigahdx claim, or with whatever error `force_unstake`
		/// raises (e.g. ongoing referendum vote, no legacy position).
		///
		/// Emits `MigratedFromLegacy` event when successful.
		#[pallet::call_index(5)]
		#[pallet::weight(T::WeightInfo::migrate().saturating_add(T::MoneyMarket::supply_weight()))]
		#[transactional]
		pub fn migrate(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;

			let hdx_unlocked = T::LegacyStaking::force_unstake(&who)?;

			// Admission runs *after* `force_unstake` so the legacy lock is no
			// longer counted against `ExternalClaims`.
			Self::ensure_stakeable(&who, hdx_unlocked)?;

			let gigahdx_received = Self::do_stake(&who, hdx_unlocked)?;
			Self::deposit_event(Event::MigratedFromLegacy {
				who,
				hdx_unlocked,
				gigahdx_received,
			});
			Ok(())
		}

		/// Realize the caller's accrued yield into their locked stake principal.
		///
		/// Moves the HDX value the caller's GIGAHDX has gained since it was last
		/// reconciled (`rate × gigahdx − Stakes[who].hdx`) from the gigapot into
		/// the caller's account, folds it into `Stakes[who].hdx`, and refreshes
		/// the lock. GIGAHDX balance and the exchange rate are unchanged. A
		/// caller with no accrued yield (or no stake) is a successful no-op.
		///
		/// Emits `YieldRealized` event when there was yield to realize.
		///
		#[pallet::call_index(6)]
		#[pallet::weight(T::WeightInfo::realize_yield())]
		pub fn realize_yield(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::do_realize_yield(&who)?;
			Ok(())
		}
	}

	impl<T: Config> Pallet<T> {
		/// Realize `who`'s accrued GIGAHDX yield into their locked stake
		/// principal: move `rate × gigahdx − Stakes[who].hdx` HDX from the
		/// gigapot into `who`, fold it into `Stakes[who].hdx`, refresh the
		/// lock. No-op (returns `0`) when there is no accrued yield or no
		/// stake. Shared by the `realize_yield` extrinsic and the liquidation
		/// seize path. `#[transactional]` so a mid-way Err rolls back.
		#[transactional]
		fn do_realize_yield(who: &T::AccountId) -> Result<Balance, DispatchError> {
			let stake = Stakes::<T>::get(who).unwrap_or_default();
			let current_value =
				Self::calculate_hdx_amount_given_gigahdx(stake.gigahdx).map_err(|_| Error::<T>::Overflow)?;
			let accrued = current_value.saturating_sub(stake.hdx);
			if accrued == 0 {
				return Ok(0);
			}

			if T::NativeCurrency::transfer(
				&Self::gigapot_account_id(),
				who,
				accrued,
				ExistenceRequirement::AllowDeath,
			)
			.is_err()
			{
				let gigapot = T::NativeCurrency::free_balance(&Self::gigapot_account_id());
				let shortfall = accrued.saturating_sub(gigapot);
				debug_assert!(
					shortfall <= MAX_GIGAPOT_ROUNDING_SHORTFALL,
					"realize_yield: gigapot short by {shortfall} (accrued {accrued}, gigapot {gigapot}) \
					 — exceeds rounding tolerance, indicates an accounting bug"
				);
				return Err(Error::<T>::GigapotInsufficient.into());
			}

			Stakes::<T>::try_mutate(who, |maybe| -> Result<(), Error<T>> {
				let s = maybe.get_or_insert_with(StakeRecord::default);
				s.hdx = s.hdx.checked_add(accrued).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			TotalLocked::<T>::mutate(|x| *x = x.saturating_add(accrued));
			Self::refresh_lock(who)?;

			Self::deposit_event(Event::YieldRealized {
				who: who.clone(),
				amount: accrued,
			});
			Ok(accrued)
		}

		/// Internal helper for `giga_unstake`. Uses `?` freely; the
		/// `#[transactional]` attribute wraps the body in its own storage
		/// layer so any Err here rolls back partial mutations.
		#[transactional]
		fn do_unstake(who: &T::AccountId, gigahdx_amount: Balance) -> Result<u32, DispatchError> {
			let stake = Stakes::<T>::get(who).ok_or(Error::<T>::NoStake)?;
			let now = frame_system::Pallet::<T>::block_number();
			let is_new_position = !PendingUnstakes::<T>::contains_key(who, now);
			if is_new_position {
				ensure!(
					(stake.unstaking_count as u32) < T::MaxPendingUnstakes::get(),
					Error::<T>::TooManyPendingUnstakes
				);
			}
			ensure!(gigahdx_amount > 0, Error::<T>::ZeroAmount);
			ensure!(gigahdx_amount <= stake.gigahdx, Error::<T>::InsufficientStake);

			// Payout reads live rate state — must run before any mint/burn below.
			let payout = Self::calculate_hdx_amount_given_gigahdx(gigahdx_amount).map_err(|_| Error::<T>::Overflow)?;

			// Reject the unstake up-front if it would breach the voting-commitment
			// guard. `committed` is the HDX backing the caller's active votes (max
			// over their reservations), pulled lazily from the rewards pallet —
			// not maintained on the voting path. `new_hdx = stake.hdx - payout`
			// (any excess comes from the gigapot as yield, not from `hdx`); we
			// need `new_hdx >= committed`.
			let (committed, votes_scanned) = <T::VotingCommitment as crate::traits::VotingCommitmentInspect<
				T::AccountId,
			>>::committed_with_count(who);
			let projected_hdx = stake.hdx.saturating_sub(payout);
			ensure!(projected_hdx >= committed, Error::<T>::StakeFrozen);

			// Pre-decrement `gigahdx` so `LockableAToken.burn`'s `freeBalance`
			// check (via the lock-manager precompile) lets the burn through.
			let new_gigahdx = stake.gigahdx.checked_sub(gigahdx_amount).ok_or(Error::<T>::Overflow)?;
			Stakes::<T>::mutate(who, |maybe| {
				if let Some(s) = maybe {
					s.gigahdx = new_gigahdx;
				}
			});

			let actual_withdrawn = T::MoneyMarket::withdraw(who, T::StHdxAssetId::get(), gigahdx_amount)
				.map_err(|_| Error::<T>::MoneyMarketWithdrawFailed)?;
			// Mismatch breaks `burn_from(Precision::Exact)` or leaks untracked
			// stHDX past cooldown accounting.
			ensure!(
				actual_withdrawn == gigahdx_amount,
				Error::<T>::MoneyMarketWithdrawFailed
			);

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

			// Full exit (all aTokens burned): any `hdx` above the committed floor
			// is unbacked rounding dust that no later `giga_unstake` could
			// release (it needs `gigahdx > 0`). Fold it into this position's
			// cooldown payout so the holder reclaims it and the record reaps
			// cleanly at `unlock`, instead of stranding the record + lock.
			let (new_hdx, payout) = if new_gigahdx == 0 {
				let dust = new_hdx.saturating_sub(committed);
				(new_hdx.saturating_sub(dust), payout.saturating_add(dust))
			} else {
				(new_hdx, payout)
			};
			let principal_consumed = stake.hdx.saturating_sub(new_hdx);

			let expires_at = now.checked_add(&T::CooldownPeriod::get()).ok_or(Error::<T>::Overflow)?;

			PendingUnstakes::<T>::mutate(who, now, |maybe| {
				let entry = maybe.get_or_insert(PendingUnstake { amount: 0 });
				entry.amount = entry.amount.saturating_add(payout);
			});

			Stakes::<T>::mutate(who, |maybe| {
				if let Some(s) = maybe {
					s.hdx = new_hdx;
					s.unstaking = s.unstaking.saturating_add(payout);
					if is_new_position {
						s.unstaking_count = s.unstaking_count.saturating_add(1);
					}
				}
			});
			TotalLocked::<T>::mutate(|x| *x = x.saturating_sub(principal_consumed));
			Self::refresh_lock(who)?;

			Self::deposit_event(Event::Unstaked {
				who: who.clone(),
				position_id: now,
				gigahdx_amount,
				payout,
				yield_share,
				expires_at,
			});
			Ok(votes_scanned)
		}
	}

	impl<T: Config> Pallet<T> {
		/// Admission gate shared by `giga_stake` and `migrate`.
		///
		/// Enforces the `MinStake` floor, the strict no-overlapping-lock policy
		/// (lock-layering via `max()` would otherwise let the same HDX back both
		/// a gigahdx stake and another pallet's claim after a single transfer of
		/// the unlocked portion), and that the caller's own commitment (active +
		/// pending unstakes) plus `amount` still fits under their free balance.
		fn ensure_stakeable(who: &T::AccountId, amount: Balance) -> DispatchResult {
			ensure!(amount >= T::MinStake::get(), Error::<T>::BelowMinStake);
			ensure!(T::ExternalClaims::on(who) == 0, Error::<T>::BlockedByExternalLock);

			let stake = Stakes::<T>::get(who).unwrap_or_default();
			let own_claim = stake.hdx.saturating_add(stake.unstaking);
			let stakeable = T::NativeCurrency::free_balance(who).saturating_sub(own_claim);
			ensure!(stakeable >= amount, Error::<T>::InsufficientFreeBalance);
			Ok(())
		}

		/// Computes the stHDX amount at the current rate, mints it into `who`,
		/// supplies it to the money market, credits the resulting aToken amount
		/// to `Stakes[who]`, locks `amount` HDX under `Config::LockId`, and emits
		/// `Staked`.
		///
		/// Caller invariant: `amount` HDX must already be in `who`'s free
		/// balance. This helper performs **no admission control** — neither
		/// the `MinStake` floor nor the `ExternalClaims`/headroom checks that
		/// `giga_stake` applies. It is intended for trusted internal callers
		/// (e.g. `cancel_unstake` rearranging already-locked HDX, or
		/// `pallet-gigahdx-rewards` compounding accrued rewards). Untrusted
		/// callers must replicate the `giga_stake` checks before invoking.
		#[transactional]
		pub fn do_stake(who: &T::AccountId, amount: Balance) -> Result<Balance, DispatchError> {
			ensure!(amount > 0, Error::<T>::ZeroAmount);
			let gigahdx_to_mint = Self::calculate_gigahdx_given_hdx_amount(amount).map_err(|_| Error::<T>::Overflow)?;
			ensure!(gigahdx_to_mint > 0, Error::<T>::ZeroAmount);

			T::MultiCurrency::mint_into(T::StHdxAssetId::get(), who, gigahdx_to_mint)
				.map_err(|_| Error::<T>::StHdxMintFailed)?;
			let actual_minted = T::MoneyMarket::supply(who, T::StHdxAssetId::get(), gigahdx_to_mint)
				.map_err(|_| Error::<T>::MoneyMarketSupplyFailed)?;
			// Silent zero-mint would strand `amount` HDX with no redeemable gigahdx.
			ensure!(actual_minted > 0, Error::<T>::MoneyMarketSupplyFailed);

			Stakes::<T>::try_mutate(who, |maybe_stake| -> Result<(), Error<T>> {
				let stake = maybe_stake.get_or_insert_with(StakeRecord::default);
				stake.hdx = stake.hdx.checked_add(amount).ok_or(Error::<T>::Overflow)?;
				stake.gigahdx = stake.gigahdx.checked_add(actual_minted).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;
			TotalLocked::<T>::mutate(|x| *x = x.saturating_add(amount));
			Self::refresh_lock(who)?;

			Self::deposit_event(Event::Staked {
				who: who.clone(),
				amount,
				gigahdx: actual_minted,
			});
			Ok(actual_minted)
		}

		/// Recompute the single combined balance lock for `who`:
		/// `lock_amount = Stakes[who].hdx + Stakes[who].unstaking`. Uses
		/// `set_lock` (not `extend_lock`) so the lock can shrink on unstake
		/// or unlock. Removes the lock entirely when the total is zero.
		#[transactional]
		fn refresh_lock(who: &T::AccountId) -> DispatchResult {
			let total = Stakes::<T>::get(who)
				.map(|s| s.hdx.saturating_add(s.unstaking))
				.unwrap_or(0);
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

	impl<T: Config> hydradx_traits::gigahdx::Seize<T::AccountId> for Pallet<T> {
		fn realize_yield(borrower: &T::AccountId) -> DispatchResult {
			Self::do_realize_yield(borrower).map(|_| ())
		}

		fn snapshot_stake(borrower: &T::AccountId) -> Result<(Balance, Balance), DispatchError> {
			let s = Stakes::<T>::get(borrower).ok_or(Error::<T>::NoStake)?;
			Ok((s.hdx, s.gigahdx))
		}

		fn on_pre_seize(borrower: &T::AccountId) -> Result<Balance, DispatchError> {
			Stakes::<T>::try_mutate(borrower, |maybe| -> Result<Balance, DispatchError> {
				let s = maybe.as_mut().ok_or(Error::<T>::NoStake)?;
				let prev = s.gigahdx;
				s.gigahdx = 0;
				Ok(prev)
			})
		}

		/// `orig_gigahdx` is the borrower's pre-seize aToken balance from
		/// `snapshot_stake`; `on_pre_seize` has since zeroed the stored value,
		/// so the residual cannot be derived from state here. Both seize amounts
		/// are clamped to the borrower's staked/snapshot values (see body) so the
		/// seize degrades gracefully if a future stHDX-invariant break ever lets
		/// the live balance exceed the snapshot, rather than reverting.
		#[transactional]
		fn on_seize(
			borrower: &T::AccountId,
			recipient: &T::AccountId,
			seize_hdx: Balance,
			seize_gigahdx: Balance,
			orig_gigahdx: Balance,
		) -> DispatchResult {
			// `seize_hdx`/`seize_gigahdx` are measured against the borrower's live
			// balances; clamp them to the staked/snapshot values so a future
			// stHDX-invariant break degrades gracefully in release instead of
			// underflow-reverting the liquidation. The debug_asserts keep the
			// broken state fail-loud under test/fuzz.
			// `None` means no stake; the try_mutate below returns `NoStake`. Only
			// assert/clamp `seize_hdx` when a stake exists so that error path stays
			// graceful.
			let maybe_staked_hdx = Stakes::<T>::get(borrower).map(|s| s.hdx);
			debug_assert!(
				seize_gigahdx <= orig_gigahdx,
				"on_seize: seize_gigahdx ({seize_gigahdx:?}) exceeds orig_gigahdx snapshot ({orig_gigahdx:?})",
			);
			debug_assert!(
				maybe_staked_hdx.is_none_or(|h| seize_hdx <= h),
				"on_seize: seize_hdx ({seize_hdx:?}) exceeds staked hdx ({maybe_staked_hdx:?})",
			);
			let seize_hdx = maybe_staked_hdx.map_or(seize_hdx, |h| seize_hdx.min(h));

			Stakes::<T>::try_mutate(borrower, |maybe| -> DispatchResult {
				let s = maybe.as_mut().ok_or(Error::<T>::NoStake)?;
				s.hdx = s.hdx.saturating_sub(seize_hdx);
				s.gigahdx = orig_gigahdx.saturating_sub(seize_gigahdx);
				Ok(())
			})?;
			// Shrink the borrower's lock *before* withdrawing. The stale
			// pre-seize ghdxlock (sized to `hdx + unstaking`) would otherwise
			// block the transfer with `LiquidityRestrictions` for any
			// borrower whose free balance equals their staked amount.
			Self::refresh_lock(borrower)?;

			if !seize_hdx.is_zero() {
				// Prefer a clean transfer. If the borrower's remaining locks
				// (e.g. uncleared `pyconvot`, vesting, or any unmanaged lock)
				// still block the move, fall back to `slash` + `resolve_creating`
				// — liquidation is top priority and must always land.
				let new_balance = T::NativeCurrency::free_balance(borrower)
					.checked_sub(seize_hdx)
					.ok_or(Error::<T>::SeizeFailed)?;
				let can_transfer =
					T::NativeCurrency::ensure_can_withdraw(borrower, seize_hdx, WithdrawReasons::TRANSFER, new_balance)
						.is_ok();
				if can_transfer {
					T::NativeCurrency::transfer(borrower, recipient, seize_hdx, ExistenceRequirement::AllowDeath)?;
				} else {
					// Intentional policy: liquidation outranks every lock. `slash` takes
					// the HDX regardless of `ormlvest` vesting, `pyconvot`, or any other
					// foreign lock; the lock owner bears any later `balance < lock`
					// shortfall. gigahdx's own ledger stays consistent regardless:
					// `seize_hdx <= active hdx` (snapshot reads only `s.hdx`) and the
					// lock invariant `balance >= hdx + unstaking` together guarantee
					// `balance_new >= hdx_new + unstaking`, so `unstaking` /
					// `PendingUnstakes` are never stranded by the slash.
					// `slash` ignores locks (unlike `transfer`), but
					// `pallet_balances` refuses to push a non-reapable
					// account below ED. Tolerate that ≤ED dust — Aave has
					// already moved the collateral aToken by this point, so
					// the seize must land. Larger shortfalls keep the
					// fail-loud tripwire for a genuinely broken stake/lock
					// ledger (the `free >= seize_hdx` staking invariant
					// bounds the shortfall to exactly the ED).
					let (imbalance, remaining) = T::NativeCurrency::slash(borrower, seize_hdx);
					let ed = T::NativeCurrency::minimum_balance();
					ensure!(remaining <= ed, Error::<T>::SeizeFailed);
					T::NativeCurrency::resolve_creating(recipient, imbalance);
				}
			}

			Stakes::<T>::try_mutate(recipient, |maybe| -> DispatchResult {
				let s = maybe.get_or_insert_with(StakeRecord::default);
				s.hdx = s.hdx.checked_add(seize_hdx).ok_or(Error::<T>::Overflow)?;
				s.gigahdx = s.gigahdx.checked_add(seize_gigahdx).ok_or(Error::<T>::Overflow)?;
				Ok(())
			})?;

			Self::refresh_lock(recipient)?;
			Ok(())
		}
	}
}
