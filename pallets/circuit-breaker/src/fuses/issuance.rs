use crate::traits::AssetDepositLimiter;
use crate::types::LockdownStatus;
use crate::{AssetLockdownState, Config, Event, Pallet};
use frame_support::traits::Get;
use frame_system::pallet_prelude::BlockNumberFor;
use orml_traits::currency::OnDeposit;
use orml_traits::{GetByKey, Handler, Happened};
use sp_runtime::{SaturatedConversion, Saturating};
use sp_std::marker::PhantomData;
//TODO: burned tokens deduct from limit to prevent DOS

//TODO: consider the solution for HDX too as that it is not involved in orml tokens

//TODO: check every he usage of saturaring

//TODO: CREATE ISSUE I guess if the supply increased (or decreased ) and the token is type external it was bridged in or out.

//TODO:

//TODO: add prop tests, also for save deposit, so only the claimed and specified amount is returned

//TODO: integration tests
// when other parachain is hacked, we should be able to lock down the asset issuance
// sending VDOT to us and exchange it
// Other parachain can send us VDOT, we exchange it for DOT and lock down the issuance
// example: byfrist xcm transfer to our asset crossing, so we should mint it, but we should not alllow
// Other test: replicate the problem we had last week, where you could mint any amount of sharetoken in stablepool
// --- we set limit for sharetoken in asset registry, when add liquidty, but when this cross this, it should reserve

//TODO: SET GLOBAL LIMIT TO 1 DAY

pub struct IssuanceIncreaseFuse<T: Config>(PhantomData<T>);

impl<T: Config> OnDeposit<T::AccountId, T::AssetId, T::Balance> for IssuanceIncreaseFuse<T> {
	fn on_deposit(currency_id: T::AssetId, who: &T::AccountId, amount: T::Balance) -> sp_runtime::DispatchResult {
		let period = <T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Period::get();
		if period == 0u128 {
			// no limit
			return Ok(());
		}

		let Some(limit) =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::DepositLimit::get(
				&currency_id,
			)
		else {
			return Ok(());
		};

		let current_block = <frame_system::Pallet<T>>::block_number();
		let lockdown_until: BlockNumberFor<T> = current_block.saturating_add(period.saturated_into());
		let asset_issuance =
			<T::DepositLimiter as AssetDepositLimiter<T::AccountId, T::AssetId, T::Balance>>::Issuance::get(
				&currency_id,
			);

		match AssetLockdownState::<T>::get(currency_id) {
			None => {
				// This happens only once - to set the initial state
				// Still check if this new deposit does not exceed the limit
				// Set the issuance without this amount because we want the amount to count towards the limit per period.
				if amount > limit {
					let to_lock = amount.saturating_sub(limit);
					Pallet::<T>::do_lock_deposit(&who, currency_id, to_lock)?;
					Pallet::<T>::do_lockdown_asset(currency_id, lockdown_until)?;
				} else {
					Pallet::<T>::do_reset_deposit_limits(currency_id, amount)?;
				}
			}
			Some(LockdownStatus::Locked(until)) if until > current_block => {
				// Asset in lockdown
				Pallet::<T>::do_lock_deposit(&who, currency_id, amount)?;
			}
			Some(LockdownStatus::Locked(_)) => {
				// Lockdown expired
				// Check if this new deposit does not exceed the limit and lock it down again if it does.
				if amount > limit {
					let to_lock = amount.saturating_sub(limit);
					Pallet::<T>::do_lock_deposit(&who, currency_id, to_lock)?;
					Pallet::<T>::do_lockdown_asset(currency_id, lockdown_until)?;
				} else {
					Pallet::<T>::do_lift_lockdown(currency_id, amount)?;
				}
			}
			Some(LockdownStatus::Unlocked((last_reset_at, _)))
				if last_reset_at.saturating_add(period.saturated_into()) <= current_block =>
			{
				// The period is over, so we can reset the limit.
				// But first, we must check if this new deposit does not exceed the limit.
				if amount > limit {
					let to_lock = amount.saturating_sub(limit);
					Pallet::<T>::do_lock_deposit(&who, currency_id, to_lock)?;
					Pallet::<T>::do_lockdown_asset(currency_id, lockdown_until)?;
				} else {
					Pallet::<T>::do_reset_deposit_limits(currency_id, amount)?;
				}
			}
			Some(LockdownStatus::Unlocked((_, last_issuance))) => {
				// If the period is not over, we check the limit by comparing issuance increase.
				let issuance_increase_in_period = asset_issuance.saturating_sub(last_issuance);
				if issuance_increase_in_period > limit {
					// We should lock only the excess, not all new deposit
					// Formula: to_lock = current_issuance - (last_issuance + limit)
					let to_lock = asset_issuance.saturating_sub(last_issuance.saturating_add(limit));
					debug_assert!(to_lock <= amount);
					Pallet::<T>::do_lock_deposit(&who, currency_id, to_lock)?;
					Pallet::<T>::do_lockdown_asset(currency_id, lockdown_until)?;
				}
			}
		};

		Ok(())
	}
}
