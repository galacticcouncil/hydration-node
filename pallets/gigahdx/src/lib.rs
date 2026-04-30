// This file is part of HydraDX.
// Copyright (C) 2020-2025  Intergalactic, Limited (GIB).
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # GIGAHDX Pallet
//!
//! Liquid Staking Token for HDX. Users stake HDX → receive GIGAHDX
//! (via intermediate stHDX token). Value accrues as trading fees
//! flow into the gigapot, increasing the exchange rate.
//!
//! ## Extrinsics
//! * `giga_stake` - Stake HDX, receive GIGAHDX
//! * `giga_unstake` - Burn GIGAHDX, receive HDX (locked with cooldown)
//! * `unlock` - Unlock HDX after cooldown expires (permissionless)

#![cfg_attr(not(feature = "std"), no_std)]

pub mod types;
pub mod weights;

#[cfg(test)]
mod tests;

pub use pallet::*;
pub use weights::WeightInfo;

use frame_support::{
	pallet_prelude::*,
	traits::{
		fungibles::{Inspect, Mutate},
		tokens::{Fortitude, Precision, Preservation},
		LockIdentifier,
	},
	PalletId,
};
use frame_system::pallet_prelude::*;
use hydradx_traits::gigahdx::{GigaHdxHooks, MoneyMarketOperations};
use orml_traits::MultiLockableCurrency;
use primitives::Balance;
use sp_arithmetic::helpers_128bit::multiply_by_rational_with_rounding;
use sp_runtime::{
	traits::{AccountIdConversion, CheckedAdd, Zero},
	FixedPointNumber, FixedU128, Rounding,
};

use types::UnstakePosition;

/// Single aggregate lock identifier shared by all unstake positions of an account.
/// The amount stored against this lock equals the sum of every active position's
/// `amount` for the account — recomputed on every stake/unlock.
const UNSTAKE_LOCK_ID: LockIdentifier = *b"gigaulok";

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Multi-asset currency supporting transfers, inspection, minting/burning.
		type Currency: Mutate<Self::AccountId, AssetId = u32, Balance = Balance>
			+ Inspect<Self::AccountId, AssetId = u32, Balance = Balance>;

		/// Multi-asset locking for HDX unlock positions.
		type LockableCurrency: MultiLockableCurrency<Self::AccountId, CurrencyId = u32, Balance = Balance>;

		/// Money Market integration for supply/withdraw operations.
		type MoneyMarket: MoneyMarketOperations<Self::AccountId, u32, Balance>;

		/// Hooks for stake/unstake lifecycle events.
		/// Implemented by pallet-gigahdx-voting for vote tracking and rewards.
		type Hooks: GigaHdxHooks<Self::AccountId, Balance, BlockNumberFor<Self>>;

		/// Pallet ID for the gigapot account.
		#[pallet::constant]
		type PalletId: Get<PalletId>;

		/// Native HDX asset ID.
		#[pallet::constant]
		type HdxAssetId: Get<u32>;

		/// stHDX asset ID — the intermediate staking token.
		#[pallet::constant]
		type StHdxAssetId: Get<u32>;

		/// GIGAHDX asset ID — the Money Market aToken.
		#[pallet::constant]
		type GigaHdxAssetId: Get<u32>;

		/// Cooldown period in blocks after giga-unstake.
		#[pallet::constant]
		type CooldownPeriod: Get<BlockNumberFor<Self>>;

		/// Minimum HDX to stake.
		#[pallet::constant]
		type MinStake: Get<Balance>;

		/// Maximum unstake positions per account.
		#[pallet::constant]
		type MaxUnstakePositions: Get<u32>;

		/// Weight information.
		type WeightInfo: WeightInfo;
	}

	// -----------------------------------------------------------------------
	// Storage
	// -----------------------------------------------------------------------

	/// Unstake positions per account (pending HDX claims after cooldown).
	#[pallet::storage]
	#[pallet::getter(fn unstake_positions)]
	pub type UnstakePositions<T: Config> = StorageMap<
		_,
		Blake2_128Concat,
		T::AccountId,
		BoundedVec<UnstakePosition<BlockNumberFor<T>>, T::MaxUnstakePositions>,
		ValueQuery,
	>;

	// -----------------------------------------------------------------------
	// Events
	// -----------------------------------------------------------------------

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		/// HDX staked, GIGAHDX received.
		Staked {
			who: T::AccountId,
			hdx_amount: Balance,
			st_hdx_minted: Balance,
			gigahdx_received: Balance,
			exchange_rate: FixedU128,
		},

		/// Giga-unstake initiated, unstake position created.
		Unstaked {
			who: T::AccountId,
			gigahdx_withdrawn: Balance,
			st_hdx_burned: Balance,
			hdx_amount: Balance,
			unlock_at: BlockNumberFor<T>,
		},

		/// HDX unlocked after cooldown expired.
		Unlocked { who: T::AccountId, hdx_amount: Balance },

		/// Fees received, exchange rate increased.
		FeesReceived {
			amount: Balance,
			new_exchange_rate: FixedU128,
		},

		/// Reward converted from HDX to GIGAHDX (via pallet-gigahdx-voting).
		RewardStaked {
			who: T::AccountId,
			hdx_amount: Balance,
			gigahdx_received: Balance,
		},
	}

	// -----------------------------------------------------------------------
	// Errors
	// -----------------------------------------------------------------------

	#[pallet::error]
	pub enum Error<T> {
		/// Zero amount not allowed.
		ZeroAmount,
		/// Stake amount below minimum.
		InsufficientStake,
		/// Remaining GIGAHDX balance after unstake would be below minimum stake (in HDX value).
		/// Either unstake everything or leave at least MinStake-equivalent behind.
		RemainingBelowMinStake,
		/// Arithmetic overflow/underflow.
		Arithmetic,
		/// No unlockable positions (none exist or all still in cooldown).
		NothingToUnlock,
		/// Too many unstake positions for account.
		TooManyUnstakePositions,
		/// Cannot unstake while votes exist in ongoing referenda.
		ActiveVotesInOngoingReferenda,
	}

	// -----------------------------------------------------------------------
	// Extrinsics
	// -----------------------------------------------------------------------

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Stake HDX to receive GIGAHDX.
		///
		/// Flow: HDX → stHDX (minted) → supply to Money Market → GIGAHDX
		#[pallet::call_index(0)]
		#[pallet::weight(T::WeightInfo::giga_stake())]
		pub fn giga_stake(origin: OriginFor<T>, hdx_amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!hdx_amount.is_zero(), Error::<T>::ZeroAmount);
			ensure!(hdx_amount >= T::MinStake::get(), Error::<T>::InsufficientStake);

			// Calculate stHDX to mint based on current exchange rate.
			let st_hdx_amount = Self::calculate_st_hdx_for_hdx(hdx_amount).ok_or(Error::<T>::Arithmetic)?;
			ensure!(!st_hdx_amount.is_zero(), Error::<T>::ZeroAmount);

			let gigapot = Self::gigapot_account_id();

			// Transfer HDX from user to gigapot.
			<T::Currency as Mutate<T::AccountId>>::transfer(
				T::HdxAssetId::get(),
				&who,
				&gigapot,
				hdx_amount,
				Preservation::Expendable,
			)?;

			// Mint stHDX to user.
			<T::Currency as Mutate<T::AccountId>>::mint_into(T::StHdxAssetId::get(), &who, st_hdx_amount)?;

			// Supply stHDX to Money Market → user receives GIGAHDX.
			let gigahdx_received = T::MoneyMarket::supply(&who, T::StHdxAssetId::get(), st_hdx_amount)?;

			// Notify hooks.
			T::Hooks::on_stake(&who, hdx_amount, gigahdx_received)?;

			Self::deposit_event(Event::Staked {
				who,
				hdx_amount,
				st_hdx_minted: st_hdx_amount,
				gigahdx_received,
				exchange_rate: Self::exchange_rate(),
			});

			Ok(())
		}

		/// Unstake GIGAHDX to receive HDX (locked with cooldown).
		///
		/// Flow: Check votes → get additional lock → on_unstake → withdraw from MM
		///       → burn stHDX → transfer HDX to user → lock HDX with dynamic cooldown.
		#[pallet::call_index(1)]
		#[pallet::weight(T::WeightInfo::giga_unstake())]
		pub fn giga_unstake(origin: OriginFor<T>, gigahdx_amount: Balance) -> DispatchResult {
			let who = ensure_signed(origin)?;

			ensure!(!gigahdx_amount.is_zero(), Error::<T>::ZeroAmount);

			let current_gigahdx = T::MoneyMarket::balance_of(&who);
			let remaining = current_gigahdx.saturating_sub(gigahdx_amount);
			if !remaining.is_zero() {
				let remaining_hdx = Self::calculate_hdx_for_st_hdx(remaining).ok_or(Error::<T>::Arithmetic)?;
				ensure!(remaining_hdx >= T::MinStake::get(), Error::<T>::RemainingBelowMinStake);
			}

			// Block if user has votes in ongoing referenda.
			ensure!(
				T::Hooks::can_unstake(&who, gigahdx_amount),
				Error::<T>::ActiveVotesInOngoingReferenda
			);

			// Reject early if the position cap is full — saves the cost of
			// running on_unstake / MM withdraw / burn / transfer just to roll
			// it all back when try_push fails below.
			ensure!(
				(UnstakePositions::<T>::decode_len(&who).unwrap_or(0) as u32) < T::MaxUnstakePositions::get(),
				Error::<T>::TooManyUnstakePositions,
			);

			// Capture additional lock period BEFORE on_unstake clears votes.
			let voting_lock = T::Hooks::additional_unstake_lock(&who);

			// Notify hooks — force-removes finished votes, records rewards.
			T::Hooks::on_unstake(&who, gigahdx_amount)?;

			// Withdraw GIGAHDX from Money Market → receive stHDX.
			let st_hdx_withdrawn = T::MoneyMarket::withdraw(&who, T::StHdxAssetId::get(), gigahdx_amount)?;

			// Calculate HDX amount based on current exchange rate.
			let hdx_amount = Self::calculate_hdx_for_st_hdx(st_hdx_withdrawn).ok_or(Error::<T>::Arithmetic)?;
			ensure!(!hdx_amount.is_zero(), Error::<T>::ZeroAmount);

			// Burn stHDX from user.
			<T::Currency as Mutate<T::AccountId>>::burn_from(
				T::StHdxAssetId::get(),
				&who,
				st_hdx_withdrawn,
				Preservation::Expendable,
				Precision::Exact,
				Fortitude::Polite,
			)?;

			// Transfer HDX from gigapot to user.
			let gigapot = Self::gigapot_account_id();
			<T::Currency as Mutate<T::AccountId>>::transfer(
				T::HdxAssetId::get(),
				&gigapot,
				&who,
				hdx_amount,
				Preservation::Expendable,
			)?;

			// Dynamic cooldown: max(base_cooldown, voting_lock).
			let current_block = frame_system::Pallet::<T>::block_number();
			let actual_cooldown = T::CooldownPeriod::get().max(voting_lock);
			let unlock_at = current_block
				.checked_add(&actual_cooldown)
				.ok_or(Error::<T>::Arithmetic)?;

			UnstakePositions::<T>::try_mutate(&who, |positions| -> DispatchResult {
				let position = UnstakePosition {
					amount: hdx_amount,
					unlock_at,
				};
				positions
					.try_push(position)
					.map_err(|_| Error::<T>::TooManyUnstakePositions)?;

				// Aggregate lock: re-set the single lock with the new total so
				// pallet-balances sees sum(positions), not max(positions).
				let total_locked = Self::total_locked(positions)?;
				T::LockableCurrency::set_lock(UNSTAKE_LOCK_ID, T::HdxAssetId::get(), &who, total_locked)?;
				Ok(())
			})?;

			// Re-apply voting-lock split against the user's new (reduced) GIGAHDX balance.
			// Caps the tracker and spills uncovered commitment to a hard HDX lock.
			// Runs after the HDX transfer so apply_lock_split observes the final balance.
			T::Hooks::on_post_unstake(&who)?;

			Self::deposit_event(Event::Unstaked {
				who,
				gigahdx_withdrawn: gigahdx_amount,
				st_hdx_burned: st_hdx_withdrawn,
				hdx_amount,
				unlock_at,
			});

			Ok(())
		}

		/// Unlock all HDX positions whose cooldown has expired.
		/// Permissionless — anyone can call for any account.
		#[pallet::call_index(2)]
		#[pallet::weight(T::WeightInfo::unlock())]
		pub fn unlock(origin: OriginFor<T>, target: T::AccountId) -> DispatchResult {
			ensure_signed(origin)?;

			let current_block = frame_system::Pallet::<T>::block_number();

			UnstakePositions::<T>::try_mutate(&target, |positions| -> DispatchResult {
				let mut unlocked_any = false;

				// Iterate in reverse so removal doesn't shift unprocessed indices.
				for i in (0..positions.len()).rev() {
					if current_block >= positions[i].unlock_at {
						let position = positions.remove(i);
						Self::deposit_event(Event::Unlocked {
							who: target.clone(),
							hdx_amount: position.amount,
						});
						unlocked_any = true;
					}
				}

				ensure!(unlocked_any, Error::<T>::NothingToUnlock);

				// Re-sync the aggregate lock with the remaining positions. If
				// nothing is left, drop the lock entirely.
				let total_locked = Self::total_locked(positions)?;
				if total_locked.is_zero() {
					T::LockableCurrency::remove_lock(UNSTAKE_LOCK_ID, T::HdxAssetId::get(), &target)?;
				} else {
					T::LockableCurrency::set_lock(UNSTAKE_LOCK_ID, T::HdxAssetId::get(), &target, total_locked)?;
				}
				Ok(())
			})
		}
	}
}

// ---------------------------------------------------------------------------
// Public functions (called by pallet-gigahdx-voting and runtime adapters)
// ---------------------------------------------------------------------------

impl<T: Config> Pallet<T> {
	/// Gigapot account holding HDX backing stHDX.
	pub fn gigapot_account_id() -> T::AccountId {
		T::PalletId::get().into_account_truncating()
	}

	/// Total HDX in gigapot (from account balance).
	pub fn total_hdx() -> Balance {
		<T::Currency as Inspect<T::AccountId>>::balance(T::HdxAssetId::get(), &Self::gigapot_account_id())
	}

	/// Total stHDX supply.
	pub fn total_st_hdx_supply() -> Balance {
		<T::Currency as Inspect<T::AccountId>>::total_issuance(T::StHdxAssetId::get())
	}

	/// Exchange rate: HDX per stHDX.
	/// Returns 1.0 when no stHDX has been minted yet.
	pub fn exchange_rate() -> FixedU128 {
		let total_st_hdx = Self::total_st_hdx_supply();
		if total_st_hdx.is_zero() {
			FixedU128::one()
		} else {
			FixedU128::checked_from_rational(Self::total_hdx(), total_st_hdx).unwrap_or(FixedU128::one())
		}
	}

	/// Convert HDX to GIGAHDX for reward claiming.
	/// Called by pallet-gigahdx-voting during `claim_rewards`.
	/// HDX should already be in the gigapot (transferred by voting pallet).
	///
	/// Uses the pre-reward exchange rate (excludes the reward HDX from the rate)
	/// so the recipient receives the full value of their reward.
	pub fn stake_rewards(who: &T::AccountId, hdx_amount: Balance) -> Result<Balance, DispatchError> {
		let total_st_hdx = Self::total_st_hdx_supply();
		let total_hdx = Self::total_hdx();

		// Exclude the reward HDX from total to get the pre-reward rate.
		let st_hdx_amount = if total_st_hdx.is_zero() {
			hdx_amount
		} else {
			let pre_reward_hdx = total_hdx.checked_sub(hdx_amount).ok_or(Error::<T>::Arithmetic)?;
			if pre_reward_hdx.is_zero() {
				// Degenerate state: stHDX exists but no pre-reward backing.
				// Avoid divide-by-zero — bootstrap-mint 1:1 so the reward is at
				// least claimable. Future deposits restore a sane rate.
				hdx_amount
			} else {
				multiply_by_rational_with_rounding(hdx_amount, total_st_hdx, pre_reward_hdx, Rounding::Down)
					.ok_or(Error::<T>::Arithmetic)?
			}
		};

		// Mint stHDX to user.
		<T::Currency as Mutate<T::AccountId>>::mint_into(T::StHdxAssetId::get(), who, st_hdx_amount)?;

		// Supply stHDX to Money Market → user receives GIGAHDX.
		let gigahdx_received = T::MoneyMarket::supply(who, T::StHdxAssetId::get(), st_hdx_amount)?;

		Self::deposit_event(Event::RewardStaked {
			who: who.clone(),
			hdx_amount,
			gigahdx_received,
		});

		Ok(gigahdx_received)
	}
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

impl<T: Config> Pallet<T> {
	/// Calculate stHDX to mint for given HDX amount.
	/// Formula: st_hdx = hdx_amount * total_st_hdx / total_hdx
	/// Rounds DOWN to prevent minting more stHDX than backed.
	fn calculate_st_hdx_for_hdx(hdx_amount: Balance) -> Option<Balance> {
		let total_st_hdx = Self::total_st_hdx_supply();
		let total_hdx = Self::total_hdx();

		if total_st_hdx.is_zero() {
			// Initial 1:1 rate.
			Some(hdx_amount)
		} else {
			multiply_by_rational_with_rounding(hdx_amount, total_st_hdx, total_hdx, Rounding::Down)
		}
	}

	/// Calculate HDX to return for given stHDX amount.
	/// Formula: hdx = st_hdx_amount * total_hdx / total_st_hdx
	/// Rounds DOWN to prevent giving more HDX than backed.
	fn calculate_hdx_for_st_hdx(st_hdx_amount: Balance) -> Option<Balance> {
		let total_st_hdx = Self::total_st_hdx_supply();
		if total_st_hdx.is_zero() {
			return None;
		}
		multiply_by_rational_with_rounding(st_hdx_amount, Self::total_hdx(), total_st_hdx, Rounding::Down)
	}

	/// Sum of every active position's `amount` for an account.
	/// Bounded by `MaxUnstakePositions` (currently 10), so the cost is constant.
	fn total_locked(
		positions: &BoundedVec<UnstakePosition<BlockNumberFor<T>>, T::MaxUnstakePositions>,
	) -> Result<Balance, Error<T>> {
		positions
			.iter()
			.try_fold(Balance::zero(), |acc, p| acc.checked_add(p.amount))
			.ok_or(Error::<T>::Arithmetic)
	}
}
